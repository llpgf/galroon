## Release update notices (2026-10-12)

All three endpoints return `{current, install_kind, enabled, checking, last_checked, last_error, update_available, latest}`. `current` is the Core version; `install_kind` is `docker`, `desktop` (Windows) or `standalone`. `last_checked` is nullable Unix seconds for the last completed attempt, including failures; `last_error` is nullable text. `checking` indicates an in-flight check. `latest` is null unless a newer full release is available; otherwise it is `{version, name, url, notes, published_at}`, with nullable ISO timestamp `published_at`. Notes are plain text, capped at 20,000 characters; the URL is restricted to the project's GitHub releases pages.

- `GET /api/updates`: any authenticated role, `WebAccess::Unchanged`; reads cached state without contacting GitHub.
- `POST /api/updates/check`: owner or paired desktop; checks now even when automatic checks are disabled. Concurrent calls share one in-flight request. Calls within 60 seconds return stored state; after a failure, retries wait at least one hour. Check failures are returned in `last_error` while retaining any previously known newer release.
- `POST /api/updates/settings`: owner or paired desktop; accepts exactly `{enabled: bool}` and rejects unknown fields. Atomically persists device-local preferences, wakes the worker and returns state. Enabling with no recent check triggers an asynchronous check. Settings write failures return an API error.

Web sessions receive 403 for both POST endpoints. A GitHub 404 is a successful check with no update; a 304 retains the cached release. Device-local state is outside catalog backups and schema migrations. Clients read on load and every six hours; they never check GitHub directly. See [UPDATES.md](../features/UPDATES.md) for scheduling, privacy and deployment overrides.

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

The Core binds loopback at a selected port. Windows Desktop obtains the local endpoint through its identity-checked control channel; do not hardcode a previous port or read/copy a token from local state. The loopback listener accepts loopback hosts only; an enabled LAN listener accepts configured hosts over HTTPS. Desktop also pins the selected collection/Core identity and retains paired credentials in Windows credential storage.

`GET /api/health` returns JSON with `name` and the compiled product `version`. It proves that an endpoint answers, not that it is the selected collection. `GET /api/context` supplies collection/device context after authentication.

Authenticated responses carry `x-galroon-library` and `x-galroon-device`. Requests can pin those values using the same headers. A mismatch receives HTTP 409 with `code: collection_changed`; the client must stop using the old selection, preserve unsaved work for review and reconnect deliberately.

The local `owner` authenticates through its private control-channel credential; this bearer is forbidden on LAN, including static files and OPTIONS. Paired `desktop` sessions use a bearer for 90 days. Password login (`POST /api/access/login`) over loopback or LAN HTTPS issues a `web_owner` session with desktop-level permissions, never owner-only permissions. Its collection-specific cookie is HttpOnly, SameSite=Strict, Path=/api, Max-Age=2592000 (30 days), and Secure on LAN. Pairing codes can only issue `desktop` or `web`; paired `web` cookies last 12 hours (Max-Age=43200). Existing password sessions stored as `web` remain read-only until expiry; no migration changes their role. Session expiry is fixed, not extended by last use. Neither browser role can authenticate as a bearer. Changing the owner password revokes every session and unused pairing code. Login and pairing require an allowed Origin and socket peer; login retains its 10/minute per-IP budget and existing IPv6/global limits. LAN Origin must exactly match `https://{Host}`; missing or foreign origins receive 403.

## Browser permission boundary

The middleware denies `web` methods other than GET/HEAD before mutation handlers, except logout. `web` reads retain path projection and denied-route restrictions. `web_owner` responses are unprojected, include real Core paths, and can access desktop routes such as `/api/issues`. Every non-GET/HEAD cookie-authenticated request, including logout and `web_owner` writes, requires `origin_allowed`; missing Origin is rejected with 403 because SameSite=Strict does not separate ports. Login/pair are bootstrap routes with their own Origin checks. Setup, password changes, pairing-code management, network settings, devices, Core diagnostics, artwork cache clearing and full backup exports remain owner-only. Hiding an editing button is not the authorization boundary.

On Unix/NAS, `galroon-core owner change-password` reads the new password from stdin, like `setup`, and revokes sessions through the private owner control channel. Browser folder pickers, restore and desktop diagnostics remain desktop-only.

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


## Web data boundary and catalog changes (schema 46)

All web sessions, including loopback, use the following route policy. Owner and desktop payloads retain their existing fields. Denied web reads return HTTP 403 with `{"error":"Desktop access required"}`. GET policies also apply to HEAD. The router coverage test requires every readable route to appear in `web_projection::WEB_ROUTES`.

`GET /api/changes` is available to every authenticated role and returns `{"seq":number,"library_id":string}`. The sequence increases on committed visible catalog changes, including relationship-only writes, source availability and completed smart results; it is not a transaction count. Authentication, cache fills, leases and diagnostics do not increment it. Poll with the library identity; values from different libraries must not be compared.

Compatibility exception: `/resources/page` is identical for desktop and web except for removed `items[].root_path` and any path-bearing text redacted by the safety boundary. `/roots` retains only id, label, role, online, missing and unverified. Resource members use source-relative `path`; job reads omit `current_path`. Unrecognized absolute paths in messages are replaced with `[redacted path]`, including POSIX, drive-letter and UNC paths on any host OS.

| GET／HEAD route | web 分類 | 說明 |
|---|---|---|
| `/api/health` | 原樣 | 無實體路徑欄位 |
| `/api/context` | 原樣 | 無實體路徑欄位 |
| `/api/access/status` | 原樣 | 無實體路徑欄位 |
| `/api/access/me` | 原樣 | 無實體路徑欄位 |
| `/api/access/pairing-codes` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/access/network` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/access/devices` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/changes` | 原樣 | 無實體路徑欄位 |
| `/api/dashboard` | 原樣 | 無實體路徑欄位 |
| `/api/roots` | 投影 | 只留 id、label、role、online、missing、unverified |
| `/api/roots/{id}/watch` | 投影 | 監看 message／exclude 的絕對路徑遮蔽 |
| `/api/jobs` | 投影 | 陣列與 ?paged=true 均移除 current_path；遮蔽 message |
| `/api/jobs/{id}` | 投影 | 移除 current_path；遮蔽 message |
| `/api/timeline` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/issues/{id}/task` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/resources/page` | 投影 | items 移除 root_path |
| `/api/resources` | 投影 | 移除 root_path；保留相對 path |
| `/api/suggestions` | 投影 | 建議內容中的絕對路徑遮蔽 |
| `/api/resources/{id}/suggestions` | 投影 | 建議內容中的絕對路徑遮蔽 |
| `/api/works` | 原樣 | 無實體路徑欄位 |
| `/api/works/lookup` | 原樣 | 無實體路徑欄位 |
| `/api/works/{id}/assets` | 投影 | resources 移除 root_path |
| `/api/collection` | 原樣 | 無實體路徑欄位 |
| `/api/issues` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/diagnostics` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/vns/{id}/relationship-decisions` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/vns/{id}/relationship-history` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/vns/{id}/relationship-corrections` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/vns/{id}/relationship-corrections/history` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/relationship-candidates/{id}` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/entities/{id}/manual-works` | 原樣 | 無實體路徑欄位 |
| `/api/entities/{id}/field-decisions` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/entities/{id}/field-history` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/works/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/works/{id}/edition-options` | 原樣 | 無實體路徑欄位 |
| `/api/releases/labels` | 原樣 | 無實體路徑欄位 |
| `/api/releases` | 原樣 | 無實體路徑欄位 |
| `/api/releases/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/releases/{id}/membership` | 原樣 | 無實體路徑欄位 |
| `/api/resources/{id}/members` | 投影 | files[].path 改為 relative |
| `/api/resources/{id}/members/page` | 投影 | 已使用 relative；仍經共用投影與遮蔽 |
| `/api/resources/{id}/matching-history` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/matching` | 投影 | 背景工作文字遮蔽 |
| `/api/works/{id}/metadata` | 原樣 | 無實體路徑欄位 |
| `/api/works/{id}/exploration` | 原樣 | 無實體路徑欄位 |
| `/api/people/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/characters/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/companies/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/discover` | 原樣 | 無實體路徑欄位 |
| `/api/discover/manual` | 原樣 | 無實體路徑欄位 |
| `/api/artwork` | 原樣 | 無實體路徑欄位 |
| `/api/artwork/cache` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/artwork/offline` | 拒絕（403） | owner／desktop 管理的永久離線圖片庫與批次下載 |
| `/api/vns/{id}/exploration` | 原樣 | 無實體路徑欄位 |
| `/api/lists` | 原樣 | 無實體路徑欄位 |
| `/api/lists/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/vns/{id}/related-tags` | 原樣 | 無實體路徑欄位 |
| `/api/works/{id}/related-tags` | 原樣 | 無實體路徑欄位 |
| `/api/entity-tags/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/custom-tags` | 原樣 | 無實體路徑欄位 |
| `/api/custom-tags/{id}/membership` | 拒絕（403） | 既有 desktop 編輯權限 |
| `/api/smart-lists/options` | 原樣 | 無實體路徑欄位 |
| `/api/smart-lists/{id}/results` | 投影 | 遮蔽背景計算 update_error 中的路徑 |
| `/api/smart-lists` | 原樣 | 無實體路徑欄位 |
| `/api/smart-lists/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/works/{id}/split-preview` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/backups` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/plans` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/plans/{id}` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/duplicate-checks/{id}` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/quarantine` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/resources/{id}/acquire-preview` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/archive/capabilities` | 原樣 | 無實體路徑欄位 |

Source remap preview is POST and remains forbidden to web sessions.

Issue reconciliation is published on startup and by the existing background worker; GET `/api/issues` reads persisted results and does not cause catalog writes. Web JSON bodies are bounded at 32 MiB; an oversized or unparseable response fails closed without returning its contents.

## AI match suggestions (schema 47, Phase 3a)

All paths below include `/api`. These are catalog endpoints under the existing authentication, identity pin, transport and role matrix. Owner is loopback-only; owner and paired desktop may mutate. Desktop works over loopback or LAN. Web GET/HEAD reads are allowed and use the shared recursive path-redacting projection, including query, rationale, source, work titles and verification strings. Web mutation attempts are 403. No new identity mechanism or UI is introduced.

| Method and path | Body / result |
|---|---|
| `POST /resources/{id}/suggestions` | `{revision, works:["v123",...], query, rationale}` → the stored suggestion. Rejects stale revisions and any primary or shared binding with 409. Requires 1–32 distinct VNDB IDs, positive revision, nonblank query (≤512 bytes), nonblank rationale (≤8192 bytes); unknown fields rejected. Fetches actual work data through the cached, spaced VNDB provider. Replaces an existing open suggestion by dismissing it, atomically. Never creates works or bindings. |
| `GET /suggestions?before=<next>` | `{items,next}`; at most 60 open suggestions, newest insertion first. `next` is a nullable integer insertion cursor; pass it unchanged to continue. Scope it to the current collection. States are read fresh, so this is not a pinned snapshot. |
| `GET /resources/{id}/suggestions` | `{suggestion: <open suggestion or null>}`. |
| `POST /suggestions/{id}/confirm` | `{revision}` → `{id,state:"confirmed",resource:{id,revision}}`. Individual manual confirmation also permits unverified suggestions. |
| `POST /suggestions/batch-confirm` | `{ids:[...],revisions:[...]}`: 1–60 distinct suggestion IDs with corresponding resource revisions in the same order. `{results:[...]}` preserves input order. Success uses the individual response; failure is `{id,confirmed:false,error}`. Unverified proposals are skipped with a per-ID error. Each suggestion is its own atomic transaction; one failure does not undo earlier successes. HTTP 200 for a valid batch, including mixed failures. |
| `POST /suggestions/{id}/dismiss` | No body required → `{id,state:"dismissed"}`. Requires an open suggestion; otherwise 409. |

Suggestion fields: `id`, `resource_id`, `resource_revision`, `works:[{vndb_id,title}]` (ordered, nonempty), `query`, `rationale`, `source`, `verified` (boolean), `verification` (per-work array), `state` (`open`, `confirmed`, `dismissed`, `stale`), `created`, `updated` (Unix seconds). Source comes from the authenticated paired session name (`mcp:<name>`) or `owner:owner`. These labels are attribution, not extra permissions. The shared resource projection (`/resources`, `/resources/page`, work assets) and issue review items also contain `suggestion`, nullable when absent.

Verification requires **each** work's own title/alttitle/titles/aliases to equal the query or a prepared resource/member title using `matching::normalized`, plus existing brand-or-date corroboration (or neither hint present). Work-owned names alone count, not related works or release-only titles. Each verification entry includes `vndb_id`, `verified`, nullable `exact_name`, `title_reason` (`exact_own_name` or `no_exact_own_name`) and `corroboration` (`no_folder_brand_or_date`, `brand_or_date_agrees`, or `brand_or_date_not_corroborated`). All entries must pass for the suggestion to be verified.

Confirmation fetches provider data before opening the write transaction, then checks the current resource revision, stored suggestion revision, open state and absence of bindings again. A mismatch returns HTTP 409 `{code:"suggestion_changed",error}` and writes no catalog data. Work creation reuses the manual dialog helper and existing VNDB identity/merge resolution. The manual binding/history primitive is shared with `/resources/{id}/bind`: first work primary, **all roles `main`**, null edition IDs, one `resource_history` record and a `manual.bind` event. All work inserts, references, revision updates, history and confirmed state commit together. Bound resources retain the existing protection against automatic rematching.

Migration 46→47 creates `match_suggestions`, the unique open-per-resource index, paging index, revision invalidation trigger, and catalog-change notifications. Any resource revision change stales open suggestions in that same transaction. Changed file observations during rescan now advance resource revisions; unchanged observations do not. Replaced/dismissed/stale/confirmed proposals remain stored. See [AI suggestions](../features/AI_SUGGESTIONS.md).

## Permanent offline artwork

Matching, metadata refresh, interactive public profiles and automatic downloads share a Core-wide VNDB request clock (at least 1.6 seconds between uncached requests). Background downloads therefore cannot add a separate stream of 350 ms requests on top of matching. Provider errors and existing cooldown/retry behavior still apply; the shared clock is per Core process. See [VNDB usage terms](https://api.vndb.org/kana).

`GET /api/artwork/offline` permits owner/desktop sessions and returns `{images,bytes,profiles,job,automatic}`. `job` is null or `{id,state,processed,discovered,errors,message}` for the latest `artwork` task. `POST /api/artwork/offline` starts a snapshot of all unmerged collection works and returns `{id}`; it requires other tasks/file operations to be idle. Read-only web sessions are denied. Existing `/api/jobs/{id}/{pause|resume|cancel}` controls apply. Progress counts works; errors count incomplete works. Resume retries from the beginning after errors, reusing already saved images. Network requests retain the artwork URL allowlist, body limit and bounded download queue. Only the Core's own data directory is written.

The permanent `artwork-library.sqlite` stores public work-page snapshots and image bodies. `/api/artwork` checks it before the evictable cache; cache clearing and LRU eviction never modify it. Stored work-page snapshots bypass the normal 24-hour provider TTL until explicit refresh, which also updates the saved snapshot. It also stores complete directly linked character, person and company profile pages (`exploration.v3.{kind}.{id}.{page}`, pages 1–1000), without recursively following their related works. Image selection is explicit: the collected work’s `vn.image`, its direct `characters[].image`, and each linked profile’s own `{kind}.image`. Images nested in `relations`, profile `works` lists or other works’ voice/character records are not prefetched. These lists retain their text and links. Already archived images are reused, not purged by a scope change. `profiles` counts all saved pages, not collection works. Partial/stale responses are not marked complete. Existing catalog fields and manual overrides are preserved when filling newly imported metadata.

Schema 48 adds `collection_downloads`, transactionally populated by work insert and provider identity/cover/source changes. Existing active works are backfilled once on upgrade. Manual relationship corrections also enqueue the affected collection works; their directly linked profiles are included. Refreshing an archived work/entity page queues affected works so new images are preserved too. One Core service processes batches of up to 20 works, retries incomplete works after 300 seconds, and retains queue generations so an older completion cannot discard a newer change. User notes/status/favorites do not enqueue provider requests. Active artwork work does not block source/file operations. `automatic` is `{pending,errors,retry_at}`; pending includes delayed retries. Pausing the current artwork task suspends automatic intake until it is resumed/cancelled. Interrupted downloads retain pending work and continue after Core restarts.

Client images are fetched only from the selected authenticated Core, cached in memory up to 64 MiB / 600 entries, and invalidated on session changes. Clearing the local cache revokes its object URLs and prevents older in-flight requests from repopulating it; it never calls a Core deletion API. UI metadata is transient client state. Neither is a backup source.

Backups include this database automatically when present, using manifest version 4. `artwork` contains `{file,bytes,sha256,images,profiles}`; `bytes` at the manifest root remains catalog bytes. The root `sha256` is SHA-256 of the UTF-8 string `<catalog-file-sha256>:<artwork-file-sha256>`, binding preview/restore to both files. Both SQLite files are online snapshots; restore verifies hashes and artwork integrity/counts before removing the incomplete marker. Versions 1–3 remain readable (v3 requires artwork); v4 permits catalog-only exports. Game files remain excluded. Portable exports strip login credentials; full exports retain them as described below.

## Bangumi two-way sync (schema 49)

See [Bangumi sync](../features/BANGUMI.md) for routes, first-sync review, status baselines, conflict behavior and credential storage. Game status sync is bidirectional and independent of local resource availability. The account API is denied to read-only web sessions.

## VNDB two-way status and rating sync (schema 50)

See [VNDB sync](../features/VNDB_SYNC.md) for account, preview/apply, link, hide and personal-rating routes. Status and rating have independent baselines and conflict choices; `works.personal_rating` is nullable 10–100 (display 1.0–10.0). Work responses additionally expose `vndb_sync_id` and `vndb_sync_hidden`. VNDB writes patch only the selected field, preserving unrelated labels and notes. The account API is denied to read-only web sessions.


## Backup modes and history — 2026-10-16

`POST /api/backups` accepts `{destination, mode?:"portable"|"full"}`. Missing `mode` defaults to `portable`; invalid values return 400. Web sessions remain forbidden. Full backups require role `owner`; `desktop` and `web_owner` sessions receive 403 with `"Owner access required for full backups"`. This route always records `origin:"manual"`.

`GET /api/backups` returns the newest 50 completed exports as `{id,path,created,manifest,origin,mode,deleted}`. `origin` is `"manual"|"scheduled"` and `mode` is `"portable"|"full"`; pre-schema-51 history defaults to manual/portable. Schema 52 adds `deleted` (boolean, default false); deleted entries remain visible in history.

New exports write manifest v4 with `mode`, `origin`, `identity`, `core_version`, and `includes_credentials` (true for full). Full includes verified `identity/` files (`device.json`, optional `network.json`, and both LAN PEM files or neither), device UUID and optional SPKI fingerprint. Every export writes sorted `SHA256SUMS` entries for its catalog, optional artwork, identity files and manifest. Checksums are informational for external tools; inspection validates the manifest and file contents independently. Full backups contain the owner password hash, sessions and LAN private key and must be stored privately. See [BACKUP_HISTORY.md](../features/BACKUP_HISTORY.md).


## Scheduled Core backups — B2, 2026-10-16

All backup schedule, run and delete routes accept `owner`, `desktop` and `web_owner`; plain `web` receives 403 (including schedule reads). Only role `owner` may save a full schedule or run a schedule whose mode is full. `web_owner` has exactly the same backup permissions as `desktop`, including cookie Origin checks. CORS permits DELETE as well as GET/POST.

`GET /api/backups/schedule` returns the device-local configuration and state, plus `running`, `next_run` (Unix UTC seconds or null when disabled without a pending explicit run), and `available_bytes` (null if unavailable):

```json
{"version":1,"enabled":false,"destination":"","frequency":"daily","minute_utc":1230,
 "weekday":null,"keep":14,"mode":"full","artwork":true,
 "last_started":null,"last_success":null,"last_error":null,"last_path":null,
 "running":false,"next_run":null,"available_bytes":null}
```

`POST /api/backups/schedule` replaces the configuration with `{enabled,destination,frequency,minute_utc,weekday,keep,mode,artwork}` and returns the same status shape. Unknown fields (including state fields and `version`) are rejected with 422. Frequency is `daily` or `weekly`; UTC minute is 0–1439; weekly requires weekday 0–6 (Sunday=0); keep is 1–100. Destination must already exist, be writable, and be outside the Core data directory and all library roots. Linked destination paths are rejected. Invalid configuration or destination returns 400 without saving. The initial configuration is disabled, daily at 20:30 UTC (04:30 Taiwan), keep 14, full with artwork. `device_dir/backup-schedule.json` uses atomic writes, rejects symlinks, and resets damaged state to defaults with diagnostics. It is never included in exported identity files.

`POST /api/backups/run` takes no configuration overrides and runs the saved configuration, even when the schedule is disabled. It records `origin:"scheduled"` and participates in retention. Returns `{path,deferred:false}` on completion; while a plan is executing it queues one run and returns `{path:null,deferred:true}`. The worker retries that pending run every 10 minutes, for at most six hours, then runs even if the plan is still executing. A repeated Run request does not reset the deferral deadline. Calling Run on a full schedule requires `owner`.

The worker checks every minute. At startup it checks once, after five minutes, for a missing success or one at least a daily/weekly period old and performs one catch-up, without replaying missed periods. Failed runs record `last_error` and diagnostics; retries, including explicit Run, are limited to once per hour using persisted `last_started`. Stopping Services immediately interrupts the worker's wait; an already-started filesystem export finishes safely.

Manual exports and scheduled runs share one gate. Concurrent manual requests or configuration/deletion while busy return 503 (`Retry in a moment.`); the worker tries again on its next tick. Both export paths require available space of at least `(catalog database + artwork database) × 1.2 + 1 GiB`, including committed WAL pages, before creating an output folder. This check includes artwork size even if the configuration omits artwork. A failed check creates no partial export and performs no retention. Exported SQLite databases use DELETE journal mode and leave no WAL/SHM sidecars.

Only after a successful export, retention scans the destination's top-level `Galroon-backup-*` non-symlink directories with a parsable Galroon manifest, `origin:"scheduled"` and the active library ID. It retains the newest `keep` by manifest `created` (the newly exported folder wins ties, then folder path). Manual, other-library, linked, unrecognized and unparsable entries are untouched. Retention does not use history to discover candidates; matching history entries are marked deleted when present.

`DELETE /api/backups/{id}` removes only an existing manual backup recorded in history, whose non-linked `Galroon-backup-*` folder and manifest still identify this library and manual origin, and whose destination is outside data/library roots. It marks the history entry `deleted:true` and returns `{deleted:true}`; repeating deletion of an already marked entry succeeds. Scheduled or mismatched entries return 400. No full-mode restriction applies to deleting an existing manual backup.
