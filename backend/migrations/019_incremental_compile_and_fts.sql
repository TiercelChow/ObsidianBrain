CREATE TABLE IF NOT EXISTS knowledge_compile_checkpoints (
    knowledge_base_id   TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    source_document_id  TEXT NOT NULL REFERENCES source_documents(id) ON DELETE CASCADE,
    source_version_id   TEXT NOT NULL REFERENCES source_versions(id) ON DELETE CASCADE,
    last_change_set_id  TEXT REFERENCES knowledge_change_sets(id) ON DELETE SET NULL,
    compiled_at         TEXT NOT NULL,
    PRIMARY KEY (knowledge_base_id, source_document_id)
);

CREATE INDEX IF NOT EXISTS idx_knowledge_compile_checkpoints_version
    ON knowledge_compile_checkpoints(knowledge_base_id, source_version_id);

DROP TABLE IF EXISTS knowledge_entries_fts;

CREATE VIRTUAL TABLE knowledge_entries_fts USING fts5(
    entry_id UNINDEXED,
    knowledge_base_id UNINDEXED,
    title,
    aliases,
    summary,
    content_md,
    tags,
    cjk_terms,
    tokenize = 'unicode61'
);

CREATE VIRTUAL TABLE IF NOT EXISTS source_spans_fts USING fts5(
    span_id UNINDEXED,
    knowledge_base_id UNINDEXED,
    source_title,
    heading,
    content,
    cjk_terms,
    tokenize = 'unicode61'
);
