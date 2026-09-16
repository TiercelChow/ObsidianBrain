-- Skill bodies are seeded from backend/skills/*/SKILL.md in this transaction.
-- Keep prior versions and user-created skills intact; switch only built-in identities.
INSERT OR IGNORE INTO skill_versions
    (id, skill_id, revision, content_hash, release_state, parent_version_id, changelog, created_at)
VALUES
    ('skill-version-book-query-v3', 'skill-book-query', 3,
     'builtin-book-query-v3-pending', 'published', 'skill-version-book-query-v2',
     '重写书内问答：问题拆解、证据映射、逐句引用、冲突与不足处理，并适配实际提示词输入。', CURRENT_TIMESTAMP),
    ('skill-version-book-research-v3', 'skill-book-research', 3,
     'builtin-book-research-v3-pending', 'published', 'skill-version-book-research-v2',
     '重写专题研究：轻量计划、证据矩阵、外部授权边界、报告结构及知识回写边界。', CURRENT_TIMESTAMP),
    ('skill-version-book-presentation-v3', 'skill-book-presentation', 3,
     'builtin-book-presentation-v3-pending', 'published', 'skill-version-book-presentation-v2',
     '按受控 PPTX 生成器实际解析能力重写叙事、页面密度、引用和输出结构。', CURRENT_TIMESTAMP),
    ('skill-version-book-lint-v2', 'skill-book-lint', 2,
     'builtin-book-lint-v2-pending', 'published', 'skill-version-book-lint-v1',
     '补全引用、冲突、陈旧、重复、知识空白五类审校及可复核诊断格式。', CURRENT_TIMESTAMP),
    ('skill-version-book-synthesis-v2', 'skill-book-synthesis', 2,
     'builtin-book-synthesis-v2-pending', 'published', 'skill-version-book-synthesis-v1',
     '补全跨章主题归并、证据对照、共识边界、分歧保留与审核建议。', CURRENT_TIMESTAMP),
    ('skill-version-markdown-collection-v2', 'skill-markdown-collection', 2,
     'builtin-markdown-collection-v2-pending', 'published', 'skill-version-markdown-collection-v1',
     '补全独立 Markdown 报告的层级、逐项引用、证据索引与来源边界。', CURRENT_TIMESTAMP);

UPDATE skill_versions
SET release_state = 'published',
    changelog = '完整语义建库指令直接启用：来源边界、证据清单、主题归并、变更分流、JSON 契约与自检。'
WHERE id = 'skill-version-book-ingest-v3';

UPDATE skills
SET current_version_id = CASE id
        WHEN 'skill-book-ingest' THEN 'skill-version-book-ingest-v3'
        WHEN 'skill-book-query' THEN 'skill-version-book-query-v3'
        WHEN 'skill-book-research' THEN 'skill-version-book-research-v3'
        WHEN 'skill-book-presentation' THEN 'skill-version-book-presentation-v3'
        WHEN 'skill-book-lint' THEN 'skill-version-book-lint-v2'
        WHEN 'skill-book-synthesis' THEN 'skill-version-book-synthesis-v2'
        WHEN 'skill-markdown-collection' THEN 'skill-version-markdown-collection-v2'
    END,
    updated_at = CURRENT_TIMESTAMP
WHERE source_type = 'builtin'
  AND id IN (
      'skill-book-ingest', 'skill-book-query', 'skill-book-research',
      'skill-book-presentation', 'skill-book-lint', 'skill-book-synthesis',
      'skill-markdown-collection'
  );

INSERT OR IGNORE INTO skill_version_origins
    (skill_version_id, repository_url, source_path, source_ref, license_spdx,
     attribution, adaptation_notes, reviewed_at)
VALUES
    ('skill-version-book-query-v3', 'https://github.com/NousResearch/hermes-agent',
     'skills/research/grounded-citations/SKILL.md', 'main', 'MIT',
     'NousResearch Hermes Agent grounded-citations (method reference)',
     '独立撰写；只借鉴论断附近引用和证据不足显式表达，改写为本书 [S<n>] 协议，不复制其脚本或文件流程。',
     CURRENT_TIMESTAMP),
    ('skill-version-book-research-v3', 'https://github.com/bytedance/deer-flow',
     'skills/public/deep-research/SKILL.md', 'main', 'MIT',
     'ByteDance DeerFlow deep-research (method reference)',
     '独立撰写；借鉴研究问题拆解与多角度核验，限定为本地数据库证据及逐任务授权的外部只读研究。',
     CURRENT_TIMESTAMP),
    ('skill-version-book-presentation-v3', 'https://github.com/anthropics/skills',
     'skills/pptx/SKILL.md', 'main', 'Reference-Only',
     'Anthropic PPTX skill (non-redistributed reference)',
     '独立撰写；仅参考单页单观点和视觉叙事原则，明确不复制上游内容或其脚本执行方案。',
     CURRENT_TIMESTAMP);
