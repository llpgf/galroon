# Windows desktop pairing

Implemented 2026-09-07. This is the **loopback Windows pairing increment**, not NAS/LAN deployment or completion of every A5 fault case.

## User flow

Settings > Collection & Core > Core connections lists the local owner connection and up to 32 saved paired connections. Enter the Core loopback address and the owner-issued 12-character code. Saving a pairing does not switch the active collection. Open collection presents an explicit unsaved-change confirmation, validates the target, saves the selection and reloads the client. The offline connection banner also exposes this manager.

Revoke & remove revokes the device token on the Core before deleting the local credential. Forget locally is an explicitly separate recovery action for an unavailable or already revoked Core; it does not claim server revocation. Switch away before removing the selected connection.

## Boundaries

- Only http://127.0.0.1:port (or localhost normalized to that address) is accepted. No userinfo, paths, query, fragment, redirects, proxy or LAN destinations. Existing loopback listeners and CSP remain unchanged.
- Tokens are stored as per-user Windows Credential Manager generic credentials, not in connections.json, browser storage, catalog or backup. JSON contains a random credential reference and pinned library/device IDs only.
- Saved selection is durable. Missing credentials, revocation, incompatibility, malformed settings or an identity mismatch fail closed; no automatic fallback local collection is created. Pairing requests are not retried automatically.
- Native Stop Core and Restore are blocked for paired connections; their UI is hidden. They remain local-owner operations. Acquisition remains on the Core computer; no remote worker/transfer is implied.
- Connection changes are serialized with the existing cross-process desktop launch lock. File writes validate link/containment chains and atomically replace the metadata file.

## Verified evidence

`test-output/delivery-20260907/pairing-tests.log`: six desktop tests passed, including real Windows Credential Manager save/read/delete, single-use code redemption, persisted selection, device identity mismatch, external revocation, self-revocation, no local fallback database, local lifecycle reconnect and restore. Tests use generated credentials/catalogs and delete their credential entries.

`frontend-tests.log`: 42 active frontend tests, including blocked writes during selection, failed selection preserving context and reload only after successful native selection. No test-output copies are counted.

Remaining: full installed interaction matrix for pairing; LAN/TLS deployment; automatic endpoint rediscovery after a Core changes its listening port; broader A5 fault/capability acceptance. Use Change address to verify and save a new loopback endpoint for the same pinned Core; valid existing credentials are retained. Revoked or missing credentials still require pairing again. Credentials are per Windows user; portable client migration is not provided.

Final NSIS installation and native Settings panel rendering verified. Full user-click pairing acceptance remains pending, distinct from the passing native protocol/credential integration tests.

## Explicit address recovery (source increment)

Change address verifies the existing pairing against the supplied loopback endpoint and checks both pinned identities before saving. No automatic port discovery, network widening or identity replacement. Saving reloads the app (explicit form copy warns about unsaved drafts). Failure retains metadata and selected collection. Native tests use generated Core restart and real Windows Credential Manager; UI fixture covers rendered failure and retained draft, not installed end-to-end success. Desktop7/frontend43 and TypeScript passed. See `test-output/m1-endpoint-recovery/desktop-tests.log`, `frontend-tests.log`, `verify-ui.cjs`, `address-rejected.png`. Installer has not yet been rebuilt for this increment.

## Connection selection versus live status — 2026-09-07

Fixed ConnectionsPanel showing Connected for a saved selected Core even when offline. It now subscribes to the same connection snapshot as the recovery banner and separately labels selection and actual connection phase. Connecting/offline messages refer to the selected Core rather than implying the local owner Core. No changes to credentials, selection persistence or network scope.

TypeScript passed; frontend43 passed. Browser fixture verified connected -> offline -> connected updates, cancel keeps the selected paired Core, saved address remains and no horizontal overflow at1100px. Inspected test-output/m1-connection-state/disconnected.png; reports and reproducible verifier alongside. This is browser fixture evidence, not installed UI pairing. Installed package remains prior E712F0A64BFB5CEA7EAFF520E0B37E0D99FE86F84513736C20F3990260C4A1D5; new frontend label fix requires next package. M1 native pairing UI/fault gates and all remaining M2–M6 requirements stay pending. Goal active.
