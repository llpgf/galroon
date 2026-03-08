-- Migration 004: Enrichment job queue (R7)
-- DB-persisted job queue with state machine for crash-safe enrichment.

CREATE TABLE IF NOT EXISTS enrichment_jobs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    work_id         TEXT NOT NULL,
    job_type        TEXT NOT NULL,          -- 'vndb_match', 'vndb_refresh', 'bangumi_refresh'
    state           TEXT NOT NULL DEFAULT 'queued',  -- queued, claimed, running, success, retry_wait, failed
    attempt_count   INTEGER NOT NULL DEFAULT 0,
    max_attempts    INTEGER NOT NULL DEFAULT 5,
    last_error      TEXT,
    next_run_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    payload         TEXT,                   -- JSON: extra data for the job
    dedup_key       TEXT,                   -- Prevent duplicate jobs

    FOREIGN KEY (work_id) REFERENCES works(id) ON DELETE CASCADE
);

-- Index for job claiming: find next available job
CREATE INDEX IF NOT EXISTS idx_jobs_claimable
    ON enrichment_jobs(state, next_run_at)
    WHERE state IN ('queued', 'retry_wait');

-- Index for dedup
CREATE UNIQUE INDEX IF NOT EXISTS idx_jobs_dedup
    ON enrichment_jobs(dedup_key)
    WHERE dedup_key IS NOT NULL AND state NOT IN ('success', 'failed');

-- Rate limiting tracker per provider (R8)
CREATE TABLE IF NOT EXISTS rate_limits (
    provider        TEXT PRIMARY KEY,
    tokens          INTEGER NOT NULL DEFAULT 10,
    max_tokens      INTEGER NOT NULL DEFAULT 10,
    refill_rate     REAL NOT NULL DEFAULT 1.0,   -- tokens per second
    last_refill_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    next_allowed_at TEXT
);

-- Seed default rate limits
INSERT OR IGNORE INTO rate_limits (provider, tokens, max_tokens, refill_rate)
VALUES
    ('vndb', 10, 10, 0.5),
    ('bangumi', 5, 5, 0.3);
