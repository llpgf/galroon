# Galroon v0.5.0 — Design Blueprint

> **Galroon** is a cross-platform galgame (visual novel) library manager built with Tauri 2 (Rust backend + React frontend). It scans local folders, auto-matches titles to VNDB/DLsite/Bangumi, displays a rich browsable library, and provides metadata enrichment, collections, tagging, and analytics.

---

## 1. Architecture Overview

```
┌──────────────────────────────────────────────────┐
│                  Tauri 2 Shell                    │
│  ┌────────────┐           ┌────────────────────┐ │
│  │  Frontend   │  IPC      │    Rust Backend     │ │
│  │  React 19   │◄─invoke──►│ galroon_lib crate   │ │
│  │  Vite 6     │           │                    │ │
│  │  TS 5.7     │           │  ┌──────────────┐  │ │
│  └────────────┘           │  │  SQLite WAL   │  │ │
│                           │  │  + DbWriter   │  │ │
│                           │  └──────────────┘  │ │
│                           └────────────────────┘ │
└──────────────────────────────────────────────────┘
         │                         │
         │                    External APIs
         │                 ┌───────┼───────┐
         │                 │       │       │
         ▼                VNDB   DLsite  Bangumi
    Local Filesystem       API    API     API
    (game folders)
```

**Key design decisions:**
- **Single-writer / multi-reader SQLite** — All writes go through a `DbWriter` actor (tokio mpsc channel), reads use a 4-connection pool. WAL mode + 5s busy_timeout eliminates `SQLITE_BUSY`.
- **Hot-reloadable config** — `SharedConfig` wraps `AppConfig` in `Arc<RwLock>` with `read()` / `update(closure)` API.
- **Workspace isolation** — Everything (DB, logs, trash, thumbnails, config) lives inside a single workspace folder. Backup = copy folder. Restore = point app at folder.
- **Lazy-loaded frontend** — Every page is `React.lazy()` for fast initial load.

---

## 2. Tech Stack

### Backend (Rust)
| Crate | Version | Purpose |
|-------|---------|---------|
| tauri | 2 | Desktop app framework, IPC |
| sqlx | 0.8 | Async SQLite (unbundled + WAL) |
| tokio | 1 (full) | Async runtime |
| reqwest | 0.12 | HTTP client (VNDB/Bangumi/DLsite) |
| serde / serde_json | 1 | Serialization |
| uuid | 1 (v4, v7) | ID generation |
| chrono | 0.4 | Date/time |
| regex | 1 | Pattern matching (DLsite RJ codes) |
| thiserror | 2 | Error types |
| tracing + tracing-subscriber | 0.1/0.3 | Structured logging (JSON + env-filter) |
| notify + debouncer | 7/0.4 | Filesystem watcher |
| image | 0.25 | Thumbnail generation |
| trash | 5 | OS-native trash |
| governor | 0.8 | Rate limiter (API) |
| toml | 0.8 | Config file parsing |

### Frontend (TypeScript)
| Package | Version | Purpose |
|---------|---------|---------|
| react | 19 | UI framework |
| react-dom | 19 | DOM rendering |
| react-router-dom | 7 | Client-side routing |
| @tauri-apps/api | 2 | Tauri IPC bridge (`invoke`) |
| zustand | 5 | State management |
| chart.js + react-chartjs-2 | 4.5/5.3 | Year-in-Review charts |
| @tanstack/react-virtual | 3 | Virtualized scrolling |
| vite | 6 | Build tool / HMR dev server |
| typescript | 5.7 | Type checking |

---

## 3. Backend Module Map (`src-tauri/src/`)

```
lib.rs                    ← Crate root, declares modules
main.rs                   ← Startup: config → DB → watcher → Tauri builder → 70+ commands

├── config.rs             ← LauncherConfig (app-level) + AppConfig (workspace-level) + SharedConfig (RwLock wrapper)
├── api/                  ← 16 Tauri command files
│   ├── mod.rs            ← Module registry
│   ├── works.rs          ← list_works, get_work, update_work_field (3)
│   ├── scanner.rs        ← trigger_scan, get_scan_status (2)
│   ├── settings.rs       ← get/update settings, workspace ops, trash mgmt (10)
│   ├── dashboard.rs      ← get_dashboard_stats, toggle_sfw (2)
│   ├── search.rs         ← search_works (1)
│   ├── thumbnails.rs     ← get_thumbnail (1)
│   ├── enrichment.rs     ← get_unmatched, search_candidates, confirm/reject (4)
│   ├── brands.rs         ← list_brands, brand_detail, list_creators, creator_detail (4)
│   ├── characters.rs     ← (stub, characters API not fully wired)
│   ├── duplicates.rs     ← find_duplicates (1)
│   ├── import.rs         ← (import pipeline, partially wired)
│   ├── collections.rs    ← CRUD + smart eval + reorder + wishlist + random + export + multi_source_match (16)
│   ├── workshop.rs       ← bulk_update, merge, reset_enrichment, year_in_review (4)
│   ├── extended.rs       ← multi-root, completion, gap analysis, translate, import queue, plugins, i18n, bulk_load (17)
│   └── tags.rs           ← user tag CRUD, tag/untag, search, bulk (9)
│
├── db/                   ← Database infrastructure
│   ├── mod.rs            ← Database struct + DbWriter actor + migrations runner
│   ├── models.rs         ← SQLx row models (Work, Character, etc.)
│   └── queries.rs        ← Reusable SQL query fragments
│
├── domain/               ← Business logic / types (no DB, no framework deps)
│   ├── mod.rs            ← Module exports
│   ├── error.rs          ← AppError enum (Database/IO/Config/Json/VndbApi/BangumiApi/NotFound/Validation/Network/...)
│   ├── work.rs           ← Work domain model
│   ├── character.rs      ← Character model
│   ├── person.rs         ← Person (voice actor, artist) model
│   ├── asset.rs          ← Asset (OST, save, patch) model
│   ├── tag.rs            ← Tag model (auto + user)
│   ├── ids.rs            ← ID type wrappers (WorkId, CharacterId, etc.)
│   └── metadata.rs       ← Metadata extraction logic
│
├── enrichment/           ← External API integration
│   ├── mod.rs            ← Submodule exports
│   ├── vndb.rs           ← VndbClient — search_by_title, get_by_id (Kana API)
│   ├── bangumi.rs        ← BangumiClient — search_by_title, get_by_id (bgm.tv API)
│   ├── dlsite.rs         ← DLsite API client
│   ├── matcher.rs        ← Fuzzy title matching (LCS algorithm, score thresholds)
│   ├── resolver.rs       ← Multi-source merge (priority: user > VNDB > Bangumi > filesystem)
│   ├── queue.rs          ← Enrichment job queue
│   └── rate_limit.rs     ← Rate limiter (governor-based, per-source limits)
│
├── scanner/              ← Filesystem scanning
│   ├── mod.rs            ← Submodule exports
│   ├── discover.rs       ← Walk directory trees, find game folders
│   ├── classifier.rs     ← 8-type file classifier (game/patch/ost/save/tool/manual/extra/unknown)
│   ├── ingest.rs         ← Create/update work records from discovered folders
│   ├── thumbs.rs         ← Thumbnail generation (image crate)
│   └── watcher.rs        ← Live filesystem watcher (notify crate, debounced)
│
├── fs/                   ← Filesystem operations
│   ├── mod.rs            ← Exports
│   ├── asset.rs          ← Asset file management
│   ├── metadata_io.rs    ← Read/write metadata files
│   ├── transaction.rs    ← Atomic file moves (rename + fallback copy)
│   └── trash.rs          ← OS-native trash + restore
│
├── observability/        ← Logging / tracing
│   └── mod.rs            ← init_logging() (JSON structured, env-filter)
│
├── platform/             ← Platform-specific helpers
│   └── mod.rs            ← OS detection, path normalization
│
└── security/             ← Security guards
    ├── mod.rs             ← Exports
    └── path_guard.rs      ← Prevent path traversal attacks
```

---

## 4. Database Schema (10 Migrations)

| Migration | Tables | Purpose |
|-----------|--------|---------|
| 001_works | `works` | Core: id, folder_path, title, developer, vndb_id, bangumi_id, dlsite_id, enrichment_state, library_status, ratings |
| 002_tags | `tags`, `work_tags` | Tag junction table (legacy, superseded by 008) |
| 003_characters | `characters`, `character_works` | Character data + voice actor mapping |
| 004_jobs | `enrichment_jobs` | Background enrichment queue |
| 005_fts | `works_fts` | Full-text search virtual table |
| 006_assets | `assets` | File assets linked to works (type: ost/save/patch/etc.) |
| 007_work_texts | `work_texts` | Multi-lang text storage (field/lang/value per work) |
| 008_dual_tags | `auto_tags`, `user_tags`, `work_auto_tags`, `work_user_tags` | Dual tag system (API-sourced vs user-created) |
| 009_collections | `collections`, `collection_items`, `wishlist`, `activity_log` | Collections (manual + smart), wishlist, activity tracking |
| 010_completion | `completion_tracking`, `import_queue`, `source_plugins` | Play tracking, import pipeline, enrichment source registry |

### Key `works` Columns

```sql
id              TEXT PRIMARY KEY    -- UUID v4
folder_path     TEXT NOT NULL UNIQUE -- Filesystem path
title           TEXT NOT NULL
developer       TEXT
vndb_id         TEXT                -- e.g., "v12345"
bangumi_id      TEXT                -- e.g., "12345"
dlsite_id       TEXT                -- e.g., "RJ123456"
enrichment_state TEXT               -- 'unmatched' | 'pending' | 'matched'
library_status  TEXT                -- 'unplayed' | 'in_progress' | 'completed' | 'shelved'
cover_path      TEXT
rating          REAL
tags            TEXT                -- JSON array
```

---

## 5. Frontend Route Map (`src/router.tsx`)

| Route | Page Component | Description |
|-------|---------------|-------------|
| `/` | Dashboard | Stats overview: total works, matched, completed, health card |
| `/library` | GalleryPage | Main library grid/list with filters, search, sort, view modes |
| `/work/:id` | WorkDetail | Detail page: metadata, cover, completion tracking, tags, characters, translate |
| `/characters` | Characters | Character browse page |
| `/creators` | Creators | Developer/brand browse page |
| `/brand/:name` | BrandDetail | Single brand detail + works by brand |
| `/person/:id` | PersonDetail | Voice actor / staff detail |
| `/collections` | Collections | Manual + smart collections, wishlist, random pick |
| `/enrichment` | EnrichmentReview | Review unmatched works, search candidates, confirm/reject |
| `/settings` | SettingsPage | Workspace, theme, locale, source plugins, API keys |
| `/year-in-review` | YearInReview | Annual stats: monthly chart, top brands, hours played |
| `/workshop` | Workshop | Health check (gap analysis), import queue, batch auto-matcher |

### Frontend Component Architecture

```
src/
├── main.tsx              ← React root + AppRouter
├── App.tsx               ← Layout shell: Sidebar + main content + theme init
├── App.css               ← Layout CSS
├── router.tsx            ← Route definitions (all lazy-loaded)
├── styles/
│   └── index.css         ← Design system: CSS custom properties, dark/light theme tokens
├── components/
│   ├── Sidebar.tsx/css   ← Navigation sidebar (grouped: Browse + Tools)
│   ├── GalleryCard.tsx/css ← Game card component (cover, title, dev, rating badge)
│   ├── GalleryGrid.tsx/css ← Grid/List dual-mode rendering
│   ├── GalleryFilters.tsx/css ← Filter bar (search, sort, status, enrichment)
│   └── Toast.tsx/css     ← Toast notification system (context provider)
├── hooks/                ← Custom React hooks
├── stores/               ← Zustand state stores
└── pages/                ← 13 page directories (each with .tsx + .css)
```

---

## 6. Data Flow Patterns

### Scan → Enrich → Display

```
1. User triggers scan (or watcher detects changes)
   ├── scanner::discover → walks library_roots, finds game folders
   ├── scanner::classifier → classifies files (game/ost/save/...)
   ├── scanner::ingest → creates/updates `works` table records
   └── scanner::thumbs → generates cover thumbnails

2. Enrichment pipeline (async, rate-limited)
   ├── enrichment::queue → picks unmatched works
   ├── enrichment::vndb → VndbClient.search_by_title()
   ├── enrichment::bangumi → BangumiClient.search_by_title()
   ├── enrichment::matcher → fuzzy title match (LCS, score ≥85 = auto, 75-84 = pending)
   ├── enrichment::resolver → merge data (priority: user > VNDB > Bangumi > FS)
   └── updates works.vndb_id, bangumi_id, dlsite_id, enrichment_state

3. Frontend display
   ├── invoke('list_works') → paginated gallery
   ├── invoke('get_work') → detail page
   └── invoke('get_dashboard_stats') → dashboard overview
```

### Smart Collection Evaluation

```
JSON Rule → evaluate_smart_collection
  ├── Parse: { operator: "and"|"or", conditions: [{field, op, value}] }
  ├── Whitelist: 10 allowed fields (developer, rating, library_status, ...)
  ├── Operators: eq, neq, gt, gte, lt, lte, contains, starts, is_null, not_null
  ├── Build SQL WHERE clause (parameterized, sanitized)
  └── Execute: SELECT id, title, ... FROM works WHERE <generated clause>
```

### Multi-Source Match Pipeline

```
multi_source_match(work_id)
  ├── Extract title + folder_path from DB
  ├── Regex scan for DLsite RJ code (RJ\d{6,8}) in title
  ├── Regex scan for DLsite RJ code in folder_path
  ├── If found → UPDATE works SET dlsite_id, enrichment_state='matched'
  └── VNDB/Bangumi → requires enrichment client instances (deferred to full pipeline)
```

---

## 7. Configuration Architecture

### Two-Layer Config

| Layer | File | Location | Purpose |
|-------|------|----------|---------|
| **Launcher** | `launcher.toml` | OS app data dir (`~/.config/galroon/`) | Tracks workspace paths, setup_complete flag |
| **Workspace** | `workspace.toml` | Inside workspace folder | library_roots, scanner settings, locale, sfw_mode |

### SharedConfig API

```rust
SharedConfig {
    read()   → RwLockReadGuard<AppConfig>      // concurrent reads
    update(|cfg| { cfg.field = value; })        // exclusive write + auto-save
    snapshot() → AppConfig                       // clone for background tasks
}
```

---

## 8. Error System

```rust
pub enum AppError {
    Database(sqlx::Error),     // DB errors
    Io(std::io::Error),        // Filesystem
    Config(String),            // Config issues
    TomlParse(toml::de::Error),// Config parsing
    Json(serde_json::Error),   // JSON parsing
    ScanAlreadyRunning,        // Concurrent scan guard
    Scanner(String),           // Scan errors
    VndbApi(String),           // VNDB API
    BangumiApi(String),        // Bangumi API
    MatchingFailed(String),    // Match errors
    RateLimited { retry_after_secs: u64 },
    WorkNotFound(String),
    InvalidWorkId(String),
    Internal(String),          // Generic internal
    NotFound(String),          // Resource not found
    Validation(String),        // Input validation
    Network(String),           // HTTP/network
}
```

All variants are serializable via `serde::Serialize` → Tauri IPC returns them as error strings.

---

## 9. Design System (CSS Custom Properties)

- **Dark-first** design with `[data-theme="light"]` override block
- Font: Inter + Noto Sans JP (sans), JetBrains Mono (mono)
- Colors: `--bg-primary: #0a0a0c`, `--accent-primary: #4a9eff`, `--accent-secondary: #7c5cff`
- Spacing scale: xs(0.25rem) → 2xl(3rem)
- Radius scale: sm(4px) → xl(16px)
- All pages use CSS modules (co-located `.css` files)

---

## 10. Security

- **Path guard** (`security/path_guard.rs`) — prevents directory traversal attacks on all file operations
- **Field whitelist** — bulk edit only allows `library_status`, `developer`, `publisher`
- **Smart collection field whitelist** — 10 allowed fields, prevents SQL injection
- **SQL sanitization** — string escaping via `replace('\'', "''")`

