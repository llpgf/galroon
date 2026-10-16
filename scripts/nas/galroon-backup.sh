#!/bin/sh
# Usage: galroon-backup.sh <backup|prune|list|verify|restore|restore-test|core-exports|prune-core-exports|help>
set -eu
umask 077

die() { printf 'Error: %s\n' "$*" >&2; exit 1; }
warn() { printf 'Warning: %s\n' "$*" >&2; }
help() {
    cat <<'USAGE'
Usage: galroon-backup.sh backup|prune|list|verify [snapshot]
       galroon-backup.sh core-exports
       galroon-backup.sh prune-core-exports --keep N [--dry-run]
       galroon-backup.sh restore <snapshot> <new-data-dir>
       galroon-backup.sh restore-test [snapshot] [--cleanup]
Config: GALROON_BASE, GALROON_BACKUP_DIR, GALROON_DATA, GALROON_SQLITE,
        GALROON_DOCKER, GALROON_BACKUP_DAILY=7, GALROON_BACKUP_WEEKLY=4,
        GALROON_BACKUP_ARTWORK=always (default)|weekly|never, GALROON_RESTORE_ROOT.
USAGE
}
[ "${1:-help}" != help ] || { help; exit 0; }
base=${GALROON_BASE:-/volume1/docker/galroon}
backup_dir=${GALROON_BACKUP_DIR:-$base/backups}
sqlite=${GALROON_SQLITE:-sqlite3}
docker=${GALROON_DOCKER:-/usr/local/bin/docker}
daily=${GALROON_BACKUP_DAILY:-7}
weekly=${GALROON_BACKUP_WEEKLY:-4}
artwork_policy=${GALROON_BACKUP_ARTWORK:-always}
for keep in "$daily" "$weekly"; do
    case "$keep" in ''|*[!0-9]*) die 'Keep counts must be integers >= 1.' ;; esac
    [ "$keep" -ge 1 ] || die 'Keep counts must be integers >= 1.'
done
case "$artwork_policy" in weekly|always|never) ;; *) die 'Invalid GALROON_BACKUP_ARTWORK.' ;; esac

env_value() {
    [ -f "$base/.env" ] || return 0
    sed -n "s/^$1=//p" "$base/.env" | sed -n '1p' | sed "s/^'\(.*\)'$/\1/; s/^\"\(.*\)\"$/\1/"
}
# Resolve existing ancestors before creating anything.
physical_path() (
    if [ -d "$1" ]; then cd "$1" && pwd -P
    else
        parent=$(dirname "$1")
        [ "$parent" != "$1" ] || exit 1
        printf '%s/%s\n' "$(physical_path "$parent")" "$(basename "$1")"
    fi
)
path_check() {
    case "$1" in /*) ;; *) die 'Paths must be absolute.' ;; esac
    case "$1" in *"'"*|*'
'*|*..*) die "Path must not contain .., single quotes or newlines: $1" ;; esac
}
data=${GALROON_DATA:-$(env_value GALROON_DATA)}
[ -n "$data" ] || die 'Set GALROON_DATA or put it in .env.'
for path in "$base" "$data" "$backup_dir"; do path_check "$path"; done
base=$(physical_path "$base")
data=$(physical_path "$data")
backup_dir=$(physical_path "$backup_dir")
# Check resolved paths too, including symlink targets.
for path in "$base" "$data" "$backup_dir"; do path_check "$path"; done
case "$backup_dir/" in "$data/"*) die 'Backup directory must not be inside the data directory.' ;; esac

locked=no
incomplete=
scratch=
test_container=no
test_success=no
cleanup_scratch=no
probe_pid=
cleanup() {
    result=$?
    trap - 0 HUP INT TERM
    if [ -n "$probe_pid" ]; then
        kill "$probe_pid" 2>/dev/null || true
        wait "$probe_pid" 2>/dev/null || true
    fi
    if [ "$test_container" = yes ]; then
        if [ "$result" -ne 0 ]; then sudo -n "$docker" logs --tail 50 galroon-restore-test >&2 || true; fi
        sudo -n "$docker" stop -t 60 galroon-restore-test >/dev/null || result=1
        sudo -n "$docker" rm galroon-restore-test >/dev/null || result=1
    fi
    if [ -n "$scratch" ]; then
        if [ "$test_success" = yes ] && [ "$cleanup_scratch" = yes ] && [ "$result" -eq 0 ]; then
            rm -rf "$scratch" || result=1
        else printf 'Scratch directory retained: %s\n' "$scratch"; fi
    fi
    if [ -n "$incomplete" ]; then rm -rf "$incomplete" || result=1; fi
    if [ "$locked" = yes ]; then rmdir "$backup_dir/.lock" || result=1; fi
    exit "$result"
}
trap cleanup 0
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

lock() {
    mkdir -p "$backup_dir"
    chmod 700 "$backup_dir" || warn "Cannot chmod $backup_dir; check DSM ACLs."
    mkdir "$backup_dir/.lock" 2>/dev/null || die "Backup lock exists: $backup_dir/.lock. If no backup/prune is running, remove it with: rmdir '$backup_dir/.lock'"
    locked=yes
}
field() { sed -n "s/^$2=//p" "$1/MANIFEST"; }
snapshots() {
    for dir in "$backup_dir"/*; do
        [ -d "$dir" ] && [ ! -L "$dir" ] && [ -f "$dir/MANIFEST" ] || continue
        name=${dir##*/}
        printf '%s\n' "$name" | LC_ALL=C grep -Eq '^[0-9]{8}T[0-9]{6}Z-(daily|weekly)$' || continue
        printf '%s\n' "$name"
    done | LC_ALL=C sort -r
}
# Core exports are manual and separate from scheduled snapshots.
core_exports() {
    for dir in "$data/backups"/Galroon-backup-*; do
        [ -d "$dir" ] && [ ! -L "$dir" ] && [ -f "$dir/manifest.json" ] || continue
        name=${dir##*/}
        printf '%s\n' "$name" | LC_ALL=C grep -Eq '^Galroon-backup-[0-9]+-[0-9a-fA-F-]+$' || continue
        printf '%s\n' "$name"
    done | LC_ALL=C sort -t - -k 3,3nr -k 4
}
core_history_note() {
    printf 'Note: Core backup_history (Backup panel recent backups) still lists deleted paths.\n'
}
list_core_exports() {
    total=0
    count=0
    for dir in "$data/backups"/Galroon-backup-*; do
        [ -d "$dir" ] && [ ! -L "$dir" ] || continue
        name=${dir##*/}
        manifest=$(printf '%s' "$dir/manifest.json" | sed "s/'/''/g")
        created=$("$sqlite" :memory: "SELECT json_extract(CAST(readfile('$manifest') AS TEXT), '$.created');" 2>/dev/null) || created=
        case "$created" in ''|*[!0-9]*) created=${name#Galroon-backup-}; created=${created%%-*} ;; esac
        size=$(du -sk "$dir" | awk '{print $1}')
        total=$((total + size))
        count=$((count + 1))
        printf '%s created=%s size_kib=%s\n' "$name" "$created" "$size"
    done
    core_history_note
    printf 'Total: folders=%s size_kib=%s\n' "$count" "$total"
}
prune_core_exports() {
    keep=
    dry_run=no
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --keep) [ "$#" -ge 2 ] || die 'Usage: prune-core-exports --keep N [--dry-run]'; keep=$2; shift 2 ;;
            --dry-run) dry_run=yes; shift ;;
            *) die 'Usage: prune-core-exports --keep N [--dry-run]' ;;
        esac
    done
    case "$keep" in ''|*[!0-9]*) die '--keep must be an integer >= 0.' ;; esac
    index=0
    core_history_note
    for name in $(core_exports); do
        index=$((index + 1))
        [ "$index" -gt "$keep" ] || continue
        if [ "$dry_run" = yes ]; then printf 'Would delete: %s\n' "$data/backups/$name"
        else rm -rf "$data/backups/$name"; printf 'Deleted: %s\n' "$data/backups/$name"; fi
    done
}
resolve_snapshot() {
    chosen=${1:-$(snapshots | sed -n '1p')}
    [ -n "$chosen" ] || die 'No completed snapshot found.'
    case "$chosen" in /*) ;; *) chosen=$backup_dir/$chosen ;; esac
    path_check "$chosen"
    [ -d "$chosen" ] && [ ! -L "$chosen" ] || die "Not a snapshot: $chosen"
    snapshot=$(cd "$chosen" && pwd -P)
    [ "$(dirname "$snapshot")" = "$backup_dir" ] || die 'Snapshot must be directly inside the backup directory.'
    snapshots | grep -Fx "${snapshot##*/}" >/dev/null || die 'Not a completed snapshot.'
}
sha256() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$@"
    else shasum -a 256 "$@"; fi
}
# Fixed relative names also prevent checksum path traversal.
hashes() (
    cd "$1"
    for file in MANIFEST library.sqlite artwork-library.sqlite device.json network.json lan-*.pem; do
        [ -e "$file" ] || continue
        [ -f "$file" ] && [ ! -L "$file" ] || die "Not a regular snapshot file: $file"
        sha256 "$file"
    done
)
integrity() {
    [ "$("$sqlite" -readonly "$1" 'PRAGMA integrity_check;')" = ok ] || die "SQLite integrity check failed: $1"
}
counts() {
    works=$("$sqlite" -readonly "$1/library.sqlite" 'SELECT count(*) FROM works;')
    resources=$("$sqlite" -readonly "$1/library.sqlite" 'SELECT count(*) FROM resources;')
    schema=$("$sqlite" -readonly "$1/library.sqlite" 'PRAGMA user_version;')
}
check_counts() {
    counts "$1"
    [ "$works" = "$(field "$2" works)" ] && [ "$resources" = "$(field "$2" resources)" ] && [ "$schema" = "$(field "$2" schema)" ] || die 'Manifest/database counts or schema differ.'
}
verify_snapshot() {
    [ -f "$snapshot/library.sqlite" ] && [ -f "$snapshot/SHA256SUMS" ] || die 'Incomplete snapshot files.'
    [ "$(hashes "$snapshot")" = "$(cat "$snapshot/SHA256SUMS")" ] || die 'SHA256SUMS mismatch.'
    [ "$(field "$snapshot" format)" = galroon-nas-backup ] && [ "$(field "$snapshot" version)" = 1 ] || die 'Unsupported manifest.'
    for db in "$snapshot"/*.sqlite; do integrity "$db"; done
    check_counts "$snapshot" "$snapshot"
    printf 'Verified %s: works=%s resources=%s schema=%s\n' "${snapshot##*/}" "$works" "$resources" "$schema"
}
prune_snapshots() {
    for tier in daily weekly; do
        case "$tier" in daily) keep=$daily ;; weekly) keep=$weekly ;; esac
        index=0
        for name in $(snapshots); do
            case "$name" in *-"$tier") ;; *) continue ;; esac
            index=$((index + 1))
            [ "$index" -le "$keep" ] || rm -rf "$backup_dir/$name"
        done
    done
    for dir in "$backup_dir"/.incomplete-*; do
        [ ! -d "$dir" ] || warn "Incomplete snapshot left untouched: $dir"
    done
}
copy_database() {
    [ -f "$data/$1" ] || die "Missing database: $data/$1"
    "$sqlite" "$data/$1" '.timeout 60000' ".backup '$incomplete/$1'"
    # A fresh WAL copy may need sidecars before its journal mode is changed.
    [ "$("$sqlite" "$incomplete/$1" 'PRAGMA integrity_check;')" = ok ] || die "SQLite integrity check failed: $1"
    [ "$("$sqlite" "$incomplete/$1" 'PRAGMA journal_mode=DELETE;')" = delete ] || die 'Could not disable snapshot WAL.'
    rm -f "$incomplete/$1-wal" "$incomplete/$1-shm"
}
backup() {
    lock
    epoch=$(date -u +%s)
    stamp=$(date -u +%Y%m%dT%H%M%SZ)
    created=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    tier=weekly
    for name in $(snapshots); do
        case "$name" in *-weekly) ;; *) continue ;; esac
        previous=$(field "$backup_dir/$name" epoch)
        case "$previous" in ''|*[!0-9]*) continue ;; esac
        if [ "$previous" -gt "$((epoch - 561600))" ]; then tier=daily; break; fi
    done
    target=$backup_dir/$stamp-$tier
    [ ! -e "$target" ] || die "Snapshot already exists: $target; retry in a second."
    pending=$backup_dir/.incomplete-$stamp
    mkdir "$pending"
    incomplete=$pending
    copy_database library.sqlite
    artwork=no
    if [ "$artwork_policy" = always ] || { [ "$artwork_policy" = weekly ] && [ "$tier" = weekly ]; }; then
        [ -f "$data/artwork-library.sqlite" ] || die 'Artwork database is missing.'
        bytes=$(wc -c < "$data/artwork-library.sqlite")
        available=$(df -Pk "$backup_dir" | awk 'END {print $4}')
        awk -v free="$available" -v bytes="$bytes" 'BEGIN {exit !(free * 1024 > bytes * 1.2 + 1073741824)}' || die 'Insufficient free space for artwork: need > 1.2 x artwork size + 1 GiB.'
        copy_database artwork-library.sqlite
        artwork=yes
    fi
    for file in "$data/device.json" "$data/network.json" "$data"/lan-*.pem; do
        [ ! -f "$file" ] || cp -p "$file" "$incomplete/"
    done
    counts "$incomplete"
    image=$(sudo -n "$docker" inspect --format '{{.Config.Image}}' galroon 2>/dev/null) || image=unknown
    image=${image:-unknown}
    cat > "$incomplete/MANIFEST" <<MANIFEST
format=galroon-nas-backup
version=1
name=$stamp
tier=$tier
epoch=$epoch
created=$created
data_dir=$data
image=$image
sqlite_version=$("$sqlite" --version)
works=$works
resources=$resources
schema=$schema
artwork=$artwork
MANIFEST
    hashes "$incomplete" > "$incomplete/SHA256SUMS"
    chmod 700 "$incomplete" || warn 'Cannot chmod snapshot directory; check DSM ACLs.'
    chmod 600 "$incomplete"/* || warn 'Cannot chmod snapshot files; check DSM ACLs.'
    sync
    mv "$incomplete" "$target"
    incomplete=
    prune_snapshots
    printf 'Backup %s: works=%s resources=%s artwork=%s\n' "${target##*/}" "$works" "$resources" "$artwork"
}
restore() {
    destination=$1
    path_check "$destination"
    [ ! -e "$destination" ] && [ ! -L "$destination" ] || die 'Restore destination already exists.'
    [ -d "$(dirname "$destination")" ] || die 'Restore parent must exist.'
    root=$(physical_path "${GALROON_RESTORE_ROOT:-$base}")
    destination=$(physical_path "$destination")
    path_check "$destination"
    case "$destination" in "$root/"*) ;; *) die "Restore destination must be under $root/." ;; esac
    case "$destination/" in "$data/"*) die 'Cannot restore into production data.' ;; esac
    verify_snapshot
    mkdir "$destination"
    chmod 700 "$destination" || warn 'Cannot chmod restore directory; check DSM ACLs.'
    for file in "$snapshot/MANIFEST" "$snapshot/SHA256SUMS" "$snapshot/library.sqlite" \
        "$snapshot/artwork-library.sqlite" "$snapshot/device.json" "$snapshot/network.json" "$snapshot"/lan-*.pem; do
        [ ! -f "$file" ] || cp -p "$file" "$destination/"
    done
    chmod 600 "$destination"/* || warn 'Cannot chmod restored files; check DSM ACLs.'
    [ "$(hashes "$destination")" = "$(cat "$destination/SHA256SUMS")" ] || die 'Restored checksum mismatch.'
    printf 'Restored to %s\nAnnounce downtime; deploy with GALROON_DATA set to this directory, or stop Core, edit .env GALROON_DATA and compose up -d --no-deps galroon. Check health/status/owner session; retain old data. See docs/development/NAS.md.\n' "$destination"
}
restore_test() {
    uid=$(id -u); gid=$(id -g)
    [ "$uid" -ne 0 ] && [ "$gid" -ne 0 ] || die 'Restore test requires a non-root uid/gid.'
    containers=$(sudo -n "$docker" ps -a --format '{{.Names}}')
    if printf '%s\n' "$containers" | grep -Fx galroon-restore-test >/dev/null; then die 'Container galroon-restore-test already exists.'; fi
    image=$(sudo -n "$docker" inspect --format '{{.Config.Image}}' galroon 2>/dev/null) || image=galroon:local
    image=${image:-galroon:local}
    library=$(env_value GALROON_LIBRARY)
    library=${library:-/volume1/Backup/Galgame}
    restore "$base/restore-test-$(date -u +%Y%m%dT%H%M%SZ)"
    scratch=$destination
    if [ -f "$scratch/network.json" ]; then mv "$scratch/network.json" "$scratch/network.json.disabled-for-test"; fi
    test_container=yes
    sudo -n "$docker" run -d --name galroon-restore-test --network none --user "$uid:$gid" --memory 768m \
        --tmpfs "/run/galroon:mode=0700,uid=$uid,gid=$gid" -v "$scratch:/data" -v "$library:$library:ro" \
        -e GALROON_INSTALL_KIND=docker "$image"
    deadline=$(($(date -u +%s) + 90))
    healthy=no
    while [ "$(date -u +%s)" -lt "$deadline" ]; do
        # Bound a stalled TCP response as well as repeated connection failures.
        sudo -n "$docker" exec galroon-restore-test bash -c 'exec 3<>/dev/tcp/127.0.0.1/14800 && printf "GET /api/health HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n" >&3 && cat <&3' > "$scratch/.restore-test-health" 2>/dev/null &
        probe_pid=$!
        while kill -0 "$probe_pid" 2>/dev/null && [ "$(date -u +%s)" -lt "$deadline" ]; do sleep 1; done
        if kill -0 "$probe_pid" 2>/dev/null; then kill "$probe_pid" 2>/dev/null || true; fi
        if wait "$probe_pid"; then response=$(cat "$scratch/.restore-test-health"); else response=; fi
        probe_pid=
        if printf '%s\n' "$response" | sed -n '1p' | grep -Eq '^HTTP/1\.[01] 200 ' && \
            printf '%s\n' "$response" | tr -d '\r' | sed '1,/^$/d' | grep -F '"name":"Galroon"' >/dev/null; then healthy=yes; break; fi
        sleep 2
    done
    rm -f "$scratch/.restore-test-health"
    [ "$healthy" = yes ] || die 'Restore test health check failed within 90 seconds.'
    counts "$scratch"
    printf 'Restore test: works=%s (expected %s) resources=%s (expected %s)\n' "$works" "$(field "$snapshot" works)" "$resources" "$(field "$snapshot" resources)"
    [ "$works" = "$(field "$snapshot" works)" ] && [ "$resources" = "$(field "$snapshot" resources)" ] || die 'Restore test counts differ from the manifest.'
    test_success=yes
    printf 'Restore test passed: %s\n' "$scratch"
}
command=$1; shift
case "$command" in
    core-exports) [ "$#" -eq 0 ] || die 'Usage: core-exports'; list_core_exports ;;
    prune-core-exports) prune_core_exports "$@" ;;
    backup) [ "$#" -eq 0 ] || die 'Usage: backup'; backup ;;
    prune) [ "$#" -eq 0 ] || die 'Usage: prune'; lock; prune_snapshots ;;
    list)
        [ "$#" -eq 0 ] || die 'Usage: list'
        for name in $(snapshots); do
            dir=$backup_dir/$name
            printf '%s tier=%s works=%s resources=%s artwork=%s size_kib=%s\n' "$name" "$(field "$dir" tier)" "$(field "$dir" works)" "$(field "$dir" resources)" "$(field "$dir" artwork)" "$(du -sk "$dir" | awk '{print $1}')"
        done ;;
    verify) [ "$#" -le 1 ] || die 'Usage: verify [snapshot]'; resolve_snapshot "${1:-}"; verify_snapshot ;;
    restore) [ "$#" -eq 2 ] || die 'Usage: restore <snapshot> <new-data-dir>'; resolve_snapshot "$1"; restore "$2" ;;
    restore-test)
        selection=
        for arg in "$@"; do
            if [ "$arg" = --cleanup ]; then cleanup_scratch=yes
            else [ -z "$selection" ] || die 'Usage: restore-test [snapshot] [--cleanup]'; selection=$arg; fi
        done
        resolve_snapshot "$selection"; restore_test ;;
    *) help >&2; die "Unknown command: $command" ;;
esac
