-- 灵感历史记录表
CREATE TABLE IF NOT EXISTS inspiration_history (
    id          TEXT PRIMARY KEY,
    type        TEXT NOT NULL,   -- "concept_combo" | "reverse_question" | "counterpoint"
    input_refs  TEXT,            -- JSON
    output      TEXT NOT NULL,
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_inspiration_type_created
    ON inspiration_history (type, created_at);
