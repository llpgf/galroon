## Schema40 separate Core and release-scale acceptance — 2026-09-07

Rebuilt debug and optimized release Core40, product version still0.0.1; installed35 and historical installed15 are unchanged. Binary identities are saved in `test-output/mvp-goal/schema40-binaries.json`: current release SHA256 `032ee802ee8beffa4e7c4890674783becda63af870000c5d8f3b61a20413e208`; debug `d77a32e66645879f85c9e95c4afe79cd12eec517caee9ec3255ea18120e24c85`. Earlier schema40 debug HTTP runs used2000545e... and remain separate evidence.

Current release passed `editor-release-schema40-r1/report.json`: exact add/rebind/hide/undo, actual alias, forward/reverse spoilers, stale preview rejection, receipt/history across restart and backup/offline restore, Web private routes403 and unchanged raw cache. It also passed NEW `verify_managed_http` / `managed-release-schema40-r1/report.json`: generated source file, competing-root approval rollback, real organize/undo, organizing facts unorganized→pending→organized→pending→unorganized, pinned smart page after move/restart/restore, HTTP backup export/local restore, restored active-root enforcement and acquisition to a separate destination without changing source organizing status. Source hash unchanged after reviewed undo/copy. These are separate executable HTTP checks, not installed Desktop UI.

The initial debug100000-work compute exceeded the verifier's20-second request deadline (`smart-native-schema40-100k-r1`); after rule-directed projection, debug with the organizing condition also exceeded it (`r2`). Both failures and cancellation logs are retained; neither is labelled passed. Smart evaluation now derives required fields once per rule and omits unused organization, personal-tag, source-location, relation and edition queries. Related-tag dependencies still load their required relation projection; direct all-facts access remains available. Full Core265/265 passed after this change (`smart-projection-core-tests.log`). A read-only2000-work diagnostic found larger statement caches did not help; cache capacity was not changed.

The actual release build passes the unchanged20-second/300ms checks with the organizing predicate explicitly included alongside status, relation tags/brand and same-edition language/availability: `smart-release-schema40-100k-r1/report.json`.100000 generated works each have cached metadata, edition and resource/file records, but zero physical game files. Manual compute4337ms; concurrent result read2ms;2000 page reads p95=2ms; full100000-member conversion3139ms; automatic update6654ms; pinned pages and idempotent conversion survive restart. This establishes release smart-list performance, not a claim that the debug deadline regression is fixed or that every collection interaction scales.

Machine: i7-14650HX,16 cores/24 threads,68413153280 bytes RAM, Windows10.0.26200.20 process samples recorded max observed working set415203328 bytes, OS lifetime peak working set520175616 bytes (about496MiB), private bytes421576704; `memory-summary.json`, `memory-samples.json`, `machine.json` beside the release-scale report. RSS acceptance threshold remains unagreed. Frontend84/build evidence unchanged. All validation Cores stopped through their identity handshake; verified no galroon-core.exe after completion. Full installed/product-version upgrade, broad catalog UI/RSS/cold-start, human matching/reference gates, remaining file faults and other M0–M6 requirements remain. Goal active.

## M3 organizing-state smart rules — 2026-09-07

Source schema40 enables `organizing_state` in the rule editor and Core projection. Values are per linked resource: organized means all recorded member locations are in the selected managed root; unorganized means outside it; pending means an approved/executing organize or undo plan; partial means a partial plan or members split across the active managed root boundary. Partial plans take priority, then pending plans, then locations. A ready preview is not approval. Several resources can supply several states. No linked resources, empty membership and legacy managed files without an active selection stay unknown; observed positive values do not make absent values known when any resource is unknown. Availability/matching/favorite/download status is independent, and these predicates never inspect or move files.

Schema40 installs plan/location/managed-setting invalidation plus a resource-plan lookup index and invalidates previous computed input once. Old snapshot pages remain immutable. Full Core **265/265** passed (`test-output/mvp-goal/smart-organization-core-tests.log`), frontend **84/84** and build passed. New cases cover source/managed/mixed/multiple/shared/empty/pending/partial/legacy-unknown states, rejected unsupported values, plan changes causing recompute, pinned prior pages, no operations/jobs from computation, backup/restore and39-to40 migration/reopen. Existing100000-work performance numbers predate this projection and do not certify its added query cost.

Real SmartListsPanel/SmartRuleEditor component fixture UI: select Organizing state, select pending, save and inspect the submitted typed rule. Final desktop capture `output/playwright/smart-organization-1200-final.png` has fully readable values and explanation; readonly390 capture `smart-organization-390.png` wraps its summary and exposes no edit/compute/write controls. Generated fixtures r2/r3/r4 are retained; the initial inherited fixture used a stale preview response shape and was replaced before acceptance. This is mocked-API component UI, separate from Core catalog tests; installed/HTTP combined rule execution remains required. Source40/debug38/installed35 are distinct. Goal active; every original gate remains.

## M3 native external reasons and refreshed test baseline — 2026-09-07

Current debug Core rebuilt. verify_entity_tags extended with cached external v99 and uncached v98: real owner and Web cookie requests return identical external reasons, missing cache yields incomplete empty reasons, and read-only SQLite proves only the original work remains (no v99 collection entry). Existing assignment replay,403 Web write denial, restart and removal assertions still pass. Core stopped cleanly. Evidence: test-output/mvp-goal/reference-reasons-native/report.json, verifier exited0.

Full178 Core library tests and58 current frontend tests passed. Source/debug schema31 aligned; installed schema15 unchanged. This remains generated separate-Core HTTP evidence, not an installed desktop or physical phone acceptance. No original game/NAS changes and no secrets in report.

Goal active. Next outstanding product gaps include personal-tag exploration/search, relationship overrides/history, entity batch undo, option scale, and the remaining M0–M6 acceptance requirements. Do not repeat the now-verified native reference reason check without a relevant change.

## M3 external work related-tag reasons — 2026-09-07

Added authenticated GET /api/vns/{id}/related-tags for valid provider VN IDs. Local and external work reasons share the same cache-only projection. No fetch, work insertion or recursive propagation occurs; missing cache returns incomplete empty reasons. ReferenceWork renders RelatedTags and opens typed entity callbacks. Parent exploration stacks pass active state so returning to a reference refreshes reasons, while hidden references do not issue reason reads.

Validation:4 focused entity_tags tests passed (178 present; last full176). New external cache test proves exact company reason, unknown missing cache, zero database changes and zero works inserted. TypeScript passed. Mocked full ReferenceWork verify-reference-reasons.cjs checks exploration then one reference reason request, company callback and zero writes; reference-reasons.png inspected. This is source/mock evidence; current native binary predates the new reference route. Last frontend58 baseline retained.

Schema31 unchanged; installed schema15 unchanged. Goal active. Remaining reference native endpoint/return-path integration, reason-to-tag search, relationship overrides, entity batch undo, full scale/privacy and all other M0–M6 gates. Originals/NAS untouched.

## M3 native reasons and responsive return navigation — 2026-09-07

Rebuilt current schema31 debug Core. Extended verify_entity_tags compares exact owner/Web related-tag reason payloads, asserts complete direct person reason and verifies removal clears reasons after restart. Fresh entity-reasons-native/report.json passes alongside existing write403, receipt and preview checks; Core stopped cleanly. No credentials in report.

Full WorkExperience mocked-browser fixture verifies keyboard activation of reason -> person profile -> Back; returning refetches reasons and removes the now-absent tag. Passed at1200px and390px with zero writes and no horizontal overflow; reason-navigation-390.png inspected. Initial fixture used an invalid string tags field; corrected to array and moved to a fresh module URL after Vite retained the earlier test-output transform. Final verifier uses reason-navigation-current.html/current.tsx. This is generated UI evidence, not integrated installed WebView or five real profiles.

Source/debug schema31 aligned; installed schema15 unchanged. Last full Core176 (177 present) and frontend58 remain baselines, no production code change this increment. Goal active; original sources/NAS untouched. Remaining reference reasons/tag search, relationship overrides, entity batch undo, broad privacy/performance and all other M0–M6 gates retained.

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

## M3 frontend manual computation cancellation — 2026-09-07

Smart list detail now exposes Cancel computation while a manual computation is pending. It aborts the compute transport, preserves the displayed snapshot, ignores a late result and retains the request receipt for same-body retry. Notice explicitly says waiting stopped and a result may already have completed; it does not promise rollback after commit. Unmount also aborts. Dedicated computeRules API retains mutation identity/uncertain-outcome handling; intentional abort does not trigger reconnect. Ordinary mutations remain unchanged.

Validation: TypeScript passed;58 current frontend tests passed (test-output historical copies excluded). Transport regression covers no reconnect after abort and uncertain classification after session drift. Mocked-browser verify-manual-cancel.cjs proves one signal abort, old result retained, same request_id retry and recovered result; manual-cancel.png inspected. This is UI/mock API evidence, paired with prior separate-Core manual-native-cancel evidence, not integrated installed WebView cancellation. Last full Core173; no Core change this increment.

Source/debug schema30; installed schema15 unchanged. Goal active. Remaining commit-phase/graceful-stop, installed/mobile/accessibility and all other M0–M6 gates retained. Originals/NAS untouched.

## M3 native manual HTTP cancellation — 2026-09-07

Added smart.manual.started/completed/cancelled/failed diagnostics with elapsed time only. New verify_manual_cancel starts two HTTP computations against100000 generated works in a separate debug Core, rejects a third request at capacity, aborts both clients while neither has completed, and observes both cancellations within63ms. Read-only SQLite inspection proves zero snapshot headers/entries/receipts. Retrying the original request ID completes100000 entries and exactly one receipt, proving both capacity and list claims are released. Final Core stop succeeded.

Evidence: test-output/mvp-goal/manual-native-cancel/report.json; cargo run -p galroon-core --example verify_manual_cancel -- test-output/mvp-goal/manual-native-cancel target/debug/galroon-core.exe exited0. Background invalidation is deliberately clean in this cancellation fixture; concurrent background behavior is covered separately by manual-concurrent-exclusive. This is HTTP disconnect evidence, not a frontend Cancel button, commit-phase cancellation timing or graceful Core stop with active manual requests. Last full173 Core tests remain the baseline; only bounded diagnostics changed production behavior this increment. Frontend unchanged, last full57.

Source/debug schema30; installed schema15 unchanged. Goal active; originals/NAS untouched; zero generated source files. Existing M0–M6 gates retained.

## M3 manual computation concurrency — 2026-09-07

Manual smart computation now evaluates through a separate read-only SQLite transaction in spawn_blocking, with two bounded permits and cooperative cancellation. Receipt replay precedes evaluation; publication checks the catalog revision and rolls back stale/cancelled work. Root observations are recorded before fixing the evaluation revision, avoiding false invalidation by the first background observation. A per-catalog/list RAII claim prevents background and manual computation of the same list overlapping; competing manual work receives an explicit retry message. Main mutex is released during evaluation; publication/insertion still holds it.

Native failures retained: manual-concurrent-populated returned400 because the initial root observation changed the revision; manual-concurrent-roots exceeded the original20s request limit while duplicate background/manual evaluation ran. Fresh manual-concurrent-exclusive passed without increasing limits:100000 populated works, manual compute14179ms, concurrent result read3ms, automatic update16098ms, conversion4035ms,2000 pages p95=4ms. Full100000-entry conversion, old snapshot, receipt replay and library identity survived restart. Evidence: test-output/mvp-goal/manual-concurrent-exclusive/report.json. Only generated database/cache/resource records; zero source files.

Validation:173 Core library tests passed, including stale publication, receipt replay, initial root observation, cancellation and claim release/isolation. Frontend unchanged (last full57). Source/debug schema30; installed schema15 unchanged. No new native UI/installer evidence. Remaining gates include commit-phase latency, manual HTTP cancellation/stop, diverse data/RSS, related tags/organizing rules and all other M0–M6 requirements. Goal remains active.

## M3 native HTTP preview cancellation — 2026-09-07

Preview tasks emit bounded started/completed/cancelled/failed diagnostics with duration only, without rule contents or result titles. Built current debug Core and ran new verify_preview_cancel against a fresh100000-work generated catalog. Two real HTTP previews were observed started before cancellation; a third concurrent request was rejected by capacity control. Aborting both HTTP clients produced two server cancellation events within91ms. A subsequent preview succeeded with total100000/sample10, demonstrating permit release. Final Core stop succeeded.

Validation: cargo run -p galroon-core --example verify_preview_cancel -- test-output/mvp-goal/preview-native-cancel target/debug/galroon-core.exe exited0; report.json stored there. All171 Core tests passed; last frontend57 remains valid (no frontend change this increment). This proves HTTP abort/permit recovery on local debug Core, not WebView/installer integration, blocked filesystem calls, or graceful-stop-with-live-preview behavior. Existing remaining gates are preserved.

Source/debug schema30 aligned; installed schema15 unchanged. Goal active; originals/NAS untouched; no source files generated.

## M3 superseded preview request cancellation — 2026-09-07

SmartRulePreview creates an AbortController per effect and aborts on definition/Retry replacement or unmount. Dedicated previewRules transport exposes cancellation only for this non-mutating preview route; ordinary mutation requests retain existing uncertain-outcome semantics. Intentional preview abort does not mark Core offline or trigger reconnect. Preview session/identity changes are treated as reads and discard stale results. Existing live flag still guards responses racing cancellation.

Validation:57 frontend tests and TypeScript passed. New tests cover abort, already-aborted calls, no reconnect/timer and old-session preview rejection. Mocked-browser verify-preview-cancel.cjs confirms the outgoing preview signal was aborted exactly once during sort change, only the current response displays; preview-cancel.png inspected. This is browser cancellation evidence, not proof that native HTTP cancellation immediately releases the server permit; that separate-process gate remains open.

Core unchanged at schema30 (171 present; last full170). Debug binary still preceding preview-worker source, installed schema15 unchanged. Remaining manual compute/commit responsiveness, native preview cancellation/stop, and other MVP gates retained. Goal active; original files/NAS untouched.

## M3 background readonly rule preview — 2026-09-07

Preview HTTP handler now runs evaluation in spawn_blocking with a separate read-only SQLite transaction, rather than holding the main catalog mutex. A process-wide two-permit semaphore bounds active computations; capacity is acquired before spawning, retained by the blocking task and released on completion. Excess requests receive an explicit busy error. Handler drop signals cooperative evaluation cancellation; individual filesystem/SQL calls remain noninterruptible. Frontend adds Retry for preview failures and preserves the existing obsolete-response guard.

Validation:26 focused smart Core tests passed (171 present; last full170). New handler test verifies result/no main-connection writes, capacity rejection and cancellation guard. TypeScript passed. Mocked browser verify-preview-retry.cjs proves capacity message then successful Retry; preview-busy.png inspected. No new native disconnect/stop/large-preview concurrency evidence. Frontend still ignores obsolete responses rather than aborting their HTTP requests; busy Retry makes this bounded behavior recoverable, not complete latest-request cancellation.

Source schema30 unchanged; debug executable remains preceding off-lock-worker build, installed schema15 unchanged. Remaining manual compute/commit responsiveness, native preview lifecycle/cancellation, frontend cancellation and wider MVP gates remain open. Goal active; originals/NAS untouched.

## M3/M5 background computation without main catalog lock — 2026-09-07

Production smart worker now plans briefly under the catalog mutex, evaluates through a separate SQLite read-only transaction, then checks the catalog revision/active jobs again before transactional publication. A changed catalog or stop request discards prepared results; later ticks retry from current inputs. Root filesystem observations also run outside the shared mutex. Common build persistence retains definition validation, receipts and cancellation checks. This enables reads/writes during evaluation; final result insertion still requires the main mutex, and manual compute/live preview/conversion remain synchronous.

Validation: all170 Core tests passed. New offlock test changes the catalog after evaluation and verifies no stale snapshot commits; cancellation before publication also leaves the old result, then retry commits current facts. Current debug binary rebuilt. Real separate-process verify_smart_stop with100000 works made5 pinned-page HTTP reads while computation was observed active: maximum8ms, stop1ms, baseline100000 entries retained, restart recomputed. Evidence test-output/mvp-goal/smart-concurrent-100000/report.json. Existing verifier now asserts reads overlap computation and each<=300ms; it retains stop/restart/SQLite count assertions.

Remaining: manual compute/preview asynchronous handling and cancellation, commit-phase responsiveness, large populated concurrent fixture, blocking syscall cancellation, NSIS/native desktop and other existing MVP gates. Source/debug schema30 unchanged; installed schema15 unchanged. Goal active; original files/NAS untouched.

## M3/M5 populated scale bottleneck fixed — 2026-09-07

The lifecycle verifier accepts a populated mode with one cached VN/tag/studio payload, edition/language, main resource binding and missing file record per work, plus one generated absent root. Rule combines backlog + source tag + same-edition Japanese/missing. No actual source files are created. Full paging, conversion, old snapshot and restart assertions remain.

10000 populated works passed: compute2902ms, update4681ms, conversion190ms, page p95=3ms. Initial100000 populated run failed the existing20s compute HTTP timeout (smart-populated-100000); failure is retained, not relabeled as success. Production optimization reuses prepared statements for per-work fact queries, identity resolution and snapshot insertion, avoiding repeated SQL compilation. No timeout/limit/rule/transaction semantics were relaxed.

Fresh100000 populated rerun passed: compute14694ms, update15663ms, conversion4292ms,2000-page p95=4ms; all100000 frozen results copied, retry/restart preserved. Evidence test-output/mvp-goal/smart-populated-100000-cached/report.json. Current debug Core rebuilt; all169 Core tests passed. This does not prove frontend concurrent-read responsiveness: long work still holds the shared DB mutex, and normal frontend GET timeout is15s. Remaining read responsiveness/async computation, cancellation during blocking SQL/filesystem, diverse cache/edition fanout and installed acceptance remain explicit gaps.

Source/debug schema30, installed schema15 unchanged. Goal active; original games/NAS untouched.

## M3/M5 native stop during smart computation — 2026-09-07

Background recomputation now emits bounded diagnostics smart.compute.started/completed/cancelled with list identity, revision or elapsed time, excluding rule contents and work titles. New verify_smart_stop seeds100000 generated works plus a complete baseline snapshot, observes a started event from the separate Core with no completed event, then requests graceful stop. Measured stop response44ms. Read-only SQLite inspection after shutdown confirms exactly one baseline header/receipt and100000 entries: cancelled work leaves no partial snapshot. Restart preserves identity and computes a second complete snapshot while retaining the first.

Validation: current debug Core built; cargo run -p galroon-core --example verify_smart_stop -- test-output/mvp-goal/smart-stop-100000 target/debug/galroon-core.exe exited0 and stopped both instances. Evidence report.json in that directory. Focused3 smart_updates tests passed; last full169 remains prior to diagnostics-only change. Earlier unit test covers cancellation after25 inserted rows; this native test stops soon after observing the worker start, not specifically during row insertion. Blocking filesystem metadata/single SQL, complex relation load, UI cancellation and NSIS acceptance remain open.

Source/debug schema30 unchanged; installed schema15 unchanged. Goal active; originals/NAS untouched. No source files in generated fixture.

## M3/M5 smart-list scale measurement — 2026-09-07

verify_smart_lifecycle now accepts an optional55..100000 generated-work count (default55), seeds in a transaction, enumerates EVERY fixed-snapshot page with global duplicate rejection/count verification, and reports compute/update/conversion/page timings. It retains real HTTP/background-worker/full-copy/restart/idempotency assertions. No acceptance threshold or production limit was relaxed; each HTTP request retains20s timeout and background update30s deadline.

Fresh independent debug-Core measurements:
-10000 works,200 pages: page API p95=15ms, compute1134ms, background update1537ms, full conversion219ms.
-100000 works,2000 pages: page API p95=3ms, compute11074ms, background update12758ms, full conversion4603ms.
Both preserve pinned pages and complete converted membership after restart, with no repeated result identities. These measured page reads satisfy the local warm-page <=300ms initial target for THESE fixtures. Timing variation is not a claim that larger catalogs are faster.

Commands: cargo run -p galroon-core --example verify_smart_lifecycle -- test-output/mvp-goal/smart-scale-10000 target/debug/galroon-core.exe 10000; repeat with smart-scale-100000 and100000. Reports are under those fresh output directories. Both exited0 and stopped their Core. Fixtures have no source files, edition bindings or metadata payloads; they evaluate status rules. They do not prove populated relation-cache/resource/complex-rule costs, UI responsiveness during long DB work, memory budget, cancellation under load, native desktop or installer acceptance. Those gates remain open.

Core/frontend product source unchanged, source/debug schema30, installed schema15; last full169 Core/55 frontend. Goal active; originals/NAS untouched.

## M3 independent Core smart-list lifecycle — 2026-09-07

Built current debug Core (schema30). New reproducible example verify_smart_lifecycle creates55 generated database works in a fresh isolated directory, starts a separate Core process through named-pipe lifecycle, and uses authenticated HTTP to create/compute a smart list. Changing one work triggers the actual background worker; the old55-result snapshot still returns its5-entry continuation while the new snapshot has54. Full conversion copies55; stop/restart preserves library identity, old paging and the converted list; repeating the conversion request returns the same receipt. Final Core stop succeeded. No credentials are written to the report.

Validation: cargo build -p galroon-core --bin galroon-core; cargo run -p galroon-core --example verify_smart_lifecycle -- test-output/mvp-goal/smart-native-30 target/debug/galroon-core.exe; full169 Core tests passed. Evidence test-output/mvp-goal/smart-native-30/report.json. This is real debug-Core HTTP/worker/restart evidence with55 generated works, NOT installed-NSIS/desktop UI, remote pairing, large-library load or mid-computation stop acceptance. Earlier mocked browser evidence remains separately scoped.

Source/debug schema30 now aligned; installed executable remains schema15. Remaining MVP gates preserved, including installed combined flows, preferred-edition/action integration, relationship overrides, tag fields, scale/cancellation and remaining fault/desktop/mobile gates. Goal active. Original game files/NAS untouched; this fixture has no source files.

## M3 list-to-work exploration navigation — 2026-09-07

Manual and smart result titles now open a shared list exploration stack. Local works reuse WorkExperience, external references reuse ReferenceWork and existing character/person/company/search screens. List panels remain mounted while exploring; return restores the original scroll position and invoking button focus without reloading/replacing the snapshot or loaded pages. Missing local references keep a returnable missing state. Manual merged entries prefer the reported merge target. Classification/navigation does not mutate files or add external works.

Validation: TypeScript and55 frontend tests passed. Mocked browser verify-navigation.cjs verifies smart external result -> reference introduction -> back, same single result request and focus restoration, zero writes; list-navigation.png inspected. No new native/live-Core evidence. Local exploration currently does not embed main-page editions/actions; list preferred-edition integration, full multi-hop/manual/merged/mobile gates remain open. Core schema30 unchanged (169 present; last full167), debug25/installed15 unchanged. Goal active; original games/NAS untouched.

## M3 cached external-reference smart facts — 2026-09-07

Source schema30 invalidates earlier computations once. StoredReferences evaluation now projects existing exploration cache for validated vndb: references: tags, studios, people/roles, visible characters and a fresh recorded release year. It retains saved list titles and IDs; local status, personal tags, source locations and edition availability remain unknown. No provider call, work insertion or file job is performed. Same-identity dedup remains in force across manual lists and local works. Collection-only scope excludes external references. Editor relation options also enumerate active manual-list references with existing caches.

Validation:24 focused smart Core tests passed (169 present; last full167). New generated fixture verifies cached external match/count/title, cross-list dedup, collection-only exclusion, unknown local facts, expiry semantics and unchanged works count. Frontend source unchanged; no new browser/native evidence. Source schema30; debug executable schema25 and installed executable schema15 unchanged. Remaining: related personal tags, organizing facts, relation overrides, broader spoiler preferences, option search/scale, navigation and installed combined gates. Goal active; original games/NAS untouched.

## M3 stable VNDB tag rules — 2026-09-07

Source schema29 invalidates older computations once. Work-detail fetch now requests tags.id/name/spoiler/lie; shared smart relation projection and editor enable direct VNDB tag IDs. Two tags with the same name retain distinct IDs. Only explicit spoiler0, lie=false records enter facts/options; hidden, malformed or missing fields cannot prove absence. Existing cache without tag objects stays unknown until ordinary expiry or explicit detail refresh; no name-to-ID guessing or unsolicited bulk provider refresh. Parent-tag propagation, spoiler preference levels and external-reference completeness remain open.

Validation:23 focused smart Core tests passed (168 present; last full167), TypeScript passed, mocked browser verify-source-tags.cjs saved/reopened g1 while displaying Adventure; smart-source-tags.png inspected. One public read-only VNDB v17 request verified203 tag objects with valid IDs and spoiler/lie fields, without catalog mutation. API contract checked against https://api.vndb.org/kana#vn-fields (direct tags only). No new installed/native validation. Source schema29; debug executable schema25 and installed executable schema15 unchanged. Original games/NAS untouched; Goal active.

## M3 cached relation rules and editor values — 2026-09-07

Source schema28 invalidates earlier computations once for the expanded fact projection. Shared smart_relations projection reads existing work cache: validated provider IDs for studios, staff, voice credits and characters; person-role values bind a specific person ID to a role (for example s1:scenario), avoiding cross-person role splicing. Missing/malformed/stale arrays cannot prove absence. Only characters with an explicit matching-work spoiler0 relation, and their linked voices, enter the projection/options. Hidden or unproven relations keep the corresponding sets incomplete. Fresh complete sets contribute the existing24-hour expiry deadline.

GET /api/smart-lists/options supplies named values from cached collection works, and the rule editor enables Studios, People, People and roles, Characters. This does not fetch new metadata. Full external-reference facts, relation overrides, user-selectable spoiler levels and option pagination/search/role-label polish remain open; current option enumeration is unpaged and requires scale work. Source Tags, related personal tags and organizing state remain unavailable.

Validation: all167 Core tests and TypeScript passed. New projection tests cover stable composite role IDs, hidden voice/character exclusion, malformed/empty/partial/stale arrays. Mocked-HTTP browser verify-relations.cjs proves named value selection, stable composite-ID save and edit reload; smart-relations.png inspected. No new installed/native acceptance. Source schema28; existing debug binary schema25 and installed binary schema15 unchanged. Original games/NAS untouched. Goal active.

## M3 time-based smart result invalidation — 2026-09-07

Source schema27 records the earliest fact-cache expiry per snapshot. Fact projection uses one computation timestamp; currently typed developer cache completeness expires after24 hours. Future-dated timestamps cannot establish completeness. The worker recomputes when a latest snapshot expires even without catalog mutations, respects the existing error retry delay, and keeps pinned pages unchanged. Results expose pending until refreshed. Expired facts retain known positive IDs but cannot prove negative absence. Migration invalidates earlier computations once so old snapshots acquire deadlines on recomputation; normal reopen does not repeatedly invalidate them.

Validation:20 focused smart tests passed (165 Core present; last full163 predates this increment). Deterministic clock test proves pre-expiry no-op, exact-boundary recomputation from a proven negative brand condition to unknown in a single-work fixture, no repeated clean computation, and old pinned page retention. Upgrade/reopen test proves one-time invalidation. No frontend/render changes, no new installed/native acceptance. Source schema27; debug executable still prior schema25 and installed executable schema15. Originals/NAS untouched. Remaining: complete typed/external facts, long-term receipt growth, scale and blocking cancellation, navigation/polish and native combined validation. Goal active.

## M3 snapshot cache retention — 2026-09-07

Source schema26 adds cache leases and expiry tombstones. Successful page reads renew a 30-minute lease (writes throttled to once per minute); the background worker prunes at most20 eligible snapshots per tick inside a transaction, retaining the latest3 per list and snapshots created/read in the last30 minutes. Only rebuildable entry payloads are removed. Headers and request receipts remain: expired page/compute replay/conversion explicitly fail rather than silently switching results. Existing Refresh opens the latest snapshot. Database free pages may be reused; no VACUUM or promise of immediate disk-size reduction. Tombstone/receipt growth, large-catalog query cost and blocking-call cancellation remain open.

Validation: all163 Core tests passed, including18 smart tests. Generated retention fixture proves active continuation, latest-result protection, expired replay/conversion refusal and removal of old payloads. No frontend changes or new browser/native validation in this increment. Source schema26; debug executable remains previous schema25 build and installed executable remains schema15. Do not open a newer catalog with either older binary. Goal remains active; game sources/NAS untouched.

## M3 cancellable recomputation and Core stop regression — 2026-09-07

Smart evaluation checks the stop callback during candidate loading, evaluation, result insertion and before receipt creation. Worker cancellation rolls back and does not persist an error/retry state. Local-Core stop now signals the smart worker BEFORE waiting for the DB lock; a refused stop starts a replacement worker so updates are not permanently disabled. Blocking root metadata calls and single expensive SQL calls are still not interruptible and remain part of scale/native fault validation.

Validation: focused smart17 tests passed (162 Core present; last full161), including cancellation exactly after25 inserted rows with no partial snapshot/receipt and old result retained. cargo check and debug Core build passed. Fresh independent-process verify_lifecycle run passed: same-instance reconnect, wrong executable and second writer refusal, active scan stop refusal,1200 generated files indexed, stop/restart and catalog identity preservation. Evidence test-output/mvp-goal/lifecycle-smart-stop/report.json. Existing lifecycle artifact allowlist now includes the established diagnostics logs directory; no other assertion removed. This regression has no large smart list during the native stop; mid-write cancellation is unit-level evidence, not that combined gate.

Source schema25/debug binary updated; installed schema15 unchanged. Remaining: retention/scale, complete relation facts and native combined UI/worker validation. Goal active; source games/NAS untouched.

## M3 automatic smart-list updates — 2026-09-07

Schema25 adds transactional catalog invalidation triggers and persisted per-list computation status. Core/local-Core start a stopped-aware background worker: poll3s, skip a busy database/active jobs/executing plans, compute one dirty active list, commit a new snapshot/status together. Root observations also invalidate; failed calculations preserve old snapshots and retry after60s or a new change, logging diagnostics. Rolled-back catalog changes do not dirty persisted revisions. Stop signals prevent starting/committing a computation; interrupting expensive computation/root I/O mid-flight and scale bounds remain to validate.

Result reads expose pending/error/newer-snapshot state. SmartListsPanel polls5s and preserves loaded entries/cursor; Show updated results explicitly opens the new snapshot. Read-only clients only read results. Validation: full161 Core tests and TypeScript passed; generated worker test verifies initial compute, no-op when clean, transaction rollback, recompute after status change, old snapshot retention and stopped no-write behavior. Mocked-browser verify-updates.cjs verifies banner without replacing old entries, explicit switch, no Web writes; update-notice.png inspected. Native long-running worker/stop, retention/scale and complete relation gates remain open. Source schema25; installed schema15 unchanged; originals/NAS untouched. Goal active.

## M3 unsaved smart-rule live preview — 2026-09-07

Added POST /api/smart-lists/preview using the shared evaluator, with full total/unknown counts and first10 result sample; no list/snapshot/receipt writes. SmartRulePreview debounces edits500ms, clears old results while loading, and suppresses obsolete responses after rule changes/unmount. Current saved compute uses the same evaluation function. Rule editor now previews unsaved conditions without requiring Save.

Validation: focused snapshot6 passed (160 Core tests present; last full157). Database total_changes assertion proves unsaved preview does not persist; unknown years remain unknown. TypeScript passed. Mocked browser verify-preview.cjs deliberately delays the old response and verifies only the newer preview appears; live-preview.png inspected. Existing verify-smart.cjs passed with preview endpoint isolated from mutations. Schema24/installed15 unchanged. Remaining: automatic recompute/freshness/retention, complete typed/external facts and spoiler handling, cancellation/scale, work navigation and native integrated validation. Goal active; originals/NAS untouched.

## M3 smart Lists frontend — 2026-09-07

Lists now has Manual/Smart tabs. SmartListsPanel and typed SmartRuleEditor expose create/edit/name/description/pin, collection/stored-reference scope, title/year sort, two-level All/Any groups, set modes/exclusion/unknown, same-edition conditions and a readable rule summary. Unavailable identity/relationship fields are disabled rather than presented as reliable. Saved results show computed time/unknown count, preserve snapshot cursor across pages, and offer full-count/name confirmation before conversion. Delete preview/restore controls are implemented; readonly hides mutations. Component draft state survives tab switches. Compute remains explicit, not automatic.

Validation: TypeScript and all55 active frontend tests passed. test-output/lists-ui/verify-smart.cjs mocked-HTTP browser flow covers same-edition Chinese+availability editing, save/compute,55-row fixed paging, full conversion preview/confirm and readonly controls; smart-editor.png/smart-convert.png inspected. This does not establish live-Core or installed combined acceptance. Core unchanged (159 present, last full157), schema24/installed15 unchanged. Remaining: unsaved-rule live result preview, automatic recompute/freshness/retention, complete fact options, work links/covers and UI polish/error/mobile/native matrix. Goal active; originals/NAS untouched.

## M3 smart snapshot to full manual list — 2026-09-07

POST /api/smart-lists/{id}/to-manual now accepts the reviewed snapshot ID, expected full count, name and request ID. It copies all snapshot entries with frozen titles/order into one new manual list; no dependency on loaded UI pages or current catalog membership. The source smart definition remains intact. Deleted/repair-needed or definition-changed sources require a fresh review. Count mismatches, foreign snapshots and receipt collisions reject. Copy and receipt commit atomically; retries return the same new list rather than duplicating it.

Validation: focused snapshot5 and access3 tests passed (159 Core tests present; last full157 predates conversion). Generated55-entry fixture proves frozen title/order after catalog changes, full copy and source retention; injected failure at position25 leaves no manual header/entries/receipt. HTTP readonly matrix denies conversion. Schema24 and installed schema15 unchanged. Next: smart Lists frontend/editor/results/conversion preview, automatic recomputation/freshness/retention and remaining facts/scale gates. Goal active; no original files/NAS changes.

## M3 immutable smart result snapshots — 2026-09-07

Schema24 persists immutable ordered smart snapshot headers/entries and compute receipts. POST /api/smart-lists/{id}/compute checks definition revision/dependencies, evaluates one catalog transaction and returns a snapshot ID. GET results pins50-item pages to that ID; continuation without snapshot is rejected. Recompute creates a new result without changing old titles/order/membership. Headers record definition revision, time, input digest, unknown and local/external counts. Broken dependencies block recompute while retaining readable old results. Stored manual references join the explicit scope with identity dedup; unprojected external metadata stays unknown. Computation is currently synchronous/on-demand, bounded100000 candidates, with retained snapshot history; scheduling, cancellation and retention policy remain pending.

Validation: full157 Core tests passed. Tests cover55-result paging across catalog mutation/recompute, unchanged old page after backup/restore, receipt replay, broken dependencies, forced insert rollback and local/external identity dedup with unknown counting. Read-only HTTP write matrix includes compute. Source schema24; installed schema15 package unchanged and cannot open this schema. No UI changes. Remaining: automatic recompute/cached freshness, full manual conversion, external/typed relation projections, snapshot retention/performance and UI. Goal active; original sources/NAS untouched.

## M3 persisted smart definitions — 2026-09-07

Schema23 adds smart_lists/smart_requests. Typed schema-versioned definitions persist scope, sort and rules plus name/description/pin/revision/deleted state. GET/POST /api/smart-lists/{id} and a bounded50-item index now provide save, soft-delete and restore. Expected revision and request-digest receipts prevent overwriting concurrent changes or duplicate retry writes; receipt failures roll back edits. Tag conditions retain stable IDs: missing/deleted dependencies block saving and read as needs_repair without modifying the stored rule. Restoring a deleted definition retains invalid dependencies for repair. StoredReferences scope is persisted but evaluation of external refs is not implemented yet.

Validation: full154 Core tests passed. New tests cover receipt replay/conflict, stale revision, deleted dependency retention, rule/receipt rollback, backup/restore and schema22-to23 upgrade with manual lists/tags preserved. HTTP read-only denial matrix includes smart-list writes. No frontend/installed change. Installed diagnostics binary remains schema15 and must not open newer catalogs. Next TL2: immutable paged results/recomputation/manual conversion, remaining fact projection and UI. Goal active; generated fixtures only, originals/NAS untouched.

## M3 smart catalog fact projection — 2026-09-07

smart_facts.rs now projects local work status/year/matching state, effective active personal tag IDs, root IDs, recorded edition languages/platforms and same-edition main-resource availability. Root-online observations are supplied by the snapshot caller; projection performs no filesystem/network operations. Empty/unrecorded languages and patch-only resources remain unknown. Cached developer IDs can supply brand evidence; old/incomplete cache cannot prove absence. Editable brand/tag names are not treated as stable IDs. People, roles, characters, related tags/source tag IDs and organizing facts remain unavailable pending typed relation integration.

Validation: focused smart_facts2 passed against generated SQLite catalogs (151 Core tests present; last full145). Checks distinguish missing Chinese + available English, corrected Chinese availability, patch-only unknown, root offline, absent relationship fields and deleted direct tags. Initial fixture omitted required files.seen_job; fixed fixture and reran successfully. Schema22/installed schema15 unchanged. Remaining: saved definitions/receipts, dependency repair, fixed result snapshots, manual conversion, rule UI and complete relation projection. Goal active; no original files/NAS changes.

## M3 smart rule evaluator foundation — 2026-09-07

Added smart_rules.rs with typed whitelisted work/edition fields, Any/All/None set matching, two group levels, year ranges, exclusion and explicit include-unknown. Partial metadata can prove an observed positive but cannot prove missing relationships. Edition conditions evaluate within one edition before existential matching. Public evaluation validates bounded rule trees/values before execution; arbitrary SQL/scripts/list references are not accepted. This module does not yet read catalog facts or expose UI/API.

Validation: focused smart_rules4 passed (149 Core tests present; last full145 predates this module). Tests cover partial/unknown vs known-empty, three-valued grouping, impossible cross-edition language/availability, depth/count/field validation. No frontend changes in this increment. Schema22 and installed schema15 unchanged. Remaining TL2: catalog fact projection with completeness/spoiler controls, saved rule CRUD/receipts, dependency repair, revision-fixed result snapshots, full manual conversion and UI. No smart-list usability or TL2 completion claim. Goal active; original games/NAS untouched.

## M3 reviewed split inheritance — 2026-09-07

Grouping split now previews every effective personal tag (including merged ancestors) and equivalent manual-list entry. Each can stay on the original, follow the new work, or remain on both; default retention is visible before confirmation. Deleted classifications remain represented. The digest binds full private metadata/revisions/variants; changed evidence or missing choices rejects the transaction. Old clients cannot split classified works without the review. New-work entries preserve notes, saved edition preference, added time and original variants; copies follow the original position. Tag/list revisions invalidate stale editors. Game files are never operated on.

Validation: all145 Core tests passed, including original/new/both outcomes, ancestor tags, explicit provider references, order/private-field retention and rollback for missing/stale/forced write failure. TypeScript passed. Mocked-browser full GroupingEditor fixture verifies selected choices and resource payload only submitted after Confirm; split-preview.png inspected. Evidence: test-output/lists-ui/verify-split.cjs. Fixed the grouping action selector accessible name during verification.

Source schema22 unchanged; installed schema15 remains old. Native split/backup combined acceptance, broad merge conflict resolution/audit, list navigation/view polish, smart Lists and remaining M0–M6 gates still open. Web cannot access the private split preview or mutate grouping. Goal active; generated fixtures only, originals/NAS untouched.

## M3 discovery search to manual list — 2026-09-07

Search work results now expose reviewed AddToList for the complete paged query. Query kind/value and spoiler mode are captured; local identities map to collection work keys, external works remain references. Character-result tabs do not add characters to work lists. Cancellation/navigation suppress late preparation; stale/incomplete pages fail before confirmation. The frozen preview covers every collected page, not only currently displayed results. Provider traversal is not an immutable provider snapshot.

Validation: TypeScript and all55 active frontend tests passed. test-output/lists-ui/verify-search.cjs verifies read-only/character-tab absence, preserved trait query/spoiler parameters, two-page preview and confirmed identity payload, and stale-page failure without another write. search-scope.png rendered and inspected. Core/schema22 and installed schema15 package unchanged; installed native combined acceptance remains open. Next manual-list gaps: split inheritance/conflict resolution and view/navigation polish; smart Lists still not implemented. Other M0–M6 gates remain. Goal active; original games/NAS untouched.

## M3 entity works to manual list — 2026-09-07

Person/character/company profiles now prepare all credit pages before opening the reviewed AddToList dialog. The scope captures owned/all and spoiler settings at start, maps known collection identities, deduplicates provider IDs, and supports cancellation. Partial/stale pages, wrong cursors, repeated pages or a scope over10000 fail without adding a partial set. Read-only users have no entry point. Navigation away cancels pending preparation. Confirmation remains the only list write.

Validation: TypeScript passed; entityListScope7 tests passed for multi-page/local identities, owned scope, spoiler protection, incomplete pages, cap and cancellation. Mocked-HTTP browser fixture verifies two-page review/confirmed addition, readonly absence, partial failure and cancellation with no extra writes; entity-scope.png inspected. Evidence: test-output/lists-ui/verify-entity.cjs. Existing Core unchanged (schema22); installed schema15 package unchanged. Traversal is provider paging, not an immutable provider snapshot; preview freezes the fetched membership. Discovery-search batch entry, split inheritance, smart lists and installed combined gates remain open. Goal active; original sources/NAS untouched.

## M3 arbitrary collection selection — 2026-09-07

Collection Select works mode adds accessible pressed-state controls outside card navigation. Selected IDs persist across collection pages and filter changes; Clear selection is explicit. Add selected to list freezes exactly selected available works into the existing reviewed dialog. If selected IDs disappear from current works, submission disables with a review message instead of silently omitting them. Selection is component-session state, not persisted after navigating away.

Validation: TypeScript passed. Browser LibraryBrowser fixture selected Work1 on page1 and Work61 on page2, changed filter to Work2, and verified submission contains exactly local:w1/local:w61. selected-preview.png inspected. Core/schema22 and installed schema15 unchanged.

Remaining: entity-filtered scope, cross-page focus, split inheritance, smart Lists and other M0–M6 gates. Goal active; originals/NAS untouched.

## M3 work-detail list entry points — 2026-09-07

AddToListButton is available in writable local work details and writable external ReferenceWork pages. It freezes one member on opening and reuses reviewed AddToList. Existing collection works use local identity; otherwise the VNDB reference is added without collection insertion. canWrite propagates through WorkExperience and defaults false in reusable external pages. External work title validation now allows up to1000 characters instead of the unrelated80-character list-name cap.

Validation: TypeScript and focused Core lists6 passed. Browser external-page fixture checks read-only button absence, explicit writable submission exclusively to /lists/l, vndb:v17 identity and success result; reference-added.png inspected. Local detail integration typechecked; installed/native combined acceptance remains pending. Schema22/installed schema15 unchanged.

Remaining: entity-filtered scope entry points, arbitrary selection, cross-page focus, split inheritance, smart Lists and remaining M0–M6 gates. Goal active; originals/NAS untouched.

## M3 collection-to-list reviewed batch addition — 2026-09-07

Collection now offers Add filtered works to list for the complete filtered set (up to the Core10000 cap), freezing work identities/titles when opening AddToList. Native dialog previews count and expandable names, loads paged destination lists, pins selected list revision, and requires Confirm addition. Result shows added/skipped counts; unchanged retries reuse request ID/body. Read-only collection hides the action. This is filtered-scope batch addition, not arbitrary multi-card selection yet.

Validation: TypeScript passed; browser fixture reviewed both works with no early POST, injected503 then retried, asserted identical request/revision/members and displayed 1added/1existing. add-preview.png inspected. Existing Core batch tests remain baseline; no installed combined claim. Schema22/installed schema15 unchanged.

Pending: work-detail and entity-scope entry points, arbitrary batch selection, cross-page focus, split inheritance, smart Lists and remaining M0–M6 gates. Goal active; originals/NAS untouched.

## M3 list cover composition — 2026-09-07

List index returns the first four ordered member keys. ListCover resolves already-loaded collection works (local/VNDB identities) and composes their covers using existing Picture safeguards. Safe mode hides images entirely; unknown refs, empty lists and failed images use placeholders. No new metadata/image acquisition path is introduced. The app passes its existing safe setting.

Validation: TypeScript and focused lists6 passed. Browser cover fixture verifies zero image elements/requests in safe mode, two failed known-cover requests after explicit safe toggle, and placeholders for failures/missing member; cover-fallback.png inspected. Successful real-artwork mosaic and full index HTTP projection acceptance remain to verify in combined tests. Schema22/installed schema15 unchanged.

Remaining: collection/detail batch entry points, cross-page focus, split inheritance, smart lists and remaining M0–M6 gates. Goal active; originals/NAS untouched.

## M3 merge-variant review UI — 2026-09-07

ListMergeHistory exposes preserved pre-merge notes/edition choices on entry rows (including read-only mode) and in the editor. Use these notes / Use this edition changes only the draft; Save submits through revisioned annotate. Original variants remain historical records after saving. Unknown edition IDs remain visible and Core validation still applies. This is explicit field selection with retained history, not a separate resolved/unresolved conflict-status system.

Validation: TypeScript passed. Browser fixture expanded history, copied notes/edition into draft, asserted zero POSTs until Save, then completed existing lifecycle/removal flow. merge-history.png inspected. Updated drag fixture to drop at row top after expanded row content changed its geometry. Schema22/Core unchanged, installed schema15 unchanged.

Pending: formal resolution audit/status if required by final contract audit, split inheritance, safe covers/batch entry points, cross-page focus, smart lists and remaining M0–M6 gates. Goal active; originals/NAS untouched.

## M3 reviewed list-entry removal — 2026-09-07

Lists UI now previews entry title and notes before removal, with cancel and explicit confirmation. It pins the reviewed list revision. Core removes only that list's entry and associated merge variants, compacts positions, preserves collection works and supports replay. Entry removal discards entry notes; whole-list soft deletion remains separately restorable.

Validation: focused lists6 passed (143 Core tests present), including cross-list rejection, receipt replay, position compaction, variant cleanup and preserved works. TypeScript passed. Browser fixture verified cancel without POST then removal leaving the other entry; remove-preview.png inspected. Schema22/installed schema15 unchanged; source-only, no native combined acceptance claim.

Remaining: merge-variant review/resolution, safe covers/batch entry points, cross-page focus, split inheritance, smart Lists and other M0–M6 gates. Goal active; originals/NAS untouched.

## M3 list drag and explicit position UI — 2026-09-07

List entries now expose an internal drag handle and Move to position editor, alongside keyboard-operable up/down controls. The position form accepts one-based integer positions across the complete list total, including unloaded pages. Drag accepts same-list internal sources only and pins the start revision; both use Core move validation. Draft refresh is disabled and errors preserve the position draft. Current reload after move returns the first page; destination-page/focus restoration remains a UX follow-up.

Validation: TypeScript passed; browser fixture moved first item to position2, then performed real browser drag back and verified first title/order. Existing entry/lifecycle flow also passed. reordered.png inspected. Mock HTTP only, not native combined or large-list drag acceptance. Schema22/installed schema15 unchanged.

Remaining: entry removal, merge conflict resolution, safe covers/batch entry points, keyboard focus and cross-page navigation polish, split inheritance, smart Lists and other M0–M6 gates. Goal active; originals/NAS untouched.

## M3 list entry notes and preference UI — 2026-09-07

Edit entry now opens a revision-pinned notes/preferred-edition draft. Choices use existing editions belonging to the entry local work. Null means use the work preference; unknown saved editions remain visible rather than silently cleared. Core still validates preference on save. Refresh is disabled during editing; cancel discards without POST and errors retain draft. Entry rows display notes and stored preference labels.

Validation: TypeScript passed; browser fixture saved notes and edition r, reopened, cancelled another draft without POST, and completed existing list lifecycle flow. entry-editor.png inspected. Fixed explicit accessible name for populated notes textarea. Vite cached an older test entry; new entry-fixture.tsx/entry.html used. Mock HTTP evidence only; native combined acceptance and offline-invalid-version matrix remain pending.

Schema22 unchanged; installed diagnostics schema15 unchanged. Remaining: merge variant resolution, drag/move-to-N, removal and batch entry points, safe covers, split inheritance, smart lists and other M0–M6 gates. Goal active; originals/NAS untouched.

## M3 Lists metadata and lifecycle UI — 2026-09-07

Lists detail now exposes Edit list (name/description/pinned), Duplicate list with editable name, reviewed deletion showing total membership count, and Restore list. Editor drafts retain their source revision; refresh is disabled while editing and failed requests preserve the draft. Cancel deletion sends no mutation. Duplication invokes the full Core copy action and opens the returned list. Read-only users see no lifecycle controls.

Validation: TypeScript passed; browser mocked-HTTP fixture exercised rename/description/pin, delete preview/cancel/no-POST, confirmed delete/restore and duplicate after the existing create/add/reorder flow. edited.png inspected. Core/schema22 unchanged; installed diagnostics schema15 unchanged. No installed combined acceptance claim.

Pending: entry notes/preferences and merge-resolution UI, drag/move-to-N, safe covers, batch entry points, robust pagination/failure/readonly matrix, split inheritance, smart Lists and remaining M0–M6 gates. Goal active; originals/NAS untouched. Earlier sections are historical.

## M3 first Lists app surface — 2026-09-07

ListsPanel is wired into the app sidebar using existing Midnight styles. Users can browse the list index/detail, create a manual list, add a local collection work, move entries up/down and load more pages. Core revision/cursors are sent; errors retain the current view/input and same logical mutation request ID. Read-only mode hides writes. Missing/external/merged references have explicit text states. No new cover fetching or game operations.

Validation: TypeScript passed; browser fixture created a list, added two works, moved Beta before Alpha and returned to index. Rendered detail.png inspected. Corrected select accessible name found during fixture testing. Evidence: test-output/lists-ui/verify.cjs, fixture.tsx and detail.png. Fixture uses mocked HTTP; installed/native combined acceptance is still pending. Schema22 unchanged; installed schema15 remains unchanged.

Remaining UI: edit/duplicate/delete/restore, notes/preferences and merge variants, drag and move-to-position, covers, collection/detail batch entry points, pagination/failure/readonly interactive matrix. Existing Core operations are not all exposed yet. Smart Lists, split inheritance and other M0–M6 gates remain open. Goal active; original files/NAS untouched.

## M3 list preservation during work merge — 2026-09-07

Schema22 adds list_entry_variants. Explicit grouping::merge reconciles source/target references inside the same transaction before changing work identities. Matching entries retain the earliest entry ID/position, become local:target, and preserve original entry fields as readable variants (notes, preference, title, identity, added time). Affected list revision advances, invalidating stale writes/pages. Existing variants follow the retained entry; list duplication copies variants too. Deleted lists participate so restore retains consistency.

Validation: full Core142 passed. New integrated test verifies earliest-ID preservation, both notes/preferences through read projection, backup/restore, and forced variant-write failure rolling back the entire work merge. No UI conflict-resolution flow yet; retained earliest fields remain active and all originals are available in merge_variants. Metadata-only provider-ID changes and split inheritance remain unresolved.

Source schema22 only; installed diagnostics schema15 unchanged. Original files/NAS untouched. Remaining: review/resolve variants UI/API, split preview/inheritance, Lists UI and smart lists, other M0–M6 gates. Goal active; earlier sections are historical.

## M3 identity-aware list addition — 2026-09-07

Batch addition now compares explicit resolved identities across existing and incoming references. Existing local merge chains resolve to the final work and its VNDB ID when known; local/VNDB equivalents are skipped in both addition orders, without rewriting or losing the original entry notes. Same-title unrelated works remain separate. Cyclic merge identities fail rather than loop. Incoming local members still require a current unmerged work; stale choices require reload.

Validation: focused lists4 passed (141 Core tests present), including local-to-external and external-to-local dedup, existing merged ancestor, same-title distinct work, retained notes and cycle rejection. Schema21 unchanged; source-only, installed schema15 unchanged. No file/NAS operations.

Remaining: retroactive reconciliation when works already in lists later merge or change provider ID, earliest-position conflict preservation/resolution, split inheritance, UI and smart lists. This increment prevents new equivalent additions; it does not silently consolidate existing conflicting entries. Goal active; earlier sections are historical.

## M3 full manual-list duplication — 2026-09-07

Duplicate action copies every persisted entry under one transaction, independent of loaded pages. New list/entry IDs preserve order, saved titles, notes, edition preferences and original membership-added times. Description copies; new list is unpinned at revision1. Source revision is checked and remains unchanged. Durable request receipts return the same new ID on replay, preventing extra copies. Deleted sources require restore before duplication.

Validation: focused list3 passed (140 Core tests present), including 55-member copy beyond page boundary, all private fields, replay, stale-source rejection, independent IDs and source retention when copied entries are removed. Schema21 unchanged, source-only; installed diagnostics schema15 unchanged. No original files/NAS operations. UI, identity dedup/merge/split, smart lists and other M0–M6 gates remain open; Goal active.

## M3 list read projection and paging — 2026-09-07

GET /api/lists provides bounded ID-cursor index pages. GET /api/lists/{id} returns metadata and 50 ordered entries with a zero-based opaque-to-client next_after cursor and revision. Continuation requires the same revision; edits reject stale continuation instead of silently duplicating/skipping entries. Manual list paging is restart-on-change, not a smart-list snapshot. Entry projection preserves saved title/notes and explicitly reports local/external/missing/merged reference state. No raw work metadata or credentials are returned. Deleted index/detail is Desktop-only.

Validation: cargo check; focused list2 tests passed, including 55-entry boundary/exhaustion, required revision, stale rejection and missing reference retention. HTTP read-only test passed with /lists/l added to the mutation-denial matrix. Total Core tests present139; last full run138 predates the added paging test. Source schema21 unchanged, installed diagnostics schema15 unchanged. No UI exposure yet; index paging is a live ID traversal, not an immutable snapshot.

Remaining: local/VNDB identity dedup and merge/split conflict preservation, list duplication, full UI including drag/keyboard/position, preferences and safe covers, smart-list snapshots/rules and remaining M0–M6 gates. Goal active. Originals/NAS untouched; earlier sections are historical.

# Lists implementation status

## M3 manual-list write foundation — 2026-09-07

Schema21 adds curated_lists, list_entries and durable request receipts. Transactional POST /api/lists/{id} supports create/update, batch add with duplicate-key counts, remove, one-based move, entry notes/local edition preference, and soft delete/restore. Revision and request digest protect writes/replays. Entries retain independent local: or vndb: references and saved titles, so creating an external reference does not insert a collection work. No game jobs are queued. Existing middleware enforces read-only Web mutations.

Validation: cargo check and full Core138 passed. New generated-fixture test exercises invalid-batch rollback, duplicate/replay/stale rejection, order/notes, invalid external edition, reopen and backup/restore; external addition leaves works count unchanged. This is a Core write foundation, NOT usable Lists delivery. No UI/read endpoint yet. Remaining: resolve local/VNDB same identity and merges/splits without note loss, read projection/paging, duplicate-list operation, UI drag/keyboard/position and covers, smart lists and other gates. Do not mark TL0–TL1 complete.

Source schema21 only, installed diagnostics schema15 unchanged; never open newer catalogs with that binary. Original files/NAS untouched. Goal active. Earlier entries are historical snapshots.


Contract: ../TAGS_AND_LISTS_PLAN.md. The current work_key namespace is provisional until cross-identity resolution is implemented; same local/VNDB work may currently have two distinct keys. Entry notes/preference survive deletion and restore. Preferences are validated against local work_releases on write; subsequent relation changes still need integrated conflict handling. No frontend exposure yet.
