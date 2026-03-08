-- Migration 001: Works table
-- The core table for game library entries.

CREATE TABLE IF NOT EXISTS works (
    id              TEXT PRIMARY KEY,
    folder_path     TEXT NOT NULL UNIQUE,
    title           TEXT NOT NULL,
    title_original  TEXT,
    title_aliases   TEXT,           -- JSON array
    developer       TEXT,
    publisher       TEXT,
    release_date    TEXT,           -- YYYY-MM-DD
    rating          REAL,
    vote_count      INTEGER,
    description     TEXT,
    cover_path      TEXT,
    tags            TEXT,           -- JSON array
    user_tags       TEXT,           -- JSON array
    library_status  TEXT NOT NULL DEFAULT 'unplayed',
    vndb_id         TEXT,
    bangumi_id      TEXT,
    dlsite_id       TEXT,
    enrichment_state TEXT NOT NULL DEFAULT 'unmatched',
    title_source    TEXT NOT NULL DEFAULT 'filesystem',
    folder_mtime    REAL NOT NULL DEFAULT 0,
    metadata_mtime  REAL NOT NULL DEFAULT 0,
    metadata_hash   TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Index for fast lookups by VNDB ID
CREATE INDEX IF NOT EXISTS idx_works_vndb_id ON works(vndb_id);

-- Index for folder_path scans (already UNIQUE, but explicit for clarity)
CREATE INDEX IF NOT EXISTS idx_works_folder_path ON works(folder_path);

-- Index for enrichment state filtering
CREATE INDEX IF NOT EXISTS idx_works_enrichment_state ON works(enrichment_state);

-- Index for library status filtering
CREATE INDEX IF NOT EXISTS idx_works_library_status ON works(library_status);

-- Index for DLsite ID
CREATE INDEX IF NOT EXISTS idx_works_dlsite_id ON works(dlsite_id);
