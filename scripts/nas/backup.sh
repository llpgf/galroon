#!/usr/bin/env bash
# Usage: scripts/nas/backup.sh <install|backup|list|verify|prune|restore|restore-test|core-exports|prune-core-exports|pull|install-mac-schedule|uninstall-mac-schedule>
set -euo pipefail
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
umask 077
[[ $# -gt 0 ]] || nas_die 'Usage: backup.sh <install|backup|list|verify|prune|restore|restore-test|core-exports|prune-core-exports|pull|install-mac-schedule|uninstall-mac-schedule>'
# launchd reads XML, including paths with shell metacharacters.
xml_escape() {
    printf '%s' "$1" | sed -e 's/&/\&amp;/g; s/</\&lt;/g; s/>/\&gt;/g; s/"/\&quot;/g' -e "s/'/\&apos;/g"
}
verify_pull() (
    cd "$1"
    [[ -f library.sqlite && -f MANIFEST && -f SHA256SUMS ]] || exit 1
    if [[ "$with_artwork" == yes ]]; then
        shasum -a 256 -c SHA256SUMS
    else
        awk '$2 != "artwork-library.sqlite"' SHA256SUMS | shasum -a 256 -c -
    fi
)
prune_pulls() {
    local dir name index=0
    while IFS= read -r name; do
        [[ -n "$name" ]] || continue
        index=$((index + 1))
        if [[ "$index" -gt "$pull_keep" ]]; then
            rm -rf "$local_dir/$name"
            printf 'Removed local snapshot: %s\n' "$local_dir/$name"
        fi
    done < <(
        for dir in "$local_dir"/*; do
            [[ -d "$dir" && ! -L "$dir" ]] || continue
            name=${dir##*/}
            [[ "$name" =~ ^[0-9]{8}T[0-9]{6}Z-(daily|weekly)$ ]] || continue
            printf '%s\n' "$name"
        done | LC_ALL=C sort -r
    )
}
command=$1; shift
case "$command" in
    install)
        [[ $# == 0 ]] || nas_die 'Usage: backup.sh install'
        remote=$(nas_quote sh -c '
set -eu
umask 077
mkdir -p "$1/bin"
tmp=$(mktemp "$1/bin/.galroon-backup.XXXXXXXX")
trap '\''rm -f "$tmp"'\'' 0
trap '\''exit 1'\'' HUP INT TERM
cat > "$tmp"
chmod 700 "$tmp"
mv "$tmp" "$1/bin/galroon-backup.sh"
' sh "$NAS_BASE")
        ssh -T "$NAS_TARGET" "$remote" < "$NAS_ROOT/scripts/nas/galroon-backup.sh"
        cat <<'STEPS'
已安裝。DSM：控制台 → 工作排程器 → 新增 → 排程的工作 → 使用者定義的指令碼。
一般：工作名稱 Galroon backup；使用者 galroon-dev。
排程：每日 04:30。
工作設定：啟用異常終止時寄送電子郵件；使用者定義的指令碼：
/volume1/docker/galroon/bin/galroon-backup.sh backup
STEPS
        ;;
    backup|list|verify|prune|restore|restore-test|core-exports|prune-core-exports|help)
        ssh -T -n "$NAS_TARGET" "$(nas_quote "$NAS_BASE/bin/galroon-backup.sh" "$command" "$@")"
        ;;
    pull)
        with_artwork=no
        while [[ $# -gt 0 ]]; do
            case "$1" in
                --with-artwork) with_artwork=yes; shift ;;
                --) shift; break ;;
                -*) nas_die 'Usage: backup.sh pull [--with-artwork] [local-dir]' ;;
                *) break ;;
            esac
        done
        [[ $# -le 1 ]] || nas_die 'Usage: backup.sh pull [--with-artwork] [local-dir]'
        local_dir=${1:-$HOME/GalroonBackups/nas}
        pull_keep=${GALROON_PULL_KEEP:-14}
        [[ "$pull_keep" =~ ^[0-9]+$ ]] && [[ "$pull_keep" -ge 1 ]] || nas_die 'GALROON_PULL_KEEP must be an integer >= 1.'
        ssh_command=${GALROON_SSH:-ssh}
        name=$("$ssh_command" -o BatchMode=yes -T -n "$NAS_TARGET" "$(nas_quote "$NAS_BASE/bin/galroon-backup.sh" list)" | sed -n '1s/ .*//p')
        [[ "$name" =~ ^[0-9]{8}T[0-9]{6}Z-(daily|weekly)$ ]] || nas_die 'No completed snapshot found.'
        mkdir -p "$local_dir"
        destination=$local_dir/$name
        if [[ -e "$destination" || -L "$destination" ]]; then
            [[ -d "$destination" && ! -L "$destination" ]] && verify_pull "$destination" || nas_die "Local snapshot failed verification: $destination; move it aside and retry."
            printf 'Already pulled and verified: %s\n' "$destination"
            exit 0
        fi
        partial=$local_dir/.partial-$name
        mkdir "$partial"
        trap 'rm -rf "$partial"' EXIT
        trap 'exit 1' HUP INT TERM
        chmod 700 "$partial"
        tar_args=(tar -C "$NAS_BASE/backups")
        if [[ "$with_artwork" == no ]]; then tar_args+=(--exclude=artwork-library.sqlite); fi
        tar_args+=(-cf - "$name")
        "$ssh_command" -o BatchMode=yes -T -n "$NAS_TARGET" "$(nas_quote "${tar_args[@]}")" \
            | tar -xf - -C "$partial" --strip-components=1
        verify_pull "$partial"
        chmod 600 "$partial"/*
        mv "$partial" "$destination"
        trap - EXIT HUP INT TERM
        printf 'Pulled snapshot: %s (with-artwork=%s)\n' "$destination" "$with_artwork"
        prune_pulls
        ;;
    install-mac-schedule)
        [[ $# -le 1 ]] || nas_die 'Usage: backup.sh install-mac-schedule [local-dir]'
        agents_dir=${GALROON_LAUNCH_AGENTS:-$HOME/Library/LaunchAgents}
        launchctl=${GALROON_LAUNCHCTL:-launchctl}
        plist=$agents_dir/com.galroon.nas-backup-pull.plist
        log=$HOME/Library/Logs/galroon-nas-backup-pull.log
        mkdir -p "$agents_dir" "${log%/*}"
        {
            cat <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>com.galroon.nas-backup-pull</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/bash</string>
PLIST
            printf '        <string>%s</string>\n' "$(xml_escape "$NAS_ROOT/scripts/nas/backup.sh")" pull
            if [[ $# == 1 ]]; then
                local_dir=$1
                case "$local_dir" in /*) ;; *) local_dir=$PWD/$local_dir ;; esac
                printf '        <string>%s</string>\n' "$(xml_escape "$local_dir")"
            fi
            cat <<'PLIST'
    </array>
    <key>StartCalendarInterval</key>
    <dict><key>Hour</key><integer>6</integer><key>Minute</key><integer>0</integer></dict>
    <key>RunAtLoad</key><false/>
PLIST
            printf '    <key>StandardOutPath</key><string>%s</string>\n' "$(xml_escape "$log")"
            printf '    <key>StandardErrorPath</key><string>%s</string>\n' "$(xml_escape "$log")"
            cat <<'PLIST'
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key><string>/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin</string>
PLIST
            if [[ -n "${GALROON_NAS:-}" ]]; then
                printf '        <key>GALROON_NAS</key><string>%s</string>\n' "$(xml_escape "$GALROON_NAS")"
            fi
            printf '    </dict>\n</dict>\n</plist>\n'
        } > "$plist"
        plutil -lint "$plist"
        "$launchctl" bootout "gui/$(id -u)/com.galroon.nas-backup-pull" 2>/dev/null || true
        "$launchctl" bootstrap "gui/$(id -u)" "$plist"
        printf 'Installed daily 06:00 pull: %s\n' "$plist"
        ;;
    uninstall-mac-schedule)
        [[ $# == 0 ]] || nas_die 'Usage: backup.sh uninstall-mac-schedule'
        agents_dir=${GALROON_LAUNCH_AGENTS:-$HOME/Library/LaunchAgents}
        launchctl=${GALROON_LAUNCHCTL:-launchctl}
        "$launchctl" bootout "gui/$(id -u)/com.galroon.nas-backup-pull" 2>/dev/null || true
        rm -f "$agents_dir/com.galroon.nas-backup-pull.plist"
        printf 'Uninstalled Mac pull schedule.\n'
        ;;
    *) nas_die "Unknown command: $command" ;;
esac
