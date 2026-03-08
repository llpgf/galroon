-- 008: Dual-track tag system — auto_tags (API-imported) + user_tags (user-created).
-- auto_tags and user_tags are INDEPENDENT namespaces.
-- Same name can exist in both without merging.

-- Auto tags (from VNDB/DLsite/Bangumi — read-only)
CREATE TABLE IF NOT EXISTS auto_tags (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    source      TEXT NOT NULL,    -- 'vndb', 'dlsite', 'bangumi'
    category    TEXT,             -- 'genre', 'theme', 'content', etc.
    UNIQUE(name, source)
);

CREATE TABLE IF NOT EXISTS work_auto_tags (
    work_id     TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    tag_id      TEXT NOT NULL REFERENCES auto_tags(id) ON DELETE CASCADE,
    rating      REAL DEFAULT 0,  -- tag relevance score (from API)
    PRIMARY KEY (work_id, tag_id)
);

-- User tags (user-created — full control)
CREATE TABLE IF NOT EXISTS user_tags (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    color       TEXT DEFAULT '#4f8cff',
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS work_user_tags (
    work_id     TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    tag_id      TEXT NOT NULL REFERENCES user_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (work_id, tag_id)
);

CREATE INDEX IF NOT EXISTS idx_work_auto_tags_work ON work_auto_tags(work_id);
CREATE INDEX IF NOT EXISTS idx_work_user_tags_work ON work_user_tags(work_id);
