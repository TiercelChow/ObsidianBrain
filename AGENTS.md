# AGENTS.md — ObsidianBrain 开发指南

> 本文件是 Codex 在本项目中工作时的核心指导。所有开发活动必须遵循本文档的规范。

---

## 项目概述

ObsidianBrain 是一个运行在本地的 **Rust 知识引擎**，对外提供标准化的 LLM Tool API（兼容 MCP 协议与 OpenAI function calling）。围绕用户的 Obsidian 知识库和本地代码仓库，提供记忆管理、代码仓概览、灵感催化、外部信息聚合、时间线回顾等能力。

**核心原则**：对话由 LLM 前端完成，本引擎是 LLM 的"手"和"眼"——负责感知和执行。

---

## 文档体系

**开发任何功能前，必须先阅读对应的需求设计和开发设计文档。**

```
docs/
├── top_design.md                    # 顶层设计（总纲，必读）
├── requirement/                     # 需求设计文档（What & Why）
│   ├── 01-infrastructure.md         # 基础设施层
│   ├── 02-tool-protocol.md          # 工具协议与 API
│   ├── 03-memory-engine.md          # 记忆引擎
│   ├── 04-timeline.md               # 时间线
│   ├── 05-code-repo.md              # 代码仓管理
│   ├── 06-inspiration.md            # 灵感熔炉
│   └── 07-radar.md                  # 智识雷达
└── development/                     # 开发设计文档（How）
    ├── 01-infrastructure.md         # 基础设施层
    ├── 02-tool-protocol.md          # 工具协议与 API
    ├── 03-memory-engine.md          # 记忆引擎
    ├── 04-timeline.md               # 时间线
    ├── 05-code-repo.md              # 代码仓管理
    ├── 06-inspiration.md            # 灵感熔炉
    └── 07-radar.md                  # 智识雷达
```

### 文档编号与模块对应

| 编号 | 模块 | 实施阶段 |
|------|------|----------|
| 01 | 基础设施层 (Infra) | Phase 0 |
| 02 | 工具协议与 API (Tool Protocol) | Phase 1 |
| 03 | 记忆引擎 (Memory Engine) | Phase 1 |
| 04 | 时间线 (Timeline) | Phase 2 |
| 05 | 代码仓管理 (Code Repo Hub) | Phase 2 |
| 06 | 灵感熔炉 (Inspiration Forge) | Phase 3 |
| 07 | 智识雷达 (Knowledge Radar) | Phase 3 |

---

## 技术栈

| 层次 | 技术 |
|------|------|
| 语言 | Rust (edition 2021) |
| Web 框架 | Axum + Tokio |
| Obsidian 集成 | Obsidian Local REST API (HTTP) |
| HTTP 客户端 | reqwest |
| 配置 | config crate (TOML) |
| 日志 | tracing + tracing-subscriber |
| 序列化 | serde + serde_json |

**注意**：项目已从混合搜索架构（Tantivy + Qdrant + Embedding）简化为直接使用 Obsidian Local REST API。不再需要本地索引、向量存储或 Embedding 服务。

---

## 目录结构规范

```
ObsidianBrain/
├── Cargo.toml
├── docker-compose.yml
├── AGENTS.md                        # 本文件
├── config/
│   └── default.toml
├── migrations/                      # SQLite 迁移脚本
├── docs/                            # 设计文档（见上）
└── src/
    ├── main.rs                      # 入口
    ├── config.rs                    # 配置管理
    ├── error.rs                     # 统一错误类型 (BrainError)
    ├── api/                         # API 层
    │   ├── mod.rs
    │   ├── router.rs
    │   ├── tool_protocol.rs
    │   └── handlers/
    ├── core/                        # 核心服务层
    │   ├── mod.rs
    │   ├── memory.rs
    │   ├── timeline.rs
    │   ├── code_repo.rs
    │   ├── inspiration.rs
    │   └── radar.rs
    ├── infra/                       # 基础设施层
    │   ├── mod.rs
    │   ├── sqlite_store.rs
    │   ├── file_watcher.rs
    │   ├── embedding.rs
    │   ├── llm_client.rs
    │   ├── qdrant_client.rs
    │   └── tantivy_index.rs
    ├── tools/                       # 工具定义与注册
    │   ├── mod.rs
    │   ├── registry.rs
    │   ├── definitions.rs
    │   └── traits.rs
    └── models/                      # 共享数据模型
        ├── mod.rs
        ├── note.rs
        ├── memory.rs
        ├── repo.rs
        └── radar.rs
```

---

## 开发工作流（必须使用 skill）

### 核心流程

每个功能开发必须遵循以下流程，并使用对应的 skill：

```
1. 阅读设计文档 → 理解需求
2. /brainstorming → 如果需求不明确，先探索方案
3. /writing-plans → 制定实施计划（拆解任务、识别风险）
4. /test-driven-development → TDD 循环（红→绿→重构）
5. /verification-before-completion → 完成前验证
6. /code-review → 自我审查代码质量
7. /simplifying → 简化冗余代码
```

### Skill 使用指南

| 场景 | 使用的 Skill | 触发时机 |
|------|-------------|----------|
| 新功能需求不明确 | `/brainstorming` | 开始任何新功能前 |
| 制定实施计划 | `/writing-plans` | 开始多步骤任务前 |
| 按计划执行 | `/executing-plans` | 执行已有的实施计划 |
| 所有代码编写 | `/test-driven-development` | **始终使用 TDD** |
| 修复 Bug | `/systematic-debugging` | 遇到任何 Bug 时 |
| 完成前检查 | `/verification-before-completion` | 认为任务完成前 |
| 代码质量审查 | `/code-review` | 完成一个功能后 |
| 代码简化 | `/simplifying` | 代码审查后 |
| 请求正式审查 | `/requesting-code-review` | PR 前 |
| 分支完成 | `/finishing-a-development-branch` | 功能开发完毕 |
| 大任务拆分 | `/subagent-driven-development` | 独立子任务可并行 |
| 并行独立任务 | `/dispatching-parallel-agents` | 多个不相关的修改 |
| 隔离开发 | `/using-git-worktrees` | 需要隔离实验时 |

### 典型开发流程示例

**开发 Memory Engine 的 search_memory 功能：**

```
1. 阅读 docs/requirement/03-memory-engine.md（理解 What）
2. 阅读 docs/development/03-memory-engine.md（理解 How）
3. /writing-plans → 制定 search_memory 的实施计划
4. /test-driven-development：
   a. 先写混合搜索的测试（RRF 融合、排序、分页）
   b. 实现 Tantivy 全文搜索
   c. 实现 Qdrant 语义搜索
   d. 实现 RRF 融合算法
   e. 测试通过
5. /verification-before-completion → 确认搜索质量
6. /code-review → 自我审查
7. /simplifying → 简化代码
```

---

## 编码规范

### Rust 风格

- **遵循 Rust 2021 Edition**，使用 `cargo fmt` 格式化
- **使用 `cargo clippy` 检查**，零 warning 标准
- **异步运行时**：统一使用 Tokio，禁止混用其他 runtime
- **错误处理**：
  - 使用 `Result<T, BrainError>` 作为函数返回类型
  - 禁止在生产代码中使用 `.unwrap()` / `.expect()`（测试除外）
  - 外部错误通过 `From` trait 转换为 `BrainError`
- **类型安全**：
  - 优先使用 newtype pattern 封装业务含义（如 `NotePath(PathBuf)`）
  - 使用 enum 表达有限状态（如 `FileChangeType`, `RadarStatus`）
  - 公共 API 必须有完整的类型签名

### Trait 抽象

所有外部依赖必须通过 trait 抽象，便于测试和切换实现：

```rust
// ✅ 正确：trait 抽象
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError>;
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError>;
    fn dimensions(&self) -> usize;
}

// ❌ 错误：直接依赖具体实现
pub struct MemoryService {
    openai_client: OpenAiClient,  // 硬编码具体实现
}
```

### 模块依赖规则

```
API 层 (api/) ──→ 核心服务层 (core/) ──→ 基础设施层 (infra/)
     │                   │                      │
     └──────────────────→├──────────────────────┘
                         │
                    共享模型 (models/)
                    统一错误 (error.rs)
                    配置管理 (config.rs)
```

**严格禁止反向依赖**：infra 不能依赖 core，core 不能依赖 api。

### 共享状态

- 使用 `Arc<T>` 共享跨线程状态
- 需要可变状态时使用 `Arc<RwLock<T>>`（读多写少）或 `Arc<Mutex<T>>`（写频繁）
- 异步上下文优先使用 `tokio::sync::RwLock` / `tokio::sync::Mutex`
- 应用上下文统一通过 `AppContext` struct 传递

### 日志规范

使用 `tracing` 宏：

```rust
tracing::info!(vault_path = %config.vault.path, "Vault 监控启动");
tracing::debug!(query = %params.query, top_k = params.top_k, "执行搜索");
tracing::warn!(error = %e, "Qdrant 不可用，降级为全文搜索");
tracing::error!(path = %note_path, "笔记解析失败");
```

- `info`：关键业务事件（启动、停止、工具调用）
- `debug`：详细处理流程（搜索、索引、LLM 调用参数）
- `warn`：可恢复的异常（降级、重试）
- `error`：不可恢复的错误

### 测试规范

- **单元测试**：每个公共函数至少 1 个正向测试 + 1 个边界测试
- **集成测试**：每个模块的核心流程至少 1 个端到端测试
- **Mock 策略**：使用 `mockall` 对 trait 做 mock，使用 `wiremock` 对 HTTP API 做 mock
- **测试命名**：`test_<被测函数>_<场景>_<预期结果>`

```rust
#[tokio::test]
async fn test_search_memory_hybrid_returns_rrf_merged_results() { ... }

#[tokio::test]
async fn test_search_memory_qdrant_down_degrades_to_fulltext() { ... }
```

---

## 构建与运行

```bash
# 构建
cargo build

# 测试
cargo test                    # 全部测试
cargo test --lib              # 仅单元测试
cargo test --test '*'         # 仅集成测试

# 代码质量
cargo fmt -- --check          # 格式检查
cargo clippy -- -D warnings   # Lint 检查（零 warning）

# 运行
cargo run                     # 开发运行
cargo run -- --config config/default.toml  # 指定配置

# Docker（Qdrant）
docker compose up -d          # 启动 Qdrant
docker compose down           # 停止
```

---

## 实施阶段路线图

### Phase 0: 基础设施搭建（优先）
- [ ] `Cargo.toml` + workspace 初始化
- [ ] `src/config.rs` — 配置加载与校验
- [ ] `src/error.rs` — BrainError 统一错误类型
- [ ] `src/infra/sqlite_store.rs` — SQLite 初始化与迁移
- [ ] `src/infra/file_watcher.rs` — Vault 文件监控
- [ ] `src/infra/tantivy_index.rs` — 全文索引基础
- [ ] `src/infra/qdrant_client.rs` — 向量存储封装
- [ ] `src/infra/embedding.rs` — Embedding Provider
- [ ] `src/infra/llm_client.rs` — LLM Provider
- [ ] `docker-compose.yml` — Qdrant 容器
- [ ] `src/main.rs` — 启动骨架

### Phase 1: 核心引擎 MVP
- [ ] Markdown 解析 + 智能分块
- [ ] 记忆 CRUD + 混合搜索 (RRF)
- [ ] MCP / HTTP Tool API 基础协议
- [ ] 核心工具：search_notes, get_note, search_memory, add_memory

**里程碑**：在 Codex 中通过 Tool API 搜索 Obsidian 笔记

### Phase 2: 代码仓 + 时间线
- [ ] 代码仓注册、元信息提取、笔记关联
- [ ] 自动文档化（LLM 生成）
- [ ] 时间线事件收集与查询

### Phase 3: 灵感 + 雷达
- [ ] 灵感熔炉三种模式
- [ ] 智识雷达外部源拉取 + 个性化排序
- [ ] 文章纳藏到 vault

### Phase 4: 打磨
- [ ] 技能 YAML 扩展系统
- [ ] 本地 ONNX Embedding
- [ ] 性能优化与内存调优

---

## 关键设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 笔记操作 | Obsidian Local REST API | 无需本地索引，直接利用 Obsidian 的搜索和存储能力 |
| 工具协议 | HTTP REST API | 通用、易于集成 |
| 错误处理 | 自定义 BrainError 枚举 | 统一类型、可降级、可映射错误码 |
| 架构简化 | 移除 Tantivy/Qdrant/Embedding | 减少复杂度，无需额外服务，部署更简单 |

---

## 安全与隐私

- **仅监听 127.0.0.1**，不暴露到外网
- **所有数据由 Obsidian 管理**，ObsidianBrain 仅通过 HTTP API 访问
- **API Key 通过环境变量传入**，不硬编码在代码或配置文件中
- **Vault 写入操作通过 Obsidian API**，由 Obsidian 处理路径校验

---

## Git 规范

- **分支命名**：`feature/<module>-<desc>`, `fix/<module>-<desc>`, `refactor/<desc>`
- **Commit 格式**：Conventional Commits
  ```
  feat(memory): implement RRF hybrid search
  fix(timeline): correct date parsing for CJK formats
  refactor(infra): extract EmbeddingProvider trait
  docs(architecture): update data flow diagrams
  test(qdrant): add integration tests for upsert/search
  ```
- **每个 commit 必须通过**：`cargo fmt --check && cargo clippy -- -D warnings && cargo test`

---

## 快速参考

### 新增一个 Tool 的步骤

1. 在 `src/tools/definitions.rs` 定义 JSON Schema
2. 在 `src/tools/traits.rs` 实现 `ToolHandler` trait
3. 在对应的 `src/core/` 模块实现业务逻辑
4. 在 `src/tools/registry.rs` 的 `initialize_tools()` 注册
5. 编写单元测试 + 集成测试
6. 更新 `docs/development/02-tool-protocol.md` 的工具清单

### 新增一个 Infra 模块的步骤

1. 在 `src/infra/` 下新建文件，定义 trait + 实现
2. 在 `src/config.rs` 添加对应配置段
3. 在 `src/error.rs` 添加错误变体
4. 在 `src/main.rs` 初始化并注入 `AppContext`
5. 编写单元测试（使用 mockall / wiremock）

### 构建命令速查

```bash
cargo build                              # 开发构建
cargo build --release                    # 生产构建
cargo test                               # 全部测试
cargo test --lib <module>                # 单模块测试
cargo clippy -- -D warnings              # Lint 检查
cargo fmt                                # 格式化
cargo doc --no-deps --open               # 生成文档
```
