CREATE TABLE IF NOT EXISTS wiki_topic_build_log (
    attempt_id        TEXT PRIMARY KEY,
    knowledge_base_id TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    status            TEXT NOT NULL,
    source_count      INTEGER NOT NULL,
    linked_sources    INTEGER NOT NULL,
    written_pages     INTEGER NOT NULL,
    planned_pages     INTEGER NOT NULL,
    recorded_at       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS wiki_topic_build_log_by_book ON wiki_topic_build_log(knowledge_base_id, recorded_at DESC);

INSERT INTO wiki_topic_build_log(attempt_id, knowledge_base_id, status, source_count,
                                 linked_sources, written_pages, planned_pages, recorded_at)
SELECT attempt_id, knowledge_base_id, status, json_array_length(state_json, '$.sources'),
       (SELECT COUNT(*) FROM json_each(state_json, '$.sources') WHERE json_extract(value, '$.status') = 'linked'),
       (SELECT COUNT(*) FROM json_each(state_json, '$.pages') WHERE json_extract(value, '$.revision_id') IS NOT NULL),
       json_array_length(state_json, '$.pages'), updated_at
FROM wiki_topic_book_builds
WHERE status <> 'running' AND json_type(state_json, '$.sources') = 'array'
                          AND json_type(state_json, '$.pages') = 'array'
ON CONFLICT(attempt_id) DO NOTHING;
