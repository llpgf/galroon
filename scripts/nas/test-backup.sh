#!/usr/bin/env bash
# Local integration tests; no Docker or SSH required.
set -Eeuo pipefail
trap 'printf "FAIL: command at line %s\n" "$LINENO" >&2' ERR
script=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/galroon-backup.sh
root=$(mktemp -d "${TMPDIR:-/tmp}/galroon-backup.XXXXXXXX")
root=$(cd "$root" && pwd -P)
writer=
feeder=
cleanup() {
    if [[ -n "$feeder" ]]; then kill "$feeder" 2>/dev/null || true; wait "$feeder" 2>/dev/null || true; fi
    exec 3>&-
    if [[ -n "$writer" ]]; then kill "$writer" 2>/dev/null || true; wait "$writer" 2>/dev/null || true; fi
    rm -rf "$root"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
export GALROON_BASE="$root/base with spaces"
export GALROON_DOCKER="$root/no-docker"
unset GALROON_DATA GALROON_BACKUP_DIR GALROON_RESTORE_ROOT
export GALROON_BACKUP_DAILY=7 GALROON_BACKUP_WEEKLY=4 GALROON_BACKUP_ARTWORK=weekly GALROON_SQLITE=sqlite3
mkdir -p "$GALROON_BASE/data"
data=$GALROON_BASE/data
backups=$GALROON_BASE/backups
printf '#!/bin/sh\nexit 1\n' > "$GALROON_DOCKER"
chmod 700 "$GALROON_DOCKER"
printf 'GALROON_DATA="%s"\n' "$data" > "$GALROON_BASE/.env"
sqlite3 "$data/library.sqlite" <<'SQL'
PRAGMA journal_mode=WAL;
CREATE TABLE works(id INTEGER);
CREATE TABLE resources(id INTEGER);
CREATE TABLE writes(id INTEGER);
WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<678) INSERT INTO works SELECT x FROM n;
WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<793) INSERT INTO resources SELECT x FROM n;
PRAGMA user_version=20;
SQL
sqlite3 "$data/artwork-library.sqlite" 'PRAGMA journal_mode=WAL; CREATE TABLE artwork(data BLOB); INSERT INTO artwork VALUES(randomblob(8192));'
for file in device.json network.json lan-key.pem lan-cert.pem; do printf 'fixture %s\n' "$file" > "$data/$file"; done
mkdir "$data/backups" "$data/migration-backups"
printf 'lock\n' > "$data/lan-tls.lock"
# A FIFO keeps one WAL connection open between transactions.
mkfifo "$root/writer-input"
sqlite3 "$data/library.sqlite" < "$root/writer-input" > "$root/writer.log" 2>&1 &
writer=$!
exec 3> "$root/writer-input"
printf '.timeout 60000\n' >&3
(
    while :; do printf 'BEGIN IMMEDIATE; INSERT INTO writes VALUES(1); COMMIT;\n'; sleep 0.05; done
) >&3 &
feeder=$!
sleep 0.2
run() { sh "$script" "$@"; }
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }
pass() { printf 'PASS: %s\n' "$*"; }
expect_failure() { if "$@" > "$root/expected-failure.log" 2>&1; then fail "Unexpected success: $*"; fi; }
field() { sed -n "s/^$2=//p" "$1/MANIFEST"; }
mode() { stat -f %Lp "$1" 2>/dev/null || stat -c %a "$1"; }

run backup
weekly=$(find "$backups" -type d -name '*-weekly')
[[ -n "$weekly" && $(field "$weekly" works) == 678 && $(field "$weekly" resources) == 793 && $(field "$weekly" schema) == 20 ]] || fail 'Weekly counts'
[[ $(field "$weekly" artwork) == yes && -f "$weekly/artwork-library.sqlite" ]] || fail 'Weekly artwork'
[[ $(field "$weekly" image) == unknown ]] || fail 'Optional image lookup'
run backup
daily=$(find "$backups" -type d -name '*-daily')
[[ -n "$daily" && $(field "$daily" artwork) == no && ! -e "$daily/artwork-library.sqlite" ]] || fail 'Daily artwork'
kill "$feeder"; wait "$feeder" 2>/dev/null || true
feeder=
exec 3>&-
wait "$writer"; writer=
[[ $(sqlite3 "$data/library.sqlite" 'SELECT count(*) FROM writes;') -gt 1 ]] || fail 'Concurrent writer'
pass '(a) online WAL backup, 678/793, weekly artwork then daily catalog'

for snapshot in "$weekly" "$daily"; do
    [[ -f "$snapshot/MANIFEST" && -f "$snapshot/SHA256SUMS" ]] || fail 'Metadata missing'
    [[ $(mode "$snapshot") == 700 ]] || fail 'Directory mode'
    for file in "$snapshot"/*; do [[ $(mode "$file") == 600 ]] || fail 'File mode'; done
    [[ -z $(find "$snapshot" -name '*-wal' -o -name '*-shm') ]] || fail 'WAL sidecars'
    [[ ! -e "$snapshot/backups" && ! -e "$snapshot/migration-backups" && ! -e "$snapshot/lan-tls.lock" ]] || fail 'Excluded files'
done
pass '(b) metadata, no sidecars or Core backup folders, modes 700/600'

run verify "${weekly##*/}"
run verify "${daily##*/}"
cp "$weekly/device.json" "$root/device-original"
printf 'corrupt\n' >> "$weekly/device.json"
expect_failure run verify "${weekly##*/}"
cp "$root/device-original" "$weekly/device.json"
pass '(c) verify passes and detects corruption'

for tier in daily weekly; do
    for day in 01 02 03 04 05 06 07 08 09 10; do
        dir="$backups/202001${day}T043000Z-$tier"
        mkdir "$dir"
        printf 'format=galroon-nas-backup\nversion=1\ntier=%s\nepoch=1\n' "$tier" > "$dir/MANIFEST"
        (cd "$dir" && shasum -a 256 MANIFEST > SHA256SUMS)
    done
done
mkdir "$backups/unrelated" "$backups/.incomplete-old" "$backups/20200111T043000Z-daily"
run prune
[[ $(find "$backups" -name MANIFEST -path '*-daily/*' | wc -l | tr -d ' ') == 7 ]] || fail 'Daily retention'
[[ $(find "$backups" -name MANIFEST -path '*-weekly/*' | wc -l | tr -d ' ') == 4 ]] || fail 'Weekly retention'
for day in 05 06 07 08 09 10; do [[ -d "$backups/202001${day}T043000Z-daily" ]] || fail 'Newest dailies'; done
for day in 08 09 10; do [[ -d "$backups/202001${day}T043000Z-weekly" ]] || fail 'Newest weeklies'; done
[[ -d "$backups/unrelated" && -d "$backups/.incomplete-old" && -d "$backups/20200111T043000Z-daily" ]] || fail 'Unrelated directories removed'
pass '(d) prune keeps newest 7 daily / 4 weekly and unrelated directories'

run restore "${weekly##*/}" "$GALROON_BASE/restored"
for file in "$weekly"/*; do cmp "$file" "$GALROON_BASE/restored/${file##*/}" || fail 'Restore contents'; done
expect_failure run restore "${weekly##*/}" "$GALROON_BASE/restored"
expect_failure run restore "${weekly##*/}" "$root/outside"
expect_failure run restore "${weekly##*/}" "$GALROON_BASE/data/../escape"
[[ ! -e "$root/outside" && ! -e "$GALROON_BASE/escape" ]] || fail 'Rejected restore wrote files'
ln -s "$root" "$GALROON_BASE/outside-link"
expect_failure run restore "${weekly##*/}" "$GALROON_BASE/outside-link/escape"
expect_failure run restore "${weekly##*/}" "$data/nested-restore"
[[ ! -e "$root/escape" && ! -e "$data/nested-restore" ]] || fail 'Restore escaped through symlink or production data'
pass '(e) restore matches; existing, outside, traversal, symlink and production destinations rejected'

expect_failure env GALROON_BACKUP_DIR="$data/nested-backups" sh "$script" backup
expect_failure env GALROON_BACKUP_DIR="$data" sh "$script" backup
[[ ! -e "$data/nested-backups" ]] || fail 'Nested backup directory created'
pass '(f) backup directory inside/equal to data rejected before writing'

mkdir "$backups/.lock"
expect_failure run backup
[[ -d "$backups/.lock" ]] || fail 'Existing lock removed'
rmdir "$backups/.lock"
pass '(g) stale lock blocks backup and remains for manual removal'

for keep in 0 -1 abc 1.5; do expect_failure env GALROON_BACKUP_DAILY="$keep" sh "$script" prune; done
expect_failure env GALROON_SQLITE=false sh "$script" backup
[[ ! -d "$backups/.lock" && $(find "$backups" -maxdepth 1 -name '.incomplete-*' | wc -l | tr -d ' ') == 1 ]] || fail 'Failed backup cleanup'
pass 'invalid retention rejected; failed backup removes its staging directory and lock'
# Manifest creation time is displayed, but pruning uses the name's epoch.
for epoch in 9 10 20 30; do
    dir="$data/backups/Galroon-backup-$epoch-abcd-1234"
    mkdir "$dir"
    printf '{"created": %s}\n' "$((100 - epoch))" > "$dir/manifest.json"
done
printf 'not json\n' > "$data/backups/Galroon-backup-10-abcd-1234/manifest.json"
mkdir "$data/backups/unrelated" "$data/backups/Galroon-backup-40-abcd" "$data/backups/Galroon-backup-99-notuuid"
printf '{}\n' > "$data/backups/Galroon-backup-99-notuuid/manifest.json"
ln -s "$data/backups/Galroon-backup-9-abcd-1234" "$data/backups/Galroon-backup-50-abcd"
listing=$(run core-exports)
[[ "$listing" == *'Galroon-backup-30-abcd-1234 created=70 size_kib='* && "$listing" == *'Galroon-backup-10-abcd-1234 created=10 size_kib='* ]] || fail 'Core export listing creation time'
[[ "$listing" == *'Total: folders=6 size_kib='* && "$listing" == *backup_history* ]] || fail 'Core export total/history note'
[[ "$listing" == *'Galroon-backup-40-abcd created=40 size_kib='* ]] || fail 'Core export missing manifest fallback'
for keep in -1 abc 1.5; do expect_failure run prune-core-exports --keep "$keep"; done
expect_failure run prune-core-exports
expect_failure run prune-core-exports --keep
run prune-core-exports --keep 2 --dry-run > "$root/dry-run.log"
[[ $(find "$data/backups" -name manifest.json | wc -l | tr -d ' ') == 5 ]] || fail 'Core dry-run deleted exports'
grep -F 'Would delete:' "$root/dry-run.log" >/dev/null || fail 'Core dry-run output'
# The default includes artwork even on a daily snapshot and never prunes exports.
mkdir "$root/default-backups"
cp -R "$weekly" "$root/default-backups/"
env -u GALROON_BACKUP_ARTWORK GALROON_BACKUP_DIR="$root/default-backups" sh "$script" backup
new_daily=$(find "$root/default-backups" -type d -name '*-daily')
[[ $(field "$new_daily" artwork) == yes && -f "$new_daily/artwork-library.sqlite" ]] || fail 'Default daily artwork'
[[ ! -e "$new_daily/backups" && $(find "$data/backups" -name manifest.json | wc -l | tr -d ' ') == 5 ]] || fail 'Scheduled backup touched Core exports'
run prune-core-exports --keep 2 > "$root/prune-exports.log"
for epoch in 20 30; do [[ -d "$data/backups/Galroon-backup-$epoch-abcd-1234" ]] || fail 'Newest Core exports removed'; done
for epoch in 9 10; do
    [[ ! -e "$data/backups/Galroon-backup-$epoch-abcd-1234" ]] || fail 'Old Core export retained'
    grep -F "Deleted: $data/backups/Galroon-backup-$epoch-abcd-1234" "$root/prune-exports.log" >/dev/null || fail 'Deleted export not printed'
done
run prune-core-exports --keep 0
[[ -d "$data/backups/unrelated" && -d "$data/backups/Galroon-backup-99-notuuid" && -d "$data/backups/Galroon-backup-40-abcd" && -L "$data/backups/Galroon-backup-50-abcd" ]] || fail 'Unrelated, incomplete or symlink Core export removed'
[[ ! -e "$data/backups/Galroon-backup-20-abcd-1234" && ! -e "$data/backups/Galroon-backup-30-abcd-1234" ]] || fail 'Keep zero'
pass 'default daily artwork; Core listing, dry-run, numeric retention and protected paths'

wrapper=${script%/*}/backup.sh
export GALROON_SSH="$root/mock-ssh" MOCK_REMOTE="$root/remote" MOCK_SSH_LOG="$root/ssh.log"
mkdir "$MOCK_REMOTE"
export MOCK_SNAPSHOT=20261016T043000Z-weekly
cp -R "$weekly" "$MOCK_REMOTE/$MOCK_SNAPSHOT"
cat > "$GALROON_SSH" <<'SSH'
#!/bin/bash
set -eu
[[ "$1" == -o && "$2" == BatchMode=yes && "$3" == -T && "$4" == -n ]] || exit 1
printf '%s\n' "$6" >> "$MOCK_SSH_LOG"
case "$6" in
    *"'list' "*) printf '%s tier=weekly\n' "$MOCK_SNAPSHOT" ;;
    *"'tar' "*)
        args=(-C "$MOCK_REMOTE")
        if [[ "$6" == *--exclude=artwork-library.sqlite* ]]; then args+=(--exclude=artwork-library.sqlite); fi
        tar "${args[@]}" -cf - "$MOCK_SNAPSHOT"
        ;;
    *) exit 1 ;;
esac
SSH
chmod 700 "$GALROON_SSH"
pulls="$root/pulled snapshots"
bash "$wrapper" pull "$pulls"
[[ -f "$pulls/$MOCK_SNAPSHOT/library.sqlite" && ! -e "$pulls/$MOCK_SNAPSHOT/artwork-library.sqlite" && ! -e "$pulls/.partial-$MOCK_SNAPSHOT" ]] || fail 'Catalog pull contents/staging'
bash "$wrapper" pull "$pulls" > "$root/noop.log"
grep -F 'Already pulled and verified:' "$root/noop.log" >/dev/null || fail 'Pull no-op output'
[[ $(grep -c "'tar'" "$MOCK_SSH_LOG") == 1 ]] || fail 'No-op downloaded again'
printf 'corrupt\n' >> "$pulls/$MOCK_SNAPSHOT/library.sqlite"
expect_failure bash "$wrapper" pull "$pulls"
[[ $(grep -c "'tar'" "$MOCK_SSH_LOG") == 1 ]] || fail 'Corrupt local snapshot overwritten'
printf 'corrupt\n' >> "$MOCK_REMOTE/$MOCK_SNAPSHOT/library.sqlite"
expect_failure bash "$wrapper" pull "$root/failed-pull"
[[ ! -e "$root/failed-pull/$MOCK_SNAPSHOT" && ! -e "$root/failed-pull/.partial-$MOCK_SNAPSHOT" ]] || fail 'Checksum failure left a snapshot or partial'
cp "$weekly/library.sqlite" "$MOCK_REMOTE/$MOCK_SNAPSHOT/library.sqlite"
full="$root/full pulls"
mkdir -p "$full/unrelated" "$full/.partial-old" "$root/link-target"
ln -s "$root/link-target" "$full/20200101T043000Z-daily"
for day in 02 03 04; do mkdir "$full/202001${day}T043000Z-daily"; done
GALROON_PULL_KEEP=2 bash "$wrapper" pull --with-artwork "$full"
[[ -f "$full/$MOCK_SNAPSHOT/artwork-library.sqlite" && -d "$full/20200104T043000Z-daily" && ! -e "$full/20200103T043000Z-daily" && ! -e "$full/20200102T043000Z-daily" ]] || fail 'Artwork pull/retention'
[[ -d "$full/unrelated" && -d "$full/.partial-old" && -L "$full/20200101T043000Z-daily" ]] || fail 'Local retention removed unrelated paths'
(cd "$full/$MOCK_SNAPSHOT" && shasum -a 256 -c SHA256SUMS)
for day in 05 06 07 08 09 10 11 12 13 14 15 16 17 18; do mkdir "$full/202001${day}T043000Z-daily"; done
export MOCK_SNAPSHOT=20261017T043000Z-daily
cp -R "$weekly" "$MOCK_REMOTE/$MOCK_SNAPSHOT"
env -u GALROON_PULL_KEEP bash "$wrapper" pull "$full"
[[ $(find "$full" -maxdepth 1 -type d -name '*-daily' -o -type d -name '*-weekly' | wc -l | tr -d ' ') == 14 ]] || fail 'Default local retention'
pass 'mock SSH pull: BatchMode, no-op, local corruption, checksum cleanup, artwork and retention'

export GALROON_LAUNCH_AGENTS="$root/Launch Agents" GALROON_LAUNCHCTL="$root/mock-launchctl" MOCK_LAUNCH_LOG="$root/launchctl.log"
cat > "$GALROON_LAUNCHCTL" <<'LAUNCH'
#!/bin/bash
printf '%s\n' "$@" >> "$MOCK_LAUNCH_LOG"
# An absent previous job must not block installation or removal.
[[ "$1" != bootout ]]
LAUNCH
chmod 700 "$GALROON_LAUNCHCTL"
export GALROON_NAS="test&user@nas"
schedule_dir="$root/Mac & <NAS> \"copy\" 'backup'"
bash "$wrapper" install-mac-schedule "$schedule_dir"
plist=$GALROON_LAUNCH_AGENTS/com.galroon.nas-backup-pull.plist
plutil -lint "$plist"
plist_value() { plutil -extract "$1" raw -o - "$plist"; }
[[ $(plist_value Label) == com.galroon.nas-backup-pull && $(plist_value ProgramArguments.0) == /bin/bash && $(plist_value ProgramArguments.1) == "$wrapper" && $(plist_value ProgramArguments.2) == pull && $(plist_value ProgramArguments.3) == "$schedule_dir" ]] || fail 'LaunchAgent arguments/XML escaping'
[[ $(plist_value StartCalendarInterval.Hour) == 6 && $(plist_value StartCalendarInterval.Minute) == 0 && $(plist_value RunAtLoad) == false ]] || fail 'LaunchAgent timing'
[[ $(plist_value StandardOutPath) == "$HOME/Library/Logs/galroon-nas-backup-pull.log" && $(plist_value StandardErrorPath) == "$(plist_value StandardOutPath)" ]] || fail 'LaunchAgent logs'
[[ $(plist_value EnvironmentVariables.PATH) == /usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin && $(plist_value EnvironmentVariables.GALROON_NAS) == "$GALROON_NAS" ]] || fail 'LaunchAgent environment'
printf 'bootout\ngui/%s/com.galroon.nas-backup-pull\nbootstrap\ngui/%s\n%s\n' "$(id -u)" "$(id -u)" "$plist" > "$root/expected-launchctl"
cmp "$root/expected-launchctl" "$MOCK_LAUNCH_LOG" || fail 'Launchctl install calls'
bash "$wrapper" uninstall-mac-schedule
[[ ! -e "$plist" ]] || fail 'LaunchAgent not removed'
[[ $(grep -c '^bootout$' "$MOCK_LAUNCH_LOG") == 2 ]] || fail 'Uninstall did not boot out'
env -u GALROON_NAS bash "$wrapper" install-mac-schedule
expect_failure plist_value ProgramArguments.3
expect_failure plist_value EnvironmentVariables.GALROON_NAS
bash "$wrapper" uninstall-mac-schedule
pass 'Mac schedule: lint, escaped paths, arguments, time/environment, bootout/bootstrap and uninstall'
printf 'All backup tests passed.\n'
