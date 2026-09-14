CREATE TABLE IF NOT EXISTS skills (
    id                  TEXT PRIMARY KEY,
    slug                TEXT NOT NULL UNIQUE,
    name                TEXT NOT NULL,
    description         TEXT NOT NULL DEFAULT '',
    source_type         TEXT NOT NULL CHECK (source_type IN ('builtin', 'custom')),
    status              TEXT NOT NULL DEFAULT 'ready'
                            CHECK (status IN ('ready', 'invalid', 'unavailable')),
    permissions_json    TEXT NOT NULL DEFAULT '[]',
    requirements_json   TEXT NOT NULL DEFAULT '[]',
    current_version_id  TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS skill_versions (
    id                  TEXT PRIMARY KEY,
    skill_id            TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    revision            INTEGER NOT NULL,
    content_hash        TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    UNIQUE (skill_id, revision),
    UNIQUE (skill_id, content_hash)
);

CREATE TABLE IF NOT EXISTS skill_files (
    skill_version_id    TEXT NOT NULL REFERENCES skill_versions(id) ON DELETE CASCADE,
    relative_path       TEXT NOT NULL,
    media_type          TEXT NOT NULL,
    content_text        TEXT NOT NULL,
    content_hash        TEXT NOT NULL,
    size_bytes          INTEGER NOT NULL,
    PRIMARY KEY (skill_version_id, relative_path),
    CHECK (relative_path <> '' AND relative_path NOT LIKE '/%' AND relative_path NOT LIKE '%..%')
);

CREATE TABLE IF NOT EXISTS knowledge_base_skill_bindings (
    knowledge_base_id   TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    skill_id            TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    usage_scope         TEXT NOT NULL DEFAULT 'both'
                            CHECK (usage_scope IN ('qa', 'research', 'both')),
    enabled             INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
    revision            INTEGER NOT NULL DEFAULT 1,
    updated_at          TEXT NOT NULL,
    PRIMARY KEY (knowledge_base_id, skill_id)
);

CREATE INDEX IF NOT EXISTS idx_skill_bindings_base
    ON knowledge_base_skill_bindings(knowledge_base_id, enabled, usage_scope);

CREATE TABLE IF NOT EXISTS agent_run_events (
    run_id              TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    sequence            INTEGER NOT NULL,
    event_type          TEXT NOT NULL,
    phase               TEXT,
    message             TEXT NOT NULL DEFAULT '',
    payload_json        TEXT NOT NULL DEFAULT '{}',
    created_at          TEXT NOT NULL,
    PRIMARY KEY (run_id, sequence)
);

CREATE INDEX IF NOT EXISTS idx_agent_run_events_created
    ON agent_run_events(run_id, created_at, sequence);

INSERT OR IGNORE INTO skills
    (id, slug, name, description, source_type, status, permissions_json,
     requirements_json, created_at, updated_at)
VALUES
    ('skill-book-query', 'book-query', '书内问答',
     '基于当前书籍证据给出可追溯回答，并明确区分结论与证据不足。',
     'builtin', 'ready', '["knowledge.read"]', '[]', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
    ('skill-book-research', 'book-research', '专题研究',
     '围绕研究目标组织结论、证据链、分歧与后续问题。',
     'builtin', 'ready', '["knowledge.read"]', '[]', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
    ('skill-book-lint', 'book-lint', '知识审校',
     '检查证据冲突、陈旧结论、缺失引用与待补充主题。',
     'builtin', 'ready', '["knowledge.read"]', '[]', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

INSERT OR IGNORE INTO skill_versions (id, skill_id, revision, content_hash, created_at)
VALUES
    ('skill-version-book-query-v1', 'skill-book-query', 1, 'builtin-book-query-v1', CURRENT_TIMESTAMP),
    ('skill-version-book-research-v1', 'skill-book-research', 1, 'builtin-book-research-v1', CURRENT_TIMESTAMP),
    ('skill-version-book-lint-v1', 'skill-book-lint', 1, 'builtin-book-lint-v1', CURRENT_TIMESTAMP);

INSERT OR IGNORE INTO skill_files
    (skill_version_id, relative_path, media_type, content_text, content_hash, size_bytes)
VALUES
    ('skill-version-book-query-v1', 'SKILL.md', 'text/markdown',
     '回答前先梳理证据覆盖范围。正文先给直接结论，再列出关键依据；每个事实性结论保留来源编号。证据无法支撑的问题必须明确标记为“证据不足”。',
     'builtin-book-query-v1', 189),
    ('skill-version-book-research-v1', 'SKILL.md', 'text/markdown',
     '将研究输出组织为：执行摘要、核心发现、证据链、存在分歧、待验证问题。不要把推测写成事实；相互冲突的来源需要并列呈现。',
     'builtin-book-research-v1', 182),
    ('skill-version-book-lint-v1', 'SKILL.md', 'text/markdown',
     '从冲突、陈旧、缺失引用、概念重复和知识空白五个方向审校。每项问题说明影响范围、引用依据与建议动作，不直接修改知识库。',
     'builtin-book-lint-v1', 183);

UPDATE skills
SET current_version_id = CASE id
    WHEN 'skill-book-query' THEN 'skill-version-book-query-v1'
    WHEN 'skill-book-research' THEN 'skill-version-book-research-v1'
    WHEN 'skill-book-lint' THEN 'skill-version-book-lint-v1'
END
WHERE id IN ('skill-book-query', 'skill-book-research', 'skill-book-lint')
  AND current_version_id IS NULL;
