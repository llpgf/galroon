-- Migration 003: Characters and persons tables

CREATE TABLE IF NOT EXISTS persons (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    name_original   TEXT,
    vndb_id         TEXT,
    bangumi_id      TEXT,
    roles           TEXT,           -- JSON array
    image_url       TEXT,
    description     TEXT
);

CREATE INDEX IF NOT EXISTS idx_persons_vndb_id ON persons(vndb_id);

CREATE TABLE IF NOT EXISTS characters (
    id              TEXT PRIMARY KEY,
    vndb_id         TEXT,
    name            TEXT NOT NULL,
    name_original   TEXT,
    gender          TEXT NOT NULL DEFAULT 'unknown',
    birthday        TEXT,
    bust            TEXT,
    height          INTEGER,
    description     TEXT,
    image_url       TEXT,
    role            TEXT NOT NULL DEFAULT 'side',
    voice_actor     TEXT,
    traits          TEXT            -- JSON array
);

CREATE INDEX IF NOT EXISTS idx_characters_vndb_id ON characters(vndb_id);

-- Junction: which characters appear in which works
CREATE TABLE IF NOT EXISTS work_characters (
    work_id         TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    character_id    TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    role            TEXT NOT NULL DEFAULT 'side',
    PRIMARY KEY (work_id, character_id)
);

-- Junction: work credits (person-to-work relationships)
CREATE TABLE IF NOT EXISTS work_credits (
    work_id         TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    person_id       TEXT NOT NULL REFERENCES persons(id) ON DELETE CASCADE,
    role            TEXT NOT NULL,
    character_name  TEXT,
    notes           TEXT,
    PRIMARY KEY (work_id, person_id, role)
);
