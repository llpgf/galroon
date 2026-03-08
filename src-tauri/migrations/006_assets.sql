-- 006: Assets table — classified files within game folders.

CREATE TABLE IF NOT EXISTS assets (
    id          TEXT PRIMARY KEY,
    work_id     TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    path        TEXT NOT NULL,
    filename    TEXT NOT NULL,
    asset_type  TEXT NOT NULL DEFAULT 'unknown',  -- game/crack/ost/save/guide/extras/dlc/unknown
    size_bytes  INTEGER NOT NULL DEFAULT 0,
    is_dir      INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(work_id, path)
);

CREATE INDEX IF NOT EXISTS idx_assets_work_id ON assets(work_id);
CREATE INDEX IF NOT EXISTS idx_assets_type ON assets(asset_type);
