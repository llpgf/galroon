-- Migration 005: FTS5 full-text search (R9)
-- Trigram tokenizer for Japanese/CJK search support.

CREATE VIRTUAL TABLE IF NOT EXISTS works_fts USING fts5(
    title,
    title_original,
    developer,
    tags,
    content='works',
    content_rowid='rowid',
    tokenize='trigram'
);

-- Triggers to keep FTS in sync with the works table.

CREATE TRIGGER IF NOT EXISTS works_fts_insert AFTER INSERT ON works BEGIN
    INSERT INTO works_fts(rowid, title, title_original, developer, tags)
    VALUES (NEW.rowid, NEW.title, NEW.title_original, NEW.developer, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS works_fts_update AFTER UPDATE ON works BEGIN
    INSERT INTO works_fts(works_fts, rowid, title, title_original, developer, tags)
    VALUES ('delete', OLD.rowid, OLD.title, OLD.title_original, OLD.developer, OLD.tags);
    INSERT INTO works_fts(rowid, title, title_original, developer, tags)
    VALUES (NEW.rowid, NEW.title, NEW.title_original, NEW.developer, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS works_fts_delete AFTER DELETE ON works BEGIN
    INSERT INTO works_fts(works_fts, rowid, title, title_original, developer, tags)
    VALUES ('delete', OLD.rowid, OLD.title, OLD.title_original, OLD.developer, OLD.tags);
END;
