## M2 persistent matching issues — 2026-09-07

Schema14 adds durable matching issues with Open/Deferred/Resolved, source/reason, evidence identity and optimistic revision. Desktop GET /api/issues reconciles current catalog observations inside one transaction before paging50 records; POST /api/issues/{id} rechecks evidence before defer/reopen. Repeated identical observations preserve deferral; resource revision/evidence changes reopen, binding/removal resolves. Pending jobs with no evidence do not replace the previous recorded observation. This materialization occurs on issue access, not a claim of a background problem service. Deferral changes issue triage only; it does not stop watching or matching.

Activity now exposes Issues with state filters, explicit refresh and matching-history link. Source-only: current installed NSIS still schema13. Schema13->14 creates a migration protection snapshot; older binaries must not be pointed at upgraded databases. Existing original game catalogs/NAS unchanged.

Core111 passed, including deferral persistence/restore, new source reopening, stale revision rejection, resolution/reopen and schema13 migration protection. Real HTTP test rejects Web reads/updates and accepts owner listing. Frontend43/TypeScript passed. Browser fixture verifies defer, reopen, conflict preserves row, refresh then retry, no horizontal overflow; screenshot inspected. Evidence test-output/m2-issues/. Initial fixture routing typo was corrected before successful run.

Remaining M2: scoped retry execution, scan/unscanned/missing/acquisition issues, broader event/manual timeline and independent30 multilingual matching samples; M1 installed UI gates and M3–M6 remain open. No full M2/MVP completion claim. Goal active.

## M2 scoped matching retry — 2026-09-07

POST /api/issues/{id}/retry uses the reviewed issue revision as a durable request key. It reconciles evidence, rejects stale/resolved issues and busy work, and transactionally queues one match item plus opens the issue. A replay returns the same job. The worker processes explicitly requested issue jobs even with global matching disabled, without enabling global matching or source scans. Existing snapshot/binding safeguards and attempt history remain in use. Issue listing exposes retry_pending; UI provides Retry matching, busy/error feedback and disables queued/running duplicate clicks. Listings still refresh explicitly.

Core113 passed: scoped two-resource fixture, only target matched, unchanged global setting, duplicate request reuse, new evidence/busy rejection and resolution. Final focused HTTP test covers Web retry denial; frontend43 and TypeScript passed. Browser fixture proves exact issue URL/revision request, busy failure retains row, success disables button; screenshot inspected. Evidence: test-output/m2-issue-retry/. A TypeScript quoting error during editing was fixed before validation. Source changes are not yet in installed NSIS.

Next: extend durable issues to scan/unscanned/missing and file-preparation failures, with source-scoped retry semantics. M1 installed UI gates, remaining M2 independent samples/manual timeline and M3–M6 remain open. Goal active; real game sources/NAS untouched.

## M2 file availability issues — 2026-09-07

Schema15 permits one issue per resource/category, preserving schema14 issue IDs, revisions and deferred state during migration. Matching and availability issues coexist. Availability reconciles stored missing/unverified file observations independently of work binding; it never probes filesystem existence or equates offline with missing. Evidence includes sorted affected file identities/status/path/size/mtime, so unchanged observations preserve deferrals and changed evidence reopens. Present files resolve availability issues; matching resolution does not. Matching retry explicitly rejects availability issues.

Activity shows source navigation for availability and matching history/retry for matching. Source navigation currently opens Sources; direct per-source scan retry and failed/unscanned jobs are still pending. Core115, frontend43 and TypeScript passed. Tests cover coexistence, deferral, matched-resource persistence, missing->unverified->present transitions, retry rejection and schema14->15 preserved triage. Browser fixture verifies distinct cards and source action with no misleading matching retry; screenshot inspected. Evidence test-output/m2-availability/. Source-only; installed NSIS remains schema13 and must not open a schema15 database.

Remaining: source/job failure issues and scoped reconciliation, acquisition failures, independent multilingual matching sample gate, M1 installed UI acceptance and M3–M6. Goal active; original game files/NAS unchanged.

## Reviewed source scans (2026-09-07)

Availability cards now expose a modal showing source path, recursive full-source scope and saved watcher exclusions. POST /api/issues/{id}/scan-preview requires the current revision; /scan requires that revision and returned digest. The digest covers evidence and source configuration. Confirmation is transactionally replayable and queues the existing scanner; it never directly moves/renames/deletes source files. Scan jobs disable repeat actions while active. Web access is rejected. Explicit refresh still reconciles issue state. Source/job-failure materialization remains pending.

Tests and browser evidence: test-output/m2-issue-scan/. Not yet packaged into the installed diagnostics build.

## Source issues (schema16, 2026-09-07)

A source issue has root_id and null resource_id. One source-category issue per root records watcher gaps or the most recent outstanding terminal scan failure. Matching and availability categories retain their existing resource identities. Sources tracked by the runtime watcher can appear before any work has been found. Reconciliation reads observations; it does not probe source existence or infer missing files from offline state. Failure coverage requires a subsequent completed error-free scan of the same or full recursive scope with equal exclusions. Active work and unrelated scoped success do not clear the failure. Current UI refresh is explicit; task failures remain available in Activity.

Source scan preview/confirm accepts either availability or source issues. Source cards omit matching actions. Schema migration, backup/restore, empty-source HTTP projection/preview, scan enqueue, deferral, reopening and resolution have local tests in source_issues.rs and access.rs. Source-only, not yet packaged. Acquisition failures remain pending.

## File preparation issues (schema17)

One acquire-category issue per job_id captures failed, interrupted and cancelled transfer/extraction tasks. resource_id/root_id may be null; list display derives known resource information from the saved job, falling back to job ID/destination. Different tasks for the same resource are not conflated. Active retries retain the issue; successful completion of that exact job resolves it. New errors reopen deferred items. Errors before task creation stay inline and in diagnostics.

Review file preparation loads /api/issues/{id}/task, an explicit desktop-only projection without raw spec or password, and resumes through the existing /jobs/{id}/resume after user confirmation. It works beyond Activity's recent100 limit. Test evidence: test-output/m2-acquire-issues/. Current changes are not in the installed diagnostics NSIS.
