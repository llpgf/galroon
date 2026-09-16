# Undo organization moves

In **Activity → File plans**, open a completed **Organize files** record and choose **Preview undo**. Review the reverse paths, approve that new plan, then execute it. Approval alone does not move files. Each original move links to its completed undo record, and the undo links back to the original move.

An undo returns the original selected files to their saved paths. It restores the resource's source root and relative path while keeping its ID, file IDs, work/edition references, metadata and notes. It does not revert subsequent title or metadata edits. Empty directories are left in place; no recursive cleanup is performed.

## Preconditions and recovery

- The original organization plan must be completed. Finish or resolve a partial original plan before preparing its undo.
- The resource must still be at that move's destination with the same file membership. For sequential moves, undo the latest location first.
- The original source root must be available and its saved mapping unchanged. Another resource or catalog file occupying the original location blocks undo.
- The moved files must match the original size and full SHA-256. Changed content or an occupied original file path is rejected. Links and cross-volume writes retain the existing Windows mutation restrictions.
- Execution checks catalog identity again and rechecks file hashes immediately before each no-replace move. A conflict after approval leaves that item untouched and produces a visible partial plan. After resolving the conflict, execute that same plan to continue.
- The existing prepared-operation journal handles a crash after filesystem rename but before the catalog update. Completed targets are reverified on retry. Competing approved reverse previews cannot successfully undo the same move twice.

These checks establish safe file relocation, not that game executables are runnable or unchanged by unrelated applications after completion.

## Persistence and API

Schema v7 adds `plan_origins` and `plan_reversals`. A new organization plan stores its original resource location atomically with its immutable item list and destination mapping. Upgrade creates a sanitized SQLite migration backup first. Old history remains readable; records without an original-location snapshot explicitly have no automatic undo. Missing information is not inferred from current folder names.

- `POST /api/plans/{id}/undo` creates a ready reverse plan; it does not approve or execute it.
- Existing `/approve` with the exact digest and `/execute` endpoints apply to reverse plans.
- `GET /api/plans/{id}` returns a single current record with operations, `has_origin`, `undo_of` and `reversed_by`.
- `GET /api/plans` returns up to 100 newest insertion-ordered records. Pass `?before=<last plan id>` for the next page. UI history retains loaded records when new entries arrive and opens each detail from the Core again. Read-only Web may inspect records but cannot create/approve/execute undo plans.

## Verified evidence

`cargo test -p galroon-core --lib` covers directory and single-archive resource restoration, stable IDs, rescan deduplication, approval gating, changed files, occupied originals, recoverable execution conflicts, filesystem-before-catalog recovery, sequential moves, membership changes, competing previews, root remapping and v6 history migration. The Web write-denial matrix includes the undo endpoint.

`test-output/ui-undo/report.json` records the live browser/Windows Core flow: older history loading (107 records, 106 explicitly synthetic empty records), visible collision rejection, approval without movement, explicit two-file restoration, original/undo history links, exact hashes and catalog identity, and desktop/390px layouts. After loading 108 records, adding three unapproved generated-file previews retained all 111 records without losing a pagination boundary row.

Only generated fixtures were modified. The supplied game originals were not touched. Native installed-desktop and broader installation/fault-matrix acceptance remain separate gates in `WINDOWS_ACCEPTANCE.md`.
