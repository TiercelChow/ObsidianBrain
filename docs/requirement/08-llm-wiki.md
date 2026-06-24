# LLM Wiki 知识引擎 — 需求设计文档

> **文档编号**: 08 | **版本**: v1.0 | **状态**: 需求分析 | **日期**: 2026-06-24
>
> **灵感来源**: [Karpathy 的 LLM Wiki 模式](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)
>
> **上游依赖**: [顶层设计](../top_design.md) · [工具协议](02-tool-protocol.md) · [知识库洞察](03-memory-engine.md)

---

## 1. 背景与动机

### 1.1 Karpathy 的 LLM Wiki 思想

Andrej Karpathy 提出了一种与 RAG 截然不同的知识管理模式：

> **RAG**：每次查询时从原始文档检索片段，LLM 从零开始推导。没有积累，没有复利。
>
> **LLM Wiki**：LLM 增量地构建和维护一个持久的、交叉引用的 Markdown Wiki。新源加入时，LLM 不只是索引它，而是读取、提取关键信息、集成到已有 Wiki 中——更新实体页、修订主题摘要、标注矛盾、强化综合论述。知识编译一次，然后持续保持最新。

核心比喻：**"Obsidian 是 IDE，LLM 是程序员，Wiki 是代码库。"**

三层架构：
- **Raw Sources**（原始资料）：不可变的信息源，LLM 只读
- **The Wiki**（知识 Wiki）：LLM 完全拥有的 Markdown 文件目录，人类只读
- **The Schema**（维护规则）：CLAUDE.md 等配置文件，告诉 LLM 如何维护 Wiki

三种操作：
- **Ingest**（摄入）：新资料 → LLM 读取 → 写摘要 → 更新 index → 更新 10-15 个相关页 → 记录到 log
- **Query**（查询）：提问 → LLM 读 index 找相关页 → 综合回答+引用 → 好的回答归档为新 Wiki 页
- **Lint**（体检）：定期健康检查 → 找矛盾、过时信息、孤岛页、缺失引用

### 1.2 与 ObsidianBrain 的结合点

ObsidianBrain 已有的能力恰好是 LLM Wiki 模式的基础设施：

| LLM Wiki 需要的能力 | ObsidianBrain 已有 |
|---|---|
| 读写 Vault 中的 Markdown 文件 | ObsidianClient（REST API 读写笔记） |
| LLM 调用（摘要、综合、问答） | LlmClient（OpenAI/Claude/Ollama） |
| 工具协议（让 LLM 操作文件） | Tool API（Axum + ToolRegistry） |
| 知识健康度分析 | KnowledgeInsightEngine（孤岛、枢纽、尘封） |
| 原始资料来源 | 智识雷达（RSS/arXiv 纳藏到 Vault） |
| 碎片记录 | 时光机（小记存储到 Vault） |

**缺口**：ObsidianBrain 目前是 LLM 的"手和眼"——帮 LLM 读写文件。但缺少一个**让 LLM 成为知识维护者**的编排层：自动摄入、交叉引用、矛盾检测、索引维护。

### 1.3 目标

在 ObsidianBrain 中实现 LLM Wiki 引擎，让 LLM 从"工具调用者"升级为"知识维护者"：

- 用户投入新资料 → LLM 自动读取、摘要、交叉引用、更新 Wiki 页面
- 用户提问 → LLM 基于已编译的 Wiki 综合回答（非 RAG 式从零检索）
- 定期 Lint → LLM 主动发现矛盾、孤岛、过时信息
- 所有 Wiki 文件存储在 Obsidian Vault 中，用户可在 Obsidian 中浏览图谱

---

## 2. 数据模型

### 2.1 目录结构

在 Obsidian Vault 中新增 `Wiki/` 目录：

```
Vault/
├── Wiki/                    ← LLM Wiki 引擎维护的目录
│   ├── index.md             ← 内容目录（所有 Wiki 页的索引）
│   ├── log.md               ← 操作日志（摄入/查询/Lint 记录）
│   ├── schema.md            ← 维护规则（LLM 的"CLAUDE.md"）
│   ├── entities/            ← 实体页（人物、项目、概念等）
│   │   ├── andrej-karpathy.md
│   │   └── obsidian-brain.md
│   ├── concepts/            ← 概念页（技术主题、理论等）
│   │   ├── llm-os.md
│   │   └── second-brain.md
│   ├── sources/             ← 源摘要页（每篇原始资料的摘要）
│   │   ├── 2026-06-24-karpathy-llm-wiki.md
│   │   └── 2026-06-20-ai-infra-article.md
│   └── synthesis/           ← 综合论述页（跨源分析、对比、综述）
│       └── knowledge-management-evolution.md
├── Raw/                     ← 原始资料（不可变，用户/雷达纳入）
│   ├── articles/
│   │   ├── 2026-06-24-karpathy-llm-wiki.md
│   │   └── 2026-06-20-ai-infra-article.md
│   └── assets/
│       └── image-001.png
├── Timeline/                ← 时光机（已有）
├── notes/                   ← 用户原有笔记（已有）
└── ...
```

### 2.2 页面格式

**实体页**（`Wiki/entities/xxx.md`）：
```markdown
---
type: entity
name: Andrej Karpathy
tags: [AI, LLM, researcher]
sources: [2026-06-24-karpathy-llm-wiki]
created: 2026-06-24
updated: 2026-06-24
---

# Andrej Karpathy

前 Tesla AI 总监，OpenAI 创始成员。提出 "LLM OS" 概念和 LLM Wiki 知识管理模式。

## 主要观点

- LLM 正在成为新型操作系统的内核
- 上下文窗口 = RAM，外部知识库 = 文件系统
- LLM Wiki 模式：LLM 增量维护持久知识库，而非每次 RAG 从零检索

## 相关概念

- [[llm-os]]
- [[second-brain]]
```

**源摘要页**（`Wiki/sources/xxx.md`）：
```markdown
---
type: source
source_path: Raw/articles/2026-06-24-karpathy-llm-wiki.md
source_type: article
source_url: https://gist.github.com/karpathy/...
ingested: 2026-06-24
entities: [andrej-karpathy]
concepts: [llm-os, second-brain, llm-wiki]
---

# 摘要：LLM Wiki — 用 LLM 构建个人知识库

## 核心观点

与 RAG 不同，LLM Wiki 模式让 LLM 增量构建持久的知识 Wiki...
（200-500 字摘要）

## 关键实体

- [[andrej-karpathy]]：提出者
- [[llm-os]]：相关概念

## 关键概念

- [[second-brain]]：知识管理模式
- [[llm-wiki]]：本文核心模式
```

**index.md**：
```markdown
# Wiki 索引

最后更新：2026-06-24
总页数：15 · 源摘要：3 · 实体：5 · 概念：4 · 综合论述：2

## 实体

- [[andrej-karpathy]] — AI 研究者，LLM OS 提出者
- [[obsidian-brain]] — 本项目

## 概念

- [[llm-os]] — LLM 作为操作系统内核
- [[second-brain]] — 个人知识管理系统
- [[llm-wiki]] — LLM 维护的持久知识库

## 源摘要

- [[2026-06-24-karpathy-llm-wiki]] — Karpathy 的 LLM Wiki 模式
- [[2026-06-20-ai-infra-article]] — AI Infra 基础系列

## 综合论述

- [[knowledge-management-evolution]] — 知识管理演进
```

**log.md**：
```markdown
# Wiki 操作日志

## [2026-06-24] ingest | Karpathy LLM Wiki
- 来源：Raw/articles/2026-06-24-karpathy-llm-wiki.md
- 新建页面：sources/2026-06-24-karpathy-llm-wiki.md
- 更新页面：entities/andrej-karpathy.md, concepts/llm-os.md, concepts/second-brain.md, concepts/llm-wiki.md, index.md
- 新建页面：concepts/llm-wiki.md

## [2026-06-24] query | LLM 和 RAG 的区别是什么？
- 检索页面：concepts/llm-wiki.md, sources/2026-06-24-karpathy-llm-wiki.md
- 回答已归档：synthesis/llm-wiki-vs-rag.md

## [2026-06-24] lint | Wiki 健康检查
- 检查页面：15
- 发现孤岛：2（concepts/llm-wiki.md 缺少入链，已修复）
- 发现矛盾：0
- 建议新源：Van Bush Memex 原始论文
```

### 2.3 Schema 文件

`Wiki/schema.md` 是 LLM 维护 Wiki 的规则手册，类似 CLAUDE.md：

```markdown
# LLM Wiki 维护规则

## 目录结构
- Wiki/entities/ — 实体页（人物、项目、工具）
- Wiki/concepts/ — 概念页（技术主题、理论）
- Wiki/sources/ — 源摘要页（每篇原始资料的摘要）
- Wiki/synthesis/ — 综合论述页（跨源分析）

## 命名规范
- 文件名：kebab-case，如 `andrej-karpathy.md`
- 源摘要：`YYYY-MM-DD-{简短描述}.md`

## Ingest 流程
1. 读取原始资料全文
2. 与用户讨论关键要点（1-3 轮对话）
3. 写源摘要页（200-500 字）
4. 提取实体和概念，创建或更新对应页面
5. 更新交叉引用（[[wikilink]]）
6. 更新 index.md
7. 追加 log.md

## Query 流程
1. 先读 index.md 找相关页面
2. 读取相关页面内容
3. 综合回答，附带 [[引用]]
4. 如果回答有价值，归档为 synthesis/ 新页面

## Lint 流程
1. 检查所有页面的 frontmatter 完整性
2. 检查交叉引用双向性（A 引用 B，B 应也引用 A）
3. 检查孤岛页（无入链的页面）
4. 检查矛盾声明（不同页面中的冲突信息）
5. 建议新探索方向
```

---

## 3. 工具接口

### 3.1 ingest_source — 摄入原始资料

```json
{
  "name": "ingest_source",
  "description": "将一篇原始资料摄入 Wiki：LLM 读取、摘要、提取实体/概念、更新交叉引用",
  "inputSchema": {
    "type": "object",
    "properties": {
      "source_path": {
        "type": "string",
        "description": "Vault 中的原始资料路径（如 Raw/articles/xxx.md）"
      },
      "source_type": {
        "type": "string",
        "enum": ["article", "paper", "book_chapter", "podcast", "meeting", "note"],
        "default": "article"
      },
      "source_url": {
        "type": "string",
        "description": "原始 URL（如有）"
      },
      "auto_update": {
        "type": "boolean",
        "default": true,
        "description": "是否自动更新相关 Wiki 页面"
      }
    },
    "required": ["source_path"]
  }
}
```

**行为**：
1. 通过 ObsidianClient 读取 `source_path` 的全文
2. 调用 LLM 生成摘要（200-500 字），提取实体列表和概念列表
3. 在 `Wiki/sources/` 创建源摘要页
4. 对每个提取的实体/概念，在 `Wiki/entities/` 或 `Wiki/concepts/` 中创建或更新页面
5. 更新所有相关页面的交叉引用
6. 更新 `index.md`
7. 追加 `log.md`
8. 返回：新建/更新的页面列表

**返回**：
```json
{
  "summary_page": "Wiki/sources/2026-06-24-karpathy-llm-wiki.md",
  "created_pages": ["Wiki/concepts/llm-wiki.md"],
  "updated_pages": [
    "Wiki/entities/andrej-karpathy.md",
    "Wiki/concepts/llm-os.md",
    "Wiki/index.md",
    "Wiki/log.md"
  ],
  "entities_extracted": ["andrej-karpathy"],
  "concepts_extracted": ["llm-os", "second-brain", "llm-wiki"]
}
```

### 3.2 query_wiki — 知识查询

```json
{
  "name": "query_wiki",
  "description": "基于已编译的 Wiki 回答问题（非 RAG，直接读 Wiki 页面综合）",
  "inputSchema": {
    "type": "object",
    "properties": {
      "question": {
        "type": "string",
        "description": "用户的问题"
      },
      "save_answer": {
        "type": "boolean",
        "default": false,
        "description": "是否将回答归档为新的 synthesis 页面"
      }
    },
    "required": ["question"]
  }
}
```

**行为**：
1. 读取 `Wiki/index.md` 获取所有页面列表
2. LLM 根据 index 判断哪些页面与问题相关
3. 读取相关页面的完整内容
4. LLM 基于这些页面综合回答，附带 `[[引用]]`
5. 如果 `save_answer=true`，将回答保存为 `Wiki/synthesis/` 新页面
6. 追加 `log.md`

**返回**：
```json
{
  "answer": "根据你的 Wiki，LLM Wiki 和 RAG 的核心区别在于...",
  "cited_pages": [
    "Wiki/concepts/llm-wiki.md",
    "Wiki/sources/2026-06-24-karpathy-llm-wiki.md"
  ],
  "saved_to": "Wiki/synthesis/llm-wiki-vs-rag.md"
}
```

### 3.3 lint_wiki — Wiki 健康检查

```json
{
  "name": "lint_wiki",
  "description": "检查 Wiki 健康度：孤岛页、矛盾、缺失引用、过时信息",
  "inputSchema": {
    "type": "object",
    "properties": {
      "auto_fix": {
        "type": "boolean",
        "default": false,
        "description": "是否自动修复可修复的问题（如添加缺失引用）"
      }
    }
  }
}
```

**行为**：
1. 列出 `Wiki/` 下所有 `.md` 文件
2. 检查每个文件的 frontmatter 完整性
3. 构建引用图谱，找出孤岛页（无入链）
4. LLM 分析潜在矛盾（不同页面中的冲突信息）
5. 找出被提及但没有独立页面的概念
6. 如果 `auto_fix=true`，自动为孤岛页添加引用、为缺失概念创建页面
7. 追加 `log.md`

**返回**：
```json
{
  "total_pages": 15,
  "issues": {
    "orphans": ["Wiki/concepts/some-concept.md"],
    "contradictions": [],
    "missing_pages": ["memex"],
    "stale_info": []
  },
  "fixed": 2,
  "suggestions": [
    "建议摄入 Vannevar Bush 的 Memex 原始论文以完善 second-brain 概念"
  ]
}
```

### 3.4 get_wiki_status — Wiki 状态

```json
{
  "name": "get_wiki_status",
  "description": "获取 Wiki 当前状态：页数、最后操作、索引状态",
  "inputSchema": {
    "type": "object",
    "properties": {}
  }
}
```

**返回**：
```json
{
  "total_pages": 15,
  "entities": 5,
  "concepts": 4,
  "sources": 3,
  "synthesis": 2,
  "last_ingest": "2026-06-24",
  "last_query": "2026-06-24",
  "last_lint": "2026-06-24",
  "raw_sources_count": 5
}
```

---

## 4. 前端界面

### 4.1 新增「Wiki」页面

在侧边栏新增「Wiki」入口，页面包含：

**状态卡片**：
- Wiki 总页数、实体数、概念数、源摘要数
- 最后摄入/查询/Lint 时间
- 「摄入资料」「查询 Wiki」「Lint 检查」三个操作按钮

**Wiki 图谱预览**：
- 调用已有的 `get_knowledge_insights` 分析 `Wiki/` 目录
- 显示 Wiki 内部的链接图谱（孤岛、枢纽、领域分布）

**最近操作**（从 log.md 解析）：
- 时间线展示最近的 ingest/query/lint 操作
- 每条操作可展开查看影响的页面列表

### 4.2 摄入对话框

- 输入：Vault 中的原始资料路径（或从 Raw/ 目录选择）
- 选项：资料类型、URL、是否自动更新
- 摄入过程显示进度：读取中 → 摘要中 → 提取实体中 → 更新页面中
- 完成后展示：新建/更新的页面列表，可点击在 Obsidian 中打开

### 4.3 知识查询

- 聊天式界面（类似知识对话，但基于 Wiki 而非 RAG）
- 回答附带引用来源（`[[wikilink]]` 格式，可点击）
- 「归档为综合论述」按钮

### 4.4 Lint 报告

- 问题列表：孤岛页、矛盾、缺失页面、过时信息
- 每个问题可展开查看详情
- 「自动修复」按钮

---

## 5. 与现有模块的集成

| 现有模块 | 集成方式 |
|---------|---------|
| **智识雷达** | 雷达纳藏的文章自动放入 `Raw/articles/`，可一键 ingest 到 Wiki |
| **时光机** | 小记可作为碎片化原始资料 ingest 到 Wiki |
| **灵感熔炉** | 概念碰撞时从 Wiki 的 concepts/ 目录选取概念 |
| **知识库洞察** | 已有的孤岛/枢纽/尘封分析直接应用于 `Wiki/` 目录 |
| **搜索** | `search_notes` 可搜索 `Wiki/` 目录下的页面 |
| **配置** | LLM Wiki 的 schema.md 可在首页配置面板编辑 |

---

## 6. 技术实现

### 6.1 LLM Wiki 引擎

```
backend/src/core/wiki/
├── mod.rs              ← 模块入口
├── engine.rs           ← WikiEngine：ingest/query/lint 编排
├── page_writer.rs      ← Wiki 页面创建/更新（通过 ObsidianClient）
├── index_manager.rs    ← index.md 和 log.md 维护
└── link_graph.rs       ← 交叉引用图谱构建与分析
```

### 6.2 Ingest 流程详解

```
1. read_source(path) → 获取原始资料全文
2. llm.summarize(content) → 生成 200-500 字摘要
3. llm.extract_entities(content) → 提取实体列表
4. llm.extract_concepts(content) → 提取概念列表
5. create_source_page(summary, entities, concepts) → 写源摘要页
6. for each entity:
     if page_exists(entity): update_page(entity, new_source_ref)
     else: create_entity_page(entity)
7. for each concept:
     if page_exists(concept): update_page(concept, new_source_ref)
     else: create_concept_page(concept)
8. update_cross_references() → 在相关页面间添加 [[wikilink]]
9. update_index() → 更新 index.md
10. append_log("ingest", source_path, affected_pages) → 追加 log.md
```

### 6.3 Query 流程详解

```
1. read_index() → 获取所有 Wiki 页面列表+摘要
2. llm.select_relevant_pages(question, index) → LLM 判断哪些页面相关
3. for each relevant_page: read_page(page) → 读取完整内容
4. llm.synthesize_answer(question, page_contents) → 综合回答+引用
5. if save_answer: create_synthesis_page(answer) → 归档
6. append_log("query", question, cited_pages)
```

### 6.4 Lint 流程详解

```
1. list_wiki_files() → 列出 Wiki/ 下所有 .md 文件
2. for each file: parse_frontmatter() → 检查完整性
3. build_link_graph() → 构建 [[wikilink]] 引用图谱
4. find_orphans(graph) → 找无入链的页面
5. find_missing_pages(graph) → 被提及但无独立页面的概念
6. llm.detect_contradictions(pages) → LLM 分析矛盾
7. if auto_fix:
     for each orphan: suggest_and_add_links()
     for each missing: create_stub_page()
8. append_log("lint", issues_found, fixed_count)
```

---

## 7. 工具注册

新增 4 个工具，注册到 `tools/handlers/wiki_handlers.rs`：

| 工具名 | 模块 | 说明 |
|--------|------|------|
| `ingest_source` | wiki | 摄入原始资料到 Wiki |
| `query_wiki` | wiki | 基于 Wiki 回答问题 |
| `lint_wiki` | wiki | Wiki 健康检查 |
| `get_wiki_status` | wiki | 获取 Wiki 状态 |

---

## 8. 验收标准

### 8.1 功能验收

- [ ] 输入一篇原始资料路径，LLM 自动生成摘要并创建/更新 Wiki 页面
- [ ] 摄入后 index.md 和 log.md 正确更新
- [ ] 提问后 LLM 基于 Wiki 页面综合回答，附带引用
- [ ] 回答可归档为 synthesis 页面
- [ ] Lint 能发现孤岛页、缺失页面、矛盾
- [ ] auto_fix 能自动修复引用缺失
- [ ] Wiki 文件在 Obsidian 中可正常浏览，图谱视图显示连接

### 8.2 与现有模块集成验收

- [ ] 智识雷达纳藏的文章可一键 ingest
- [ ] 知识库洞察页面可分析 Wiki/ 目录
- [ ] 灵感熔炉可从 Wiki concepts/ 选取概念

---

## 9. 实施路线

### Phase 1: 基础引擎 + Ingest
- `WikiEngine` 核心结构
- `ingest_source` 工具（读取→摘要→提取→写页面→更新index/log）
- Schema 文件初始化
- 前端摄入对话框

### Phase 2: Query + Lint
- `query_wiki` 工具（读index→选页→综合回答→归档）
- `lint_wiki` 工具（引用图谱→孤岛→矛盾→修复）
- 前端查询聊天界面 + Lint 报告

### Phase 3: 深度集成
- 智识雷达 → 一键 ingest
- 时光机小记 → 批量 ingest
- 灵感熔炉 → 从 Wiki 选取概念
- 首页 Wiki 状态卡片

---

## 10. 修订历史

| 版本 | 日期 | 内容 |
|------|------|------|
| v1.0 | 2026-06-24 | 初始需求分析，基于 Karpathy LLM Wiki 模式 |
