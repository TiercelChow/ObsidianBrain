CREATE TABLE IF NOT EXISTS wiki_topic_book_builds (
    knowledge_base_id TEXT PRIMARY KEY REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    attempt_id        TEXT NOT NULL,
    status            TEXT NOT NULL CHECK(status IN ('running', 'ready', 'partial', 'failed')),
    stop_requested    INTEGER NOT NULL DEFAULT 0 CHECK(stop_requested IN (0, 1)),
    snapshot_hash     TEXT,
    state_json        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS wiki_topic_book_plans (
    knowledge_base_id TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    input_hash        TEXT NOT NULL,
    plan_json         TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    PRIMARY KEY(knowledge_base_id, input_hash)
);
