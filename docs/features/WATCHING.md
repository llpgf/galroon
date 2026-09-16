# Source watching

Sources have automatic updates enabled by default. The Windows Core uses native filesystem notifications through the pinned `notify` 8.2.0 dependency. Events are hints; the scanner remains responsible for validating paths and proving file availability. Network filesystems may not deliver reliable events, so manual scanning remains available. See the [upstream limitations](https://docs.rs/notify/8.2.0/notify/#known-problems) and [rescan event contract](https://docs.rs/notify/8.2.0/notify/struct.Event.html#method.need_rescan).

## Behavior

- Notifications are coalesced for three quiet seconds per source. A modified file queues a shallow enumeration of its parent. A new or moved-in directory additionally queues a recursive scan of that directory, without visiting unrelated siblings. Renames can produce both source and destination hints.
- Pending directory scopes are stored in SQLite. Job creation, frontier insertion and removal from the pending set share a transaction. Jobs use the existing pause, resume, cancellation and restart behavior. Automatic scans appear as **Automatic source update** in Activity.
- Automatic admission shares the mutation lock and idle check used by manual scans, file plans and acquisition. A paused or failed automatic task holds further automatic work for that source. Cancelling it disables watching for that source; the user can enable watching again. Disabling watching does not cancel an already running task.
- Each source has persistent watch exclusions. **Scan with watch exclusions** copies these into the manual scan form. Other manual scans keep their explicit per-task settings. Changed settings do not rewrite existing jobs.
- Core-internal state, `.galroon-*` transfer staging, standard ignored folders and excluded paths do not generate indexing work. Scanners continue to reject links and paths outside the root. Watching never starts hashing, archive inspection or file mutation plans.

## Gaps and failures

Core restart, initial attachment, source reconnection, notification overflow and backend errors mark the source as needing a scan. This flag is cleared only by a later successful recursive full-source scan with matching exclusions. A scoped automatic update does not imply that changes made while the Core was offline have been reconciled.

The event channel holds at most 256 notifications per source, and the durable pending set at most 128 coalesced scopes. Overflow requests a manual scan rather than silently assuming nothing changed. Backend failures show a manual-scanning status and retry after 30 seconds. Reconnection attaches a new native watcher. Taking a root offline preserves the existing catalog; it does not mark every file missing.

## API and storage

Schema v6 adds `source_watch`. `GET /api/roots/{id}/watch` returns configuration, status, pending scope count, notification-gap state and the tracked automatic job. It does not create configuration or mutate source files. `POST` on the same route requires a write-capable desktop identity and `{revision, enabled, exclude}`. Stale configuration revisions and exclusions outside the source are rejected. Web cookies cannot change watch settings.

The independent Core stops automatic admission under the same lock used for checking active work before accepting a shutdown. Native watcher ownership is scoped to the Core service; reopening a desktop window does not start a second watcher service.

## Evidence

- `cargo test -p galroon-core --lib`: scoped/coalesced hints, ignored paths, shallow enumeration, deleted subtree detection, unchanged siblings, durable queue recovery, paused/cancelled tasks, shutdown admission, overflow and stale settings. The HTTP access suite denies browser writes to the new route.
- `verify_watching <fresh-output> <core-exe>`: actual Windows notifications and a different Core process after restart. Tests new files, excluded files, deleted directories, unvisited siblings, durable pending changes, visible restart gaps, root offline/reconnect, disabled watching and unchanged original fixture bytes.
- `test-output/watch-lifecycle-02/report.json`: native process acceptance.
- `test-output/ui-watching/report.json` and `output/playwright/watching-*.png`: UI settings, watch-profile scanning and rendered layouts.

All changes and failures in these tests use generated fixtures. No original `sandbox_data` file is modified. Linux/NAS and macOS watcher behavior has not been validated by this Windows acceptance.
