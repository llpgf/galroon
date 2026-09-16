# VNDB matching improvement — 2026-09-06

## Algorithm v2 — applied to Windows (2026-09-06)

The nine remaining review cases were inspected individually. Eight already had exact VNDB candidates but ancillary audio/store/runtime/patch names vetoed admission. One abbreviated title, Mama x Kano EX, needed the user's existing manual confirmation for another resource of the same work.

- Ignore recognized technical directory subtrees, media extensions, generic bonus/audio labels and patch version suffixes when deriving title evidence. Keep actual work ancestors, subtitles and sequel numbers.
- Allow compact romanized filenames to corroborate an independently strong candidate, including `haisonsyoujo2` versus `Haison Shoujo [Ni]`. This does not create strong evidence by itself, and different sequel numbers remain conflicts.
- A current manual binding can corroborate an exact cleaned full label only within the same source and bracketed brand. Conflicting identities and resources previously matched automatically are excluded. The manual precedent is rechecked inside the admission transaction.
- Record algorithm version 2 in decisions and queue state. Upgrade retries old unchanged review cases once; already matched resources are excluded and paused/interrupted queues remain paused. Refresh remains available for an explicit retry.
- Show a human-readable `Why unmatched` reason on remaining unmatched rows, including missing title, partial title, ambiguity, conflicting name or incomplete provider results.

Actual VNDB re-query accepted all nine cases (`test-output/matching-v2/live-report.json`); this is a targeted regression sample, not a general accuracy claim. The earlier offline report is intermediate evidence. Full Core90/frontend23 tests and the Windows NSIS build passed. Native release was reopened and automatically completed **9 matched / 0 review**, producing **30 works / 32 resources / 32 bindings / 2 sources / 0 unmatched**. The UI displayed the completion notification and Collection posters. All 23 prior bindings, prior work metadata apart from legitimate revision increments, source paths, indexed file metadata and resource membership were preserved; only scan observation markers changed during restart reconciliation. No file plans or operations were created.

Recovery backup: `test-output/matching-v2/catalog-backup.sqlite`. Before/after snapshots, verification script and current artifact hashes: `test-output/matching-v2/apply-before.json`, `apply-after.json`, `verify_apply.py`, `build-report.json`. This remains schema 11. The unrelated MVP backlog stays paused, with no Goal created/resumed. The sections below describe the first algorithm iteration and historical results.

This records the first scoped matching-algorithm fix after the UI review stop. The later automatic workflow is described in [AUTOMATIC_INTAKE.md](AUTOMATIC_INTAKE.md): schema is now 11, and a separate queue automatically admits reliable unique matches. The search API itself still only suggests candidates and never creates works, binds resources or moves files. Other MVP work stays paused; no Goal was created or resumed.

## What changed

Previously the scan display title went directly to one VNDB search with ten results and no explicit sort. Numeric directories, edition labels, extras and repeated titles reduced retrieval. There was no local ranking.

- Normalize NFKC, case, whitespace and kana. Remove known distribution labels, archive/part extensions and edition/bonus suffixes. Retain meaningful title brackets, sequel numbers, remake names and subtitles. Preserve slash-containing manual titles such as Fate/stay night.
- Use resource-relative paths and at most 32 already-indexed member paths to recover names inside numeric/store-code directories. No game, archive or metadata.json content is read. A numeric label without usable hints asks for a title; store IDs are not treated as VNDB IDs. Explicit vNNN or its VNDB URL uses the ID filter.
- Request searchrank and up to 30 results per query; compare all returned multilingual titles, romanizations and aliases. Exact matches precede fuzzy bigram overlap. Number conflicts, differing subtitles, short names, ties and incomplete searches stay review cases. Internal scores are ranking weights, not probabilities.
- Query release titles when work-title evidence is insufficient, then follow exact cleaned release-title links to works. A compilation linking multiple works remains ambiguous. This identifies a work, not the exact edition or completeness of local files.
- Limit a search to three work queries, one release query and one batched ID lookup. Each request times out at eight seconds. One active search, 1.6-second uncached request spacing, and a 32-response/ten-minute memory cache bound work. Provider errors stop further requests and label retained results incomplete. Metadata refresh still has its separate provider path.
- Show searched names and title/alias/release/subtitle/number evidence. Editing the query, closing the drawer or choosing another resource invalidates older responses. Candidate buttons disable during pending actions. Existing scanned display titles and confirmed matches are not rewritten; rescanning is unnecessary.

API fields, sorting and release relationships were checked against the [official VNDB Kana documentation](https://api.vndb.org/kana).

## Verification

Seven new Core tests cover twelve distribution-name cases, numeric/member hints, multilingual/alias/kana/full-width/slash titles, sequel conflicts, same-title ambiguity, deduplication, cached/429 partial failures, compilation links, authentication and no catalog writes. Full Core76/frontend23 and production frontend passed; the final slash-name refinement reran matching7 successfully.

Eight convenience samples from sandbox_data names were compared with actual VNDB queries. Old queries retrieved candidates for 1/8; new queries for 8/8. Five have exact title evidence, two release links, one only a partial title. This is a small retrieval regression, not a representative accuracy percentage or the independent ~30-group MVP matching gate.

| Resource sample | First work | Evidence |
|---|---|---|
| ママ×カノEX | v59911 | Partial title; review |
| 猫忍えくすはーとSPIN! 2 | v58009 | Exact title; base/SPIN LOVE+PLUS show number conflicts |
| やりなおしクランクイン, repeated | v59027 | Exact title |
| 下級生リメイク | v2341 | Release r140695 and other remake releases |
| レイブン・ブラック・ラック・ライフ… | v59044 | Exact original title |
| 1261651 / アンラベル・トリガー | v47547 | Member filename, exact title |
| 1321849 / 廃村少女［弐］… | v53486 | Member filename, bracketed sequel retained |
| 1327332 / アンラベル・トリガー -Prelude to War- | v47547 | Release r133215 |

Evidence is in test-output/matching: samples.json, live-report-final.json, release-reference.json. live-report.json is the earlier result before release lookup.

The browser fixture used two dummy files and normal source/scan/search UI with real Core HTTP plus a test-only native IPC bridge. Numeric-parent lookup returned v47547; sequel lookup ranked v58009 first with conflicts visible. Desktop/390px screenshots were inspected without horizontal overflow. A deliberately delayed old response was discarded after editing; a numeric-only override sent no provider queries and requested a title. Catalog remained zero works/bindings/plans. See ui-report.json, release-desktop.png, sequel-mobile.png and needs-title-mobile.png. This is not installed-native acceptance.

The first source action navigated to Sources, so the form wait timed out before any write; a fresh snapshot allowed recovery with one source creation. A background server launch was rejected by automatic review without a detailed reason. A permitted tool-managed fixture was used instead; its local session and bridge files contain test credentials and are excluded from reports.

Remaining: learned corrections, a translation/transliteration engine, publisher/date weighting, durable evidence cache, batch automatic matching, calibrated confidence and broader independent samples. Other Windows MVP gates remain pending. Final package/reopen evidence is recorded in PROGRESS.md and test-output/matching/build-report.json.

Final Windows NSIS build passed; controller restore/restart tests passed 2/2 against that Core. The same user catalog was reopened in the rebuilt native App. Native search of resource 1261651 produced v47547 first with exact-title evidence, and v55273 with a differing-subtitle warning. No candidate was confirmed. Works/resources/bindings/roots remained hash-identical before restart and after search. The native App is left open for review; installer lifecycle acceptance remains separate.
