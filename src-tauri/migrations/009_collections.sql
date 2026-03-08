-- 009: Collections + Playlists + Wishlist.

-- Collections (manual groupings — smart or manual)
CREATE TABLE IF NOT EXISTS collections (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT DEFAULT '',
    is_smart    INTEGER NOT NULL DEFAULT 0,
    smart_rule  TEXT,              -- JSON rule for smart collections
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Collection items (many-to-many: collection ↔ work)
CREATE TABLE IF NOT EXISTS collection_items (
    collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    work_id       TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL DEFAULT 0,
    added_at      TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (collection_id, work_id)
);

CREATE INDEX IF NOT EXISTS idx_collection_items_coll ON collection_items(collection_id);

-- Wishlist entries (virtual — no actual files, just metadata)
CREATE TABLE IF NOT EXISTS wishlist (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    developer   TEXT,
    vndb_id     TEXT,
    dlsite_id   TEXT,
    notes       TEXT DEFAULT '',
    priority    INTEGER NOT NULL DEFAULT 0,  -- 0=none, 1=low, 2=medium, 3=high
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Activity log (timeline of user actions)
CREATE TABLE IF NOT EXISTS activity_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    action      TEXT NOT NULL,     -- 'scan', 'match', 'favorite', 'tag', 'import', etc.
    target_id   TEXT,
    target_type TEXT,              -- 'work', 'collection', 'tag'
    detail      TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_activity_log_time ON activity_log(created_at DESC);
