ALTER TABLE knowledge_bases ADD COLUMN compile_processed_sources INTEGER NOT NULL DEFAULT 0;
ALTER TABLE knowledge_bases ADD COLUMN compile_total_sources INTEGER NOT NULL DEFAULT 0;
ALTER TABLE knowledge_bases ADD COLUMN pending_review_count INTEGER NOT NULL DEFAULT 0;

ALTER TABLE knowledge_entries ADD COLUMN aliases_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE knowledge_entries ADD COLUMN edit_policy TEXT NOT NULL DEFAULT 'agent_managed'
    CHECK (edit_policy IN ('agent_managed', 'human_protected'));

CREATE TABLE IF NOT EXISTS knowledge_entry_versions (
    id                  TEXT PRIMARY KEY,
    entry_id            TEXT NOT NULL REFERENCES knowledge_entries(id) ON DELETE CASCADE,
    revision            INTEGER NOT NULL,
    title               TEXT NOT NULL,
    summary             TEXT NOT NULL,
    content_md          TEXT NOT NULL,
    aliases_json        TEXT NOT NULL DEFAULT '[]',
    status              TEXT NOT NULL,
    confidence          REAL,
    changed_by_run_id   TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
    change_set_id       TEXT,
    created_at          TEXT NOT NULL,
    UNIQUE (entry_id, revision)
);

INSERT OR IGNORE INTO knowledge_entry_versions
    (id, entry_id, revision, title, summary, content_md, aliases_json, status,
     confidence, changed_by_run_id, created_at)
SELECT 'entry-version-' || id || '-' || revision, id, revision, title, summary, content_md,
       aliases_json, status, confidence, updated_by_run_id, updated_at
FROM knowledge_entries;

CREATE TABLE IF NOT EXISTS knowledge_change_sets (
    id                  TEXT PRIMARY KEY,
    knowledge_base_id   TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    agent_run_id        TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
    title               TEXT NOT NULL,
    reason              TEXT NOT NULL DEFAULT '',
    risk_level          TEXT NOT NULL DEFAULT 'low'
                            CHECK (risk_level IN ('low', 'medium', 'high')),
    status              TEXT NOT NULL DEFAULT 'proposed'
                            CHECK (status IN ('proposed', 'approved', 'rejected', 'applied', 'conflicted')),
    idempotency_key     TEXT NOT NULL UNIQUE,
    created_at          TEXT NOT NULL,
    resolved_at         TEXT,
    resolved_by         TEXT
);

CREATE INDEX IF NOT EXISTS idx_knowledge_change_sets_review
    ON knowledge_change_sets(knowledge_base_id, status, created_at DESC);

CREATE TABLE IF NOT EXISTS knowledge_changes (
    id                  TEXT PRIMARY KEY,
    change_set_id       TEXT NOT NULL REFERENCES knowledge_change_sets(id) ON DELETE CASCADE,
    ordinal             INTEGER NOT NULL,
    operation           TEXT NOT NULL
                            CHECK (operation IN ('create', 'update', 'merge', 'split', 'archive', 'restore')),
    object_type         TEXT NOT NULL CHECK (object_type IN ('entry', 'claim', 'relation')),
    object_id           TEXT NOT NULL,
    expected_revision   INTEGER,
    before_json         TEXT,
    after_json          TEXT NOT NULL,
    UNIQUE (change_set_id, ordinal)
);

CREATE TABLE IF NOT EXISTS knowledge_reviews (
    id                  TEXT PRIMARY KEY,
    knowledge_base_id   TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    change_set_id       TEXT NOT NULL UNIQUE REFERENCES knowledge_change_sets(id) ON DELETE CASCADE,
    status              TEXT NOT NULL DEFAULT 'pending'
                            CHECK (status IN ('pending', 'approved', 'rejected')),
    note                TEXT NOT NULL DEFAULT '',
    created_at          TEXT NOT NULL,
    resolved_at         TEXT
);

CREATE TABLE IF NOT EXISTS knowledge_artifacts (
    id                  TEXT PRIMARY KEY,
    knowledge_base_id   TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    knowledge_task_id   TEXT REFERENCES knowledge_tasks(id) ON DELETE SET NULL,
    agent_run_id        TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
    skill_id            TEXT REFERENCES skills(id) ON DELETE SET NULL,
    artifact_type       TEXT NOT NULL CHECK (artifact_type IN ('pptx', 'pdf', 'image', 'report')),
    title               TEXT NOT NULL,
    relative_path       TEXT NOT NULL UNIQUE,
    mime_type           TEXT NOT NULL,
    content_hash        TEXT NOT NULL,
    size_bytes          INTEGER NOT NULL,
    validation_state    TEXT NOT NULL DEFAULT 'pending'
                            CHECK (validation_state IN ('pending', 'valid', 'warning', 'invalid')),
    validation_message  TEXT NOT NULL DEFAULT '',
    created_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_knowledge_artifacts_task
    ON knowledge_artifacts(knowledge_task_id, created_at DESC);

CREATE TABLE IF NOT EXISTS knowledge_artifact_citations (
    artifact_id         TEXT NOT NULL REFERENCES knowledge_artifacts(id) ON DELETE CASCADE,
    ordinal             INTEGER NOT NULL,
    entry_id            TEXT REFERENCES knowledge_entries(id) ON DELETE SET NULL,
    entry_revision      INTEGER,
    title_snapshot      TEXT NOT NULL,
    source_path_snapshot TEXT,
    PRIMARY KEY (artifact_id, ordinal)
);

ALTER TABLE knowledge_tasks ADD COLUMN deliverable_type TEXT NOT NULL DEFAULT 'report'
    CHECK (deliverable_type IN ('report', 'presentation'));
ALTER TABLE knowledge_tasks ADD COLUMN artifact_state TEXT NOT NULL DEFAULT 'not_requested'
    CHECK (artifact_state IN ('not_requested', 'pending', 'ready', 'failed'));
ALTER TABLE knowledge_tasks ADD COLUMN knowledge_change_state TEXT NOT NULL DEFAULT 'none'
    CHECK (knowledge_change_state IN ('none', 'proposed', 'applied', 'rejected'));
ALTER TABLE knowledge_tasks ADD COLUMN cancel_requested INTEGER NOT NULL DEFAULT 0
    CHECK (cancel_requested IN (0, 1));

INSERT OR IGNORE INTO skills
    (id, slug, name, description, source_type, status, permissions_json,
     requirements_json, created_at, updated_at)
VALUES
    ('skill-book-ingest', 'book-ingest', '语义建库',
     '从版本化来源中提炼跨章节概念、论断和关系，并生成待审核变更集。',
     'builtin', 'ready', '["knowledge.read","knowledge.propose"]', '[]',
     CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
    ('skill-book-presentation', 'book-presentation', '演示文稿',
     '把研究结论和来源组织为可编辑、可追溯的 PPTX 演示文稿。',
     'builtin', 'ready', '["knowledge.read","artifact.render"]', '[]',
     CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

INSERT OR IGNORE INTO skill_versions (id, skill_id, revision, content_hash, created_at)
VALUES
    ('skill-version-book-ingest-v1', 'skill-book-ingest', 1, 'builtin-book-ingest-v1', CURRENT_TIMESTAMP),
    ('skill-version-book-presentation-v1', 'skill-book-presentation', 1, 'builtin-book-presentation-v1', CURRENT_TIMESTAMP);

INSERT OR IGNORE INTO skill_files
    (skill_version_id, relative_path, media_type, content_text, content_hash, size_bytes)
VALUES
    ('skill-version-book-ingest-v1', 'SKILL.md', 'text/markdown',
     '提取跨章节重复出现的概念、方法、比较和问题。先检索既有语义条目，再提出新增或更新；每个条目必须引用本批提供的来源片段，不得直接覆盖人工保护内容。',
     'builtin-book-ingest-v1', 213),
    ('skill-version-book-presentation-v1', 'SKILL.md', 'text/markdown',
     '将研究结论组织为封面、概览、核心概念、关系或比较、注意事项和总结。每页控制信息密度，讲者备注保留来源，禁止编造证据中不存在的数字。',
     'builtin-book-presentation-v1', 204);

UPDATE skills
SET current_version_id = CASE id
    WHEN 'skill-book-ingest' THEN 'skill-version-book-ingest-v1'
    WHEN 'skill-book-presentation' THEN 'skill-version-book-presentation-v1'
END
WHERE id IN ('skill-book-ingest', 'skill-book-presentation')
  AND current_version_id IS NULL;
