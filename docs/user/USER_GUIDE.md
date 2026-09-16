## Regroup resource files

In Organize, choose **Group files**. Browse or search the files, select members, then review the proposed group. Both file browsing and preview show 60 files per page; the selected count includes other pages. Review every page you need before confirming. Cancel stops waiting in the window and preserves the selection; a Core query may still finish in the background. If the catalog changes, review again before confirming. Regrouping changes catalog membership; it does not move the physical files.

## Work details and shared editions

Related works and exploration results check whether each work is in your collection. If that check fails, use Retry; an unavailable check does not mean a work is absent. Preparing a full profile or search result for a List includes later pages. If the collection changes during preparation, retry to review current results.

Edition comparison shows how many works share an edition. Editing an edition shows60 works per page. Search covers the collection; existing works on other pages stay associated. The selected count includes them, and Membership changes lists only your additions/removals. Undo change restores that item to its saved state. Retry a failed membership check; unknown membership is never shown as unchecked. If the collection changes, your draft remains visible: close and reopen the editor to load current data before reapplying it. Saving an edition shared by many works can take several seconds. Unsaved work notes stay in place during background refreshes; a conflicting revision is rejected instead of silently replacing newer data.

## Browsing a large collection

Collection displays 60 works per page. Search, filters, tag counts and sorting cover the entire collection. Selections survive page changes; adding filtered results to a List reviews all matches, not only the visible page. A single selection is limited to 10,000 works and larger requests show an error. If the collection or source availability changes while paging, use Retry to return to the first page with current results. Update Core if the connection screen reports that collection paging is unsupported.

# Galroon 0.0.2 — Windows quick guide

This is a development build. Installation and selected recovery flows have been tested on the development Windows machine; clean-machine and full native acceptance are still pending.

## Open your collection

Install with the supplied x64 setup program, then open Galroon. Keep its installed Core, runtime, Web and notice folders together. The desktop connects to an independent Core running as your Windows user. Closing the window leaves the Core and tasks running. Use **Settings → Stop Core and close** when you want to stop both.

Keep the active catalog on a local disk. Do not share the active database between two Core processes or place it on an SMB share. Catalog backups and game files are separate.

## Add and browse works

1. In **Sources**, add a folder. Review the scan scope and exclusions, then scan. Scanning indexes files in place. Automatic source monitoring can be enabled or disabled per source.
2. Inspect matching results in **Activity**. Uncertain or conflicting matches remain for review. Use **Organize** to search for a work or create a manual work, then confirm a binding.
3. Browse **Collection** using title, status, tags and display preferences. Work pages show editions/resources, personal notes, characters, staff and related works. Refreshing provider metadata preserves manual overrides.
4. Use **Lists** for manual reading order and private entry notes, or smart rules with saved result snapshots. Work-level edition preferences and list-entry preferences are distinct. Tags, list edits and browsing do not start file operations.

Unknown language, availability or relationship information remains unknown. External references are not automatically added to your collection. Spoiler and image privacy controls affect exploration; do not interpret hidden data as confirmed absent.

Once connected, the first Tab stop offers **Skip to content**. Press Enter to bypass the persistent navigation; the next Tab reaches search. Settings categories support arrow keys, Home and End.

## Work with files

For organization, preview the destination paths, approve that exact plan, then execute it. Only one managed organization root is active for a collection. Choosing an acquisition destination does not change that root. Existing destination files are not overwritten.

**Get files** copies selected resources to a separate local destination. **Use existing files** records reuse at the source and copies zero bytes; it does not extract or launch a game. Extraction requires a separate destination. Review file selection, archive volume groups and available space before starting.

Activity shows running, paused, interrupted and failed tasks. A retry rechecks recorded source/staging evidence. Copying can resume a validated prefix; extraction restarts the current archive inside staging. Re-enter an archive password when needed after a restart. A failed task is not a completed publication.

Completed file plans have a **Preview undo** action in Activity. Inspect and approve the reverse plan separately. Duplicate isolation and restore also use reviewed plans. Restoring a catalog backup does not reverse filesystem operations.

## Back up and recover

In **Settings → Backup & restore → Catalog backup**, choose an existing destination and export. Keep `manifest.json` and `collection.sqlite` together. The backup includes catalog metadata, personal tags/lists/preferences, corrections and history; it excludes games, saves, artwork cache, credentials and device-local transfer state.

To restore, choose a backup, inspect it and confirm the reviewed snapshot. The app protects the current catalog before switching into a newly restored directory. **Previous collections** lists recovery snapshots: select one, inspect and confirm to return through another protected restore. Old catalog files are retained.

After restoring, review source locations and scan state. Source watching and automatic matching remain disabled until deliberately enabled. If a source moved, use its location-change preview; relinking is a catalog change, not a content-hash proof. A scan rechecks observations. Old unfinished file operations are not replayed automatically.

Do not open a newer catalog with an older Core. Use its pre-upgrade backup if you need to inspect the older format.

## Browser and connection access

Set up owner access in **Settings → Account & devices**. The displayed local Core address provides a read-only browser login. Pairing, device revocation and connection repair are separate controls. Pairing in this build is limited to loopback on the same computer; LAN/NAS/TLS deployment is outside the validated scope.

When a connection fails, wait for reconnection or review its saved address and identity. Do not repeatedly submit a result-unknown write: inspect Activity and the affected record first. Connection failures do not silently select another collection.

## Get useful diagnostics

Open **Settings → Backup & restore → Diagnostics**, review logs, then download them if needed. The desktop can also review its own logs while disconnected. Browser diagnostics contain only that tab's client log. Nothing uploads automatically. Review an export before sharing because local paths may remain in error text.

For a useful report, record the app version, the exact action, whether the problem followed a restart/restore, and the visible request or job ID. See [SUPPORT.md](SUPPORT.md) for tested formats and limits, [API_CONTRACT.md](../development/API_CONTRACT.md) for the local client protocol, and `../third-party/README.md` in the installation for notices and source archives.

### Saving widely shared editions

Saving an edition shared by many works can take several seconds because each affected work must be updated. Wait for the save to finish before closing its editor. Once saving succeeds, the change is committed; Core may continue database housekeeping in the background and waits for that work during normal shutdown. Keep the collection's database and its companion files together; use Backup & restore to create a portable copy.

### Earlier tasks

Activity shows the latest100 tasks with live updates. Use Older records or Newer records to page through earlier tasks; each card includes its creation date and task ID. Older pages stay in place while new tasks arrive. Use their Refresh button to check current status, or Latest tasks to return to live updates. Available pause/resume/stop actions apply to the selected task.

### File-operation history

In the desktop Activity timeline, file-plan, individual-move and transfer entries show recorded state changes. Open file plan reviews the retained plan and its current results. Inspect current task fetches the task now; its status can differ from the historical event. Refresh that detail when checking progress. Earlier plans remain in File plans; file events from before schema41 are not reconstructed. Catalog upgrades first preserve a credential-free migration snapshot.

### Task summaries
Activity shows saved entity totals for new scan and matching tasks. These survive pause, cancellation, restart and catalog restore. Scanning and matching have separate summaries; file totals are never presented as work totals. Earlier tasks explicitly show that totals were not recorded. Encountered-error totals retain entries that failed even if retry succeeds; inspect task messages for the current result.
