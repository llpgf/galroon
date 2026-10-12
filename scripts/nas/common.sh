#!/usr/bin/env bash
# Usage: source this file from a scripts/nas entry point; no remote actions here.
set -euo pipefail

NAS_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
NAS_TARGET=${GALROON_NAS:-galroon-dev@192.168.50.154}
NAS_BASE=/volume1/docker/galroon
NAS_DOCKER=/usr/local/bin/docker

nas_die() { printf 'Error: %s\n' "$*" >&2; exit 1; }

# ssh concatenates arguments into a remote shell command. Quote for POSIX sh,
# including spaces, apostrophes and shell metacharacters; never use eval locally.
nas_quote() {
    local arg escaped_quote="'\\''"
    for arg in "$@"; do
        printf "'%s' " "${arg//\'/$escaped_quote}"
    done
}

nas_artifact_check() {
    local dir=$1
    [[ -f "$dir/galroon-core" && -f "$dir/web/index.html" && -f "$dir/BUILD_SHA" && -f "$dir/SHA256SUMS" ]] || nas_die 'Incomplete artifact; run fetch-artifact.sh first.'
    NAS_SHA=$(cat "$dir/BUILD_SHA")
    [[ "$NAS_SHA" =~ ^[0-9a-f]{40}$ ]] || nas_die 'Invalid BUILD_SHA in artifact.'
    (cd "$dir" && shasum -a 256 -c SHA256SUMS) || nas_die 'Artifact checksum verification failed.'
}
