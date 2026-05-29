# 记忆引擎 (Memory Engine) — 需求设计文档

> **模块编号**: 03 | **版本**: v1.0 | **状态**: 设计中 | **关联**: [顶层设计文档](../top_design.md §5.1)

---

## 1. 模块概述

记忆引擎是 ObsidianBrain 的**核心中枢**，承担知识的自动采集、组织、检索与管理职责。它将用户的 Obsidian Vault 从被动的文件存储，升级为可被 LLM 主动查询和操作的**活体知识库**。

### 1.1 核心定位

```
         用户写作/编辑笔记
               │
               ▼
        ┌─────────────┐
        │  文件监控层   │ ← notify 实时感知变更
        └──────┬──────┘
               ▼
        ┌─────────────┐
        │  记忆引擎    │ ← 自动解析、分块、索引、向量化
        │  (本模块)    │
        └──────┬──────┘
               │
     ┌─────────┼─────────┐
     ▼         ▼         ▼
  全文索引   向量索引   元数据存储
 (Tantivy)  (Qdrant)   (SQLite)
```

### 1.2 核心价值

| 能力 | 说明 |
|---|---|
| **自动索引** | Vault 文件变更时无需人工干预，自动完成解析、分块、索引全流程 |
| **语义检索** | 超越关键词匹配，理解查询意图，找到语义相关的记忆片段 |
| **混合搜索** | 全文检索 + 语义检索双通道融合，兼顾精确匹配与模糊理解 |
| **记忆管理** | 完整 CRUD 操作，LLM 可主动添加、修改、删除记忆 |
| **引用溯源** | 每条记忆均可追溯到原始笔记，支持一键跳转 Obsidian 编辑器 |

### 1.3 与其他模块的关系

- **文件监控层** (`infra/file_watcher.rs`)：提供文件变更事件，触发记忆引擎的索引更新流程
- **时间线模块** (`core/timeline.rs`)：记忆引擎的索引事件（新增/修改/删除）作为时间线事件源
- **智识雷达** (`core/radar.rs`)：雷达纳藏的文章写入 Vault 后，由记忆引擎自动索引
- **灵感熔炉** (`core/inspiration.rs`)：灵感生成时查询记忆引擎获取相关笔记片段
- **API 层** (`api/handlers/`)：将记忆操作暴露为标准 Tool API

---

## 2. 功能需求

### 2.1 自动提取 (Auto Extraction)

**需求描述**：当 Vault 中的 Markdown 文件发生变更（新增、修改、删除）时，系统自动完成记忆单元的提取与索引更新，无需用户手动操作。

**详细流程**：

1. **变更感知**：文件监控层捕获文件系统的 Create / Modify / Remove 事件
2. **防抖聚合**：300ms 内的同一文件多次变更合并为一次处理
3. **Markdown 解析**：提取 frontmatter 元数据 + 正文内容
4. **智能分块**：将正文按规则拆分为多个 Chunk（记忆单元）
5. **索引更新**：
   - 删除该文件对应的旧 Chunk 索引
   - 为新 Chunk 建立全文索引（Tantivy）
   - 批量生成 Embedding 向量
   - 写入向量索引（Qdrant upsert）
6. **元数据记录**：更新 SQLite 中的文件状态与 Chunk 映射

**触发条件**：

| 事件类型 | 处理方式 |
|---|---|
| 文件新增 | 完整解析 → 分块 → 全量索引 |
| 文件修改 | 删除旧索引 → 重新解析 → 重建索引 |
| 文件删除 | 删除该文件所有关联的全文索引 + 向量索引 |
| 文件重命名 | 等同于删除 + 新增 |
| 目录变更 | 递归处理目录下所有文件 |

### 2.2 智能分块 (Smart Chunking)

**需求描述**：将 Markdown 正文按语义边界拆分为合适大小的记忆单元，保证每个 Chunk 既包含完整的语义信息，又不会过大影响检索精度。

**分块规则**：

1. **标题层级分割**（优先级从高到低）：
   - 首先按 H1 (`#`) 分割为一级区块
   - 每个一级区块内按 H2 (`##`) 分割
   - 每个二级区块内按 H3 (`###`) 分割
   - 更深层标题（H4-H6）不再分割，作为内容的一部分

2. **段落边界分割**：
   - 当某个标题区块超过 `chunk_max_tokens`（默认 800 token）时
   - 按段落（空行分隔）边界进行二次分割
   - 每个子块目标大小：`chunk_min_tokens` ~ `chunk_max_tokens`

3. **代码块保护**：
   - 围栏代码块（\`\`\` ... \`\`\`）视为不可分割单元
   - 即使代码块超过 `chunk_max_tokens`，也不在代码块内部切割
   - 代码块前后保留至少一个段落的上下文

4. **上下文保留**：
   - 每个 Chunk 附带其所在的标题路径（breadcrumb）
   - 格式：`# 一级标题 > ## 二级标题 > ### 三级标题`
   - 相邻 Chunk 之间保留 1-2 句重叠文本，避免语义断裂

**配置参数**：

| 参数 | 默认值 | 说明 |
|---|---|---|
| `chunk_min_tokens` | 300 | 最小 Chunk token 数 |
| `chunk_max_tokens` | 800 | 最大 Chunk token 数 |
| `chunk_overlap_sentences` | 1 | 相邻 Chunk 重叠句数 |

### 2.3 全文检索 (Full-text Search)

**需求描述**：基于 Tantivy 提供高性能的关键词全文检索能力，支持中文分词。

**功能要求**：

1. **索引构建**：
   - 使用 Tantivy 建立倒排索引
   - 中文内容使用 jieba 分词器进行分词
   - 英文内容使用标准英文分词器（小写化 + 词干提取）
   - 索引字段包括：内容、标签、来源笔记路径、标题路径

2. **查询能力**：
   - 支持关键词搜索（AND/OR 布尔查询）
   - 支持短语搜索（引号包裹的精确短语）
   - 支持标签过滤
   - BM25 评分排序

3. **性能要求**：
   - 索引常驻内存，搜索延迟 < 50ms
   - 支持增量更新（单文件变更不触发全量重建）

### 2.4 语义检索 (Semantic Search)

**需求描述**：基于 Qdrant 向量数据库提供语义相似度检索能力，弥补关键词检索在语义理解上的不足。

**功能要求**：

1. **向量化**：
   - 使用配置的 Embedding 模型（默认 OpenAI text-embedding-3-small）将 Chunk 转为向量
   - 向量维度由模型决定（text-embedding-3-small 为 1536 维）
   - 批量调用 Embedding API，减少请求次数

2. **搜索能力**：
   - 查询文本先向量化，再在 Qdrant 中做近邻搜索
   - 使用余弦相似度（Cosine Similarity）作为距离度量
   - 支持按标签过滤（通过 Qdrant payload filter）
   - 返回 top-K 最相似结果

3. **HNSW 索引参数**（可调优）：
   - `m`: 16（每个节点的最大连接数）
   - `ef_construct`: 100（构建时的搜索宽度）
   - `ef`: 128（搜索时的候选队列大小）

### 2.5 混合检索 (Hybrid Search)

**需求描述**：将全文检索与语义检索的结果融合，取长补短，提供更高质量的搜索结果。

**融合策略 — RRF (Reciprocal Rank Fusion)**：

1. 分别执行全文检索和语义检索，各取 top-20 候选
2. 对两路结果应用 RRF 公式融合：
   ```
   RRF_score(d) = Σ 1 / (k + rank_i(d))
   ```
   其中 `k` 为常数（默认 60），`rank_i(d)` 为文档 d 在第 i 路结果中的排名（从 1 开始）
3. 合并去重后按 RRF 评分降序排列
4. 根据 `importance` 和 `access_count` 做微调（±5%）
5. 返回 `top_k` 条结果

**降级策略**：

| 场景 | 降级方式 |
|---|---|
| Qdrant 不可用 | 仅使用 Tantivy 全文检索 |
| Embedding API 超时 | 仅使用 Tantivy 全文检索 |
| 全文索引异常 | 仅使用 Qdrant 语义检索 |

### 2.6 记忆 CRUD 操作

记忆引擎对外暴露以下 Tool API，供 LLM 直接调用：

#### 2.6.1 `search_memory`

- **输入**：`query: string, top_k?: int (默认5), tags?: string[]`
- **输出**：`Memory[]` — 匹配的记忆单元列表
- **说明**：混合检索（全文 + 语义），RRF 融合排序。每条结果附带来源笔记路径、Obsidian URI、相关度评分
- **副作用**：被访问的记忆 `access_count` +1

#### 2.6.2 `add_memory`

- **输入**：`note_path: string, content: string, tags?: string[]`
- **输出**：`Memory` — 新创建的记忆单元
- **说明**：
  1. 在指定笔记末尾追加内容（以分隔符标记为记忆段落）
  2. 对新内容分块、生成 Embedding、写入索引
  3. 如果笔记不存在，自动创建
- **副作用**：触发时间线 `MemoryCreated` 事件

#### 2.6.3 `update_memory`

- **输入**：`memory_id: string, content: string`
- **输出**：`Memory` — 更新后的记忆单元
- **说明**：
  1. 根据 memory_id 查找对应记忆
  2. 更新来源笔记中的对应段落
  3. 重新生成 Embedding，更新全文索引和向量索引

#### 2.6.4 `forget_memory`

- **输入**：`memory_id: string`
- **输出**：`bool` — 是否删除成功
- **说明**：
  1. 从 Tantivy 删除对应文档
  2. 从 Qdrant 删除对应向量
  3. 从来源笔记中移除对应段落（可选，通过 `remove_from_note` 参数控制）

#### 2.6.5 `get_memory_stats`

- **输入**：无
- **输出**：`{ total: int, by_tag: HashMap<String, int>, recent: Memory[], index_size_mb: f64 }`
- **说明**：返回记忆库的统计概览——总数、标签分布、最近创建的记忆、索引占用空间

### 2.7 引用溯源 (Citation & Provenance)

**需求描述**：每条搜索结果都必须携带完整的来源信息，让用户可以一键跳转到原始笔记。

**溯源信息包含**：

1. **笔记路径**：Vault 内相对路径（如 `programming/rust-async.md`）
2. **Obsidian URI**：`obsidian://open?vault=<vault_name>&file=<encoded_path>`
3. **标题路径**：Chunk 所在的标题层级（如 `## 异步编程 > ### tokio select!`）
4. **行号范围**：Chunk 在原文中的大致行号范围（`line_start` ~ `line_end`）

### 2.8 笔记搜索工具

除记忆级别的搜索外，还提供笔记级别的搜索工具：

#### 2.8.1 `search_notes`

- **输入**：`query: string, top_k?: int, tags?: string[]`
- **输出**：`NoteSearchResult[]` — 匹配的笔记列表，每篇附带最佳匹配片段
- **说明**：与 `search_memory` 共享检索引擎，但结果按笔记聚合（同一笔记的多个 Chunk 合并展示）

#### 2.8.2 `get_note`

- **输入**：`path: string`
- **输出**：`Note` — 笔记完整内容（含 frontmatter、正文、标签）
- **说明**：直接从文件系统读取，不经过索引

#### 2.8.3 `list_recent_notes`

- **输入**：`days?: int (默认7), limit?: int (默认20)`
- **输出**：`NoteSummary[]` — 最近修改的笔记摘要列表
- **说明**：基于文件修改时间排序，返回标题、路径、修改时间、标签

---

## 3. 用户故事

### US-01：写作时自动建立索引

**角色**：知识工作者小明
**场景**：小明在 Obsidian 中新建了一篇笔记 `机器学习/transformer-architecture.md`，写下了 Transformer 架构的学习心得。
**期望**：
- 保存笔记后 1 秒内，记忆引擎自动完成该笔记的解析、分块、索引
- 无需任何手动操作，新笔记的内容即可被后续的搜索查询到
- 如果笔记包含代码示例，代码块不会被切断

**验收条件**：
- 新增笔记后，`search_memory("transformer")` 能返回该笔记的相关 Chunk
- Chunk 的代码块内容完整，没有在 \`\`\` 内部被截断

### US-02：通过 LLM 搜索历史知识

**角色**：小明在与 Claude 对话
**场景**：小明问 Claude："我之前记过关于 Rust 生命周期的一些笔记，能帮我找一下吗？"
**期望**：
- Claude 调用 `search_memory(query="Rust 生命周期", top_k=5)`
- 返回的记忆片段包含精确匹配"生命周期"关键词的结果，也包含语义相关（如 "borrow checker"、"引用"）的结果
- 每条结果附带 Obsidian URI，小明可以直接点击跳转到原文

**验收条件**：
- 搜索结果中既有包含"生命周期"字面量的 Chunk，也有语义相关但不含该关键词的 Chunk
- 每条结果的 `obsidian_uri` 字段非空，URI 格式正确
- 搜索延迟 < 500ms

### US-03：修改笔记后索引自动更新

**角色**：小明在修改一篇已有笔记
**场景**：小明打开 `programming/rust-async.md`，在 "tokio select!" 章节新增了两段说明文字并保存。
**期望**：
- 旧版本对应的 Chunk 索引被清除
- 新版本重新分块后建立索引
- 未修改的章节不会重复生成 Embedding（增量优化）
- 搜索该笔记时，返回的是最新版本的内容

**验收条件**：
- 修改后 `search_memory("tokio select")` 返回更新后的内容
- 不会搜索到旧版本的内容（无重复/过期结果）

### US-04：让 LLM 添加新记忆

**角色**：小明在对话中产生了一个新想法
**场景**：小明对 Claude 说："帮我记一下：在 Rust 中使用 Arc<Mutex<T>> 时，要注意避免死锁，尤其是在异步上下文中。把它加到我的 Rust 并发笔记里。"
**期望**：
- Claude 调用 `add_memory(note_path="programming/rust-concurrency.md", content="...", tags=["rust", "concurrency"])`
- 笔记文件被追加了新内容
- 新内容被索引，后续可搜索到
- 时间线记录了此事件

**验收条件**：
- `get_note("programming/rust-concurrency.md")` 可以看到追加的内容
- `search_memory("Arc Mutex 死锁")` 能返回该记忆
- `get_timeline` 中可以看到 `MemoryCreated` 事件

### US-05：删除过时记忆

**角色**：小明发现一篇旧笔记中的信息已过时
**场景**：小明对 Claude 说："我之前记的那条关于 Python 2 的 print 语句的记忆已经过时了，帮我删掉。"
**期望**：
- Claude 先搜索找到对应记忆，获取 memory_id
- 调用 `forget_memory(memory_id="xxx")`
- 该记忆从全文索引和向量索引中移除
- 后续搜索不再返回该记忆

**验收条件**：
- 删除后 `search_memory` 不再返回该 memory_id 对应的结果
- `get_memory_stats` 的 total 计数减少

### US-06：中文笔记的精准搜索

**角色**：小明有大量中文笔记
**场景**：小明搜索"分布式系统一致性"，希望找到讨论 Raft 共识算法的笔记，即使笔记中没有出现"一致性"三个字，而是用的"consensus"或"共识"。
**期望**：
- 全文检索通过 jieba 分词正确切分中文
- 语义检索理解"一致性"与"共识"的语义关联
- 混合检索融合两路结果，Raft 相关笔记排在前列

**验收条件**：
- 搜索结果包含不含"一致性"字面量但语义相关的 Chunk
- 中文分词结果合理（不出现无意义的单字切分）

### US-07：代码块完整性保护

**角色**：小明是开发者，笔记中包含大量代码片段
**场景**：一篇笔记包含一个 50 行的 Rust 函数示例，前后有文字说明。
**期望**：
- 分块时整个代码块被保留在同一个 Chunk 中
- 不会将函数的前半部分和后半部分分到不同 Chunk
- 代码块前后的说明文字作为上下文一并保留

**验收条件**：
- 分块结果中，完整的 50 行代码出现在同一个 Chunk 的 content 中
- 代码块的语法标记（\`\`\`rust ... \`\`\`）完整保留

### US-08：查看记忆库状态

**角色**：小明想了解系统当前管理了多少知识
**场景**：小明对 Claude 说："我的知识库里现在有多少条记忆？按标签分布是怎样的？"
**期望**：
- Claude 调用 `get_memory_stats()`
- 返回总数、标签分布（如 "rust: 45, ai: 32, 随笔: 18"）、最近添加的记忆
- Claude 基于统计数据生成自然语言摘要

**验收条件**：
- 统计数据与实际索引内容一致
- 标签分布覆盖所有已索引标签
- 返回的 recent 列表按时间倒序排列

### US-09：大批量 Vault 首次索引

**角色**：小明第一次启动 ObsidianBrain，Vault 中已有 500+ 篇笔记
**期望**：
- 首次启动时自动执行全量索引
- 索引过程不阻塞 API 服务（后台异步进行）
- 索引进度可通过日志或 API 查询
- 全部索引完成后，所有笔记均可搜索

**验收条件**：
- 全量索引完成后 `get_memory_stats` 的 total 与实际 Chunk 数一致
- 索引期间 `search_memory` 可返回已索引部分的结果（渐进可用）
- 索引过程不会导致 OOM 或 CPU 满载

### US-10：外部文章纳藏后自动索引

**角色**：小明通过智识雷达发现了一篇好文章
**场景**：小明调用 `add_to_vault(article_id="xxx")` 将文章保存到 Vault 的 `radar/` 目录。
**期望**：
- 文章保存为 Markdown 文件后，文件监控自动触发记忆引擎索引
- 文章内容被分块、向量化、索引
- 后续可通过 `search_memory` 搜索到该文章的内容

**验收条件**：
- 纳藏文章后 2 秒内，`search_memory` 可搜索到该文章的 Chunk
- 文章的来源信息（URL、标题）保留在 Chunk 的元数据中

---

## 4. 非功能需求

### 4.1 性能需求

| 指标 | 目标值 | 说明 |
|---|---|---|
| 全文搜索延迟 | P95 < 50ms | Tantivy 索引常驻内存 |
| 语义搜索延迟 | P95 < 200ms | Qdrant HNSW 索引 |
| 混合搜索延迟 | P95 < 300ms | 两路并行 + RRF 融合 |
| 文件变更处理 | < 500ms/文件 | 含防抖 300ms + 索引更新 |
| 全量索引吞吐 | ≥ 50 文件/分钟 | 500 篇笔记在 10 分钟内完成 |
| Embedding 批处理 | 单批 ≤ 100 条 | 避免 API 单次请求过大 |

### 4.2 存储效率

| 指标 | 约束 |
|---|---|
| Tantivy 索引大小 | 每万条 Chunk < 100MB |
| Qdrant 向量存储 | 每万条 Chunk < 200MB（1536 维 × 4 bytes） |
| 内存占用（空闲） | < 100MB（不含 Qdrant 容器） |
| 内存占用（搜索） | < 200MB 峰值 |

### 4.3 可靠性

- 索引操作具有幂等性：同一文件重复索引结果一致
- 索引过程中进程崩溃，重启后自动通过全量扫描修复不一致
- Embedding API 调用失败时自动重试（3 次，指数退避），最终失败时仅建立全文索引（降级）
- Qdrant 写入失败时记录待同步队列，恢复后自动补偿

### 4.4 可扩展性

- 分块策略可配置（参数化，非硬编码）
- Embedding 模型可切换（OpenAI / Ollama / 本地 ONNX）
- 搜索排序策略可插拔（当前为 RRF，后续可扩展）

---

## 5. 与其他模块的接口

### 5.1 文件监控层 → 记忆引擎

```
事件类型: FileEvent
  - Created(path: PathBuf)
  - Modified(path: PathBuf)
  - Removed(path: PathBuf)
  - Renamed { from: PathBuf, to: PathBuf }

接口: MemoryEngine::on_file_event(event: FileEvent) -> Result<()>
```

**说明**：文件监控层捕获事件后通过 EventBus 发布，记忆引擎订阅 `FileEvent` 并处理。记忆引擎内部做防抖（300ms），合并同文件多次变更。

### 5.2 记忆引擎 → 时间线模块

```
事件类型: MemoryEvent
  - Indexed { path: PathBuf, chunk_count: usize }
  - Updated { path: PathBuf, chunk_count: usize }
  - Removed { path: PathBuf, chunk_count: usize }
  - MemoryCreated { memory_id: Uuid, note_path: PathBuf }

接口: EventBus::publish(MemoryEvent)
```

**说明**：记忆引擎完成索引操作后发布事件，时间线模块订阅并记录。

### 5.3 智识雷达 → 记忆引擎

```
场景: add_to_vault 纳藏文章
流程:
  1. 雷达模块将文章写入 Vault 文件（radar/xxx.md）
  2. 文件监控捕获新文件事件
  3. 记忆引擎自动索引（与普通笔记一致）

无需直接接口调用，通过文件监控间接触发。
```

### 5.4 灵感熔炉 → 记忆引擎

```
接口: MemoryEngine::search(query: &str, top_k: usize, tags: &[String]) -> Result<Vec<MemorySearchResult>>
```

**说明**：灵感熔炉需要获取相关笔记片段作为创意素材时，调用记忆引擎的搜索接口。

### 5.5 API 层 → 记忆引擎

```
Tool API 映射:
  search_memory   → MemoryEngine::search()
  add_memory      → MemoryEngine::add()
  update_memory   → MemoryEngine::update()
  forget_memory   → MemoryEngine::forget()
  get_memory_stats → MemoryEngine::stats()
  search_notes    → MemoryEngine::search_notes()
  get_note        → MemoryEngine::get_note()
  list_recent_notes → MemoryEngine::list_recent()
```

---

## 6. 约束与假设

### 6.1 约束

1. **Embedding API 依赖**：语义搜索依赖外部 Embedding API（或本地模型），API 不可用时降级为纯全文搜索
2. **Qdrant 容器依赖**：向量存储需要 Qdrant Docker 容器运行，容器不可用时降级处理
3. **文件格式**：仅支持 `.md` (Markdown) 文件，其他格式（PDF、图片等）不在处理范围内
4. **Vault 大小**：初期设计目标为 ≤ 5000 篇笔记，更大规模需后续优化
5. **单机部署**：不支持多实例并发写入同一 Vault（单用户场景）
6. **中文分词词典**：jieba 默认词典可能无法覆盖用户自定义术语，需提供自定义词典加载机制

### 6.2 假设

1. 用户的 Obsidian Vault 路径在配置中指定，运行期间不变
2. 笔记文件使用 UTF-8 编码
3. 笔记内容以 Markdown 格式编写，frontmatter 使用 YAML 格式
4. 用户有可用的 Embedding API Key（或配置了本地模型）
5. Qdrant 容器通过 docker compose 启动，与 ObsidianBrain 在同一主机
6. 网络环境允许访问 OpenAI API（如使用 OpenAI Embedding）

---

## 7. 验收标准

### 7.1 功能验收

| 编号 | 验收条件 | 验证方法 |
|---|---|---|
| AC-01 | 新增 `.md` 文件后 1 秒内，其内容可被 `search_memory` 搜索到 | 集成测试 |
| AC-02 | 修改 `.md` 文件后，搜索结果反映最新内容，无旧版本残留 | 集成测试 |
| AC-03 | 删除 `.md` 文件后，其内容不再出现在搜索结果中 | 集成测试 |
| AC-04 | 分块后每个 Chunk 的 token 数在 `[chunk_min_tokens, chunk_max_tokens]` 范围内（代码块超长例外） | 单元测试 |
| AC-05 | 代码块（\`\`\`...\`\`\`）在分块后保持完整，不被切割 | 单元测试 |
| AC-06 | 每个 Chunk 携带正确的标题路径 breadcrumb | 单元测试 |
| AC-07 | `search_memory` 返回结果包含 Obsidian URI，URI 可正确打开对应笔记 | 集成测试 |
| AC-08 | 混合搜索结果优于单一检索通道（通过人工评估 10 组查询） | 人工评测 |
| AC-09 | `add_memory` 后新内容写入笔记文件并可搜索 | 集成测试 |
| AC-10 | `forget_memory` 后该记忆不再出现在搜索结果中 | 集成测试 |
| AC-11 | `update_memory` 后搜索结果反映更新内容 | 集成测试 |
| AC-12 | `get_memory_stats` 返回的统计数据与实际一致 | 集成测试 |
| AC-13 | 中文查询"分布式一致性"能搜到包含"共识算法"的笔记（语义检索） | 集成测试 |
| AC-14 | Qdrant 不可用时，`search_memory` 降级为全文搜索并正常返回 | 故障注入测试 |
| AC-15 | 500 篇笔记全量索引在 10 分钟内完成 | 性能测试 |

### 7.2 性能验收

| 编号 | 验收条件 | 验证方法 |
|---|---|---|
| PA-01 | 全文搜索 P95 延迟 < 50ms（1000 条 Chunk 数据量级） | 基准测试 |
| PA-02 | 语义搜索 P95 延迟 < 200ms | 基准测试 |
| PA-03 | 混合搜索 P95 延迟 < 300ms | 基准测试 |
| PA-04 | 空闲时内存占用 < 100MB | 运行时监控 |
| PA-05 | 文件变更处理延迟 < 500ms（含防抖） | 端到端测试 |

### 7.3 可靠性验收

| 编号 | 验收条件 | 验证方法 |
|---|---|---|
| RA-01 | 进程崩溃重启后，索引数据不丢失，搜索功能正常 | 故障恢复测试 |
| RA-02 | Embedding API 连续 3 次失败后降级为全文索引，不中断服务 | 故障注入测试 |
| RA-03 | 对同一文件重复索引，结果幂等（无重复 Chunk） | 单元测试 |

---

## 8. 术语表

| 术语 | 说明 |
|---|---|
| **Chunk** | 记忆单元，从笔记正文中按规则切分出的文本片段 |
| **Embedding** | 将文本转换为高维浮点向量的过程 |
| **BM25** | Best Matching 25，经典的全文检索评分算法 |
| **RRF** | Reciprocal Rank Fusion，倒数排名融合算法 |
| **HNSW** | Hierarchical Navigable Small World，近似最近邻搜索算法 |
| **Breadcrumb** | Chunk 所在的标题层级路径 |
| **Upsert** | Update or Insert，存在则更新、不存在则插入 |
| **防抖 (Debounce)** | 将短时间内的多次事件合并为一次处理 |
| **降级 (Fallback)** | 主路径不可用时切换到备用路径 |

---

> **下一步**：详见 [开发设计文档](../development/03-memory-engine.md)，包含技术架构、数据结构、算法实现与代码组织。
