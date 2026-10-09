# Glass UI (frontend redesign)

2026-10-09. Replaces the Lantern look (see [LANTERN_UI.md](LANTERN_UI.md) for the behaviour it kept). The user asked for a livelier, Apple-like interface with a showcase home page and a light/dark switch. Core, API and data behaviour are unchanged.

## Design system

Styling still lives in `src/theme.css` and keeps the existing class names. Only the tokens and the component rules changed.

- Palette: black (dark) or grouped grey (light) backgrounds, white or `#1c1c1e` cards, one system-blue accent (`--accent`), and system status colours (`--ok`, `--warn`, `--danger`, `--pink`, `--purple`). Light mode uses darker status colours for contrast on white. Covers remain the main source of colour.
- Type: the platform UI font (Segoe UI Variable on Windows, SF Pro elsewhere) with Microsoft JhengHei / PingFang for Traditional Chinese. Headings are bold system type instead of a serif. The bundled Hanken Grotesk, Instrument Serif and JetBrains Mono imports were removed.
- Shape: hairline borders, 10–30 px radii, pill buttons, tinted secondary buttons (`--fill`), and grey segmented controls (`.tabs`) whose selected segment is a raised pill.
- Chrome: the sidebar, topbar, collection toolbar, menus, drawers and dialogs are translucent (`--glass`, `--blur`). The sidebar floats as a rounded panel; on phones it becomes a floating tab bar.
- Motion: one spring curve (`--ease`), staggered rise on page load, covers that lift and scale on hover, a slowly drifting work-hero backdrop and a floating cover. All of it is disabled under `prefers-reduced-motion`.

## Changes by page

- **Home** (`src/HomePanel.tsx`, new default view): a featured carousel (works in progress, then recent additions) over a blurred copy of the cover; a "what to play next" picker over backlog or on-hold works, filtered by "ready to play" (availability) and the most common tags, with a random pick; an overview of status counts; shelves for recent additions and favourites. It only reads `/collection` and changes nothing.
- **Collection**: a status segmented control with counts sits at the start of the floating toolbar. Covers show a status badge, and in poster view a translucent card with studio, year and tags appears on hover.
- **Work page**: larger title, floating cover, pill facts, and the section links become a sticky segmented control. Character cards show tall art (contained, so full-body images are not cropped), the character's role and a "CV" block with voice credits.
- **Profiles**: rounded portrait, facts in one summary strip, credit cards with lift on hover, sticky full-height character art.
- **Settings**: grouped rows and iOS-style switches.
- **Sidebar**: a light / dark / system segmented control under Settings writes the same preference as Settings → Appearance (`src/themeChoice.ts`).

## Not covered

- There is no people/companies index page: Core has no endpoint that lists every person, character or company in the collection. Profiles are still reached from works.
- Home has no "recently organised" or play-time data; Core does not record either.
- `@fontsource` packages are still listed in `package.json` and the third-party inventory although nothing imports them; removing them should go with a notice regeneration.
