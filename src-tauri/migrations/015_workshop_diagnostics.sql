CREATE TABLE IF NOT EXISTS workshop_ignored_diagnostics (
    work_id TEXT NOT NULL,
    category TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (work_id, category)
);

CREATE INDEX IF NOT EXISTS idx_workshop_ignored_diagnostics_work
    ON workshop_ignored_diagnostics(work_id);
