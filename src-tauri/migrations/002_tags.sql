-- Migration 002: Tags tables
-- Tag registry + work-tag junction table.

CREATE TABLE IF NOT EXISTS tags (
    id          TEXT PRIMARY KEY,
    key         TEXT NOT NULL UNIQUE,    -- Normalized key (lowercase, no spaces)
    label       TEXT NOT NULL,           -- Display label
    category    TEXT NOT NULL DEFAULT 'genre',
    usage_count INTEGER NOT NULL DEFAULT 0,
    vndb_tag_id TEXT
);

CREATE TABLE IF NOT EXISTS work_tags (
    work_id     TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    tag_id      TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    score       REAL,
    spoiler_level INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (work_id, tag_id)
);

CREATE INDEX IF NOT EXISTS idx_work_tags_tag_id ON work_tags(tag_id);
