# Edition comparison and work preference

The work page offers Compare editions and an explicit preferred-edition choice. A preference is catalog data belonging to the stable work ID, not a file command. It never chooses a latest/largest package, starts a job, changes the cover/title or marks a game playable. Get files still starts from a resource the user explicitly selects. A one-time alternative choice does not alter the preference.

Comparison displays recorded language/platform, date, edition type, user-recorded official/community origin, notes/patch requirements, linked patch count, shared edition membership, physical resource groups, source labels, sizes and indexed availability. Unknown values remain unknown. Counts are computed for the same work/edition binding; a Japanese edition cannot borrow the English edition's available files. Shared physical resources count once within a work's edition. Indexed availability is not full archive verification or proof of launchability.

An offline preferred edition keeps its identity and shows an unavailable message; alternative editions remain visible. Unavailable resource acquisition controls are disabled, while Core still independently validates availability and the reviewed manifest. Source status updates when catalog data is refreshed. Clearing or selecting preference preserves an unsaved work-note draft and uses the exact revision returned by that preference write; it does not replace drafts with a fresh server work object.

## Core contract

Schema v9 adds release platforms/origin/notes, work_preferences and preference_history. The work-list API includes nullable preferred_release_id. POST /api/works/{id}/preferred-edition requires a work revision and an explicit release_id string or null. Missing release_id is rejected. Foreign-edition choices, merged works and stale revisions are rejected. Repeating the current value with the current revision is a no-op. Choices write no file/resource/job/plan rows.

Existing edition API clients omitting the new optional fields preserve their stored values. New edition records default to unknown. Removing a preferred work/edition association requires explicitly clearing that work's preference first. Work merge keeps the destination preference and retains the source choice on its historical work; a new split work starts without a preference. The grouping preview explains this policy. Metadata refresh never replaces preferences. The normal consistent sanitized catalog backup includes preferences and history; restore checks their foreign keys.

List-item preferences and their override ordering remain part of TL1. This delivery establishes the work preference and explicit one-time resource choice; it does not claim the manual-list layer already exists. The broader RG-01 installed/list acceptance stays open in EXTENDED_ACCEPTANCE.md.

## Retained evidence

- Core suite: 65/65. Five new tests cover allowed/stale/foreign choices, no job creation/private state loss, clear-before-detach, backward optional-field preservation, metadata refresh, reopening, isolated backup restore, merge preservation and actual v8 migration defaults.
- Frontend: 19/19, including three new independent edition-availability cases for separate editions, shared/patch bindings and missing/unverified/unknown states.
- HTTP fixture confirms omitted release_id is rejected; read-only Web denial matrix includes the preference route (31 protected mutations).
- ui-preference/report.json: two works, three editions, four generated resources. UI chose Japanese, reloaded, took its generated source offline, showed 0/1 versus alternative 3/3 availability and kept the preference. One explicitly chosen alternative copy completed (35 bytes), with all original fixture hashes intact.
- UI cleared and reselected preference without losing a note draft, then saved it successfully. Platform/notes edits persisted. Desktop and 390px screenshots were inspected; no horizontal overflow, and no page errors after the final reload. The temporary offline fixture was reattached after checking hashes.
- The UI uses real frontend/Core HTTP with a test-only Tauri IPC bridge. This is not installed native-WebView acceptance or real archive-format verification. Actual sandbox_data originals remained untouched.

- Final schema v9 NSIS build and release-Core controller 2/2 passed; executable hashes are in ui-preference/build-report.json. Current production Web cookie login at 390px showed no preference mutation controls, rejected direct mutation with 403 and had no horizontal overflow.
