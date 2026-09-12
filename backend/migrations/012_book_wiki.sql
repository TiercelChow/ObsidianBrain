CREATE TABLE IF NOT EXISTS reader_books (
    id              TEXT PRIMARY KEY,
    path            TEXT NOT NULL UNIQUE,
    kind            TEXT NOT NULL CHECK (kind IN ('folder', 'pdf')),
    name            TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    category        TEXT NOT NULL DEFAULT '',
    added_at        INTEGER NOT NULL,
    progress_json   TEXT,
    shelf_state     TEXT NOT NULL DEFAULT 'active'
                        CHECK (shelf_state IN ('active', 'removed')),
    removed_at      TEXT,
    revision        INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_reader_books_shelf
    ON reader_books(shelf_state, added_at DESC);

CREATE TABLE IF NOT EXISTS knowledge_bases (
    id                TEXT PRIMARY KEY,
    book_id           TEXT NOT NULL UNIQUE REFERENCES reader_books(id),
    lifecycle         TEXT NOT NULL DEFAULT 'active'
                          CHECK (lifecycle IN ('uninitialized', 'active', 'paused', 'archived')),
    sync_state        TEXT NOT NULL DEFAULT 'outdated'
                          CHECK (sync_state IN ('clean', 'outdated', 'scanning', 'extracting', 'ingesting', 'failed')),
    health_state      TEXT NOT NULL DEFAULT 'healthy'
                          CHECK (health_state IN ('healthy', 'warning', 'needs_review')),
    last_error        TEXT,
    last_synced_at    TEXT,
    last_scanned_at   TEXT,
    revision          INTEGER NOT NULL DEFAULT 1,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_knowledge_bases_status
    ON knowledge_bases(lifecycle, sync_state, health_state);

CREATE TABLE IF NOT EXISTS source_documents (
    id                  TEXT PRIMARY KEY,
    knowledge_base_id   TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    source_type         TEXT NOT NULL CHECK (source_type IN ('markdown', 'pdf')),
    original_path       TEXT NOT NULL,
    relative_path       TEXT NOT NULL,
    title               TEXT NOT NULL,
    mime_type           TEXT NOT NULL,
    ordinal             INTEGER NOT NULL DEFAULT 0,
    current_version_id  TEXT,
    sync_status         TEXT NOT NULL DEFAULT 'current'
                            CHECK (sync_status IN ('current', 'changed', 'missing', 'failed')),
    extraction_status   TEXT NOT NULL DEFAULT 'ready'
                            CHECK (extraction_status IN ('ready', 'pending_harness', 'failed')),
    metadata_json       TEXT NOT NULL DEFAULT '{}',
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    UNIQUE (knowledge_base_id, original_path)
);

CREATE INDEX IF NOT EXISTS idx_source_documents_base
    ON source_documents(knowledge_base_id, ordinal);

CREATE TABLE IF NOT EXISTS source_versions (
    id                  TEXT PRIMARY KEY,
    source_document_id  TEXT NOT NULL REFERENCES source_documents(id) ON DELETE CASCADE,
    content_hash        TEXT NOT NULL,
    size_bytes          INTEGER NOT NULL,
    modified_at         TEXT,
    extraction_version  TEXT NOT NULL,
    extraction_status   TEXT NOT NULL,
    error               TEXT,
    created_at          TEXT NOT NULL,
    UNIQUE (source_document_id, content_hash)
);

CREATE TABLE IF NOT EXISTS source_spans (
    id                  TEXT PRIMARY KEY,
    knowledge_base_id   TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    source_version_id   TEXT NOT NULL REFERENCES source_versions(id) ON DELETE CASCADE,
    ordinal             INTEGER NOT NULL,
    page_number         INTEGER,
    heading             TEXT,
    anchor              TEXT,
    line_start          INTEGER,
    line_end            INTEGER,
    content             TEXT NOT NULL,
    content_hash        TEXT NOT NULL,
    token_estimate      INTEGER NOT NULL DEFAULT 0,
    UNIQUE (source_version_id, ordinal)
);

CREATE INDEX IF NOT EXISTS idx_source_spans_base
    ON source_spans(knowledge_base_id, source_version_id, ordinal);

CREATE TABLE IF NOT EXISTS agent_runs (
    id                  TEXT PRIMARY KEY,
    knowledge_base_id   TEXT REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    runtime             TEXT NOT NULL,
    task_type           TEXT NOT NULL,
    status              TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled')),
    input_json          TEXT NOT NULL DEFAULT '{}',
    output_json         TEXT,
    error               TEXT,
    started_at          TEXT,
    finished_at         TEXT,
    created_at          TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS knowledge_entries (
    id                  TEXT PRIMARY KEY,
    knowledge_base_id   TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    origin_document_id  TEXT REFERENCES source_documents(id) ON DELETE SET NULL,
    entry_type          TEXT NOT NULL,
    slug                TEXT NOT NULL,
    title               TEXT NOT NULL,
    summary             TEXT NOT NULL DEFAULT '',
    content_md          TEXT NOT NULL DEFAULT '',
    status              TEXT NOT NULL DEFAULT 'draft'
                            CHECK (status IN ('draft', 'verified', 'stale', 'archived')),
    confidence          REAL,
    revision            INTEGER NOT NULL DEFAULT 1,
    created_by_run_id   TEXT REFERENCES agent_runs(id),
    updated_by_run_id   TEXT REFERENCES agent_runs(id),
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    UNIQUE (knowledge_base_id, entry_type, slug)
);

CREATE INDEX IF NOT EXISTS idx_knowledge_entries_base
    ON knowledge_entries(knowledge_base_id, status, entry_type, updated_at DESC);

CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_entries_fts USING fts5(
    entry_id UNINDEXED,
    knowledge_base_id UNINDEXED,
    title,
    summary,
    content_md,
    tokenize = 'unicode61'
);

CREATE TABLE IF NOT EXISTS knowledge_claims (
    id                   TEXT PRIMARY KEY,
    knowledge_base_id    TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    entry_id              TEXT NOT NULL REFERENCES knowledge_entries(id) ON DELETE CASCADE,
    subject_entry_id      TEXT REFERENCES knowledge_entries(id) ON DELETE SET NULL,
    predicate             TEXT NOT NULL,
    object_entry_id       TEXT REFERENCES knowledge_entries(id) ON DELETE SET NULL,
    object_text           TEXT,
    claim_text            TEXT NOT NULL,
    confidence            REAL,
    verification_status   TEXT NOT NULL DEFAULT 'unverified'
                              CHECK (verification_status IN ('unverified', 'supported', 'disputed', 'rejected', 'stale')),
    revision              INTEGER NOT NULL DEFAULT 1,
    created_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS knowledge_relations (
    id                   TEXT PRIMARY KEY,
    knowledge_base_id    TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    from_entry_id        TEXT NOT NULL REFERENCES knowledge_entries(id) ON DELETE CASCADE,
    to_entry_id          TEXT NOT NULL REFERENCES knowledge_entries(id) ON DELETE CASCADE,
    relation_type        TEXT NOT NULL,
    strength             REAL,
    evidence             TEXT,
    revision             INTEGER NOT NULL DEFAULT 1,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL,
    UNIQUE (knowledge_base_id, from_entry_id, to_entry_id, relation_type)
);

CREATE TABLE IF NOT EXISTS knowledge_citations (
    id                   TEXT PRIMARY KEY,
    knowledge_base_id    TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    entry_id              TEXT REFERENCES knowledge_entries(id) ON DELETE CASCADE,
    claim_id              TEXT REFERENCES knowledge_claims(id) ON DELETE CASCADE,
    source_span_id        TEXT NOT NULL REFERENCES source_spans(id) ON DELETE CASCADE,
    quote_text            TEXT,
    created_at            TEXT NOT NULL,
    CHECK (entry_id IS NOT NULL OR claim_id IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_knowledge_citations_entry
    ON knowledge_citations(knowledge_base_id, entry_id);

CREATE TABLE IF NOT EXISTS knowledge_tasks (
    id                   TEXT PRIMARY KEY,
    knowledge_base_id    TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    title                TEXT NOT NULL,
    description          TEXT NOT NULL DEFAULT '',
    task_type            TEXT NOT NULL DEFAULT 'research'
                             CHECK (task_type IN ('research', 'refresh', 'review')),
    status               TEXT NOT NULL DEFAULT 'draft'
                             CHECK (status IN ('draft', 'queued', 'running', 'completed', 'failed', 'cancelled')),
    result_summary       TEXT NOT NULL DEFAULT '',
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_knowledge_tasks_base
    ON knowledge_tasks(knowledge_base_id, status, updated_at DESC);

CREATE TABLE IF NOT EXISTS knowledge_config_documents (
    id                   TEXT PRIMARY KEY,
    knowledge_base_id    TEXT REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    scope                TEXT NOT NULL CHECK (scope IN ('global', 'book')),
    name                 TEXT NOT NULL,
    content_md           TEXT NOT NULL DEFAULT '',
    revision             INTEGER NOT NULL DEFAULT 1,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL,
    UNIQUE (knowledge_base_id, scope, name)
);

CREATE TABLE IF NOT EXISTS agent_runtime_profiles (
    id                   TEXT PRIMARY KEY,
    name                 TEXT NOT NULL UNIQUE,
    runtime              TEXT NOT NULL CHECK (runtime IN ('deepseek_harness', 'claude_code')),
    executable           TEXT NOT NULL,
    model                TEXT NOT NULL DEFAULT '',
    enabled              INTEGER NOT NULL DEFAULT 1,
    config_json          TEXT NOT NULL DEFAULT '{}',
    revision             INTEGER NOT NULL DEFAULT 1,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL
);

INSERT OR IGNORE INTO agent_runtime_profiles (
    id, name, runtime, executable, model, enabled, created_at, updated_at
) VALUES (
    'runtime-deepseek-harness', 'DeepSeek Harness', 'deepseek_harness',
    'deepseek-harness', '', 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
);
