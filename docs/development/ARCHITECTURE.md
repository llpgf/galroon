# Architecture

Galroon separates a persistent Core from clients. The Windows Tauri shell discovers or starts the local Core and handles native dialogs, device identity and collection recovery. React provides the desktop UI and the read-only browser UI.

| Area | Location | Responsibility |
|---|---|---|
| Core | `crates/core/src/` | SQLite catalog, metadata, matching, durable jobs, filesystem plans, REST/SSE and permissions |
| Desktop | `apps/desktop/src-tauri/` | Local Core lifecycle, Windows integration and NSIS packaging |
| Frontend | `src/` | Collection, discovery, editing, job/history and settings views |
| Build helpers | `scripts/` | Third-party notices, bundle cleanup generation and repository hygiene |

The catalog distinguishes works, editions, resources and files. Multiple editions or resources do not create duplicate work cards, and a shared resource may reference multiple works. Catalog changes and physical filesystem operations have separate review/confirmation paths.

SQLite catalogs are stored on local disk. Jobs persist enough state for interruption and controlled resume. Copy/extraction uses staging; file plans require reviewed approval, and catalog restore is not a filesystem undo. Core enforces permissions independently of the UI; browser access is read-only.

Required bundled resources are declared in `apps/desktop/src-tauri/tauri.conf.json`. User and API documents are sourced from `docs/` but retain stable installed paths under `docs/`. The third-party runtime is separate from application code.

Version sources: `package.json`, workspace crate manifests and Tauri configuration (product); `crates/core/src/db.rs` (catalog schema). Exact API details are in [API_CONTRACT](API_CONTRACT.md).
