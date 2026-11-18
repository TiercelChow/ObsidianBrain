CREATE TABLE IF NOT EXISTS wiki_topic_reviews (
    knowledge_base_id TEXT PRIMARY KEY REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    snapshot_hash TEXT NOT NULL,
    report_json TEXT NOT NULL CHECK (json_valid(report_json))
);
