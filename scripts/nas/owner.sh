#!/usr/bin/env bash
# Usage: scripts/nas/owner.sh <setup|change-password|pair --name NAME [--role desktop|web]|pairing-codes [revoke ID]|network show|network set ...|devices|revoke ID|session>
# setup and change-password read one password line from stdin, or prompts without echo on a terminal.
set -euo pipefail
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
[[ $# -gt 0 ]] || nas_die 'Usage: owner.sh <owner args...>'
for arg in "$@"; do
    [[ "$arg" != --data-dir* && "$arg" != --control-dir* ]] || nas_die 'This wrapper fixes --data-dir and --control-dir.'
done
command=(sudo -n "$NAS_DOCKER" exec)
if [[ "$1" == setup || "$1" == change-password ]]; then
    [[ $# == 1 ]] || nas_die 'Usage: owner.sh <setup|change-password> (password via stdin only)'
    command+=(-i)
fi
command+=(galroon /opt/galroon/galroon-core owner "$@" --data-dir /data --control-dir /run/galroon/control)
remote=$(nas_quote "${command[@]}")
if [[ ( "$1" == setup || "$1" == change-password ) && -t 0 ]]; then
    IFS= read -r -s -p 'Owner password: ' secret
    printf '\n' >&2
    printf '%s\n' "$secret" | ssh -T "$NAS_TARGET" "$remote"
    unset secret
elif [[ "$1" == setup || "$1" == change-password ]]; then
    ssh -T "$NAS_TARGET" "$remote"
else
    ssh -T -n "$NAS_TARGET" "$remote"
fi
