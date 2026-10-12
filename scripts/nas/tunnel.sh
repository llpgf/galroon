#!/usr/bin/env bash
# Usage: [GALROON_NAS=user@host] scripts/nas/tunnel.sh (foreground; Ctrl-C closes it)
set -euo pipefail
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
[[ $# == 0 ]] || nas_die 'Usage: tunnel.sh'
exec ssh -N -o ExitOnForwardFailure=yes -o ServerAliveInterval=30 -o ServerAliveCountMax=3 \
    -L 127.0.0.1:14800:127.0.0.1:14800 "$NAS_TARGET"
