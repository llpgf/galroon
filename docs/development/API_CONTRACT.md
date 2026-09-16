## Durable task summaries (API 1 / schema 43)

`GET /jobs` (including paged results) and `GET /jobs/:id` expose optional `summary`: null for historical tasks and unsupported kinds, otherwise `{version:1,new_works,new_editions,new_resources,new_files,updated_files,unmatched_resources,needs_review,skipped,failed}`. Only scan/match jobs created after migration record totals. Counters are deduplicated by entity identity within each task and persist across interruption, resume and catalog backup/restore. New files are excluded from updated-existing-file totals within the same task.

Scan totals describe observed files/resources; scans create no works or editions. Matching is a separate task and creates no editions. Unmatched/review totals record each resource's last observation within that task; later tasks do not rewrite this history. Skipped/error counts use distinct filesystem entries for scan (unidentifiable directory-entry errors use the containing folder) or distinct resources for matching. Encountered errors remain counted after successful retry; task-level failures with no identified entry are conveyed by task state/message, not invented entity counts. Internal accounting identities/paths are not included in the public summary.

## Regroup preview page (source capability, 2026-09-12)

`POST /api/resources/{id}/regroup/preview/page` accepts `{selection, offset: 0, digest?: string, library_id?: string}`. `selection` retains the exact existing regroup Selection shape. First page may omit both pins; subsequent pages require both original digest and library_id. Offset must be a multiple of60 within the selected member count. Unknown top-level fields are rejected. Owner/paired Desktop authorization remains required; readonly Web cookies cannot POST this preview.

Response keeps full-preview `digest`, `source`, `target`, `target_title`, `target_relative`, but removes source/target `files` and adds `file_count`. It replaces `selected` with at most60 `items` containing only id/relative/size/availability; `selected_count`, `selected_bytes`, `offset`, `next_offset` and `library_id` describe traversal. `retired_plans_count` and `retired_jobs_count` replace full retired ID arrays. Member order is deterministic English numeric path collation then exact file ID. Existing references/bindings remain in source/target summaries.

Digest is computed from the unchanged full canonical snapshot, independent of page offset. Every page recomputes within a readonly transaction and rejects changed digest/library. Confirm still uses the existing `/regroup` Selection plus digest write endpoint. Capability `resource_preview_paging: true` advertises support; API1/schema42 unchanged. Old full preview endpoint retained for existing clients. Current source frontend uses this endpoint and requires the resource_preview_paging capability; release/installed runtime validation is recorded separately.

Current source validates and hashes source/target/selected file rows incrementally under the same read transaction, then fetches at most60 summary rows using SQLite English numeric path collation. It no longer constructs full member Value arrays for this endpoint. Selected-ID requests/sets, bindings/strings, SQLite sorting and full confirmation/history still have data-dependent costs; this is not a constant-memory guarantee. Shared readonly queries now set a sticky cancellation flag when the request future is dropped; SQLite checks it every1000 VM operations. Real HTTP/1.1 socket-disconnect propagation has a focused source test. This does not preempt arbitrary pure-Rust computation or establish a hard cancellation latency. Installed runtime evidence is versioned separately.

## Member cursor revision scope (source schema42, 2026-09-12)

The member-page endpoint's `catalog_revision` now identifies the dedicated `member_catalog_state` revision, not `smart_catalog_state`. Never interchange it with revisions from other endpoints. Triggers invalidate it on resource, file and resource-file membership INSERT/UPDATE/DELETE; root online observation alone does not. Library/query/resource pins still apply. Cursor version1 is required; legacy member cursors without a version are rejected. Migration41→42 creates the state/triggers under the existing backed-up atomic migration. API1 remains unchanged. Regroup source/destination CAS and digest checks are unchanged.

## Resource member pages (source 2026-09-12)

GET `/resources/{id}/members/page` is an additive authenticated endpoint; normal readonly Web sessions may read it. Optional `query` (up to512 UTF-8 bytes) matches source-relative paths using Unicode lowercase substring search; optional `before` is the opaque returned cursor (up to4096 bytes). Optional `catalog_revision` and `library_id` must be supplied together. They pin even a fresh first page or changed search query to the existing selection snapshot; mismatches are rejected. Unknown query keys are rejected. Results contain at most60 `items` with `id`, `relative`, `size`, `availability`; absolute file paths and full bindings are not returned. Ordering is source-relative path using an ICU English numeric collation, then file ID; numeric filenames retain natural order (2 before 10).

Response fields: `source` (`id`, `title`, `revision`, `root_id`), filtered `total`, unfiltered `file_count`, `offset`, nullable `next`, `library_id`, `catalog_revision`. Reads run in a separate readonly transaction through the existing bounded read gate. Cursor identity includes library/catalog revision/resource/query/offset; invalid, oversized, negative-offset, foreign-scope or stale cursors are rejected. Restart from page one after catalog changes. Existing full `/members` and regroup write contracts remain unchanged. The current source frontend consumes this endpoint and requires `capabilities.resource_members_paging: true`. Source browsing uses60-row pages and pinned search; select-all traverses every matching page with cancellation and commits IDs only on full success. Regroup preview responses still contain complete arrays. API1/schema41 unchanged.

## Scoped resource grouping (2026-09-09)

Current frontend additionally requires `capabilities.resource_grouping_scoped: true`; older Core builds must be updated before entering the collection. API1/schema41 remain unchanged.

GET `/resources/page` accepts optional `root_id` and `exclude_id` strings (up to512 bytes each). Filtering precedes60-result paging; cursor scope includes both fields and rejects reuse with different values. Omitted values retain unscoped behavior. Prior unscoped cursors default both fields to empty.

GET `/resources/{id}/members` and regroup previews include nullable `work_title` and `edition_label` on each exact binding. These labels participate in the preview digest; changed labels require a fresh preview before applying. Missing labels retain identity fallback in the frontend. Grouping changes catalog membership only. Per-resource member snapshots remain complete arrays.

## Large edition commit policy (2026-09-09)

File-backed WAL/FULL edition saves affecting at least1000 existing or incoming members temporarily defer automatic checkpointing through transaction commit. Success still means the transaction committed with synchronous=FULL, including all existing revision and membership guards. The previous automatic checkpoint threshold is restored on success or rollback. Successful large writes queue a coalesced PASSIVE checkpoint on a separate connection; busy readers/writers may leave safe committed WAL pages for a later checkpoint. Core shutdown drains scheduled checkpoints after HTTP/jobs drain and before releasing its collection lock. WAL files remain SQLite-owned and must never be manually deleted. Bundled SQLite is3.53.2. API1/schema41 are unchanged. This reduces response-path work, not total disk writes or the cost of updating all affected rows.

## Bounded edition membership editing (2026-09-08)

Current frontend requires `capabilities.edition_membership_edit: true` in addition to collection paging and scoped work reads. Unsupported Core builds receive an update message before entering the collection. API1 legacy full replacement remains supported; schema41 is unchanged.

GET `/releases/{id}/membership` takes `ids` (a JSON array of up to60 distinct local work identities), edition `revision`, `collection_revision` and `library_id`. It returns exact `ids` and selected `members`, plus the pinned revisions/identity. Missing or merged work identities and changed snapshots are explicit failures, never unchecked choices. Empty IDs validate the snapshot. Reads use the shared independent read-only SQLite snapshot gate.

Owner/Desktop POST to that same path accepts `{edition, add_work_ids, remove_work_ids, collection_revision, library_id}`. `edition` has the existing EditionInput metadata and revision, with `work_ids: []`. Full replacement IDs in this endpoint are rejected. Core derives the complete membership within the write transaction, validates distinct/disjoint additions/removals, preserves untouched members including off-page rows, requires at least one active work, and retains preferred-edition/resource-removal guards. Library and catalog revisions catch grouping changes that do not advance the edition revision. Failed edits roll back metadata and membership together. Web POST remains forbidden.

The editor uses60-work collection pages, checks membership for each visible page and retains only explicit additions/removals across pages and searches. Existing summary work_count contributes to the selected count. Unknown lookup disables choices and saving until retry; conflicts retain metadata and selection drafts. Closing and reopening reloads the scoped edition. New editions submit their explicitly selected IDs through the existing creation endpoint. Saving a very widely shared edition still updates affected work/resource revisions and can take seconds; bounded network payload is not constant-time write performance.

## Scoped work reads (2026-09-08)

The current frontend also requires `capabilities.work_scoped_reads: true`; paging-only Core builds receive an explicit update message. API range1 and legacy arrays remain supported.

Authenticated GET `/works/lookup?ids=<JSON array>` resolves up to60 VNDB work IDs to complete active local Work records. `local=true` resolves exact local IDs instead. Duplicate input IDs are deduplicated, missing/merged IDs are returned explicitly in `missing`, and supplied `revision` and `library_id` must both match. Empty lookup validates the current snapshot. Frontend full-profile/search List preparation resolves every provider/manual page at a single catalog revision and validates it again before showing review. An incomplete lookup is never treated as absence.

GET `/works/{id}/assets` reads a separate SQLite snapshot containing that Work, its indexed resources with complete bindings and physical-primary title, and scoped edition summaries. Each edition summary has `work_id` and exact active `work_count`; it deliberately has no `work_ids` field. GET `/releases/{id}` retains the full existing Edition contract for legacy clients and explicit full reads. Shared membership is not replaced by a partial ID array. Legacy GET `/resources` adds optional `primary_work_title`; other resource fields and file-state counts retain their meaning.

Lookup requests contain at most60 identities. Assets are scoped to one work but resource/binding/edition arrays within that work remain complete and uncapped; very dense single-work collections still require further paging. Lists, Organize, matching and other management flows retain their separate scale acceptance requirements. Read handlers use independent read-only snapshots and two concurrent work-query slots. Existing schema41 collections gain non-unique covering indexes; data format and API1 compatibility remain unchanged.

## Collection paging capability (2026-09-08)

`GET /context` advertises `capabilities.collection_paging: true`. This frontend requires that capability and reports an actionable Core update message if absent. API range remains 1; existing clients and legacy arrays remain supported.

Authenticated `GET /collection` returns up to 60 complete Work records plus global counts, facets and custom-tag summaries. Query/search/filter/sort/title-mode/locale are evaluated across the collection. Continuation cursors bind the library, revision, query and observed source availability; a stale cursor returns an explicit error and the client retries from page one. The client aborts obsolete requests and ignores late responses. Compact global metadata is still O(collection); this is not a constant-memory guarantee.

Owner/Desktop `POST /collection/selection` resolves explicit cross-page IDs or all filtered IDs against the reviewed view token. It returns exact IDs/titles for the existing List confirmation flow, rejects stale views and selections over 10,000 instead of truncating, and does not itself mutate a List. Web callers remain read-only. `GET /works` retains its legacy array contract; `GET /works/{id}` and `GET /works?paged=true` remain available.

# Local Core client contract — 0.0.2 / schema 41

This is the contract used by the bundled Desktop/Web clients. It is not a public or version-stable third-party service API. Ship matching Core and frontend versions; schema versions are catalog migration versions, not HTTP API versions.

## Transport and identity

The Core binds loopback at a selected port. Windows Desktop obtains the local endpoint through its identity-checked control channel; do not hardcode a previous port or read/copy a token from local state. Host validation accepts loopback names only. Desktop also pins the selected collection/Core identity and retains paired credentials in Windows credential storage.

`GET /api/health` returns JSON with `name` and the compiled product `version`. It proves that an endpoint answers, not that it is the selected collection. `GET /api/context` supplies collection/device context after authentication.

Authenticated responses carry `x-galroon-library` and `x-galroon-device`. Requests can pin those values using the same headers. A mismatch receives HTTP 409 with `code: collection_changed`; the client must stop using the old selection, preserve unsaved work for review and reconnect deliberately.

The local owner authenticates through its private control-channel credential. Paired desktops use a bearer session. Browser login creates an HTTP-only collection-specific session cookie. Browser sessions cannot use that cookie as a desktop bearer credential. Owner changes and revocation invalidate sessions. Login and pairing validate origin, have attempt limits and require explicit owner setup/authorization.

## Read-only Web boundary

The middleware denies authenticated Web methods other than GET/HEAD before mutation handlers. Login/pair are bootstrap routes with separate checks; same-origin logout is the session-management exception. Some owner-only reads, including Core diagnostics and device administration, are also denied to Web/paired sessions. Hiding an editing button is not the authorization boundary.

Reads may populate provider caches, update session last-use or materialize normal derived catalog state. “Read-only” means the browser cannot issue owner edits or file-operation commands; it does not mean the Core never maintains internal cache/session records while serving a read.

## Mutations, retries and jobs

Normal catalog edits carry the current revision; stale values fail. Tag/List and correction operations additionally use their documented request IDs/digests/receipts. Reusing a request key with changed content is rejected. File plans have separate preview, approval and execution phases with recorded evidence; editing metadata does not approve or execute a plan.

Do not retry every POST automatically. After a timeout/disconnect, inspect the affected record, receipt or job before retrying a result-unknown write. Task creation/control responses and Activity state, rather than a button click, establish execution state. Pause/cancel take effect at safe checkpoints; failed/interrupted is not completed.

Most domain/validation failures currently use HTTP 400 with `{ "error": "..." }`; they are not consistently HTTP 409. Missing/invalid authentication uses 401, denied access uses 403, and pinned-collection mismatch uses 409. Framework malformed-input and unknown-route errors can differ from that JSON envelope. Treat HTTP status and response shape defensively, and display meaningful error text without exposing credentials.

## Paging and privacy

`GET /api/jobs` preserves the legacy latest100 array. `GET /api/jobs?paged=true` returns `{items,next}` with at most100 tasks and an opaque next cursor; pass it URL-encoded as `before` with `paged=true`. Older pages preserve creation order and an insertion ceiling, while task states are read fresh on each request. Later insertions appear when returning to the latest page. Cursors belong to the current collection/session and must be discarded after switching or restoring. No task spec or credentials are returned.

Lists, smart snapshots, profiles, history and search use endpoint-specific cursors. Keep cursors opaque and in the context in which they were returned. A page is not the complete collection. Stable smart snapshot pages remain pinned until the client explicitly adopts updated results; full conversion uses the complete snapshot.

Missing provider data stays unknown/partial. Spoiler and hidden-relationship projection affects returned edges, counts and discovery membership. Raw provider cache is not a substitute for the effective relationship projection. External references do not become owned works merely because the client reads them.

## Request families and authoritative source

| Family | Purpose |
|---|---|
| `/api/access/*`, `/api/context` | Owner/session/device and collection context. |
| `/api/works`, `/api/resources`, `/api/releases` | Local work, resource binding and edition metadata. |
| `/api/roots`, `/api/scans`, `/api/jobs`, `/api/issues`, `/api/timeline` | Source observations, durable tasks and problem/history review. |
| `/api/vns/{id}/relationship-*` | Reviewed effective relationship corrections and history. |
| Tags, manual Lists and smart Lists routes | Personal classification, revision/receipt edits and stable result snapshots. |
| Backup, file-plan and acquisition routes | Reviewed recovery and explicit filesystem operations. |

The exact route declarations live in `crates/core/src/lib.rs`; each handler's Rust request struct and the matching `src/api.ts`/feature client define its current payload. This table is an overview, not an invented OpenAPI schema. Updating a payload requires matching client changes and affected acceptance checks.

API responses/logs use request correlation IDs where supplied. Review diagnostics locally; do not put credentials in URLs, test output, issue descriptions or API examples. See [USER_GUIDE.md](../user/USER_GUIDE.md) for recovery and [SUPPORT.md](../user/SUPPORT.md) for deployment limits.

`GET /api/jobs/{id}` returns the same projected task fields for exact timeline inspection. `GET /api/timeline` remains Desktop/owner only. Schema41 adds transactional file-plan/operation/transfer transition events with IDs and state summaries; paths, raw errors and transfer specs are excluded. The plan/task endpoints provide the current details. A migration boundary records existing counts without inventing older events.


## Resource page (current source, not yet packaged)

`GET /api/resources/page?query=...&status=all|matched|unmatched&before=...` is authenticated like the existing resources read. Omitted status means all. Title/path substring matching uses Unicode lowercase conversion (not ASCII-only SQLite lower). Query is bounded to512 UTF-8 bytes; the opaque JSON cursor is bounded to4096 bytes and pins library, catalog revision, query and status. It rejects changed scope/revision instead of combining pages.

Response: `items` (at most60 complete existing resource projections), `next` (nullable cursor), `total` (filtered count), `offset`, `revision`, `library_id`. Order is title then resource ID. Only resources joined to a root and at least one existing file are eligible, matching existing projection behavior. Matching status is based on physical primary `work_id`, not any secondary binding. Items retain all bindings and missing/unverified/unknown file counts. This bounds resources per page; it does not cap bindings within an individual resource. Match-review text is current advisory metadata; cursor revision follows existing catalog invalidation triggers.

The unpaged `/api/resources` remains available for compatibility. Current-source Organize now uses this route; the installed lists-index package predates it.
