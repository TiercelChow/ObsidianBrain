ALTER TABLE knowledge_bases ADD COLUMN compile_mode TEXT NOT NULL DEFAULT 'chapter'
    CHECK (compile_mode IN ('chapter', 'smart'));
ALTER TABLE knowledge_bases ADD COLUMN compile_state TEXT NOT NULL DEFAULT 'not_started'
    CHECK (compile_state IN ('not_started', 'outdated', 'compiling', 'ready', 'failed'));
ALTER TABLE knowledge_bases ADD COLUMN compile_error TEXT;
ALTER TABLE knowledge_bases ADD COLUMN last_compiled_at TEXT;

ALTER TABLE knowledge_message_citations ADD COLUMN entry_revision INTEGER;
ALTER TABLE knowledge_message_citations ADD COLUMN knowledge_base_id_snapshot TEXT;
ALTER TABLE knowledge_message_citations ADD COLUMN entry_type_snapshot TEXT;
ALTER TABLE knowledge_message_citations ADD COLUMN slug_snapshot TEXT;
ALTER TABLE knowledge_message_citations ADD COLUMN title_snapshot TEXT;
ALTER TABLE knowledge_message_citations ADD COLUMN summary_snapshot TEXT;
ALTER TABLE knowledge_message_citations ADD COLUMN status_snapshot TEXT;
ALTER TABLE knowledge_message_citations ADD COLUMN confidence_snapshot REAL;
ALTER TABLE knowledge_message_citations ADD COLUMN source_path_snapshot TEXT;
ALTER TABLE knowledge_message_citations ADD COLUMN updated_at_snapshot TEXT;

UPDATE knowledge_message_citations
SET entry_revision = (
        SELECT revision FROM knowledge_entries WHERE id = knowledge_message_citations.entry_id
    ),
    knowledge_base_id_snapshot = (
        SELECT knowledge_base_id FROM knowledge_entries WHERE id = knowledge_message_citations.entry_id
    ),
    entry_type_snapshot = (
        SELECT entry_type FROM knowledge_entries WHERE id = knowledge_message_citations.entry_id
    ),
    slug_snapshot = (
        SELECT slug FROM knowledge_entries WHERE id = knowledge_message_citations.entry_id
    ),
    title_snapshot = (
        SELECT title FROM knowledge_entries WHERE id = knowledge_message_citations.entry_id
    ),
    summary_snapshot = (
        SELECT summary FROM knowledge_entries WHERE id = knowledge_message_citations.entry_id
    ),
    status_snapshot = (
        SELECT status FROM knowledge_entries WHERE id = knowledge_message_citations.entry_id
    ),
    confidence_snapshot = (
        SELECT confidence FROM knowledge_entries WHERE id = knowledge_message_citations.entry_id
    ),
    source_path_snapshot = (
        SELECT sd.relative_path
        FROM knowledge_entries ke
        LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
        WHERE ke.id = knowledge_message_citations.entry_id
    ),
    updated_at_snapshot = (
        SELECT updated_at FROM knowledge_entries WHERE id = knowledge_message_citations.entry_id
    );

CREATE INDEX IF NOT EXISTS idx_message_citations_entry_revision
    ON knowledge_message_citations(entry_id, entry_revision);
