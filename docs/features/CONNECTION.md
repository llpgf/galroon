# Windows Core connection recovery

The Windows client reacquires its local session through the existing trusted `core_session` IPC command after a transport failure, read timeout or HTTP 401. The controller discovers the running Core or starts it through the existing singleton protocol. The frontend does not discover credentials in files or retain the old token as a fallback.

Recovery attempts are coalesced and back off from 500 ms through 1, 2, 4, 8 and 16 seconds to a 30-second ceiling. Manual reconnect joins an existing attempt. Ordinary application errors, such as a conflicting file destination, do not restart Core. A deliberate stop request pauses automatic recovery even if its acknowledgement fails; the user can reconnect manually.

While disconnected, the current view and unsaved fields stay mounted. An accessible connection banner remains available while the application controls are inert. On recovery, catalog lists and access capabilities refresh. An unsaved work-detail draft keeps its old revision, so a later conflicting edit is rejected by Core rather than overwriting a newer revision. Open detail/review dialogs are not silently resubmitted or relabelled as current server state.

Every connection transition invalidates responses from the previous session. Late reads are discarded. A write whose result was lost or became stale reports an uncertain outcome and asks the user to check Activity and the affected item. No GET or POST is automatically replayed. Newly initiated reads refresh the catalog after connection recovery. Reads have a 15-second timeout; writes are not given that short timeout because verified file operations may legitimately take longer.

This is local Windows recovery. Browser cookie login, remote desktop pairing, network discovery and credential persistence are separate concerns. The reconnect flow does not provide a UI for selecting another catalog.

## Retained verification

- `npm test`: 16 assertions/tests passed across API transitions (10) and collection behavior (6). API coverage includes coalescing, new URL/token use, no write replay, stale writes, retry delay, HTTP 401 versus ordinary errors, read timeout, long writes, restore recovery and explicit stop suppression.
- `cargo test -p galroon-desktop --bin galroon-desktop`: 2 controller tests passed against the real release Windows Core. One test externally stops/restarts Core and verifies that the controller follows the replacement instance, URL and token while preserving catalog data. The other exercises backup restore and preservation of the old database.
- `test-output/ui-reconnect/report.json`: a generated catalog's Core process was terminated and restarted with rotated credentials. The browser rendered the real frontend using an explicitly test-only Tauri IPC bridge. Controls paused, automatic retry recovered, and an unsaved note survived. Desktop and 390-pixel screenshots were inspected; mobile had no horizontal overflow.
- The browser fault probe let the Core commit a note, then aborted the HTTP response. Exactly one POST was observed, the revision advanced from 1 to 2, the stored note matched the draft, and the UI displayed the uncertain-outcome message after recovery.

The browser bridge is not evidence of an installed Tauri WebView session. Clean installed-app, native post-restore UI and install/update/uninstall acceptance remain tracked in `WINDOWS_ACCEPTANCE.md`. All mutations used generated fixture data; supplied game originals were untouched.

## Installed native local endpoint recovery — 2026-09-12

`test-output/mvp-goal/native-reconnect-current-r1/report.json` records installed Core274b453e... recovery from an externally stopped Core on port12925 to an automatically started replacement on8400. The native window retained its Organize view and newly opened Group files fetched125 members/60 first-page rows. Seven identity/catalog/history tables match the baseline; both processes closed. This supplements the earlier browser bridge with an actual installed Tauri positive recovery case. Unsaved writes, revoked pairings and identity-mismatch faults remain separate gates.
