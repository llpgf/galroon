#!/usr/bin/env bash
# Usage: scripts/nas/fetch-artifact.sh <run-id|branch>
# Requires authenticated gh and shasum; a branch selects its latest successful NAS run.
set -euo pipefail
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
[[ $# == 1 ]] || nas_die 'Usage: fetch-artifact.sh <run-id|branch>'
cd "$NAS_ROOT"
run=$1
if [[ ! "$run" =~ ^[0-9]+$ ]]; then
    run=$(gh run list --workflow nas-image.yml --branch "$run" --status success --limit 1 --json databaseId --jq '.[0].databaseId // empty')
fi
[[ "$run" =~ ^[0-9]+$ ]] || nas_die 'No successful NAS artifact run found for this branch.'
conclusion=$(gh run view "$run" --json conclusion --jq .conclusion)
[[ "$conclusion" == success ]] || nas_die 'The selected run has not completed successfully.'
mkdir -p "$NAS_ROOT/target"
staging=$(mktemp -d "$NAS_ROOT/target/nas-fetch.XXXXXX")
gh run download "$run" --name galroon-nas-x86_64 --dir "$staging"
nas_artifact_check "$staging"
printf '%s\n' "$run" > "$staging/RUN_ID"
# Keep the previous download until the new one is complete and verified.
if [[ -e "$NAS_ROOT/target/nas" ]]; then
    mv "$NAS_ROOT/target/nas" "$staging.previous"
fi
mv "$staging" "$NAS_ROOT/target/nas"
printf 'Downloaded run %s, commit %s to target/nas/\n' "$run" "$NAS_SHA"
