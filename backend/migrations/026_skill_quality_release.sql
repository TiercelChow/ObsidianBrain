ALTER TABLE skill_versions ADD COLUMN release_state TEXT NOT NULL DEFAULT 'published'
    CHECK (release_state IN ('candidate', 'published', 'retired'));
ALTER TABLE skill_versions ADD COLUMN parent_version_id TEXT REFERENCES skill_versions(id) ON DELETE SET NULL;
ALTER TABLE skill_versions ADD COLUMN changelog TEXT NOT NULL DEFAULT '';

CREATE TABLE IF NOT EXISTS skill_version_origins (
    skill_version_id    TEXT PRIMARY KEY REFERENCES skill_versions(id) ON DELETE CASCADE,
    repository_url      TEXT NOT NULL,
    source_path         TEXT NOT NULL DEFAULT '',
    source_ref          TEXT NOT NULL DEFAULT '',
    license_spdx        TEXT NOT NULL,
    attribution         TEXT NOT NULL DEFAULT '',
    adaptation_notes    TEXT NOT NULL DEFAULT '',
    reviewed_at         TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS skill_evaluation_suites (
    id                  TEXT PRIMARY KEY,
    slug                TEXT NOT NULL UNIQUE,
    name                TEXT NOT NULL,
    description         TEXT NOT NULL DEFAULT '',
    usage_scope         TEXT NOT NULL CHECK (usage_scope IN ('qa', 'research', 'ingest', 'presentation')),
    pass_score          REAL NOT NULL DEFAULT 0.75 CHECK (pass_score >= 0 AND pass_score <= 1),
    created_at          TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS skill_evaluation_cases (
    id                  TEXT PRIMARY KEY,
    suite_id            TEXT NOT NULL REFERENCES skill_evaluation_suites(id) ON DELETE CASCADE,
    ordinal             INTEGER NOT NULL,
    name                TEXT NOT NULL,
    description         TEXT NOT NULL DEFAULT '',
    required_concepts_json TEXT NOT NULL DEFAULT '[]',
    forbidden_concepts_json TEXT NOT NULL DEFAULT '[]',
    weight              REAL NOT NULL DEFAULT 1 CHECK (weight > 0),
    UNIQUE (suite_id, ordinal)
);

CREATE TABLE IF NOT EXISTS skill_evaluation_runs (
    id                  TEXT PRIMARY KEY,
    skill_version_id    TEXT NOT NULL REFERENCES skill_versions(id) ON DELETE CASCADE,
    suite_id            TEXT NOT NULL REFERENCES skill_evaluation_suites(id) ON DELETE CASCADE,
    baseline_version_id TEXT REFERENCES skill_versions(id) ON DELETE SET NULL,
    score               REAL NOT NULL CHECK (score >= 0 AND score <= 1),
    baseline_score      REAL,
    passed              INTEGER NOT NULL CHECK (passed IN (0, 1)),
    findings_json       TEXT NOT NULL DEFAULT '[]',
    created_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_skill_evaluation_runs_version
    ON skill_evaluation_runs(skill_version_id, created_at DESC);

INSERT OR IGNORE INTO skill_evaluation_suites
    (id, slug, name, description, usage_scope, pass_score, created_at)
VALUES
    ('skill-suite-ingest', 'semantic-ingest', '语义建库固定评测',
     '检查变更分流、引用约束、冲突保留和无变化退出条件。', 'ingest', 0.80, CURRENT_TIMESTAMP),
    ('skill-suite-query', 'grounded-query', '书内问答固定评测',
     '检查问题拆解、事实引用、证据不足和冲突呈现。', 'qa', 0.75, CURRENT_TIMESTAMP),
    ('skill-suite-research', 'evidence-research', '专题研究固定评测',
     '检查研究计划、证据链、来源边界、差距与不确定性。', 'research', 0.75, CURRENT_TIMESTAMP),
    ('skill-suite-presentation', 'evidence-presentation', '演示文稿固定评测',
     '检查叙事结构、信息密度、来源追溯和禁止编造。', 'presentation', 0.75, CURRENT_TIMESTAMP);

INSERT OR IGNORE INTO skill_evaluation_cases
    (id, suite_id, ordinal, name, description, required_concepts_json, forbidden_concepts_json, weight)
VALUES
    ('skill-case-ingest-routing', 'skill-suite-ingest', 1, '变更分流',
     '必须区分新增、更新、争议和无实质变化。', '["新增","更新","争议","无实质变化"]', '[]', 1.5),
    ('skill-case-ingest-citations', 'skill-suite-ingest', 2, '引用约束',
     '条目和原子论断必须保留来源引用。', '["引用","来源","论断"]', '["忽略引用"]', 1.5),
    ('skill-case-ingest-conflict', 'skill-suite-ingest', 3, '冲突保留',
     '不得强行消解相互矛盾的证据。', '["冲突","不得强行"]', '[]', 1.0),
    ('skill-case-query-grounding', 'skill-suite-query', 1, '事实可追溯',
     '事实性结论必须可追溯到来源。', '["事实","引用","来源"]', '["无需引用"]', 1.5),
    ('skill-case-query-uncertainty', 'skill-suite-query', 2, '不确定性',
     '覆盖不足时明确证据不足。', '["证据不足","冲突"]', '[]', 1.0),
    ('skill-case-query-decompose', 'skill-suite-query', 3, '问题拆解',
     '复杂问题先拆解再综合回答。', '["拆解","综合"]', '[]', 1.0),
    ('skill-case-research-plan', 'skill-suite-research', 1, '研究计划',
     '先规划问题和证据，再综合结论。', '["计划","证据","综合"]', '[]', 1.5),
    ('skill-case-research-boundary', 'skill-suite-research', 2, '来源边界',
     '区分书内证据与外部资料。', '["书内","外部","来源"]', '[]', 1.0),
    ('skill-case-research-gaps', 'skill-suite-research', 3, '差距与分歧',
     '保留分歧并给出待验证问题。', '["分歧","待验证"]', '[]', 1.0),
    ('skill-case-presentation-story', 'skill-suite-presentation', 1, '叙事结构',
     '先设计受众、主线与页面层级。', '["受众","主线","页面"]', '[]', 1.0),
    ('skill-case-presentation-density', 'skill-suite-presentation', 2, '页面密度',
     '限制每页信息密度并使用讲者备注。', '["信息密度","讲者备注"]', '[]', 1.0),
    ('skill-case-presentation-grounding', 'skill-suite-presentation', 3, '证据约束',
     '关键结论可追溯且禁止编造。', '["来源","禁止编造"]', '[]', 1.5);

INSERT OR IGNORE INTO skill_versions
    (id, skill_id, revision, content_hash, release_state, parent_version_id, changelog, created_at)
VALUES
    ('skill-version-book-ingest-v2', 'skill-book-ingest', 2, 'builtin-book-ingest-v2-quality', 'candidate', 'skill-version-book-ingest-v1',
     '加入变更分流、引用审计、冲突保留和无实质变化退出条件。', CURRENT_TIMESTAMP),
    ('skill-version-book-query-v2', 'skill-book-query', 2, 'builtin-book-query-v2-quality', 'candidate', 'skill-version-book-query-v1',
     '加入问题拆解、逐项证据覆盖和冲突表达。', CURRENT_TIMESTAMP),
    ('skill-version-book-research-v2', 'skill-book-research', 2, 'builtin-book-research-v2-quality', 'candidate', 'skill-version-book-research-v1',
     '加入研究计划、来源分层、证据矩阵和差距分析。', CURRENT_TIMESTAMP),
    ('skill-version-book-presentation-v2', 'skill-book-presentation', 2, 'builtin-book-presentation-v2-quality', 'candidate', 'skill-version-book-presentation-v1',
     '加入受众目标、叙事主线、信息密度和证据页脚要求。', CURRENT_TIMESTAMP);

INSERT OR IGNORE INTO skill_files
    (skill_version_id, relative_path, media_type, content_text, content_hash, size_bytes)
VALUES
    ('skill-version-book-ingest-v2', 'SKILL.md', 'text/markdown',
     '# 语义建库\n\n先检索既有 Wiki，再按稳定主题跨章节归并。对每个结果明确分为：新增、更新、争议、无实质变化。更新必须沿用既有 slug；争议必须并列保存相互冲突的条件和来源，不得强行消解；无高价值变化时明确返回无实质变化。每个条目和原子论断都必须保留当前来源引用，引用必须足以支持对应事实。不要逐段摘要，不得覆盖人工保护内容，不得扩大工具权限。输出前检查别名去重、论断原子性、关系方向和引用完整性。',
     'builtin-book-ingest-v2-quality', 588),
    ('skill-version-book-query-v2', 'SKILL.md', 'text/markdown',
     '# 书内问答\n\n先把复杂问题拆解成可验证的子问题，逐项检索书内条目和来源，再综合成直接回答。每个事实性结论必须带来源引用；区分原文事实、跨来源综合和推断。证据覆盖不足时明确写出“证据不足”，来源冲突时并列说明冲突条件和各自依据，不得用模型常识补齐书中不存在的事实。结尾给出最关键的证据与仍待确认的问题。',
     'builtin-book-query-v2-quality', 516),
    ('skill-version-book-research-v2', 'SKILL.md', 'text/markdown',
     '# 专题研究\n\n先提出研究计划：核心问题、子问题、所需证据和停止条件。建立证据矩阵后再综合结论，明确区分书内来源、获准的外部来源和分析推断。输出包含执行摘要、核心发现、证据链、分歧、知识差距与待验证问题。每个关键发现都要可追溯到来源；相互矛盾的材料必须保留条件差异。没有足够证据时缩小结论，不得编造数据或引用。',
     'builtin-book-research-v2-quality', 538),
    ('skill-version-book-presentation-v2', 'SKILL.md', 'text/markdown',
     '# 演示文稿\n\n先定义受众、演示目标和一句话主线，再设计封面、问题、证据、洞察、行动与总结的页面层级。每页只承载一个主要观点，控制信息密度；复杂证据优先使用比较、流程或关系结构。关键数字与事实必须保留来源，讲者备注列出完整依据和限定条件。禁止编造素材或数据。生成器只负责渲染，Skill 负责内容结构；输出前检查标题可扫描性、页面节奏、来源完整性和结论一致性。',
     'builtin-book-presentation-v2-quality', 574);

INSERT OR IGNORE INTO skill_version_origins
    (skill_version_id, repository_url, source_path, source_ref, license_spdx, attribution,
     adaptation_notes, reviewed_at)
VALUES
    ('skill-version-book-ingest-v2', 'https://github.com/NousResearch/hermes-agent',
     'website/docs/user-guide/skills/bundled/research/research-llm-wiki.md', 'main', 'MIT',
     'NousResearch Hermes Agent LLM Wiki workflow',
     '仅吸收分阶段研究、证据驱动和可恢复工作流思想，指令为本项目重新编写。', CURRENT_TIMESTAMP),
    ('skill-version-book-query-v2', 'https://github.com/Weizhena/deep-research-skills',
     '', 'main', 'MIT', 'Weizhena deep-research-skills',
     '借鉴证据优先、差距分析和可追溯输出方法，按本地书籍问答边界重新编写。', CURRENT_TIMESTAMP),
    ('skill-version-book-research-v2', 'https://github.com/Weizhena/deep-research-skills',
     '', 'main', 'MIT', 'Weizhena deep-research-skills',
     '借鉴研究规划、证据矩阵和多来源综合方法，按 DeepSeek Harness 工具边界重新编写。', CURRENT_TIMESTAMP),
    ('skill-version-book-presentation-v2', 'https://github.com/anthropics/skills',
     'skills/slides', 'main', 'Reference-Only', 'Anthropic skills repository',
     '上游演示文稿技能不是可直接再分发的开源许可；仅作质量参考，当前指令由本项目独立编写。', CURRENT_TIMESTAMP);
