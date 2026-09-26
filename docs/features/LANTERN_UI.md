# Lantern UI (frontend redesign)

2026-09-24. Replaces the Midnight Library look (see [MIDNIGHT_UI.md](MIDNIGHT_UI.md) for the behaviour it kept). The user asked for a new visual design and easier daily use; Core, API and data behaviour are unchanged.

## Design system

All styling lives in `src/theme.css`, which replaces `style.css` and `midnight.css`. It is organised by area (tokens, controls, shell, collection, work, exploration, workbench, dialogs, settings, responsive) and keeps the existing class names, so components did not need restyling one by one.

- Palette: warm ink surfaces (`--bg`, `--surface`…), one vermilion accent (`--accent`, 朱), plus `--ok`/`--warn`/`--danger` for status. Covers are the main source of colour.
- Type: Instrument Serif for page and work titles, Hanken Grotesk for the interface, JetBrains Mono for paths and IDs. CJK text falls back to Noto/Yu Gothic/JhengHei.
- Motion: a short staggered rise for covers and dialogs. Everything is disabled under `prefers-reduced-motion`.

## Usability changes

- Sidebar is grouped into Library (Collection, Lists), Workbench (Organize, Activity) and Storage (Sources, Quarantine).
- Collection: one toolbar row with filters, sort and view. Title display and list actions live in a "More" menu, and selection mode gets its own action bar. Covers without artwork show the title in a stable tint instead of a numbered placeholder.
- Work page: a hero lit by a blurred copy of the cover, **Editions & files** as the primary action, and rarely used actions (edit metadata, correct grouping, refresh) in a "More" menu. Long descriptions collapse. Empty related-tag sections are hidden.
- Activity: compact task cards that show only non-zero summary counts, with relative times. IDs and Core messages sit under "Details" unless the task failed. Empty issues collapse to a single line.
- Organize: one primary action per row (Match work, or Preview move once matched). History, references and grouping move to a row menu.
- Page titles and helper text were rewritten in plain language. Verbose contract-style hints were shortened without dropping safety facts (restore, undo and move behaviour).

## Follow-up polish

- Works without a VNDB link show one notice in place of the empty Characters, Staff and Related sections, and the section links are hidden. Their hero cover uses the same tinted title placeholder as the collection grid (`coverTone` in `src/library.ts`).
- Related personal tags only take a section when there are tags (or an error to retry).
- Side drawers (`.scrim > .drawer`) share keyboard handling from `src/drawerKeys.ts`: focus moves into the drawer, Escape does what its close button does, and focus returns to the opener.
- Disclosures use one chevron instead of the platform triangle. The Settings cover toggle is a switch.
- Task history hides its pager when there is only one page. Timeline entries show translated states and shortened IDs (full value on hover).
- Resource kinds (Organize) and source roles (Sources) are translated. An empty Lists page shows an empty-state panel.

## Language, names and theme

- Interface language: English and Traditional Chinese (`src/locales/zh-TW.ts`). Any `zh*` system locale uses Traditional Chinese, and missing keys fall back to English. In Chinese the UI font prefers Microsoft JhengHei so characters use Taiwanese glyph forms.
- Name display (Settings → Appearance & language): romanized or original-script names for people, characters and companies (VNDB `name` vs `original`). Profiles always show the other spelling as a subtitle. Implemented once in `src/display.tsx` (`useNames`) and shared with the existing title display setting.
- Theme: system, dark or light (`src/themeChoice.ts`). The resolved theme is written to `html[data-theme]` before React renders; the light palette overrides the same tokens in `theme.css`.

## Browser preview for development

`vite.config.ts` has a dev-only bridge. Set `GALROON_DEV_SESSION` (in the environment or a git-ignored `.env.development.local`) to a session JSON printed by `cargo run -p galroon-core --example ui_core -- <state dir> 14800`. The dev page then connects like the desktop shell does. Desktop-only commands such as saved connections report that they are unavailable in the preview.
