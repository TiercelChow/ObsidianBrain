-- Expired evidence requires explicit review. Historical bodies and citations
-- are retained; this is not a semantic assertion that an old claim is false.
CREATE TABLE IF NOT EXISTS knowledge_source_impacts (
    knowledge_base_id TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    entry_id TEXT NOT NULL REFERENCES knowledge_entries(id) ON DELETE CASCADE,
    source_document_id TEXT NOT NULL REFERENCES source_documents(id) ON DELETE CASCADE,
    previous_version_id TEXT NOT NULL REFERENCES source_versions(id) ON DELETE CASCADE,
    current_version_id TEXT REFERENCES source_versions(id) ON DELETE SET NULL,
    root_entry_id TEXT NOT NULL REFERENCES knowledge_entries(id) ON DELETE CASCADE,
    reason TEXT NOT NULL CHECK(reason IN ('source_changed','source_missing','source_reindexed')),
    detected_at TEXT NOT NULL,
    resolved_at TEXT,
    PRIMARY KEY(entry_id, source_document_id, previous_version_id, root_entry_id)
);
CREATE INDEX IF NOT EXISTS idx_source_impacts_base_open
    ON knowledge_source_impacts(knowledge_base_id, resolved_at, entry_id);
