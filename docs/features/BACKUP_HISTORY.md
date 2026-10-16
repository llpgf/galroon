# Backup history and manifest v2

Source increment, 2026-09-07. Schema13 adds backup_history; only completed exports passing inspect are recorded. GET /api/backups returns the newest 50 records and refuses Web sessions. The database snapshot precedes recording its own success, so it contains earlier history only. A removed or relocated backup can still have a history record; selecting it performs a fresh inspection.

Manifest v2 contains library identity, schema, actual table names, file size/SHA256 and integrity-and-hash verification. Inspect validates these claims against the snapshot. V1 remains readable without claiming identity fields it lacks. Credentials are sanitized as before. Artwork cache, game data/saves and machine-local credentials/transfers are outside this catalog snapshot.

Galroon-backup-* and .galroon-backup-* are reserved backup directory prefixes, case-insensitive. Manual scans reject those roots/scopes; traversal, stale queued frontier and watcher hints exclude them even before a manifest exists. Renaming a backup to an arbitrary non-reserved directory is not automatic protection.

Evidence in test-output/m1-backup-history: core-tests.log, backup-tests.log, frontend-tests.log, core-build.log, verify-ui.cjs and backup-preview.png. UI uses generated mocked API data. No installed full restore or extended acceptance pass is implied. Current-before-replacement snapshot and history return are now implemented (details below); installed combined and full extended-model restore acceptance remain required.

## M1 restore protection and return flow — 2026-09-07

- Before changing the active catalog pointer, desktop restore stops the old writer normally, holds its Core lock, exports and verifies a credential-sanitized current snapshot under .galroon-restore-snapshots, then records snapshot identity/hash with the previous catalog. Protection failure leaves the old pointer authoritative; reconnect reopens the original collection.
- Previous collections in Backup settings lists up to 50 recovery snapshots. Selection fills the existing inspect/confirm restore flow. Returning restores a fresh isolated copy (normal task/plan reset), and first protects the collection being left; it does not directly reactivate an old journal. Older history entries without recovery snapshots are not offered by this new picker; their retained database paths remain untouched.
- Desktop7 passed with expanded actual-Core acceptance: obstructed snapshot destination refuses switch, original reconnect succeeds, restore and return preserve identities/titles, a second recovery snapshot protects the intervening catalog, and reopen retains selection. Frontend43 and TypeScript passed. UI fixture verifies previous selection and fresh inspection rendering with no horizontal overflow; native command/API responses are mocked only in the rendering fixture. Evidence: test-output/m1-restore-protection/.
- Source increment only; NSIS/installed app still need rebuilding and full interaction acceptance. A6 remains partial pending installed combined cases and the later complete extended-data model. Originals/NAS unchanged. Next: package these M1 increments and run installed pairing/restore/return verification before M2.


## Restore watcher replay boundary fixed — 2026-09-07

- Audit found that restore interrupted old jobs/plans but retained enabled source watchers. A new filesystem event could therefore trigger scanning before paths were reviewed. This was not covered by the prior installed normal restore/return acceptance.
- restore_new now materializes watcher rows for all restored roots (including legacy snapshots), disables them, clears pending hints/job links, increments revision and requires reconciliation. Startup preserves disabled watcher explanations. Restore copy tells users to check paths and enable monitoring again. Automatic matching remains off after existing credential/settings sanitization; no protection was relaxed.
- Core105 passed, including actual watcher startup plus generated post-restore file event with no new job, retained interrupted job/ready plan and missing legacy watcher-table case. TypeScript and release Core build passed. Evidence: test-output/m1-restore-watch/.
- Current installed package still predates this fix. Update package and repeat installed restored-source behavior before closing M1. Installed pairing/fault cases and later M2–M6 remain pending. Only generated fixtures were changed; originals/NAS unaffected.


## Installed Core recovery verification — 2026-09-07

Rebuilt NSIS with restore watcher disabling and performed a same-version update of the isolated test installation (exit0). Installed Core SHA256 matches release: CFD9C8DEF2F9CF7AED6BF116B66629631341ECD1554F4C0E4D367E91F610C2E3. NSIS SHA256: E712F0A64BFB5CEA7EAFF520E0B37E0D99FE86F84513736C20F3990260C4A1D5.

Desktop tests now accept GALROON_TEST_CORE; 8/8 passed using the installed Core executable. This includes real Core pairing/persistence/revocation/pinned identity/endpoint recovery/reconnect/restore return and a restored enabled-source fixture: after starting Core and adding a generated file, monitoring stays disabled and no jobs appear after 5 seconds. Evidence: test-output/m1-installed-core/build.log, desktop-tests.log, package-report.json. This is controller integration against installed Core, not installed UI pairing acceptance. Core105 and frontend43 remain prior unaffected baselines. Native pairing/fault UI cases and M2–M6 remain pending; Goal active. Original game files/NAS untouched.


## Manifest v4 and full backups — 2026-10-16

New exports use manifest v4. Portable remains the manual default: it removes sessions, pairing codes, access attempts and private settings. Full preserves sessions, the owner password hash and other settings, removes only pairing codes/access attempts, and requires owner access through the API. History now records mode and origin; older records default to portable/manual. Artwork can be omitted through the Core export options; the existing catalog-plus-artwork combined hash is unchanged.

Full exports include `identity/device.json`, optional `identity/network.json`, and both `identity/lan-key.pem` and `identity/lan-cert.pem` or neither. Lock files are excluded. Inspection verifies file sizes/hashes, device UUID, the TLS key/certificate pair and SPKI fingerprint. Both modes include a path-sorted `SHA256SUMS` file covering the catalog, optional artwork, identity and manifest, usable with `sha256sum -c SHA256SUMS` from the backup folder. Failed exports remove their partial folder on a best-effort basis. Versions 1–3 remain readable.

`restore_new` always strips credentials and ignores identity, even for full backups. `restore_new_full` requires a full backup, preserves credentials and stages identity under the new destination's `identity/`. Both interrupt jobs, reset plans and disable source watchers. Neither installs identity into a device directory, switches the running Core nor restarts it.

**Security:** Full backups are unencrypted and contain the owner password hash, session token hashes and LAN private key. Someone with the private key can impersonate this Core; previously issued bearer tokens remain usable after full restore until revoked or expired. Unix full-backup directories use 0700 and files use 0600; Synology ACLs may broaden access. Keep these backups out of shared SMB folders and cloud uploads.
