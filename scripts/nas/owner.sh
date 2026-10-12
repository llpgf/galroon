#!/usr/bin/env bash
# Usage: scripts/nas/owner.sh <setup|pair --name NAME [--role desktop]|devices|revoke ID|session>
# setup reads one password line from stdin, or prompts without echo on a terminal.
set -euo pipefail
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
[[ $# -gt 0 ]] || nas_die 'Usage: owner.sh <owner args...>'
for arg in "$@"; do
    [[ "$arg" != --data-dir && "$arg" != --data-dir=* ]] || nas_die 'This wrapper fixes --data-dir to /data.'
done
command=(sudo -n "$NAS_DOCKER" exec)
if [[ "$1" == setup ]]; then
    [[ $# == 1 ]] || nas_die 'Usage: owner.sh setup (password via stdin only)'
    command+=(-i)
fi
command+=(galroon /opt/galroon/galroon-core owner "$@" --data-dir /data)
remote=$(nas_quote "${command[@]}")
if [[ "$1" == setup && -t 0 ]]; then
    IFS= read -r -s -p 'Owner password: ' secret
    printf '\n' >&2
    printf '%s\n' "$secret" | ssh -T "$NAS_TARGET" "$remote"
    unset secret
elif [[ "$1" == setup ]]; then
    ssh -T "$NAS_TARGET" "$remote"
else
    ssh -T -n "$NAS_TARGET" "$remote"
fi
