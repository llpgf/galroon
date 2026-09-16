# Reviewed resource file grouping

In Organize, use **Group files** on a resource. Filter and select its indexed files, choose a new group or another resource in the same source, review the exact members and references, then confirm. This changes catalog membership only. It never renames, copies or moves source files.

A new group inherits the source work/edition references. An existing destination keeps its own references. The preview displays these references and the number of files left behind. File IDs remain stable. Empty source groups remain available to history and the scanner but are hidden from the active resource list and work resource counts.

Schema v8 separates manual groups from the automatic unique source/path grouping rule. Several manual groups can share one directory. Each group stores a real common source-relative directory, preserving nested names when organizing or acquiring its members. Same-source files in different subdirectories can be combined. A manual group does not own every directory below that common parent: a managed destination inside it is allowed, while only reviewed member files move. Cross-source regrouping is rejected; physically moving files remains a separately reviewed operation.

The scanner preserves assigned file IDs. Newly discovered files still follow automatic grouping; manual grouping is a selection of current files, not an implicit rule that absorbs future files. Missing/unverified members can be classified in the catalog, but existing availability checks still block unsafe file operations.

Preview includes both resources, revisions, current file records, selected IDs and affected pending records in its digest. Confirmation rechecks the whole snapshot in one transaction. Duplicate, foreign or ambiguous file membership and changed previews are rejected. Approved/partial move plans affecting either resource must be finished first. Ready file plans and unfinished acquisition jobs for affected members are superseded; existing staging is retained and a fresh preparation task is required. Completed file history remains immutable. Undo of an old move rejects changed membership.

The v8 migration takes the existing sanitized backup before changes, rebuilds the resource table atomically, checks foreign keys before commit and reenables enforcement before normal operations. The new membership_history records before/after resource states. There is not yet a user-facing grouping-history undo viewer.

## Evidence

- Seven membership tests cover preview/no changes, split/rescan identity and hashes, merging and stale evidence, approved plan blocking and old task retirement, split/move/undo next to the original group, invalid selections and actual v7 table migration with retained references/enforced foreign keys.
- Full Core suite: 60 tests. Existing browser write-denial coverage now includes both regroup POST endpoints (30 protected mutation paths).
- `test-output/ui-membership/report.json`: generated 66-file fixture; UI splits 5 of 65 files, then combines an extra into the 6-file destination. Empty donor hides, rescan preserves IDs/membership/references, and all source hashes remain unchanged. Desktop and 390px screenshots inspected.
- UI evidence uses the actual frontend and HTTP Core with a test-only Tauri IPC bridge. It does not claim installed native-WebView acceptance. Supplied sandbox_data originals were untouched. These generated .bin members are not an ISO-format acceptance test.
