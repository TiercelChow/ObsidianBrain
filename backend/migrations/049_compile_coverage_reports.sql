-- Metadata only. Original bodies and model artifacts keep their existing stores.
CREATE TABLE IF NOT EXISTS knowledge_compile_reports (
    id TEXT PRIMARY KEY,
    knowledge_base_id TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    fingerprint TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('running','waiting_review','no_material','failed','cancelled')),
    selected_sources INTEGER NOT NULL,
    current_sources INTEGER NOT NULL,
    selected_spans INTEGER NOT NULL,
    planned INTEGER NOT NULL DEFAULT 0,
    change_set_id TEXT REFERENCES knowledge_change_sets(id) ON DELETE SET NULL,
    error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_compile_report_active ON knowledge_compile_reports(knowledge_base_id) WHERE status='running';
CREATE INDEX IF NOT EXISTS idx_compile_report_history ON knowledge_compile_reports(knowledge_base_id,created_at DESC);
CREATE TABLE IF NOT EXISTS knowledge_compile_report_fragments (
    report_id TEXT NOT NULL REFERENCES knowledge_compile_reports(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    batch INTEGER NOT NULL,
    source_document_id TEXT NOT NULL REFERENCES source_documents(id) ON DELETE CASCADE,
    source_version_id TEXT NOT NULL REFERENCES source_versions(id) ON DELETE CASCADE,
    source_span_id TEXT NOT NULL REFERENCES source_spans(id) ON DELETE CASCADE,
    source_path TEXT NOT NULL,
    line_start INTEGER,
    line_end INTEGER,
    locator_json TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('unprocessed','analyzing','analyzed','no_material','failed')),
    run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
    candidate_slugs_json TEXT NOT NULL DEFAULT '[]',
    reason TEXT,
    PRIMARY KEY(report_id,ordinal)
);
CREATE INDEX IF NOT EXISTS idx_compile_report_batch ON knowledge_compile_report_fragments(report_id,batch,status);
CREATE TABLE IF NOT EXISTS knowledge_compile_report_topics (
    report_id TEXT NOT NULL REFERENCES knowledge_compile_reports(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    outcome_json TEXT NOT NULL,
    PRIMARY KEY(report_id,ordinal)
);
