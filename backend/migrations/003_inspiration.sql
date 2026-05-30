CREATE TABLE IF NOT EXISTS inspiration_history (
    id          TEXT PRIMARY KEY,
    type        TEXT NOT NULL CHECK(type IN ('concept_combo','reverse_question','counterpoint')),
    input_refs  TEXT,
    output      TEXT NOT NULL,
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_inspiration_type ON inspiration_history(type);
CREATE INDEX IF NOT EXISTS idx_inspiration_created ON inspiration_history(created_at DESC);