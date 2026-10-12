#!/usr/bin/env bash
# Usage: [GALROON_NAS=user@host] scripts/nas/status.sh
# Read only: galroon status/logs, cgroup peak (including children/cache), host memory.
set -euo pipefail
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
[[ $# == 0 ]] || nas_die 'Usage: status.sh'
ssh -T "$NAS_TARGET" 'sh -s' <<'REMOTE'
set -eu
docker=/usr/local/bin/docker
sudo -n "$docker" inspect --format 'Status={{.State.Status}} OOMKilled={{.State.OOMKilled}} RestartCount={{.RestartCount}} StartedAt={{.State.StartedAt}} Image={{.Config.Image}} User={{.Config.User}}' galroon
printf '\nLast 50 log lines:\n'
sudo -n "$docker" logs --tail 50 galroon
id=$(sudo -n "$docker" inspect --format '{{.Id}}' galroon)
peak_found=false
for path in "/sys/fs/cgroup/memory/docker/$id/memory.max_usage_in_bytes" \
    "/sys/fs/cgroup/memory/system.slice/docker-$id.scope/memory.max_usage_in_bytes" \
    "/sys/fs/cgroup/docker/$id/memory.peak" \
    "/sys/fs/cgroup/system.slice/docker-$id.scope/memory.peak"; do
    if [ -r "$path" ]; then
        case "$path" in *memory.max_usage_in_bytes) version=v1 ;; *) version=v2 ;; esac
        printf '\nCgroup %s memory peak (bytes), host %s: ' "$version" "$path"
        cat "$path"
        peak_found=true
        break
    fi
done
# Private cgroup namespaces can expose the container's own counters at the root.
if [ "$peak_found" = false ]; then
    membership=$(sudo -n "$docker" exec galroon cat /proc/1/cgroup 2>/dev/null || true)
    for path in /sys/fs/cgroup/memory.peak /sys/fs/cgroup/memory/memory.max_usage_in_bytes; do
        # Only a private namespace rooted at this container makes a root counter
        # meaningful; otherwise it could be the whole host's aggregate peak.
        case "$path" in
            *memory.max_usage_in_bytes) relative=$(printf '%s\n' "$membership" | awk -F: '$2 ~ /(^|,)memory(,|$)/ {print $3}') ;;
            *) relative=$(printf '%s\n' "$membership" | awk -F: '$1 == "0" && $2 == "" {print $3}') ;;
        esac
        [ "$relative" = / ] || continue
        if peak=$(sudo -n "$docker" exec galroon cat "$path" 2>/dev/null); then
            case "$path" in *memory.max_usage_in_bytes) version=v1 ;; *) version=v2 ;; esac
            printf '\nCgroup %s memory peak (bytes), container %s: %s\n' "$version" "$path" "$peak"
            peak_found=true
            break
        fi
    done
fi
if [ "$peak_found" = false ]; then
    printf '\nMemory peak unavailable: cgroup counter is absent/unreadable, or the container has stopped. Capture it while running.\n'
fi
printf '\nNAS host memory/swap (kB):\n'
awk '/^(MemAvailable|MemFree|SwapTotal|SwapFree):/ {print}' /proc/meminfo
REMOTE
