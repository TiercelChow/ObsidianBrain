ALTER TABLE knowledge_tasks ADD COLUMN external_research_enabled INTEGER NOT NULL DEFAULT 0
    CHECK (external_research_enabled IN (0, 1));
ALTER TABLE knowledge_tasks ADD COLUMN external_domains_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE knowledge_tasks ADD COLUMN external_request_limit INTEGER NOT NULL DEFAULT 0
    CHECK (external_request_limit BETWEEN 0 AND 50);
ALTER TABLE knowledge_tasks ADD COLUMN external_requests_used INTEGER NOT NULL DEFAULT 0
    CHECK (external_requests_used BETWEEN 0 AND external_request_limit);

INSERT OR IGNORE INTO skills
    (id, slug, name, description, source_type, status, permissions_json,
     requirements_json, created_at, updated_at)
VALUES
    ('skill-book-synthesis', 'book-synthesis', '跨章综合',
     '把分散在不同章节的观点综合为可追溯的主题、共识、分歧与开放问题。',
     'builtin', 'ready', '["knowledge.read","knowledge.propose"]', '[]',
     CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
    ('skill-markdown-collection', 'markdown-collection', 'Markdown 资料汇编',
     '把研究结果整理为结构稳定、可下载和继续编辑的 Markdown 文档。',
     'builtin', 'ready', '["knowledge.read","artifact.report"]', '[]',
     CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

INSERT OR IGNORE INTO skill_versions (id, skill_id, revision, content_hash, created_at)
VALUES
    ('skill-version-book-synthesis-v1', 'skill-book-synthesis', 1,
     'builtin-book-synthesis-v1', CURRENT_TIMESTAMP),
    ('skill-version-markdown-collection-v1', 'skill-markdown-collection', 1,
     'builtin-markdown-collection-v1', CURRENT_TIMESTAMP);

INSERT OR IGNORE INTO skill_files
    (skill_version_id, relative_path, media_type, content_text, content_hash, size_bytes)
VALUES
    ('skill-version-book-synthesis-v1', 'SKILL.md', 'text/markdown',
     '先按主题而不是文件顺序聚合证据，再输出：综合结论、跨章证据、分歧与边界、开放问题。每项事实必须保留 [S<n>] 引用；仅提交候选变更，不直接写入正式知识。',
     'builtin-book-synthesis-v1', 229),
    ('skill-version-markdown-collection-v1', 'SKILL.md', 'text/markdown',
     '输出单份 Markdown：一级标题为主题，随后依次为执行摘要、核心发现、证据索引、待确认事项。使用短段落和语义化标题；所有来源沿用 [S<n>]，不得生成脚本或请求宿主文件权限。',
     'builtin-markdown-collection-v1', 246);

UPDATE skills
SET current_version_id = CASE id
    WHEN 'skill-book-synthesis' THEN 'skill-version-book-synthesis-v1'
    WHEN 'skill-markdown-collection' THEN 'skill-version-markdown-collection-v1'
END
WHERE id IN ('skill-book-synthesis', 'skill-markdown-collection')
  AND current_version_id IS NULL;
