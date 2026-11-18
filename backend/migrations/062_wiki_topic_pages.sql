CREATE TABLE IF NOT EXISTS wiki_topic_pages (
    id                 TEXT PRIMARY KEY,
    knowledge_base_id  TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    topic_key          TEXT NOT NULL,
    title              TEXT NOT NULL,
    published_id       TEXT,
    draft_id           TEXT,
    running_attempt    TEXT,
    attempt_deadline   TEXT,
    last_request_json  TEXT,
    last_error         TEXT,
    created_at         TEXT NOT NULL,
    UNIQUE (knowledge_base_id, topic_key)
);

CREATE TABLE IF NOT EXISTS wiki_topic_revisions (
    id                 TEXT PRIMARY KEY,
    page_id            TEXT NOT NULL REFERENCES wiki_topic_pages(id) ON DELETE CASCADE,
    parent_id          TEXT REFERENCES wiki_topic_revisions(id) ON DELETE SET NULL,
    run_id             TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    input_hash         TEXT NOT NULL,
    markdown           TEXT NOT NULL,
    created_at         TEXT NOT NULL,
    published_at       TEXT
);
CREATE INDEX IF NOT EXISTS wiki_topic_revisions_page ON wiki_topic_revisions(page_id, created_at DESC);

CREATE TABLE IF NOT EXISTS wiki_topic_questions (
    id            TEXT PRIMARY KEY,
    page_id       TEXT NOT NULL REFERENCES wiki_topic_pages(id) ON DELETE CASCADE,
    run_id        TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    question      TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    resolved_at   TEXT,
    UNIQUE (page_id, question)
);

CREATE TABLE IF NOT EXISTS wiki_topic_revision_sources (
    revision_id        TEXT NOT NULL REFERENCES wiki_topic_revisions(id) ON DELETE CASCADE,
    source_document_id TEXT NOT NULL,
    source_version_id  TEXT NOT NULL,
    PRIMARY KEY (revision_id, source_document_id)
);

CREATE VIEW IF NOT EXISTS wiki_topic_stale_revisions AS
    SELECT refs.revision_id
    FROM wiki_topic_revision_sources refs
    LEFT JOIN source_documents doc ON doc.id = refs.source_document_id
    WHERE doc.current_version_id IS NOT refs.source_version_id
       OR doc.sync_status IS NOT 'current';

CREATE VIRTUAL TABLE IF NOT EXISTS wiki_topic_sections_fts USING fts5(
    page_id UNINDEXED, ordinal UNINDEXED, title, heading, markdown, cjk_terms,
    tokenize = 'unicode61'
);

-- The search projection is rebuilt at publish time and follows page deletion.
CREATE TRIGGER IF NOT EXISTS wiki_topic_sections_delete AFTER DELETE ON wiki_topic_pages BEGIN
    DELETE FROM wiki_topic_sections_fts WHERE page_id = OLD.id;
END;
