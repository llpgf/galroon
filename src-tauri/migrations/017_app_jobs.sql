CREATE TABLE IF NOT EXISTS app_jobs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    kind            TEXT NOT NULL,
    state           TEXT NOT NULL DEFAULT 'queued',
    title           TEXT NOT NULL,
    progress_pct    REAL NOT NULL DEFAULT 0,
    current_step    TEXT,
    checkpoint_json TEXT,
    payload         TEXT,
    result_json     TEXT,
    last_error      TEXT,
    can_pause       INTEGER NOT NULL DEFAULT 1,
    can_resume      INTEGER NOT NULL DEFAULT 1,
    can_cancel      INTEGER NOT NULL DEFAULT 1,
    dedup_key       TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    started_at      TEXT,
    finished_at     TEXT
);

CREATE INDEX IF NOT EXISTS idx_app_jobs_claimable
    ON app_jobs(state, created_at)
    WHERE state IN ('queued');

CREATE UNIQUE INDEX IF NOT EXISTS idx_app_jobs_dedup
    ON app_jobs(dedup_key)
    WHERE dedup_key IS NOT NULL AND state IN ('queued', 'running', 'paused');

CREATE TABLE IF NOT EXISTS app_runtime_flags (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

INSERT OR IGNORE INTO app_runtime_flags (key, value)
VALUES
    ('enrichment_paused', 'false');
