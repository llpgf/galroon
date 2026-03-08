CREATE TABLE IF NOT EXISTS canonical_works (
    canonical_key      TEXT PRIMARY KEY,
    preferred_work_id  TEXT NOT NULL UNIQUE,
    title              TEXT NOT NULL,
    cover_path         TEXT,
    developer          TEXT,
    rating             REAL,
    library_status     TEXT NOT NULL,
    enrichment_state   TEXT NOT NULL,
    tags               TEXT,
    release_date       TEXT,
    vndb_id            TEXT,
    bangumi_id         TEXT,
    dlsite_id          TEXT,
    description        TEXT,
    variant_count      INTEGER NOT NULL DEFAULT 1,
    asset_count        INTEGER NOT NULL DEFAULT 0,
    asset_types        TEXT,
    primary_asset_type TEXT,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS work_variants (
    work_id            TEXT PRIMARY KEY,
    canonical_key      TEXT NOT NULL,
    is_representative  INTEGER NOT NULL DEFAULT 0,
    created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    FOREIGN KEY (work_id) REFERENCES works(id) ON DELETE CASCADE,
    FOREIGN KEY (canonical_key) REFERENCES canonical_works(canonical_key) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS canonical_variant_overrides (
    work_id               TEXT PRIMARY KEY,
    manual_group_key      TEXT NOT NULL,
    make_representative   INTEGER NOT NULL DEFAULT 0,
    created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    FOREIGN KEY (work_id) REFERENCES works(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_canonical_works_preferred_work_id
    ON canonical_works(preferred_work_id);

CREATE INDEX IF NOT EXISTS idx_canonical_works_developer
    ON canonical_works(developer);

CREATE INDEX IF NOT EXISTS idx_work_variants_canonical_key
    ON work_variants(canonical_key);

CREATE INDEX IF NOT EXISTS idx_canonical_variant_overrides_group_key
    ON canonical_variant_overrides(manual_group_key);
