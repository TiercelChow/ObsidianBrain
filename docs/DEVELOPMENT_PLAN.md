# ObsidianBrain 开发计划

> 基于 `docs/top_design.md` 和各模块设计文档拆分

---

## 当前状态

✅ 已完成：
- 项目文档体系（顶层设计 + 14 个需求/开发设计文档）
- 后端 Rust 骨架（Axum + 配置 + 错误类型 + 健康检查 API）
- 前端 Vue 3 骨架（Element Plus + 路由 + 侧边栏 + 6 个页面）
- Docker Compose（Qdrant）
- CLAUDE.md 开发指南

---

## Phase 0: 基础设施搭建（剩余部分）

**目标**：完成所有基础设施模块，为上层服务提供完整支撑

**预计时间**：1 周

### 0.1 配置系统完善
- [ ] 接入 `config` crate，实现 TOML 实际解析
- [ ] 环境变量覆盖（`OBRAIN_*` 前缀）
- [ ] 配置校验逻辑（`Validate` trait）
- [ ] 热重载支持（可选，Phase 4 实现）

**参考文档**：`docs/development/01-infrastructure.md` §3

**验收标准**：
- `config/default.toml` 中的值能正确加载到 `AppConfig`
- 环境变量 `OBRAIN_SERVER__PORT=9999` 能覆盖配置

### 0.2 SQLite 元数据存储
- [ ] 添加 `rusqlite` 依赖（bundled feature）
- [ ] 实现 `SqliteStore` 结构体（连接初始化、WAL 模式）
- [ ] 实现迁移框架（版本化 SQL 迁移执行）
- [ ] 编写完整迁移脚本（code_repos, radar_items, inspiration_history, timeline_events, app_state）
- [ ] 实现 CRUD 辅助方法（事务管理、状态读写）

**参考文档**：`docs/development/01-infrastructure.md` §4

**验收标准**：
- 首次启动自动创建 `data/brain.db` 并执行所有迁移
- 再次启动跳过已执行的迁移
- 单元测试覆盖迁移执行和基本 CRUD

### 0.3 文件监控
- [ ] 添加 `notify` 依赖
- [ ] 实现 `FileWatcher` 结构体
- [ ] 实现防抖器（300ms Debouncer）
- [ ] 实现事件过滤（排除 `.obsidian/`, `.trash/` 等）
- [ ] 实现 `FileChangeEvent` 和 `FileChangeType` 枚举
- [ ] 通过 `tokio::sync::mpsc` 分发事件

**参考文档**：`docs/development/01-infrastructure.md` §5

**验收标准**：
- 监控指定 Vault 目录，创建/修改/删除 `.md` 文件时发送事件
- 300ms 内的多次修改合并为一个事件
- 排除模式中的目录不触发事件

### 0.4 Qdrant 客户端
- [ ] 添加 `qdrant-client` 依赖（或使用 HTTP API）
- [ ] 实现 `QdrantStore` 结构体
- [ ] 实现连接管理（health check）
- [ ] 实现 collection 创建（HNSW 参数配置）
- [ ] 实现 `upsert_points`, `search`, `delete_points` 方法
- [ ] 定义 `ChunkPayload` schema

**参考文档**：`docs/development/01-infrastructure.md` §8

**验收标准**：
- 启动时自动创建 `obsidian_brain` collection（如不存在）
- 能写入向量并搜索返回结果
- Qdrant 不可用时返回 `BrainError::QdrantError`

### 0.5 Embedding Provider
- [ ] 添加 `reqwest` 依赖（json + stream features）
- [ ] 定义 `EmbeddingProvider` trait（`embed_text`, `embed_batch`）
- [ ] 实现 `OpenAiEmbedder`（调用 `/v1/embeddings`）
- [ ] 实现批量处理（100 条/批）
- [ ] 实现指数退避重试（3 次）
- [ ] 实现 `EmbeddingFactory::create()`
- [ ] 预留 `OnnxEmbedder` 结构体

**参考文档**：`docs/development/01-infrastructure.md` §6

**验收标准**：
- 调用 OpenAI API 返回 1536 维向量
- 批量 200 条文本能正确分批处理
- API 超时/错误时重试 3 次后返回错误

### 0.6 LLM Client
- [ ] 定义 `LlmProvider` trait（`chat`, `chat_stream`）
- [ ] 实现 `OpenAiProvider`（调用 `/v1/chat/completions`）
- [ ] 实现流式响应（SSE 解析）
- [ ] 实现 `OllamaProvider`
- [ ] 实现 `LlmClientFactory::create()`
- [ ] 定义 `ChatMessage`, `ChatResponse`, `StreamChunk` 结构体

**参考文档**：`docs/development/01-infrastructure.md` §7

**验收标准**：
- 能调用 OpenAI API 返回完整响应
- 流式调用能逐块返回内容
- Token 估算函数误差 < 20%

### 0.7 Tantivy 全文索引
- [ ] 添加 `tantivy` + `tantivy-jieba` 依赖
- [ ] 实现 `TantivyIndex` 结构体
- [ ] 定义 Schema（title, content, path, tags, created_at, updated_at）
- [ ] 注册 Jieba 中文分词器
- [ ] 实现 `add_document`, `update_document`, `delete_document`, `commit`
- [ ] 实现搜索查询（单字段、布尔查询、标签过滤）
- [ ] 实现摘要片段生成

**参考文档**：`docs/development/01-infrastructure.md` §9

**验收标准**：
- 能索引中文笔记并通过关键词搜索到
- 标签过滤正常工作
- 搜索结果包含摘要片段

### 0.8 集成到 AppContext
- [ ] 在 `main.rs` 中初始化所有基础设施
- [ ] 将 `SqliteStore`, `QdrantStore`, `TantivyIndex`, `EmbeddingProvider`, `LlmProvider` 注入 `AppContext`
- [ ] 启动 FileWatcher（如果 `vault.watch_enabled = true`）
- [ ] 更新健康检查返回各组件状态

**验收标准**：
- 启动日志显示所有组件初始化成功
- `/v1/health` 返回各组件 `ok` 状态
- 任一组件初始化失败时优雅降级或报错

---

## Phase 1: 核心引擎 MVP

**目标**：实现记忆引擎 + Tool API，能在 Claude 中搜索 Obsidian 笔记

**预计时间**：2-3 周

### 1.1 Markdown 解析器
- [ ] 添加 `pulldown-cmark` + `gray_matter` 依赖
- [ ] 实现 frontmatter 提取（`ParsedDocument` 结构体）
- [ ] 实现正文解析（标题层级、段落分割）
- [ ] 实现代码块识别（保持完整性）
- [ ] 实现 `Section` 和 `CodeBlock` 结构体

**参考文档**：`docs/development/03-memory-engine.md` §3.1

**验收标准**：
- 能正确解析含 YAML frontmatter 的 Markdown
- 能提取所有 H1/H2/H3 标题及其内容
- 代码块不被分割

### 1.2 智能分块器
- [ ] 实现 `Chunker` 结构体
- [ ] 实现分块算法（按标题层级 + 段落边界）
- [ ] 实现 300-800 token 目标范围
- [ ] 实现代码块保护（不在代码块内部分割）
- [ ] 实现 `Chunk` 结构体（content, heading_path, tags 等）

**参考文档**：`docs/development/03-memory-engine.md` §3.2

**验收标准**：
- 1000 token 的段落能正确分割为 2-3 块
- 代码块保持完整
- 每块附带正确的标题路径

### 1.3 记忆引擎服务
- [ ] 实现 `MemoryService` 结构体
- [ ] 实现文件变更 → 索引更新流程（监听 FileWatcher 事件）
- [ ] 实现批量 Embedding 生成
- [ ] 实现增量索引（仅重新索引变更的 chunk）
- [ ] 实现记忆 CRUD（`add_memory`, `update_memory`, `forget_memory`）
- [ ] 实现 `get_memory_stats`

**参考文档**：`docs/development/03-memory-engine.md` §3.6

**验收标准**：
- 创建/修改/删除笔记时自动更新索引
- `add_memory` 写入笔记并索引
- `forget_memory` 从索引中删除

### 1.4 混合搜索引擎
- [ ] 实现 Tantivy 全文搜索（BM25 排序，取 top 20）
- [ ] 实现 Qdrant 语义搜索（余弦相似度，取 top 20）
- [ ] 实现 RRF (Reciprocal Rank Fusion) 融合算法（k=60）
- [ ] 实现并行执行（`tokio::join!`）
- [ ] 实现降级策略（Qdrant 不可用时仅全文搜索）

**参考文档**：`docs/development/03-memory-engine.md` §3.6

**验收标准**：
- `search_memory` 返回 RRF 融合后的结果
- 全文和语义搜索并行执行，延迟 < 300ms
- Qdrant 宕机时降级为全文搜索并日志告警

### 1.5 Tool API 基础协议
- [ ] 实现 `ToolHandler` trait
- [ ] 实现 `ToolRegistry`（工具注册表）
- [ ] 定义工具 JSON Schema（`search_notes`, `get_note`, `search_memory`, `add_memory`）
- [ ] 实现 HTTP API：`GET /v1/tools`, `POST /v1/tools/call`
- [ ] 实现参数校验（`jsonschema` crate）
- [ ] 实现错误响应标准化

**参考文档**：`docs/development/02-tool-protocol.md`

**验收标准**：
- `GET /v1/tools` 返回所有工具的 JSON Schema
- `POST /v1/tools/call` 能正确调用工具并返回结果
- 参数校验失败时返回清晰的错误信息

### 1.6 核心工具实现
- [ ] `search_notes(query, top_k?, tags?)` — 混合搜索笔记
- [ ] `get_note(path)` — 获取笔记完整内容
- [ ] `list_recent_notes(days?, limit?)` — 最近修改的笔记
- [ ] `search_memory(query, top_k?, tags?)` — 记忆搜索
- [ ] `add_memory(note_path, content, tags?)` — 添加记忆
- [ ] `update_memory(memory_id, content)` — 更新记忆
- [ ] `forget_memory(memory_id)` — 删除记忆
- [ ] `get_memory_stats()` — 记忆库统计

**参考文档**：`docs/development/02-tool-protocol.md` §工具清单

**验收标准**：
- 所有 8 个工具能通过 HTTP API 调用
- 搜索结果包含 Obsidian URI（`obsidian://open?vault=...&file=...`）
- 工具调用日志记录完整

**🎯 里程碑**：在 Claude Desktop 中配置 MCP Server，通过自然语言搜索 Obsidian 笔记

---

## Phase 2: 代码仓 + 时间线

**目标**：实现代码仓管理和时间线功能

**预计时间**：1-2 周

### 2.1 代码仓管理
- [ ] 添加 `git2` 依赖
- [ ] 实现 `RepoManager` 结构体
- [ ] 实现仓库注册（`add_code_repo`）：路径校验 → git2 提取元数据 → 写入 SQLite
- [ ] 实现 `GitExtractor`：分支、commit 历史、工作区状态
- [ ] 实现 `LanguageDetector`：文件扩展名统计（排除 .git/vendor/node_modules）
- [ ] 实现仓库列表和详情查询
- [ ] 实现仓库状态监控（`.git/HEAD` 文件监控）

**参考文档**：`docs/development/05-code-repo.md`

**验收标准**：
- 能注册本地 Git 仓库并提取元信息
- 语言占比统计准确（误差 < 5%）
- 仓库 dirty 状态实时更新

### 2.2 笔记 ↔ 仓库关联
- [ ] 实现 `NoteLinker`：在笔记末尾插入标准引用块
- [ ] 实现关联记录管理（SQLite `note_repo_links` 表）
- [ ] 实现自动关联建议（关键词匹配）

**参考文档**：`docs/development/05-code-repo.md` §3.3

**验收标准**：
- `link_note_to_repo` 在笔记中插入格式正确的引用块
- 引用块包含仓库信息和 VSCode 链接

### 2.3 自动文档化
- [ ] 实现仓库信息提取（目录结构、README、Cargo.toml/package.json）
- [ ] 实现 LLM prompt 模板（项目文档生成）
- [ ] 实现文档写入 Vault（指定目录）
- [ ] 实现可配置模板（`config/doc_template.md`）

**参考文档**：`docs/development/05-code-repo.md` §3.4

**验收标准**：
- `generate_docs` 生成结构化的项目文档笔记
- 文档包含概述、目录结构、核心模块、技术栈

### 2.4 VSCode 集成
- [ ] 实现 `vscode://file/...` URI 生成
- [ ] 实现 `code` 命令调用（可选）

**验收标准**：
- `open_in_vscode` 返回正确的 URI

### 2.5 时间线
- [ ] 实现事件收集器（5 种数据源）：
  - Frontmatter 日期提取
  - 文件名日期提取（正则匹配）
  - 内容 `#date` 标签提取
  - 文件监控事件
  - Git commit 事件
- [ ] 实现事件存储（SQLite `timeline_events` 表）
- [ ] 实现事件查询（日期范围、类型过滤、按日分组）
- [ ] 实现统计聚合（计数、频率、趋势）
- [ ] 实现 LLM 摘要生成（时间线摘要 + 周报）

**参考文档**：`docs/development/04-timeline.md`

**验收标准**：
- `get_timeline(start, end)` 返回结构化的每日事件列表
- 统计信息准确（事件数、类型分布）
- 周报包含活动总结和关键洞察

### 2.6 工具实现
- [ ] `add_code_repo(path, name)`
- [ ] `list_code_repos()`
- [ ] `get_repo_detail(name)`
- [ ] `link_note_to_repo(note_path, repo_name)`
- [ ] `generate_docs(repo_name, target_path?)`
- [ ] `open_in_vscode(repo_name)`
- [ ] `get_timeline(start_date, end_date)`

**验收标准**：
- 所有 7 个工具能通过 API 调用
- 工具返回结构化 JSON

---

## Phase 3: 灵感 + 雷达

**目标**：实现灵感熔炉和智识雷达

**预计时间**：1-2 周

### 3.1 灵感熔炉
- [ ] 实现概念池构建器（`ConceptPool`）：
  - 从 vault 标签提取（TF-IDF 加权）
  - 从笔记标题提取关键词
  - 从仓库名提取技术概念
- [ ] 实现概念距离矩阵（Jaccard 距离）
- [ ] 实现概念选择器（距离加权随机）
- [ ] 实现三种 LLM 模式：
  - `concept_combo`：随机概念组合
  - `reverse_question`：反向提问
  - `counterpoint`：对立观点
- [ ] 实现结果格式化（Obsidian URI 链接）
- [ ] 实现灵感历史记录（SQLite）

**参考文档**：`docs/development/06-inspiration.md`

**验收标准**：
- 三种模式各返回一个创意 + 相关链接
- 概念组合的两个概念标签共现度低（距离远）
- 历史记录不重复

### 3.2 智识雷达
- [ ] 实现源管理器（`radar_sources.toml` 解析）
- [ ] 实现内容抓取器：
  - RSS（`feed-rs`）
  - arXiv API
  - HackerNews API
  - Reddit API
- [ ] 实现定时调度（`tokio-cron-scheduler`，每 6 小时）
- [ ] 实现内容处理（清洗、去重）
- [ ] 实现相关性引擎：
  - 用户兴趣向量构建（最近 30 天笔记 embedding 加权平均）
  - 文章 embedding 生成
  - 余弦相似度计算
  - 多因子排序（相似度 × 可信度 × 时效性）
- [ ] 实现纳藏器（正文提取 + Obsidian 笔记生成）
- [ ] 实现状态管理（New / Read / Saved / Dismissed）

**参考文档**：`docs/development/07-radar.md`

**验收标准**：
- 定时拉取外部源并缓存
- `get_radar(limit)` 返回按相关性排序的文章
- `add_to_vault(article_id)` 生成笔记并索引
- 已读/已忽略的文章不再推荐

### 3.3 工具实现
- [ ] `get_inspiration(type?, note_path?)`
- [ ] `get_radar(limit?, query?)`
- [ ] `add_to_vault(article_id, target_dir?)`
- [ ] `dismiss_radar_item(article_id)`

**验收标准**：
- 所有 4 个工具能通过 API 调用

---

## Phase 4: 打磨与增强（持续）

**目标**：优化性能、扩展功能

### 4.1 技能系统
- [ ] 实现 YAML 技能定义解析
- [ ] 实现技能编排器（多步骤执行）
- [ ] 实现内置技能：`summarize_today_notes`, `weekly_review`
- [ ] 支持用户自定义技能

### 4.2 MCP 协议完善
- [ ] 实现 stdio 传输（Claude Desktop 集成）
- [ ] 实现 SSE 传输（可选，Web 前端）
- [ ] 实现工具列表变更通知

### 4.3 本地 Embedding
- [ ] 实现 ONNX Runtime 集成
- [ ] 加载本地模型（如 `all-MiniLM-L6-v2`）
- [ ] 性能优化（批处理、GPU 加速）

### 4.4 性能优化
- [ ] 内存占用调优（目标空闲 < 100MB）
- [ ] 搜索延迟优化（P95 < 300ms）
- [ ] 大 vault 支持（> 10000 篇笔记）
- [ ] 索引增量更新优化

### 4.5 前端增强
- [ ] 实现各模块的实际交互界面（当前为占位）
- [ ] 记忆搜索 UI
- [ ] 代码仓卡片展示
- [ ] 时间线可视化
- [ ] 灵感结果展示
- [ ] 雷达文章列表 + 纳藏操作

---

## 开发规范提醒

每个功能开发前必须：

1. **阅读设计文档**：对应的 `docs/requirement/` 和 `docs/development/`
2. **使用 Skill**：
   - `/writing-plans` — 制定实施计划
   - `/test-driven-development` — TDD 循环
   - `/verification-before-completion` — 完成前验证
   - `/code-review` — 代码审查
3. **遵循 CLAUDE.md**：编码规范、日志规范、测试规范
4. **更新文档**：实现完成后更新对应的设计文档

---

## 依赖添加顺序

按 Phase 逐步添加，避免一次性引入过多依赖：

**Phase 0**：
```toml
config = "0.14"
rusqlite = { version = "0.31", features = ["bundled"] }
notify = "6"
reqwest = { version = "0.12", features = ["json", "stream"] }
tantivy = "0.22"
tantivy-jieba = "0.2"
```

**Phase 1**：
```toml
pulldown-cmark = "0.10"
gray_matter = "0.2"
async-trait = "0.1"
futures = "0.3"
jsonschema = "0.18"
```

**Phase 2**：
```toml
git2 = "0.19"
```

**Phase 3**：
```toml
feed-rs = "1"
tokio-cron-scheduler = "0.10"
readability = "0.3"
```

---

## 测试策略

每个模块完成后必须：

- [ ] 单元测试覆盖率 > 80%
- [ ] 至少 1 个集成测试（端到端流程）
- [ ] Mock 外部依赖（`mockall` + `wiremock`）
- [ ] 性能基准测试（搜索延迟、索引速度）

运行测试：
```bash
cargo test                    # 全部测试
cargo test --lib              # 仅单元测试
cargo clippy -- -D warnings   # Lint 检查
cargo fmt -- --check          # 格式检查
```
