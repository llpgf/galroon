# Diagnostics

Use Windows Desktop: Settings > Backup & restore > Diagnostics > Review logs > Download diagnostics. Download currently saves galroon-diagnostics.json to the Windows Downloads folder. Review exports before sharing: error text can include local paths. Nothing uploads automatically. Disconnected Desktop can review its own logs through the Core connections panel.

Core writes <active collection>/logs/core.jsonl with core.1.jsonl and core.2.jsonl rotations. Desktop writes <base data directory>/desktop-diagnostics/logs/core*.jsonl independently. Each process retains at most three 1 MiB files. Entries include Unix time, app version, severity, code, message, optional job_id and request_id. HTTP failures return x-galroon-request-id and log route templates, status and elapsed time. Bodies, headers and query strings are not captured. Common labeled secrets, bearer tokens and URLs are scrubbed; this is not a guarantee that arbitrary free-form error text contains no private information.

Coverage: Core startup/fatal errors, panic location (not payload), API/HTTP failures, scan/acquire/hash/matching failures, Desktop connection errors and frontend error/unhandledrejection. Desktop uses native IPC to log without a live Core. Browser-only client errors are retained in sessionStorage for this tab across reloads (not across normal tab closure; browser session restoration may retain them). Web Settings > Account & devices > Diagnostics exposes only this tab log, with review/export/clear and no Core log request. The log keeps at most100 entries /65536 serialized UTF-16 code units (at most128KiB code-unit payload), scrubs common labeled secrets and HTTP URLs, and falls back to bounded memory with a visible warning when storage is denied. It does not capture request bodies, headers or page contents. GET /api/diagnostics is local-owner only; paired/Web sessions do not receive the logs. Errors writing logs appear in the snapshot. Force-kill, power loss and failures before logger initialization may leave no new entry.

Validation 2026-09-07: Core117, frontend45, cargo check Desktop, NSIS build passed. Unit tests exercise JSONL rotation, bounded messages, common credential masking and offline native logging. Browser fixture verifies review/export with Core offline. Installed native app successfully read both logs and downloaded valid JSON containing both process startup records. Evidence: test-output/diagnostics/. No clean-machine or full MVP completion claim.

Smart background recomputation also emits smart.compute.started, smart.compute.completed and smart.compute.cancelled events. They identify the list and timing without rule contents or work titles. Native100000-work cancellation/restart evidence: test-output/mvp-goal/smart-stop-100000/report.json.

Preview tasks emit smart.preview.started/completed/cancelled/failed with elapsed time and no rule or result payload. Native HTTP cancellation evidence: test-output/mvp-goal/preview-native-cancel/report.json.

Manual computations emit smart.manual.started/completed/cancelled/failed, with elapsed time and no list/rule/result payload. Native HTTP abort and retry evidence: test-output/mvp-goal/manual-native-cancel/report.json.

2026-09-07 follow-up: shared scrubbing now handles quoted JSON credential keys, escaped values, access_token, refresh_token and api_key. Targeted Core3/frontend2 tests passed. Source-only change; installed executable has not been rebuilt for this hardening.

Artwork downloads emit artwork.download.started/completed/failed with timing only, no URL or title. Native12-request dedup proof: test-output/mvp-goal/artwork-concurrent-native/report.json (one download start). Cancellation during download may leave only a started event.


2026-09-07 focused revalidation: current source passed all three Core diagnostics tests (rotation, bounded messages and credential scrubbing) and both desktop client diagnostics tests (offline IPC and browser isolation). This revalidation does not rebuild the installed executable. This earlier check predates the browser log follow-up below.

2026-09-07 browser logging follow-up: error and unhandledrejection events now record tab-local logs. Build and all70 current frontend tests passed (test command also ran23 archived tests). Real Edge component checks at1200/390 verify both events, masking before storage, reload persistence, JSON export, clearing/reload, no horizontal overflow and zero API requests. Evidence: test-output/relationship-ui/browser-logs-report.json and verify-browser-logs.cjs. Installed executable unchanged; no full installed or physical-phone claim.


Quota recovery fix: dirty in-memory entries remain authoritative after failed storage writes/deletes. Old persisted entries may reappear after reload if deletion is denied; the UI warns explicitly. Targeted7 tests and Edge390 export/clear failure scenario passed; evidence in browser-logs-quota-report.json.
