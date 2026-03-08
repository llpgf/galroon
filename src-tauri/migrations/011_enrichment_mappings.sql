-- Migration 011: Enrichment title mapping cache
-- Stores normalized local title variants mapped to external metadata IDs.

CREATE TABLE IF NOT EXISTS enrichment_mappings (
    normalized_title TEXT NOT NULL,
    source           TEXT NOT NULL,
    external_id      TEXT NOT NULL,
    resolved_title   TEXT NOT NULL,
    title_original   TEXT,
    developer        TEXT,
    rating           REAL,
    confidence       REAL NOT NULL DEFAULT 0,
    created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (normalized_title, source)
);

CREATE INDEX IF NOT EXISTS idx_enrichment_mappings_source_id
    ON enrichment_mappings(source, external_id);
