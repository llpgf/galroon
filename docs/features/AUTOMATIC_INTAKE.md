# Automatic discovery to collection — 2026-09-06

Latest update: matching algorithm v2 is applied to the Windows release. An algorithm upgrade retries prior-version review cases once, while preserving existing bindings and paused/interrupted queues. Same-source/brand manual confirmation and ancillary filename handling are documented in MATCHING.md. The actual remaining nine resources all matched: 30 works, 32 resources, 32 bindings, zero unmatched. Current release evidence is `test-output/matching-v2/build-report.json`; the original intake verification below remains historical.

The user explicitly requested a continuous Roon-like workflow: watch sources, notify on arrivals, match automatically with visible progress, and show posters in Collection. This extends the scoped matching fix, not the unrelated MVP backlog. No Goal was created or resumed.

## User flow

1. Save a source once. Watching is enabled by default. The local/paired desktop enables automatic intake; Core then performs the initial read-only scan when idle.
2. Native file events are coalesced after a three-second quiet period. Core scans affected folders. Initial attachment, reconnect and missed-event gaps trigger a full reconciliation scan. Disabled watchers remain disabled; paused, interrupted or failed scans require attention in Activity instead of endless retries.
3. Newly created resource groups produce durable, grouped in-app Updates notifications. Counts are resources, not games or files. Events survive closing the frontend; unread state is local to each frontend and collection. These are in-app notifications, not Windows system toasts.
4. After completed scans, Core matches unmatched resources in a durable queue. Unique strong title/alias/release/explicit-ID evidence binds automatically, creating or reusing a VNDB work. Contradictory meaningful member titles, subtitles, sequel conflicts, ambiguous or incomplete results stay Unmatched.
5. Catalog polling updates work cards and Matched status as items finish. Collection defaults to Recently added; filters start collapsed. Progress expands while work runs and collapses when idle. Updates links lead to the collection or remaining resources.
6. Refresh matches retries all remaining Unmatched resources. It never overwrites existing bindings. Repeated requests while active reuse the sequence. Automatic scans avoid repeatedly retrying unchanged review cases. Pause/Resume/Stop retain per-resource progress; provider failures pause for retry. A Core restart retains progress as interrupted, ready for Resume.

## Integrity and implementation

- Schema 11 adds `match_items` and lookup indexes. Existing migration machinery makes a credential-sanitized SQLite backup before upgrading schema 10.
- Membership/revision/fingerprint are rechecked in the same transaction that creates/reuses the work, follows merge history, binds the resource, records resource history and marks the queue item complete. A concurrent manual edit wins.
- Original titles/notes/overrides on existing works remain intact. Matched resources receive an unclassified edition and unknown role; work identification does not establish exact edition, completeness or file equivalence.
- `matching::search` remains a read-only candidate API. `automatch` performs catalog admission; `/api/matching/start` bootstraps once and `/api/matching/refresh` retries. Browser read-only sessions can see progress/updates but cannot start or control matching.
- No automatic rename, move, extraction, hashing, quarantine or file-plan approval. Original game files remain in place. SQLite stays local to one Core.
- Source watcher and matcher run independently of the frontend. Core shutdown stops scheduling and retains existing busy-job protection.

## Verification

- Core 84 tests passed, including seven automatic matching tests and an automatic reconciliation/notification regression. Coverage includes ambiguity, conflicting names, partial provider responses, unique-work reuse, original fixture bytes, no plans, manual edits during search, changed membership/revisions, pause/cancel/error recovery, restart/idempotence and refresh deferral. Browser mutation denial covers the two new write routes.
- Frontend 23 tests and TypeScript/production build passed. Browser checks verified desktop and 390-pixel layouts, Updates controls, Refresh, no horizontal document overflow and loaded VNDB poster images.
- Generated fixture: saving a source containing `Clannad.rar` and an unidentified numeric ZIP produced one automatic scan, one match, one review, and the CLANNAD poster. Adding `Steins;Gate.rar` later triggered watcher discovery, a second scan and one new automatic match without pressing Scan/Match. Refresh retried only the remaining numeric resource. Two work cards, three resource-discovery events, zero file plans. Evidence: `test-output/automatch/flow-report.json` and screenshots.
- The generated filenames are test inputs; the files contain only short fixture text. Real game content was not read or changed by these tests. This small flow verification is not a large-library matching-accuracy benchmark.

Release-native verification and current catalog counts are recorded in PROGRESS.md and the build/user-after reports in `test-output/automatch`.
