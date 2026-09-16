## M3 deletion impact compatibility guard — 2026-09-07

Tag deletion no longer substitutes zero when an older Core omits entity_count, smart_list_count or impact_revision. The dialog reports unavailable impact and disables confirmation until a compatible Core response is loaded. This does not change legacy server acceptance; it protects the current frontend from a misleading preview. TypeScript and extended mocked-browser tag flow passed, including omitted impact revision, explicit warning and disabled confirmation. impact-unavailable.png inspected.

Responsive evidence correction: direct DOM bounds show the Staff checkbox label is fully within390px (right=390), so the earlier screenshot suspicion of clipping was not confirmed. No CSS change made for it. reason-navigation verifier now asserts label bounds as well as document width;1200/390 keyboard navigation/back refresh passes again.

Source schema31; debug31 predates this frontend-only change, installed15 unchanged. Core177 present/last full176; frontend58 baseline. Goal active; originals/NAS untouched. All remaining M0–M6 gates retained.

## M3 work-related tag reasons — 2026-09-07

GET /api/works/{id}/related-tags returns personal tags grouped by ID with directly related person/character/company reasons and completeness. It uses only the existing spoiler-filtered relation projection and local assignments, no provider fetch or recursive propagation. Alias/role duplicates resolve to one entity reason. Deleted tags are excluded. Missing/stale/hidden relations remain incomplete; hidden IDs and names are absent from the response.

WorkExperience now renders Related personal tags and opens typed entity profiles from reasons. It refreshes after exploration loading and returning from a profile, so edited assignments can be reflected. The section displays incomplete data explicitly. Validation:3 focused entity_tags tests passed, including exact grouped reason counts and absence of hidden IDs. TypeScript passed. Mocked-browser verify-related-tags.cjs covers grouped reasons, incomplete notice, person/company callbacks and zero writes; related-tags.png inspected. Full WorkExperience back-stack and installed/native reason endpoint remain unverified this increment. Last full Core176 (177 present), frontend58 retained.

Schema31 unchanged; debug remains preceding reason/deletion-preview source, installed schema15 unchanged. Goal active; originals/NAS untouched. Remaining: complete reference-work reason integration, direct reason-to-tag search, relationship overrides, entity batch undo, scale and full native/mobile/privacy acceptance plus all other M0–M6 gates.

## M3 tag deletion smart dependency preview — 2026-09-07

Tag listing computes active smart-list dependency counts using typed PersonalTags/RelatedPersonalTags rules, recursively through groups and deduplicated per list; deleted lists do not count. Tag deletion preview shows affected smart-list count and repair behavior alongside works/entity counts. It submits the observed catalog impact_revision; Core rejects changed impact before mutation while preserving idempotent replay. Legacy callers without this optional field retain the prior revision-only behavior; no claim of strict impact review for old clients.

Validation:7 custom_tags Core tests passed (177 present; last full176). New test counts duplicate direct/related references once, excludes deleted lists, rejects a stale review without deleting, then accepts current review and replays it; affected smart list reports needs_repair. TypeScript passed. Mocked-browser verify-ui.cjs verifies two affected lists and submitted impact revision while preserving existing flows; delete-preview.png inspected. No new installed/native endpoint evidence for this increment.

Schema31 unchanged; debug schema31 predates this source change, installed schema15 unchanged. Goal active; originals/NAS untouched. Remaining related explanations/navigation/overrides, entity batch undo, native impact-review integration, options scale and all other M0–M6 gates retained.

## M3 native entity tags and Web enforcement — 2026-09-07

Built schema31 debug Core and ran verify_entity_tags against a fresh one-work generated catalog in a separate process. Real HTTP creates a tag, assigns a person, replays the same receipt, reads direct entity_count=1, and previews RelatedPersonalTags with expected local:w. A real Web login cookie reads the assignment; a POST removal receives403 and leaves revision2 unchanged. Stop/restart preserves assignment and receipt. Owner removal then yields preview total0. Final Core stop succeeded.

Evidence: test-output/mvp-goal/entity-tags-native/report.json; cargo run -p galroon-core --example verify_entity_tags -- test-output/mvp-goal/entity-tags-native target/debug/galroon-core.exe exited0. The verifier uses a generated ephemeral password and does not write password/cookie/token into its report. No network beyond loopback or original source files. This proves native API/guard behavior for a generated person relation, not installed WebView/profile UI, all entity kinds, five real works, or automatic recomputation/paging in this fixture. Existing independent nine-work rule matrix remains separate evidence.

Source/debug schema31 aligned; installed schema15 unchanged. Last full Core176 and frontend58 remain valid (no production source change). Goal active. Remaining relation explanations/navigation/overrides, dependency deletion preview, related-entity batch undo and all other Windows gates retained.

## M3 related tag expected-set matrix and deletion impact — 2026-09-07

Added a nine-work independent expected-set test covering direct studio/person/character tags, shared aliases/roles, multi-entity deduplication, hidden character/voice links, missing cache, stale positive evidence, stale empty evidence and no second-hop propagation through related works. Preview assertions compare exact work keys for any/none/all and explicit include-unknown. Removing a studio assignment changes the expected set and increments catalog invalidation. This is a generated deterministic Core matrix, not five real independently confirmed VNDB works.

Tag listing now reports directly assigned entity_count. Deletion review shows that count alongside linked works, without exposing entity names or deriving hidden relation identities. Mocked browser verify-ui.cjs checks the count, cancellation before mutation and existing delete/restore/batch/rename/undo flows; delete-preview.png inspected. TypeScript passed. Core176 library tests passed. Last full frontend58 remains baseline.

Source schema31, debug schema30 and installed schema15 unchanged. Remaining: native entity-tag API/read-only proof, relation explanations/navigation and overrides, smart dependency deletion preview, related-entity batch undo, complete privacy/native/mobile gates and the rest of M0–M6. Goal active; originals/NAS untouched.

## M3 personal entity tags and rule projection — 2026-09-07

Schema31 adds custom_tag_entities keyed by stable VNDB person/character/company IDs. Authenticated read and owner-edit routes retain tag revision CAS and digest receipts, reject stale/deleted tags, and retain assignments through tag deletion/restoration, reopen and backup restore. New links invalidate smart results. Smart facts project RelatedPersonalTags only through already-visible direct studio/person/character IDs; deduplicate tags and retain unknown completeness for missing/stale/hidden relations. This does not traverse related works or propagate tags recursively.

Entity profile My tags UI assigns/removes existing tags; read-only profiles show assignments without controls. Smart rule editor enables Related personal tags with existing tag choices. Validation:175 Core library tests and58 frontend tests passed; TypeScript passed. New Core tests cover receipts/stale identity, reopen/backup restore, deleted tag suppression/restore and hidden relation absence. Mocked browser verify-entity-tags.cjs covers assign/remove/reassign revision flow and read-only display; entity-tags.png inspected. Not yet integrated native API/UI evidence or full independent related-rule expected-set test.

Source schema31; debug remains schema30 and installed schema15. Do not open schema31 data with those binaries. Remaining: direct relation explanations/navigation, relationship overrides, tag deletion preview entity counts/undo coverage, option scale, independent full rule sets, installed/mobile/privacy audit and other M0–M6 gates. Goal active; originals/NAS untouched.

# Personal tag management

Collection > Tags > Manage custom tags supports selecting works, showing how many already have the selected tag, adding/removing the tag for all selected works, deleting and restoring tags. Select all results selects the current search result set; Clear selection resets all selection. VNDB tags are unaffected. Deleted tags keep their members but are omitted from normal collection filters; management can restore them.

Schema18 adds revision/deleted columns and tag_requests. API edits require revision, action, work_ids and request_id. Request digests reject ID reuse with another payload and return a persisted result for an identical replay. The whole batch is transactional; invalid/merged targets require refresh. Merge-aware removal clears memberships attached to prior identities resolving into the selected work. Requests are catalog metadata, never file operations. Deleted-tag management is Desktop-only.

Evidence: test-output/m3-tags/ with Core and browser verification. Existing normalization is NFKC, and the NFC contract still needs reconciliation. Dedicated old-version migration/deleted-state restore and installed UI acceptance remain open. This source change is not in the schema15 diagnostics installer.

Deletion review (2026-09-07): shows linked-work count, supports cancellation without mutation and pins the reviewed revision. Smart-list dependency preview remains pending until Lists exist. Evidence: test-output/m3-tags/delete-preview.png and verify-ui.cjs.

Rename tag preserves ID and memberships with revision and request-replay protection. Name comparison remains NFKC; NFC migration is pending. Focused Core4 and browser rename passed on 2026-09-07.

Schema19 supersedes the NFKC notes above: create/rename now use trimmed NFC case-insensitive keys, preserving internal spaces and fullwidth distinctions. Migration keeps deleted-tag state and memberships. Full Core136 passed. Similar-name warning remains pending.

Schema20: Undo last batch edit persists raw before/after links and relevant identity state. It survives reopen/backup but rejects stale revision, changed links or merged identity changes. Latest batch only, no redo stack. Core137 and browser undo passed.

Similar-name guidance now covers compatibility glyphs and repeated spaces in create/rename forms, without merging. Uses loaded tags; Core remains authoritative for exact NFC duplicates. Frontend48, TypeScript and browser warning/submission verified.

Batch receipts now report processed/changed/unchanged/failed counts for distinct canonical works. Successful atomic batches have zero failures; validation errors rollback, while transport errors remain uncertain until replay. Focused Core6, TypeScript and browser summary verified.
