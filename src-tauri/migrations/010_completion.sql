-- 010: Completion tracking + import queue.

-- Completion tracking per work
CREATE TABLE IF NOT EXISTS completion_tracking (
    work_id       TEXT PRIMARY KEY REFERENCES works(id) ON DELETE CASCADE,
    status        TEXT NOT NULL DEFAULT 'not_started', -- not_started, in_progress, completed, on_hold, dropped
    progress_pct  INTEGER DEFAULT 0,                   -- 0-100
    playtime_min  INTEGER DEFAULT 0,                   -- tracked minutes
    started_at    TEXT,
    completed_at  TEXT,
    notes         TEXT DEFAULT '',
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Import queue — files/folders pending processing
CREATE TABLE IF NOT EXISTS import_queue (
    id            TEXT PRIMARY KEY,
    source_path   TEXT NOT NULL,
    file_name     TEXT NOT NULL,
    file_size     INTEGER DEFAULT 0,
    detected_type TEXT DEFAULT 'unknown',  -- game, ost, save, patch, etc.
    status        TEXT NOT NULL DEFAULT 'pending', -- pending, processing, done, error
    target_work   TEXT REFERENCES works(id),
    error_msg     TEXT,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_import_queue_status ON import_queue(status);

-- Source plugins registry
CREATE TABLE IF NOT EXISTS source_plugins (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,         -- 'vndb', 'dlsite', 'steam', 'bangumi'
    enabled     INTEGER DEFAULT 1,
    priority    INTEGER DEFAULT 0,     -- higher = preferred
    config_json TEXT DEFAULT '{}',
    last_sync   TEXT
);

-- Seed default plugins
INSERT OR IGNORE INTO source_plugins (id, name, priority) VALUES
    ('vndb', 'VNDB', 10),
    ('dlsite', 'DLsite', 8),
    ('bangumi', 'Bangumi', 6),
    ('steam', 'Steam', 4);
