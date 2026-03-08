-- 007: Multi-language text storage for i18n field variants.

CREATE TABLE IF NOT EXISTS work_texts (
    work_id     TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    field_name  TEXT NOT NULL,    -- 'title', 'description', 'bio', etc.
    lang        TEXT NOT NULL,    -- 'ja', 'en', 'zh-Hans', 'zh-Hant'
    source      TEXT NOT NULL,    -- 'vndb', 'bangumi', 'dlsite', 'ai_translation', 'user'
    content     TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (work_id, field_name, lang, source)
);

CREATE INDEX IF NOT EXISTS idx_work_texts_work ON work_texts(work_id);
CREATE INDEX IF NOT EXISTS idx_work_texts_lang ON work_texts(lang);
