# Plan: matching accuracy and AI-assisted matching (2026-10-12)

Planned by Claude Code and executed by Codex. Do one phase per PR, in order. Do not start a phase until the previous one is merged, unless this plan says otherwise. Stop and report at each **STOP** gate.

## Where we are

- Matcher `ALGORITHM_VERSION` is 6 on master (`176347c`). PR #31 treats "+" as part of a title. PR #32 matches a compilation to the work it is named after.
- Acceptance on `sandbox_data` passes: Playwright 31/31, golden `tests/e2e/golden/sandbox_data.json` 37 ok / 0 false.
- The real NAS library has 792 resources. The last full run, `~/GalroonTestData/nas-2026-10-11-v6` (algorithm 5), matched 561 and left 231 for review:

  | Reason | Count |
  |---|---|
  | A different source name needs review | 98 |
  | No VNDB candidate found | 51 |
  | Multiple works have equally strong evidence | 30, about 16 after #31 and #32 |
  | Only a partial title matches | 22 |
  | The name is too short | 13 |

- An offline replay checked #31 and #32: it re-ranked the stored VNDB candidates with the new code, with no NAS or network. The replay was a throwaway test. Phase 1 makes it a real tool.

## Rules that apply to every phase

1. **Zero false matches.** A resource bound to the wrong work is a hard failure. Leaving a resource unmatched is acceptable. A change that turns any resource into a strong match for a different work than before must not merge.
2. **Never write to the user's library.** The NAS share `/Volumes/Backup` is read-write. Run Core against it only under `sandbox-exec -p '(version 1)(allow default)(deny file-write* (subpath "/Volumes/Backup"))'`, with a fresh data directory. Never open `~/GalroonTestData/*/data` in place: copy it first and open the copy, or open it read-only (`SQLITE_OPEN_READ_ONLY`).
3. **Code style.** The codebase is deliberately dense. Do not run `cargo fmt`. Match the surrounding style.
4. **Tests on macOS.** `export PATH="$HOME/.cargo/bin:$PATH"; cargo test -p galroon-core -p galroon-mcp --lib --bins --tests`. Without `--lib --bins --tests`, cargo also builds Windows-only examples and fails. A single example builds on its own with `cargo run -p galroon-core --example <name>`.
5. **Acceptance.** `npx playwright test` takes about 13 minutes and needs `/Volumes/新增磁碟區/sandbox_data` mounted. It must end with `Golden: 37 ok / 0 unmatched / 0 FALSE`. Run it for every phase that changes matching or UI.
6. **Bump `matching::ALGORITHM_VERSION`** whenever ranking or review decisions change, so unmatched resources are evaluated again.
7. **Branches.** Work on a branch and open a PR against `master`. Do not merge; the owner merges. Put before/after numbers in the PR description.
8. **Privacy.** NAS folder names are the owner's private collection. Reports with real names stay under `~/GalroonTestData/`, never in the repository or a PR. PR descriptions may quote a few names as examples.

---

## Phase 1: offline replay tool

**Goal:** answer "what does this matcher change on the real library?" in under a minute, without the NAS or VNDB.

### Build
1. Add a public function in `crates/core/src/automatch.rs`, for example `pub fn replay(c:&Connection,rid:&str,response:&Value)->R<Option<Value>>`. It builds the matcher input exactly as `snapshot` does: same hints, same 31-member cap, same bundle-member handling. Unlike `snapshot`, it must also work for resources that are already bound. Then it re-ranks the stored candidates with `matching::rank` and returns the same result and decision as `explain`.
   - Factor the input-building part of `snapshot` into a shared helper rather than copying it.
   - `snapshot` and `explain` must keep their current behaviour.
2. Add `crates/core/examples/match_replay.rs`. Use `examples/diagnose_matching.rs` as a model.

   ```
   match_replay <library.sqlite> --out <report.json> [--baseline <report.json>] [--answers <answers.json>]
   ```

   - Open the database with `SQLITE_OPEN_READ_ONLY`.
   - For every resource, take the latest `match_items.evidence` (join `jobs`, order by `jobs.rowid`). Re-rank its `results` (drop each candidate's old `match`) and run the decision.
   - Each report row: resource id, relative path, chosen VNDB id, strength, ambiguous flag, reason, review reason or `null`, and the stored outcome (`match_items.state`, `work_id`, and the work's `vndb_id`).
   - `--baseline`: print a diff grouped as **gained** (review → strong), **lost** (strong → review), **changed** (strong to a different VNDB id) and **reason changed**. `changed` is the false-match alarm.
   - `--answers`: a JSON map `{ "<relative path>": { "vndb": "v123", "source": "user" | "auto-2026-10-11" } }`. Count **agree**, **false** (strong to a different id than the answer) and **missing** (answer exists, result not strong).
   - Exit with a non-zero code when `changed` > 0 or `false` > 0.
3. Limitation to print in the report header: replay only sees candidates stored in the earlier run. A change that would make Core send different VNDB queries can only be confirmed by a live run (Phase 2).

### NAS answer key (outside the repository)
4. Create `~/GalroonTestData/nas-answers.json` from the v6 run: every resource matched there gets `"source": "auto-2026-10-11"`. The owner sampled 60 of them and found 0 false matches. Later, owner confirmations change entries to `"source": "user"`.

### Verify
5. `cargo test` passes, including a new unit test for `replay` on a small fixture: one bound resource, one unmatched resource, one compilation.
6. Run the tool on a copy of `~/GalroonTestData/nas-2026-10-11-v6/data/library.sqlite`, twice:
   - with master before this phase's code: no matcher change, so 0 changed;
   - with the algorithm-6 matcher, compared with the algorithm-5 baseline. Expected: about 14 gained, 0 changed. The 4 "+" titles are アマナツ, アマナツ＋, アマカノ2 and アマカノ2＋. The 10 compilations are listed in PR #32.
7. Document the tool in `docs/features/MATCHING.md`, in a short "Replay" section.

**STOP:** open the PR with the replay numbers. The owner reviews and merges.

---

## Phase 2: live NAS re-run and AI suggestion pilot (read-only)

**Needs:** the NAS mounted at `/Volumes/Backup/Galgame`. If it is not mounted, do step 2 on a copy of the v6 data directory instead, and say so in the report.

1. **Live re-run** with master (algorithm 6): use `~/GalroonTestData/nas-2026-10-11-v6/run.mjs` and a new output folder `~/GalroonTestData/nas-<date>-a6`. Check that the before and after fingerprints are identical. Run Phase 1's replay tool with `--answers`. Report matched, review, gained, lost, changed and false counts.
2. **AI pilot.** Run Codex with the galroon MCP server against that run's Core (`--url`; see `docs/features/MCP.md`). Use only read tools: `list_resources`, `list_resource_files`, `search_vndb`, `get_work`. **Do not call `match_resource` or any write tool.**
   - Scope: every resource whose review reason is "No VNDB candidate found" (about 51), then "Only a partial title matches" (about 22).
   - For each resource, try at most 4 queries, for example: the Japanese title from a Chinese or English name, a romaji spelling, an expanded abbreviation, or a brand plus a date.
   - Output: `~/GalroonTestData/ai-pilot-<date>.csv` with the columns `resource_id, path, suggested_vndb, suggested_title, query_used, evidence, confidence (high/medium/low), notes`. `evidence` must be checkable, for example "VNDB alttitle equals 'X'", "release r123 title equals the folder name" or "brand Y and date 2021-04-23 match release r456". Use "no suggestion" when unsure. A wrong guess is worse than no guess.
3. **STOP:** the owner reviews the CSV and marks each row correct, wrong or unsure. Then compute precision per confidence level.
   - Continue to Phase 3 only if **high** confidence has 0 wrong rows and medium is at least 90% correct.
   - Add the owner-confirmed rows to `nas-answers.json` with `"source": "user"`.

---

## Phase 3: suggest-only AI matching in the product

Start only after the Phase 2 gate passes. Before you write code, write the user story and add it to `docs/acceptance/USER_STORIES.md`. The owner must approve the new story, so **STOP** until then.

Design that the owner has already approved in principle:
- **AI suggests and the person confirms.** A new MCP tool, `suggest_match(resource_id, revision, vndb_id, query, rationale)`, records a suggestion. It never binds a resource.
- **Storage.** A schema migration adds a `match_suggestions` table. A suggestion is void when the resource's revision changes. It stays in place across rescans of an unchanged resource.
- **Verified label.** Core checks the suggestion deterministically. It is **verified** when the VNDB candidate's title, alias or release title exactly equals the given `query` after normalization, **and** a folder brand or date corroborates it, or the folder has neither. Anything else shows as **unverified**.
- **UI.** In the review list, a resource with a suggestion shows an "AI 建議" badge (zh-TW; English "AI suggestion"), the rationale, and a Confirm button. Confirm goes through the existing manual-match path, so the binding history records it as a person's confirmation. Batch-confirm is allowed for verified suggestions only.
- **Out of scope.** No automatic binding from AI, even when verified. That would need its own decision from the owner later.
- **Tests.** Unit tests for verification and revision invalidation. An MCP test that `suggest_match` never creates a binding. A Playwright story: suggestion shown → confirm → card appears → rescan keeps it.

---

## Phase 4: turn review cases into matcher rules (repeat as needed)

Start after Phase 1. This phase can run alongside Phases 2 and 3.

For one review category at a time, starting with "A different source name needs review" (98):
1. Group that category's cases from the replay report into patterns, for example "Chinese patch file name", "translated title" or "artbook/OST member with its own title". Write the groups in the PR description, using no more than a few real names.
2. Propose one rule per PR. Each PR must:
   - include a unit test with real-shaped names;
   - include a replay diff against the latest live run: gained listed in full, **changed = 0**, lost explained;
   - pass Playwright acceptance with golden 37/0/0;
   - bump `ALGORITHM_VERSION`.
3. Rules the owner has already decided:
   - Members that point to a different game still go to review. Examples are アママネ 2 with アママネ inside, BaldrSky with Dive1/Dive2, and 竜姫ぐーたらいふ with 2 and LOVE+PLUS.
   - Short titles under 4 characters stay review cases unless corroborated, as before. 善悪 and "[Key] AIR" stay manual.
   - Pure box sets stay review cases.
   - An append that VNDB lists only as a release of the base work matches the base work, for example 1327332 → v47547.

Report after each PR: category count before and after, and the total matched count from the replay.
