# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## 项目概况

ObsidianBrain 是本地优先的 **Rust 知识引擎 + 个人工作台**：后端 Axum/Tokio，Vue 3 前端在**编译期嵌入**为单一二进制（rust-embed）。数据全部落在 `~/.obsidian-brain/`（SQLite `brain.db` 为唯一事实来源）。

对外以 **Tool API** 暴露 90 个工具，兼容 MCP 与 OpenAI function calling。主要产品面：阅境轩（Markdown/PDF 阅读器 + 每书隔离的 Book Wiki）、时光机与任务中枢、灵感熔炉与智识雷达。

**权威文档**：`docs/top_design.md`（v1.3 总纲）、`docs/development/08-llm-wiki.md`（v3.14，按迁移编号记录 Book Wiki 实施状态，是理解旗舰特性最快的入口）。

---

## 常用命令

```bash
# ── 后端（在 backend/ 下）──
cargo test                              # 当前基线：279 passed, 0 failed, 2 ignored
cargo test semantic_compile             # 按名称过滤单个测试
cargo fmt -- --check                    # 当前 clean
cargo clippy --all-targets -- -D warnings   # 当前 clean
cargo run -- start --foreground         # 前台运行；无子命令等价于前台启动

# ── 前端（在 frontend/ 下）──
npm test                                # node --test，当前 194 passed
npx vue-tsc -b                          # 类型检查
npm run dev                             # Vite :5173，代理 /v1 → 127.0.0.1:9876

# ── 一体化构建（顺序很重要，见「易踩的坑 #3」）──
make build                              # = frontend(dist_new) + backend(release)
make install                            # → ~/.local/bin/（macOS 自动 codesign，无需 sudo）
```

**隔离数据库做手工验证**（注意前缀后是**双**下划线）：

```bash
OBRAIN__SERVER__PORT=9988 OBRAIN__STORAGE__DB_PATH=/tmp/ob.db cargo run -- start --foreground
OBRAIN_DATA_DIR=/tmp/ob-preview         # 一次性隔离 db/日志/缩略图
# OBRAIN_STORAGE__DB_PATH（少一个下划线）不会覆盖正式库，会静默写错地方
```

服务默认监听 `0.0.0.0:9876`；`GET /v1/health` 返回组件状态与工具数。

---

## 架构

### 工具优先，不是 REST 资源 API

业务能力**只**通过 `POST /v1/tools/call`（`{tool, arguments}`）暴露。只有真正需要字节流/上传/SSE/MCP-jsonrpc 的场景才有专用路由（`backend/src/api/router.rs:33` 是唯一的扁平路由表，其后挂 SPA fallback）。

- **新增能力时，默认写一个 `ToolHandler`，而不是新增 REST 端点。**
- `ToolHandler` trait：`backend/src/tools/traits.rs:13`（注意方法是 `input_schema()`，不是 `schema()`）。
- 注册：在 `backend/src/tools/handlers/<module>_handlers.rs` 实现，再到 `handlers/mod.rs` 的 `register_all_tools` 里 `registry.register(Arc::new(...))`。`list_tools` 与 MCP 暴露自动生效。
- `register_all_tools` 的 `_ctx` 参数**被有意忽略**，context 是通过 `handle(args, ctx)` 传入的。
- 阻塞型工作（文件系统 / rusqlite / zip）要包 `spawn_blocking`；长任务应入队而非在请求里 await。

### 分层与共享状态

```
api/ (handlers, router)  →  core/ (业务逻辑)  →  infra/ (SQLite / HTTP 客户端 / ACP)
                                    ↓
                        models/ · error.rs · config.rs
```

禁止反向依赖。`AppContext`（`backend/src/main.rs:37`）是唯一共享状态结构，始终以 `Arc<AppContext>` 传递，**没有** `FromRef` 子状态提取——handler 直接拿整个 ctx。

### 持久化

- 单条 `rusqlite::Connection` 放在 `std::sync::Mutex` 后面（`backend/src/infra/sqlite_store.rs:15`）：无连接池，DB 实际是单线程瓶颈。
- 写路径 `transaction()`（`BEGIN IMMEDIATE`），读路径 `with_connection()`。后者在锁中毒时返回错误，**其他多数方法 `.lock().unwrap()` 会 panic**。
- 迁移：`include_str!` 编译期嵌入 + `_migrations` 表跟踪版本（**不是** `PRAGMA user_version`），所有待执行迁移在**一个事务**内跑完。迁移 29–34 还带 Rust 侧 Skill 种子逻辑（`run_migrations` 里的版本条件 `match`），加这类迁移要同时改两处。
- **全文检索是 SQLite FTS5**（migration 019：`knowledge_entries_fts` / `source_spans_fts`，`unicode61` + 手写 `cjk_terms` 二元组列处理中文）。Tantivy/Qdrant/Embedding 是已废弃方向，见「陈旧文件」。

### Book Wiki 管线（最大的特性，改动前必读 08 号文档）

```
来源 Markdown → 不可变 version/span（内容哈希去重、保留旧版本供引用）
   → 语义编译（受审计 ACP Run，增量：脚本按 compile_fingerprint + 检查点跳过未变来源）
   → knowledge_change_sets（候选）
   → 人工批准 → 正式实体/论断/关系/引用（单事务）
   → 问答（SSE 流式，带 [S#] 引用快照）
   → 研究任务（SQLite 租约队列，单 base 一个 running）
   → PPTX（本地手写 OOXML 渲染，非库）
```

**Rust 与 Agent 的边界（最重要的一条）**：Rust 拥有全部持久状态、检索、提示词组装、JSON 契约校验、审核门禁、配额与 PPTX 渲染；DeepSeek Harness 只是**文本进出的一次性 ACP 子进程**，从不接触数据库。

- Agent 的一切产出都是 **proposal**：`knowledge_propose_changes` 只创建 `proposed` 变更集，只有 `resolve_change_set(approve=true)` 才写正式知识。驳回会**删除编译检查点**（该来源需重新编译）。
- Harness 的权限请求一律自动应答 `Cancelled`（`backend/src/infra/deepseek_harness.rs`）。密钥只以**环境变量名**形式写进 0600 临时 patch，从不落盘。
- 唯一对外网络出口是 `book_fetch_external`：仅 HTTPS/443、仅公网 IP、不跟随重定向、配额在请求前就扣减。
- 各任务类型的 MCP 工具目录不同（`backend/src/core/book_wiki.rs:2226`）；`knowledge_ingest`、`skill_benchmark` 与演示策划 Run 是**零工具**，旁边有内联注释说明原因，别"顺手"挂上工具目录。
- 不要硬编码 9876：MCP 网关 URL 由实际绑定端口推导（`main.rs` 中构造 `BookWikiService` 处）。

### 前端

- 路由：`frontend/src/router/index.ts` 单文件扁平表，无守卫。视图文件很大（`Reader.vue` 2500+ 行），子组件在 `src/components/{reader,tasks,knowledge,motion}/`。视图状态多放在 **URL query** 而非 store。
- API：`src/api/index.ts` 的 axios 实例，**响应拦截器返回 `response.data`**（所以 `api.get()` 拿到的是 body，不要再 `.data`）。业务调用统一走 `callTool()`。
- Markdown 三层管线，边界是设计核心：
  1. 纯同步、无 DOM 的 core（`src/markdown/renderMarkdown.ts`）——这是它能进 Worker 的前提；
  2. Web Worker（`src/workers/markdown.worker.ts`），只能传字符串，函数型 resolver 传不过去；
  3. DOM 增强（`src/composables/useMarkdownRender.ts`），懒加载 highlight.js / mermaid / KaTeX，用 generation 计数器 + `isConnected` 防竞态，mermaid 串行化在一个队列里。
- Pinia 只有两个 store（`app.ts` 壳层状态、`tasks.ts` 领域数据）；其余是 composable / 组件局部状态。

### 配置优先级（低 → 高）

代码默认值 < `config/default.toml` < `OBRAIN__*` 环境变量 < 数据库内 `system_config` < CLI `--host/--port`。

`backend/config/default.toml` 有意只留 `[server]` 和 `[storage]`；Vault / Obsidian / LLM 配置正常从 Web 控制面板写入数据库。

---

## 易踩的坑

1. **`/v1/tools/call` 失败也返回 HTTP 200**，错误在 body 的 `status:"error"` 与 `error.code` 里（已实测）。只看 HTTP 状态码会把所有失败当成功。
2. **未匹配的路径返回 `index.html` + 200**，不是 404（SPA fallback 覆盖了 `/v1`）。验证新端点必须用精确路径，否则会被"看起来 200"骗过。
3. **`cargo build` 不会重建前端**：`#[folder = "../frontend/dist_new/"]` 是编译期嵌入，改了前端不重新构建就会嵌进旧 UI。构建顺序必须先 frontend 再 backend（`make build` 已保证）。
4. **`DefaultBodyLimit` 只作用于它上方已声明的路由**（`router.rs:38` 那行 `.layer()` 的位置），后续路由回到 2MB 默认值。需要大 body 的新路由要把 layer 放在路由声明之前。
5. **新增 hast 属性必须同步加进 `rehype-sanitize` schema**，否则被静默剥离、无报错。
6. **`node --test` 不解析 `@/` 别名**：任何被测试导入的模块，其静态 import 图必须用相对路径 + 显式 `.ts`。（生产代码里已为此写成动态 import，见 `useBookshelf.ts`。）
7. **大量前端测试是读 `.vue` 源码做正则断言的"结构契约"**（`tests/knowledgeWorkflow.test.ts` 等），不是行为测试。重命名函数、调整 class、改 aria-label 都会让它们失败——要**有意识地同步契约**，不要靠弱化断言"修复"。
8. 给 `renderMarkdown` 传函数型 resolver 却不传 `resourceContext` → **静默退回主线程渲染**（丢掉 Worker）。
9. 加迁移要同时改 `MIGRATIONS` 数组；播种 Skill 内容的迁移还要改 `run_migrations` 的版本条件 `match`。
10. 后台轮询必须调用 `canFocusDocument(document)`，有测试专门断言这条。

---

## 陈旧文件（不要相信它们的描述）

| 文件 | 问题 |
|---|---|
| `docs/DEVELOPMENT_PLAN.md`（2026-08-17） | 仍在规划 Qdrant + Embedding + RRF 混合搜索，是**已废弃方向**，且目录树是根级 `src/` |
| `AGENTS.md`（2026-08-18） | 与本文件旧版同源，描述的 `src/` 根布局不存在 |
| 根 `docker-compose.yml` | Qdrant 容器，已无代码使用 |
| `backend/Cargo.toml` 的 `tantivy`、`error.rs` 的 `QdrantError`/`EmbeddingError`、`config.rs` 的 `QdrantConfig` | 遗留物；实际检索是 FTS5 |
| `backend/src/core/chunker.rs` | 孤儿文件，从未声明 `mod`，从不参与编译 |
| `backend/src/core/markdown_parser.rs`、`backend/src/infra/file_watcher.rs` | 标了 `#[allow(dead_code)]`，未接线（file_watcher 在 `main.rs` 中无引用） |
| `backend/src/core/wiki/` | 空目录 |

判断某模块是否真在用，最可靠的方法是查 `mod.rs` 的声明与 `main.rs` 的接线，而不是读文件名。

---

## 开发规范

**流程**：先读 `docs/requirement/NN-*.md`（What & Why）与 `docs/development/NN-*.md`（How）再动手。需求不明确先 `/brainstorming`，多步任务先 `/writing-plans`，写码走 `/test-driven-development`，完成前 `/verification-before-completion`。实施计划与设计规格落在 `docs/superpowers/plans/` 与 `docs/superpowers/specs/`（`.superpowers/` 是 gitignore 的草稿区）。

**代码**：

- 生产代码禁止 `.unwrap()` / `.expect()`（测试除外）；错误用 `BrainError`（`backend/src/error.rs`），它实现了 `IntoResponse`，handler 可直接 `Result<_, BrainError>`。
- 遵循 Rust 2021、`cargo fmt`、clippy 零 warning；异步只用 Tokio。
- 用户可见文案（错误信息、UI）是**中文**。
- 测试命名 `test_<函数>_<场景>_<预期>`；单元测试内联在各模块的 `#[cfg(test)]`（无 `tests/` 目录），handler 集成测试可用 `AppContext::for_test()`（`backend/src/main.rs:636`）。

**提交**：Conventional Commits（`feat(wiki): ...`、`fix(reader): ...`）。提交前跑通 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 与前端 `npm test`。