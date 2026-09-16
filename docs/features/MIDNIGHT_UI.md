# Midnight Library implementation

2026-09-06. The user authorized implementing the approved Figma UI in the application. This is a scoped UI and supporting metadata update; it does not resume unrelated MVP work or create a Goal.

## Design and behavior

Reference: https://www.figma.com/design/yb8y1m18oLzs3bSu1rVP1K
Frames: Collection `6:2`, work `6:98`, character `8:116`, person `8:190`, organize preview `10:186`. Local reference screenshots are in `../design/figma-midnight`.

- Shared charcoal/lavender palette, Libre Baskerville headings, DM Sans interface text, narrow sidebar, poster collection and responsive layouts.
- Work details scroll through introduction, paired characters/voice actors, staff, related works and existing editions/resources/notes. Existing matching, source watching, progress, status, favorites, metadata correction and file operations remain connected to the Core.
- Character artwork/name and the actor avatar/name have independent click targets. Multiple voice credits and credited aliases remain distinct. Staff defaults to original-edition credits, with an all-editions toggle and expansion.
- Character and person views read actual VNDB data. Missing person photos use Unicode name-derived initials. No Figma sample metadata was imported into the user's library.
- Back from character/person restores work scroll position; returning to Collection preserves collection filters and scroll position.
- File operations remain preview → explicit approval → separate execution. The preview uses the existing journal-backed Core implementation.
- All added interface labels use the existing English/i18next language foundation. Desktop and narrow web layouts share the frontend; this update does not enable remote network access.

## Metadata implementation and limits

`GET /api/works/{local-work-id}/exploration` and `GET /api/people/{staff-id}?page=1` authenticate through existing Core access control. Provider metadata is cached separately in SQLite settings under `exploration.v2.*` for one day. Stale data can be returned with a warning on provider failure. Provider requests are serialized, delayed and time limited, with HTTP-failure cooldowns. Existing editable work metadata is not overwritten by exploration fetches.

VNDB API reference: https://api.vndb.org/kana. Person identity is separate from credited aliases. The API does not provide staff portrait photos; current VNDB-backed person avatars therefore use names. Character images and biographies come from VNDB; missing artwork remains an explicit fallback. Tagged description spoilers and work-specific character spoiler associations are hidden by default. This relies on provider tagging and is not a guarantee that every biography is spoiler-free.

Character queries page up to 1,000 entries with a visible limit notice. Person filmography loads ten works at a time. Its “In my collection” filter currently applies to loaded filmography pages; “Load more credits” expands that set. Related works in the collection open in-app; external references open VNDB. Character pages are available for the current work's fetched characters; other filmography characters link to VNDB. This is not a complete offline VNDB graph or a new source for staff portraits.

## Verification and handoff

Evidence: `test-output/figma-implementation/`.

- Frontend: 27 tests passed, excluding backup copies under test-output. Production TypeScript/Vite and Windows NSIS builds passed.
- Core: full 93-test suite passed; after adding the endpoint authentication/local-work test, all four exploration tests passed (90 unrelated tests filtered).
- Live VNDB integration: STEINS;GATE returned 17 characters, 16 voice pairs and 242 edition-inclusive staff credits; Imai Asami returned ten filmography works. These counts are a dated provider snapshot, not requirements.
- Isolated browser harness and real release Tauri/WebView2 exercised character → person → Back → work; work scroll was restored exactly. All-edition expansion displayed 191 distinct identity/alias/role rows. Collection/work/character/person layouts had no document overflow at 390px, with no page errors in that run.
- Real release app also verified local-only work empty states, collection search and persistence of notes. Generated file fixtures verified an unapproved organize preview; no file execution was performed. The safe-artwork check exposed duplicate placeholder accessibility labels; they were fixed, repackaged and verified in the final native app.
- Mutation tests used isolated test state. The final native check used the original collection for read-only metadata browsing and verified the artwork toggle, restoring its original visible-artwork setting. The original user catalog was backed up to `user-catalog-before.sqlite`; original work/binding/source data is checked again at handoff. Runtime helper credentials are not part of application assets or distributables.

Open the existing collection with `Open-VNDB-Demo.ps1`. Executable: `target/release/galroon-desktop.exe`; installer: `target/release/bundle/nsis/Galroon_0.0.1_x64-setup.exe`. Build hashes and final preservation results are in `test-output/figma-implementation/build-report.json`. Native rendering and release packaging were verified; clean-machine installer acceptance and unrelated MVP backlog remain separate work.
