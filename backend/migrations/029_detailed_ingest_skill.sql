-- The SKILL.md body is seeded from backend/skills/book-ingest/SKILL.md inside
-- the same migration transaction, so the readable Markdown is the sole source.
INSERT OR IGNORE INTO skill_versions
    (id, skill_id, revision, content_hash, release_state, parent_version_id, changelog, created_at)
VALUES
    ('skill-version-book-ingest-v3', 'skill-book-ingest', 3,
     'builtin-book-ingest-v3-pending', 'candidate', 'skill-version-book-ingest-v2',
     '补全数据库语义建库的适用边界、证据清单、主题归并、变更分流、输出契约与自检步骤；暂不启用。',
     CURRENT_TIMESTAMP);

INSERT OR IGNORE INTO skill_version_origins
    (skill_version_id, repository_url, source_path, source_ref, license_spdx,
     attribution, adaptation_notes, reviewed_at)
VALUES
    ('skill-version-book-ingest-v3', 'https://github.com/NousResearch/hermes-agent',
     'skills/research/llm-wiki/SKILL.md',
     '1c9433897c7b8a3b2754a84875e8f86d0a42991a', 'MIT',
     'NousResearch Hermes Agent LLM Wiki (method reference)',
     '本项目独立撰写；借鉴既有知识定位、按主题维护、证据溯源、冲突保留和低价值退出，改写为 SQLite 候选与人工审核流程，不复制上游文件操作。',
     CURRENT_TIMESTAMP);
