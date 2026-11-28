#!/usr/bin/env bash
# Usage: [GALROON_NAS=user@host] [GALROON_UID=1032 GALROON_GID=100] scripts/nas/deploy.sh
# Optional: GALROON_DATA, GALROON_BACKUPS, GALROON_LIBRARY, GALROON_WATCH_FORCE_POLLING/POLL_SECONDS.
# Upload prebuilt files; only this project's galroon service is built/started.
set -euo pipefail
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
[[ $# == 0 ]] || nas_die 'Usage: deploy.sh (configuration comes from environment)'
uid=${GALROON_UID:-1032}
gid=${GALROON_GID:-100}
data=${GALROON_DATA:-$NAS_BASE/data}
backups=${GALROON_BACKUPS:-$NAS_BASE/core-backups}
library=${GALROON_LIBRARY:-/volume1/Backup/Galgame}
force=${GALROON_WATCH_FORCE_POLLING:-}
seconds=${GALROON_WATCH_POLL_SECONDS:-}
artifact_dir=${GALROON_ARTIFACT_DIR:-$NAS_ROOT/target/nas}
ssh_command=${GALROON_SSH:-ssh}
[[ "$uid" =~ ^[1-9][0-9]*$ && "$gid" =~ ^[1-9][0-9]*$ ]] || nas_die 'UID and GID must be positive numeric IDs.'
# These values enter compose .env and bind syntax: reject interpolation and delimiters.
for path in "$data" "$backups" "$library"; do
    [[ "$path" =~ ^/[a-zA-Z0-9_./\ -]+$ && "$path" != *'/../'* && "$path" != */.. && "$path" != *'/./'* && "$path" != */. && "$path" != *'//'* && "$path" != */ ]] || nas_die 'Use an absolute path without .. or compose/shell metacharacters.'
done
[[ "$data" == "$NAS_BASE/"* ]] || nas_die "GALROON_DATA must be below $NAS_BASE (never under /volume1/Backup)."
[[ "$backups" == "$NAS_BASE/"* ]] || nas_die "GALROON_BACKUPS must be below $NAS_BASE (never under /volume1/Backup)."
if [[ "$backups" == "$data" || "$backups" == "$data/"* || "$data" == "$backups/"* ]]; then
    nas_die 'GALROON_BACKUPS must not equal or overlap GALROON_DATA.'
fi
if [[ "$backups" == "$library" || "$backups" == "$library/"* || "$library" == "$backups/"* ]]; then
    nas_die 'GALROON_BACKUPS must not equal or overlap GALROON_LIBRARY.'
fi
[[ -z "$force" || "$force" == 0 || "$force" == 1 ]] || nas_die 'Polling switch must be empty, 0 or 1.'
[[ -z "$seconds" || "$seconds" =~ ^[1-9][0-9]*$ ]] || nas_die 'Polling seconds must be empty or a positive integer.'
nas_artifact_check "$artifact_dir"

# Remote scripts use sh for DSM compatibility; sudo is ONLY used for Docker.
prepare=$(nas_quote sh -s -- "$NAS_BASE" "$data" "$backups" "$library" "$uid" "$gid")
context=$("$ssh_command" -T "$NAS_TARGET" "$prepare" <<'REMOTE'
set -eu
base=$1; data=$2; backups=$3; library=$4; uid=$5; gid=$6
[ "$(id -u)" = "$uid" ] && [ "$(id -g)" = "$gid" ] || { echo 'SSH account uid/gid must match the container user.' >&2; exit 1; }
umask 077
mkdir -p "$base"
[ "$(cd "$base" && pwd -P)" = "$base" ] || { echo 'Deployment base must not traverse symlinks.' >&2; exit 1; }
# Reject symlinked ancestors before mkdir, so no creation can escape the base.
for dir in "$data" "$backups"; do
    parent=$dir
    while [ "$parent" != "$base" ]; do
        [ ! -L "$parent" ] || { echo 'Data and backup paths must not traverse symlinks.' >&2; exit 1; }
        parent=$(dirname "$parent")
    done
done
[ -d "$library" ] && [ -r "$library" ] && [ -x "$library" ] || { echo 'Library must already exist and be readable by the SSH account.' >&2; exit 1; }
library_real=$(cd "$library" && pwd -P)
for dir in "$data" "$backups"; do
    if [ "$dir" = "$library_real" ] || [ "${dir#"$library_real"/}" != "$dir" ]; then echo 'Data and backups must not be inside the library.' >&2; exit 1; fi
    if [ "${library_real#"$dir"/}" != "$library_real" ]; then echo 'Library must not be inside data or backups.' >&2; exit 1; fi
done
if [ "$backups" = "$data" ] || [ "${backups#"$data"/}" != "$backups" ] || [ "${data#"$backups"/}" != "$data" ]; then
    echo 'Backups must not equal or overlap data.' >&2
    exit 1
fi
mkdir -p "$data" "$backups"
[ "$(cd "$data" && pwd -P)" = "$data" ] || { echo 'Data path must not traverse symlinks.' >&2; exit 1; }
[ "$(cd "$backups" && pwd -P)" = "$backups" ] || { echo 'Backup path must not traverse symlinks.' >&2; exit 1; }
[ "$(stat -c %u "$data")" = "$uid" ] && [ "$(stat -c %g "$data")" = "$gid" ] && [ -w "$data" ] || { echo 'Data directory needs the configured owner/group and write access; fix DSM permissions.' >&2; exit 1; }
[ "$(stat -c %u "$backups")" = "$uid" ] && [ "$(stat -c %g "$backups")" = "$gid" ] && [ -w "$backups" ] || { echo 'Backup directory needs the configured owner/group and write access; fix DSM permissions.' >&2; exit 1; }
# A fresh build context prevents stale Web assets and excludes persistent data.
mktemp -d "$base/release.XXXXXXXX"
REMOTE
)
[[ "$context" == "$NAS_BASE/release."* && "$context" != *$'\n'* ]] || nas_die 'Unexpected remote staging path.'
tar -C "$artifact_dir" -cf - galroon-core web BUILD_SHA SHA256SUMS \
    -C "$NAS_ROOT/docker" Dockerfile compose.yaml .dockerignore entrypoint.sh \
    | "$ssh_command" -T "$NAS_TARGET" "$(nas_quote tar -xf - -C "$context")"

activate=$(nas_quote sh -s -- "$NAS_BASE" "$context" "$NAS_SHA" "$uid" "$gid" "$data" "$backups" "$library" "$force" "$seconds")
"$ssh_command" -T "$NAS_TARGET" "$activate" <<'REMOTE'
set -eu
base=$1; context=$2; sha=$3; uid=$4; gid=$5; data=$6; backups=$7; library=$8; force=$9; seconds=${10}
docker=/usr/local/bin/docker
cd "$base"
# Refuse to replace a same-name container belonging to another compose project.
if sudo -n "$docker" container inspect galroon >/dev/null 2>&1; then
    project=$(sudo -n "$docker" inspect --format '{{index .Config.Labels "com.docker.compose.project"}}' galroon)
    service=$(sudo -n "$docker" inspect --format '{{index .Config.Labels "com.docker.compose.service"}}' galroon)
    [ "$project" = galroon ] && [ "$service" = galroon ] || { echo 'Existing galroon container is not this compose service.' >&2; exit 1; }
fi
(cd "$context" && sha256sum -c SHA256SUMS)
sudo -n "$docker" build -t "galroon:$(printf '%.12s' "$sha")" -t galroon:local "$context"
# Retain the actual running image even when :local has just moved to the new build.
if sudo -n "$docker" container inspect galroon >/dev/null 2>&1; then
    previous=$(sudo -n "$docker" inspect --format '{{.Image}}' galroon)
    sudo -n "$docker" image tag "$previous" galroon:previous
    printf '%s\n' "$previous" > "$base/previous-image-id"
fi
umask 077
cat > "$context/deploy.env" <<ENV
GALROON_UID=$uid
GALROON_GID=$gid
GALROON_DATA='$data'
GALROON_BACKUPS='$backups'
GALROON_LIBRARY='$library'
GALROON_WATCH_FORCE_POLLING=$force
GALROON_WATCH_POLL_SECONDS=$seconds
GALROON_IMAGE=galroon:$(printf '%.12s' "$sha")
ENV
sudo -n "$docker" compose --project-name galroon --env-file "$context/deploy.env" -f "$context/compose.yaml" config --quiet
cp "$context/compose.yaml" "$base/compose.yaml"
cp "$context/deploy.env" "$base/.env"
sudo -n "$docker" compose --project-name galroon --env-file "$base/.env" -f "$base/compose.yaml" up -d --no-deps galroon
printf 'Deployed commit %s. Previous image (if any): galroon:previous\n' "$sha"
REMOTE
