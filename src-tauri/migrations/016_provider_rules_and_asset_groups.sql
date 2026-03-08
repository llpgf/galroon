CREATE TABLE IF NOT EXISTS provider_field_defaults (
    field       TEXT PRIMARY KEY,
    source      TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE IF NOT EXISTS canonical_asset_groups (
    canonical_key           TEXT NOT NULL,
    asset_type              TEXT NOT NULL,
    relation_role           TEXT NOT NULL DEFAULT 'supplemental',
    parent_asset_type       TEXT,
    asset_count             INTEGER NOT NULL DEFAULT 0,
    variant_count           INTEGER NOT NULL DEFAULT 0,
    representative_work_id  TEXT,
    representative_path     TEXT,
    created_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (canonical_key, asset_type),
    FOREIGN KEY (canonical_key) REFERENCES canonical_works(canonical_key) ON DELETE CASCADE,
    FOREIGN KEY (representative_work_id) REFERENCES works(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_provider_field_defaults_source
    ON provider_field_defaults(source);

CREATE INDEX IF NOT EXISTS idx_canonical_asset_groups_canonical_key
    ON canonical_asset_groups(canonical_key);

CREATE INDEX IF NOT EXISTS idx_canonical_asset_groups_asset_type
    ON canonical_asset_groups(asset_type);
