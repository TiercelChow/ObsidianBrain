# ObsidianBrain — 顶层设计文档

> **版本**: v1.1 | **最后更新**: 2026-08-17 | **状态**: 持续演进

---

## 1. 项目概述

### 1.1 定位

ObsidianBrain 是一个运行在本地的 **Rust 知识引擎**，对外提供标准化的 LLM Tool API（兼容 MCP 协议与 OpenAI function calling 格式）。它围绕用户的 **Obsidian 知识库** 和 **本地代码仓库**，提供记忆管理、代码仓概览、灵感催化、外部信息聚合、时间线回顾和个人任务管理等能力。

**核心原则**：对话由 Claude / ChatGPT 等 LLM 前端完成，本引擎是 LLM 的 **"手"和"眼"**——负责感知（读取 vault、代码仓、外部信息）和执行（写入笔记、打开编辑器、保存文章）。

### 1.2 目标用户

个人知识工作者——同时使用 Obsidian 做知识管理、在本地维护多个代码仓库的开发者/研究者。

### 1.3 核心价值

| 痛点 | ObsidianBrain 的解决方式 |
|---|---|
| 笔记写了就忘，难以跨笔记关联 | 通过 Obsidian API 快速检索笔记，支持全文搜索和标签过滤 |
| 代码仓库与知识笔记割裂 | 代码仓 Hub 打通笔记 ↔ 仓库双向链接 |
| 缺乏跨界灵感触发 | 灵感熔炉主动制造知识碰撞 |
| 外部信息过载，手动筛选成本高 | 智识雷达基于个人知识图谱做个性化推荐 |
| 时间维度上的知识演变不可见 | 时间线回溯知识动态 |
| 短期待办容易遗忘，长期目标难以持续拆解和追踪 | 个人任务模块统一管理待办、任务树、进展和日历，并保存到 Obsidian |

### 1.4 非功能性需求

- **隐私优先**：所有数据本地存储，服务仅监听 `127.0.0.1`，不上传任何用户数据（LLM API 调用除外）。
- **低资源占用**：空闲时内存 < 50MB，CPU 接近零。
- **快速响应**：只读工具调用 P95 延迟目标 < 300ms；需要写入 Obsidian 的操作目标 < 500ms（不含外部异常重试与 LLM 调用）。
- **可靠运行**：单进程，panic 自动重启（通过 systemd/launchd），数据持久化不丢失。
- **易部署**：单一二进制 + Obsidian Local REST API 插件，无需额外服务。

---

## 2. 系统架构

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                    LLM 前端 (Claude / ChatGPT)               │
│              通过 HTTP Tool API 交互                          │
└──────────────────────────┬──────────────────────────────────┘
                           │  HTTP (127.0.0.1:9876)
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    ObsidianBrain Engine                       │
│  ┌───────────┐ ┌───────────┐                                │
│  │ API 网关   │ │ 工具注册表 │                                │
│  │ (Axum)    │ │ (Tool     │                                │
│  │           │ │  Registry)│                                │
│  └─────┬─────┘ └─────┬─────┘                                │
│        │             │                                      │
│  ┌─────┴─────────────┴────────────────────────────────────┐ │
│  │                    核心服务层                            │ │
│  │  ┌──────────────┐  ┌──────────────┐                    │ │
│  │  │ MemoryService│  │ TaskService  │                    │ │
│  │  └──────┬───────┘  └──────┬───────┘                    │ │
│  └─────────┼──────────────────────────────────────────────┘ │
│            │                                                │
│  ┌─────────┴──────────────────────────────────────────────┐ │
│  │                    基础设施层                            │ │
│  │  ┌──────────────────┐  ┌──────────────────┐            │ │
│  │  │ ObsidianClient   │  │ SQLite 投影索引   │            │ │
│  │  └──────────────────┘  └──────────────────┘            │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│          Obsidian (Local REST API Plugin)                    │
│          (HTTP 127.0.0.1:27123)                             │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 请求处理流程

```
LLM 调用 tool
    │
    ▼
API 网关接收 REST 请求
    │
    ▼
工具注册表查找对应 Tool Handler
    │
    ▼
Tool Handler 调用 MemoryService
    │
    ├─→ 搜索笔记 ──→ Obsidian API /search/simple
    ├─→ 读取笔记 ──→ Obsidian API /vault/{path} (GET)
    ├─→ 写入笔记 ──→ Obsidian API /vault/{path} (PUT/POST)
    ├─→ 删除笔记 ──→ Obsidian API /vault/{path} (DELETE)
    └─→ 列出文件 ──→ Obsidian API /vault/ (GET)
    │
    ▼
组装结果，返回结构化 JSON
    │
    ▼
LLM 整合结果，生成自然语言回复给用户
```

### 2.3 目录结构（实际实现）

```
ObsidianBrain/
├── Cargo.toml
├── docker-compose.yml          # Qdrant（可选，当前未使用）
├── config/
│   ├── default.toml            # 默认配置文件
│   └── radar_sources.toml      # 智识雷达源配置
├── docs/
│   └── top_design.md           # 本文档
├── migrations/                 # SQLite schema 迁移
├── backend/
│   └── src/
│       ├── main.rs             # 入口：启动、优雅关闭
│       ├── config.rs           # 配置加载与校验
│       ├── error.rs            # 统一错误类型
│       ├── api/                # API 层
│       │   ├── mod.rs
│       │   ├── router.rs       # Axum 路由定义
│       │   └── handlers/       # 各工具的请求处理器
│       ├── core/               # 核心服务层
│       │   ├── mod.rs
│       │   ├── memory_service.rs    # 记忆引擎（通过 Obsidian API）
│       │   ├── timeline.rs          # 时间线
│       │   ├── tasks/               # 个人任务管理
│       │   ├── code_repo/           # 代码仓管理
│       │   ├── inspiration/         # 灵感熔炉
│       │   └── radar/               # 智识雷达
│       ├── infra/              # 基础设施层
│       │   ├── mod.rs
│       │   ├── sqlite_store.rs      # SQLite 元数据存储
│       │   ├── file_watcher.rs      # notify 文件监控（用于 Timeline）
│       │   ├── obsidian_client.rs   # Obsidian Local REST API 客户端
│       │   └── llm_client.rs        # LLM 调用封装（多 provider）
│       ├── tools/              # 工具定义与注册
│       │   ├── mod.rs
│       │   ├── registry.rs          # 工具注册表
│       │   ├── definitions.rs       # 工具 schema 定义（JSON Schema）
│       │   └── handlers/            # 工具处理器
│       └── models/             # 共享数据模型
│           ├── mod.rs
│           ├── note.rs
│           ├── memory.rs
│           ├── repo.rs
│           ├── radar.rs
│           ├── inspiration.rs
│           ├── timeline.rs
│           └── task.rs
└── frontend/                   # Vue3 前端
    ├── src/
    │   ├── views/              # 页面组件
    │   ├── components/         # 通用组件
    │   ├── api/                # API 客户端
    │   └── ...
    └── ...
```

---

## 3. 技术栈

| 层次 | 组件 | 技术选型 | 选型理由 |
|---|---|---|---|
| Web 框架 | HTTP 服务 | **Axum 0.7 + Tokio** | 高性能异步框架，生态成熟，类型安全的路由 |
| 序列化 | JSON 处理 | **serde + serde_json** | Rust 标准选择 |
| Obsidian 集成 | 笔记操作 | **Obsidian Local REST API** | 直接通过 HTTP 调用 Obsidian 插件，无需本地索引 |
| HTTP 客户端 | API 调用 | **reqwest 0.12** | 异步 HTTP 客户端，支持连接池 |
| 数据库 | 元数据存储 | **SQLite** (rusqlite) | 轻量级嵌入式数据库，用于元数据和缓存 |
| Git 操作 | 代码仓信息 | **git2** | libgit2 绑定，无需系统 Git |
| 文件监控 | 文件变更检测 | **notify 6** | 跨平台文件监控，用于 Timeline 模块 |
| LLM 调用 | AI 能力 | **reqwest** (OpenAI/Ollama API) | 多 provider 支持，流式响应 |
| 配置管理 | 应用配置 | **config** crate | 支持 TOML/ENV 多来源配置 |
| 日志 | 运行日志 | **tracing** + **tracing-subscriber** | 结构化日志，支持 span 追踪 |
| 前端 | Web UI | **Vue 3 + Vite + Element Plus** | 现代化前端框架 |

### 3.1 部署架构

```
┌──────────────────────────────────────┐
│         ObsidianBrain                │
│                                      │
│  ┌──────────────┐                    │
│  │obsidianbrain │                    │
│  │  (Rust bin)  │                    │
│  │  127.0.0.1   │                    │
│  │  :9876       │                    │
│  └──────┬───────┘                    │
│         │                           │
│         ├── Config: ./config/       │
│         └── SQLite（可重建查询投影）  │
└─────────┼────────────────────────────┘
          │  HTTP (127.0.0.1:27123)
          ▼
┌──────────────────────────────────────┐
│  Obsidian (Local REST API Plugin)    │
│  (笔记存储由 Obsidian 管理)          │
└──────────────────────────────────────┘
```

**注意**：不再需要 Qdrant Docker 容器或笔记全文/向量索引。笔记与任务的权威内容由 Obsidian 管理；SQLite 只保存代码仓、雷达、时间线和任务等模块的可重建查询投影。

---

## 4. 数据模型

### 4.1 笔记 (Note)

```rust
struct Note {
    path: PathBuf,              // vault 内相对路径
    title: String,              // 从文件名或 frontmatter 提取
    content: String,            // 原始 Markdown 内容
    frontmatter: HashMap<String, serde_json::Value>,  // YAML frontmatter
    tags: Vec<String>,          // 标签列表（#tag 和 frontmatter.tags 合并）
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    word_count: usize,
}
```

### 4.2 记忆单元 (Memory)

当前实现中，记忆管理直接通过 Obsidian API 操作笔记，不维护独立的记忆索引。

```rust
// 通过 Obsidian API 进行笔记操作
// search_notes: Obsidian API /search/simple
// get_note: Obsidian API /vault/{path} (GET)
// write_note: Obsidian API /vault/{path} (PUT)
// delete_note: Obsidian API /vault/{path} (DELETE)
```

**注意**：当前版本不实现独立的记忆索引，所有搜索和检索都通过 Obsidian Local REST API 完成。这简化了架构，但搜索能力受限于 Obsidian 的搜索功能。

### 4.3 代码仓库 (CodeRepo)

```rust
struct CodeRepo {
    name: String,               // 显示名称（用户自定义）
    path: PathBuf,              // 本地绝对路径
    current_branch: String,
    language_stats: HashMap<String, f32>,  // 语言 → 占比
    is_dirty: bool,             // 有无未提交更改
    recent_commits: Vec<CommitSummary>,
    last_activity: DateTime<Utc>,
    linked_notes: Vec<PathBuf>, // 关联的笔记路径
    registered_at: DateTime<Utc>,
}

struct CommitSummary {
    hash: String,
    author: String,
    message: String,
    timestamp: DateTime<Utc>,
}
```

### 4.4 雷达条目 (RadarItem)

```rust
struct RadarItem {
    id: Uuid,
    title: String,
    summary: String,
    source: String,             // "arxiv" | "hackernews" | "reddit" | "rss:xxx"
    url: String,
    relevance_score: f32,       // 与用户知识的相似度（基于标签匹配）
    related_notes: Vec<PathBuf>,// 关联的笔记（基于标签匹配）
    status: RadarStatus,        // New | Read | Saved | Dismissed
    fetched_at: DateTime<Utc>,
    published_at: Option<DateTime<Utc>>,
}

enum RadarStatus { New, Read, Saved, Dismissed }
```

**注意**：当前版本的相关性计算基于标签匹配，不使用向量相似度。这简化了架构，但相关性精度受限于标签系统的覆盖度。

### 4.5 时间线事件 (TimelineEvent)

```rust
struct TimelineEvent {
    date: NaiveDate,
    event_type: EventType,      // NoteCreated | NoteModified | RepoCommit | RadarSaved
    title: String,
    summary: String,
    tags: Vec<String>,
    related_paths: Vec<PathBuf>,
}

enum EventType { NoteCreated, NoteModified, RepoCommit, RadarSaved, MemoryCreated }
```

### 4.6 个人任务 (Task)

```rust
struct TaskNode {
    id: Uuid,
    root_id: Uuid,
    parent_id: Option<Uuid>,
    kind: TaskKind,             // Short | Long
    role: TaskRole,             // Root | Subtask
    title: String,
    description: String,
    start_date: NaiveDate,
    end_date: NaiveDate,
    importance: TaskImportance, // Low | Normal | High | Urgent
    status: TaskStatus,
    position: i32,
    revision: i64,
}

struct ProgressEntry {
    id: Uuid,
    root_id: Uuid,
    task_id: Uuid,
    recorded_at: DateTime<Utc>,
    note: String,
    percent_after: Option<u8>,
}
```

短期待办按创建月份保存在 `Tasks/Short/YYYY-MM.md`；长期任务按根任务一文件保存在 `Tasks/Long/{slug}--{id8}.md`。Obsidian Markdown 是权威数据源，SQLite 仅保存可重建投影。完整字段和生命周期见 [任务需求设计](requirement/09-task-management.md)。

### 4.7 SQLite Schema

```sql
-- 代码仓库注册信息
CREATE TABLE code_repos (
    name        TEXT PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    registered_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    metadata    JSON            -- 缓存的元信息
);

-- 笔记与仓库的关联
CREATE TABLE note_repo_links (
    note_path   TEXT NOT NULL,
    repo_name   TEXT NOT NULL REFERENCES code_repos(name),
    linked_at   DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (note_path, repo_name)
);

-- 雷达条目缓存
CREATE TABLE radar_items (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    summary     TEXT,
    source      TEXT NOT NULL,
    url         TEXT NOT NULL UNIQUE,
    status      TEXT DEFAULT 'new',
    relevance_score REAL,
    fetched_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
    published_at DATETIME
);

-- 灵感历史记录
CREATE TABLE inspiration_history (
    id          TEXT PRIMARY KEY,
    type        TEXT NOT NULL,   -- "concept_combo" | "reverse_question" | "counterpoint"
    input_refs  JSON,            -- 输入的笔记/仓库引用
    output      TEXT NOT NULL,
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 应用状态与元信息
CREATE TABLE app_state (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

---

## 5. 功能模块详细设计

### 🧠 5.1 第二大脑引擎（核心底座）

这是整个系统的内核，其他所有模块的功能都通过它暴露给 LLM。

#### 5.1.1 记忆系统 (Memory)

**记忆单元**：当前实现中，记忆管理直接通过 Obsidian API 操作笔记，不维护独立的记忆索引。

**笔记操作流程**：

```
LLM 调用 tool
    │
    ▼
MemoryService 接收请求
    │
    ├─→ 搜索笔记 ──→ Obsidian API /search/simple
    ├─→ 读取笔记 ──→ Obsidian API /vault/{path} (GET)
    ├─→ 写入笔记 ──→ Obsidian API /vault/{path} (PUT)
    └─→ 删除笔记 ──→ Obsidian API /vault/{path} (DELETE)
```

**记忆操作工具**：

| 工具 | 参数 | 返回 | 说明 |
|---|---|---|---|
| `search_notes` | `query: string, top_k?: int, tags?: string[]` | `Note[]` | 通过 Obsidian API 搜索笔记 |
| `get_note` | `path: string` | `Note` | 读取笔记完整内容 |
| `list_recent_notes` | `days?: int, limit?: int` | `Note[]` | 列出最近修改的笔记 |
| `get_memory_stats` | 无 | `{total_notes, total_files, tags}` | 记忆库统计信息 |

**引用溯源**：每条搜索结果都附带 Obsidian URI：

```
obsidian://open?vault=<vault_name>&file=<encoded_path>
```

**搜索策略**：
1. 调用 Obsidian API `/search/simple` 进行全文搜索
2. 支持标签过滤
3. 返回 top_k 条结果，附带 Obsidian URI

**注意**：当前版本不实现独立的记忆索引，所有搜索和检索都通过 Obsidian Local REST API 完成。这简化了架构，但搜索能力受限于 Obsidian 的搜索功能。

#### 5.1.2 技能系统 (Skill)

**定义**：技能是一组预定义的后端操作编排，有明确的输入/输出描述，可直接映射为一个 LLM tool。

**内置技能**：

| 技能 ID | 描述 | 编排的操作 |
|---|---|---|
| `summarize_today_notes` | 总结今日修改/新增的笔记 | 文件监控日志 → 读取今日笔记 → LLM 摘要 |
| `weekly_review` | 生成本周知识动态摘要 | 时间线查询 → 统计聚合 → LLM 综述 |
| `find_inspiration` | 触发灵感熔炉 | 随机选取 → LLM 创意生成 |
| `fetch_radar` | 触发智识雷达 | 查询雷达缓存 → 排序过滤 |

**技能扩展（YAML 配置）**：

```yaml
# config/skills/my_skill.yml
name: "research_assistant"
description: "辅助研究：搜索相关笔记 + 查找外部论文 + 生成研究摘要"
parameters:
  topic:
    type: string
    description: "研究主题"
    required: true
  depth:
    type: string
    enum: ["quick", "deep"]
    default: "quick"
steps:
  - action: search_memory
    params:
      query: "{{topic}}"
      top_k: 10
  - action: get_radar
    params:
      query: "{{topic}}"
      limit: 5
  - action: llm_summarize
    params:
      context: "{{steps[0].results + steps[1].results}}"
      instruction: "基于以下资料，生成关于 '{{topic}}' 的研究摘要"
```

#### 5.1.3 工具协议 (Tool Protocol)

**同时支持两种协议**，通过配置选择：

**方式一：MCP (Model Context Protocol)**

实现标准 MCP Server，Claude Desktop / Claude Code 可直接连接：

```json
// Claude Desktop 配置
{
  "mcpServers": {
    "obsidian-brain": {
      "command": "/path/to/obsidianbrain",
      "args": ["serve", "--protocol", "mcp"]
    }
  }
}
```

**方式二：HTTP REST API（兼容 OpenAI function calling）**

```
GET  /v1/tools              → 返回所有可用工具的 JSON Schema 列表
POST /v1/tools/call         → 调用指定工具
GET  /v1/health             → 健康检查
```

工具调用请求格式：

```json
{
  "tool": "search_memory",
  "arguments": {
    "query": "Rust 异步编程",
    "top_k": 5
  }
}
```

工具调用响应格式：

```json
{
  "tool": "search_memory",
  "status": "success",
  "result": {
    "memories": [...],
    "total": 5
  }
}
```

**核心工具集完整列表**：

| 分类 | 工具名 | 参数 | 说明 |
|---|---|---|---|
| **笔记检索** | `search_notes` | `query, top_k?, tags?` | 通过 Obsidian API 搜索笔记 |
| | `get_note` | `path` | 获取笔记完整内容 |
| | `list_recent_notes` | `days?, limit?` | 列出最近修改的笔记 |
| **记忆管理** | `search_memory` | `query, top_k?, tags?` | 记忆语义搜索 |
| | `add_memory` | `note_path, content, tags?` | 添加记忆 |
| | `update_memory` | `memory_id, content` | 更新记忆 |
| | `forget_memory` | `memory_id` | 删除记忆 |
| **代码仓** | `add_code_repo` | `path, name` | 注册代码仓库 |
| | `list_code_repos` | 无 | 列出所有仓库摘要 |
| | `get_repo_detail` | `name` | 仓库详细信息 |
| | `link_note_to_repo` | `note_path, repo_name` | 笔记关联仓库 |
| | `generate_docs` | `repo_name, target_path?` | 自动生成文档笔记 |
| | `open_in_vscode` | `repo_name` | VSCode 打开仓库 |
| **时间线** | `get_timeline` | `start_date, end_date` | 查询时间线事件 |
| **个人任务** | `create_task` | `kind, title, description?, start_date, end_date, importance` | 创建短期或长期任务 |
| | `list_tasks` | `filters?, sort?, cursor?, limit?` | 查询任务列表 |
| | `get_task` | `task_id, include_tree?, include_progress?` | 获取任务详情 |
| | `update_task` | `task_id, patch, expected_version` | 编辑任务字段 |
| | `set_task_status` | `task_id, status, closure_note?, cascade?, expected_version` | 关闭、完成、取消或重新打开 |
| | `add_subtask` | `parent_id, fields, expected_version` | 拆解长期任务 |
| | `move_subtask` | `task_id, new_parent_id, position, expected_version` | 移动或排序子任务 |
| | `add_task_progress` | `task_id, note, percent_after?, expected_version` | 追加任务进展 |
| | `get_task_calendar` | `start_date, end_date, filters?` | 查询日历范围内的任务 |
| | `archive_task` | `task_id, archived, expected_version` | 归档或恢复任务 |
| | `sync_tasks` | `dry_run?` | 从 Obsidian 刷新任务索引 |
| **灵感** | `get_inspiration` | `type?, note_path?` | 获取灵感（概念碰撞/反向提问/对立观点） |
| **雷达** | `get_radar` | `limit?, query?` | 获取外部信息推荐 |
| | `add_to_vault` | `article_id, target_dir?` | 文章保存到 vault |
| | `dismiss_radar_item` | `article_id` | 标记为已忽略 |
| **系统** | `get_stats` | 无 | 系统统计信息 |

---

### 📅 5.2 时间线 (Timeline)

#### 数据源

| 来源 | 提取方式 | 示例 |
|---|---|---|
| 笔记 frontmatter | `date` / `created` / `modified` 字段 | `date: 2026-05-28` |
| 文件名 | 正则匹配日期模式 | `2026-05-28-meeting.md` |
| 内容标签 | `#date/YYYY-MM-DD` 格式 | `#date/2026-05-28` |
| 文件监控日志 | 实时记录文件变更事件 | 文件创建/修改时间戳 |
| Git commit 记录 | 已注册仓库的提交历史 | commit timestamp |

#### 工具

```
get_timeline(start_date: "2026-05-01", end_date: "2026-05-28")
```

返回结构：

```json
{
  "events": [
    {
      "date": "2026-05-28",
      "events": [
        {
          "type": "note_modified",
          "title": "Rust 异步编程笔记",
          "summary": "新增了 tokio select! 的使用场景章节",
          "tags": ["rust", "async"],
          "path": "programming/rust-async.md"
        }
      ]
    }
  ],
  "summary": "本月共新增 12 篇笔记，修改 34 次，新增 2 个代码仓..."
}
```

#### 与其他模块的协作

- **灵感熔炉**：可查询"去年今日的笔记"作为创意素材
- **周报生成**：`weekly_review` 技能基于时间线数据聚合
- **雷达**：时间线事件作为用户兴趣漂移的参考信号

---

### 📦 5.3 本地代码仓管理 (Code Repository Hub)

#### 定位

轻量级代码仓聚合面板。**不涉及代码语义检索**，只做仓库元信息展示、笔记关联、一键跳转和自动文档化。

#### 5.3.1 仓库注册

```
add_code_repo(path: "/Users/me/projects/my-app", name: "my-app")
```

注册流程：
1. 校验路径是否存在且为 Git 仓库
2. 通过 git2 提取元数据（分支、最近提交、语言统计）
3. 写入 SQLite `code_repos` 表
4. 设置文件监控，跟踪仓库 `.git/HEAD` 变更

#### 5.3.2 仓库卡片信息

`list_code_repos` / `get_repo_detail` 返回的信息：

```json
{
  "name": "my-app",
  "path": "/Users/me/projects/my-app",
  "current_branch": "main",
  "is_dirty": false,
  "languages": {"Rust": 0.72, "TypeScript": 0.18, "Python": 0.10},
  "recent_commits": [
    {"hash": "a1b2c3d", "author": "TiercelChow", "message": "feat: add auth module", "time": "2026-05-28T14:30:00Z"}
  ],
  "last_activity": "2026-05-28T14:30:00Z",
  "linked_notes": ["projects/my-app-notes.md"],
  "vscode_uri": "vscode://file/Users/me/projects/my-app"
}
```

#### 5.3.3 笔记 ↔ 仓库关联

**手动关联**：`link_note_to_repo(note_path, repo_name)` 在笔记末尾插入标准引用块：

```markdown

---
## 🔗 相关代码仓库
- **my-app** — `/Users/me/projects/my-app`
  - [在 VSCode 中打开](vscode://file/Users/me/projects/my-app)
  - 最后活动: 2026-05-28 | 分支: main
```

**自动关联建议**：后端对比笔记关键词与仓库名 / commit message 的相似度。当检测到高匹配时，在工具返回结果中附带建议，由 LLM 向用户提示。

#### 5.3.4 自动文档化

`generate_docs(repo_name, target_path?)` 流程：

```
1. 提取仓库信息：
   ├── 目录结构（排除 .git, node_modules, target 等）
   ├── README 内容
   ├── Cargo.toml / package.json 等配置
   └── 核心源文件头部注释

2. 组装 prompt → 调用 LLM 生成文档

3. 输出到 vault：
   └── <vault>/<target_dir>/<repo_name>-docs.md
       包含：项目概述、目录结构说明、核心模块、技术栈、依赖列表
```

文档模板可配置（通过 `config/doc_template.md`）。

---

### ⚡ 5.4 灵感熔炉 (Inspiration Forge)

#### 目标

故意制造"知识碰撞"——用用户自己的笔记和代码库为原料，产生新想法。

#### 5.4.1 三种灵感模式

| 模式 | `type` 参数 | 机制 |
|---|---|---|
| 🎲 随机概念组合 | `"concept_combo"` | 从 vault 标签/关键词和仓库名中随机抽取两个距离较远的概念，LLM 生成跨界联想 |
| ❓ 反向提问 | `"reverse_question"` | 选取一篇笔记（指定或最近修改），LLM 生成 3 个用户可能没想过的问题 |
| ⚔️ 对立观点 | `"counterpoint"` | 对指定笔记生成反方观点和逻辑漏洞分析 |

#### 5.4.2 随机概念组合算法

```
1. 构建概念池：
   ├── vault 所有标签（按 TF-IDF 加权）
   ├── 仓库名称 + 主要技术栈
   └── 近期高频关键词（从最近 30 天笔记提取）

2. 选取两个概念：
   ├── 第一个：随机选取
   └── 第二个：与第一个的标签共现度最低的 top-10 中随机选取
       （确保"距离远"但非完全无关）

3. LLM 生成创意 prompt：
   "概念 A: {concept_a} (来源: {source_a})
    概念 B: {concept_b} (来源: {source_b})
    请提出一个将这两个概念结合的创新想法，
    并给出具体的实践建议。"

4. 附带相关笔记和代码的 Obsidian 链接
```

#### 5.4.3 工具调用

```
get_inspiration(type: "concept_combo")
get_inspiration(type: "reverse_question", note_path: "essays/sleep-experiment.md")
get_inspiration(type: "counterpoint", note_path: "essays/ai-future.md")
```

返回示例：

```json
{
  "type": "concept_combo",
  "concept_a": {"term": "缓存替换策略", "source": "note: cs/cache-algorithms.md"},
  "concept_b": {"term": "睡眠实验", "source": "note: life/sleep-experiment.md"},
  "inspiration": "试着用 LRU 缓存的思路优化你的睡眠实验数据记录：将最近 7 天的数据视为'热数据'保持高频记录，超过 7 天的自动降级为每周摘要——正如缓存淘汰冷数据...",
  "related_links": [
    "obsidian://open?vault=brain&file=cs/cache-algorithms.md",
    "obsidian://open?vault=brain&file=life/sleep-experiment.md"
  ]
}
```

---

### 📡 5.5 智识雷达 (Knowledge Radar)

#### 目标

让外部信息来找你的笔记，而不是你去搜索——基于你的知识图谱做个性化推荐。

#### 5.5.1 外部源管理

配置文件 `config/radar_sources.toml`：

```toml
[[sources]]
name = "hackernews"
type = "hackernews"
enabled = true
filter = "score > 50"

[[sources]]
name = "arxiv-cs"
type = "arxiv"
enabled = true
categories = ["cs.AI", "cs.CL", "cs.SE"]
query = "LLM OR large language model OR retrieval"

[[sources]]
name = "tech-rss"
type = "rss"
enabled = true
feeds = [
    "https://blog.rust-lang.org/feed.xml",
    "https://simonwillison.net/atom/everything/",
]

[[sources]]
name = "reddit-programming"
type = "reddit"
enabled = false
subreddits = ["programming", "rust"]
```

**定时拉取**：通过 tokio-cron-scheduler 每 6 小时执行一次（可配置）。

#### 5.5.2 个性化相关性排序

```
1. 构建用户兴趣向量：
   ├── 最近 30 天活跃笔记的 embedding 加权平均
   ├── 权重：越近越高，access_count 越高权重越大
   └── 标签频率作为辅助信号

2. 新文章处理：
   ├── 提取标题 + 摘要 → 生成 embedding
   ├── 与用户兴趣向量计算余弦相似度
   └── 过滤：相似度 > 阈值（默认 0.7）且不在已读/已忽略列表

3. 排序：
   ├── 主排序：语义相似度
   ├── 加权：来源可信度、时效性（越新越高）
   └── 去重：与已有笔记内容重复的降权
```

#### 5.5.3 工具调用

```
get_radar(limit: 5)
```

返回：

```json
{
  "items": [
    {
      "id": "radar-xxxx",
      "title": "Retrieval-Augmented Generation for Large Language Models: A Survey",
      "summary": "本文系统综述了 RAG 技术的最新进展...",
      "source": "arxiv",
      "url": "https://arxiv.org/abs/xxxx.xxxxx",
      "relevance_score": 0.89,
      "related_notes": ["ai/rag-notes.md", "ai/llm-architecture.md"],
      "published_at": "2026-05-25"
    }
  ]
}
```

**一键纳藏**：`add_to_vault(article_id, target_dir?)`

- 下载文章正文（readability 提取）
- 生成 Obsidian 笔记（含来源、链接、自动摘要）
- 写入 vault 指定目录（默认 `radar/`）
- 记忆系统自动索引

---

### ✅ 5.6 个人任务管理 (Tasks)

#### 定位

用两种清晰的心智模型覆盖个人工作安排：短期待办强调快速记录和关闭说明，长期任务强调多级拆解和进展追踪。任务视图与日历视图消费同一数据模型。

#### 核心能力

- 短期待办：标题、描述、开始/结束日期、重要程度、完成/取消及关闭说明。
- 长期任务：根任务与多级子任务共享通用字段，每个节点可追加进展。
- 任务视图：桌面端 master-detail，手机端列表进入独立详情与底部操作面板。
- 日历视图：桌面端月历加日程侧栏，手机端紧凑月历加选中日日程。
- 可靠存储：Obsidian 先写、SQLite 后索引，revision + 内容哈希冲突检测，索引可从 Vault 重建。

#### 存储约定

```text
Tasks/
├── Short/YYYY-MM.md
└── Long/{slug}--{id8}.md
```

首版日期语义为本地全天日期；不包含提醒、重复规则、任务依赖和外部日历同步。详细产品边界见 [需求设计](requirement/09-task-management.md)，实现方案见 [开发设计](development/09-task-management.md)。

---

## 6. 配置规范

主配置文件 `config/default.toml`：

```toml
[server]
host = "127.0.0.1"
port = 9876
protocol = "mcp"              # "mcp" | "http" | "both"

[vault]
path = "/Users/me/ObsidianVault"
name = "brain"
watch_enabled = true
exclude_patterns = [".obsidian/", "templates/", ".trash/"]

[qdrant]
url = "http://127.0.0.1:6333"
collection_name = "obsidian_brain"
vector_size = 1536            # OpenAI text-embedding-3-small 维度

[embedding]
provider = "openai"           # "openai" | "ollama" | "onnx"
model = "text-embedding-3-small"
api_key_env = "OPENAI_API_KEY"
# ollama 配置
# provider = "ollama"
# model = "nomic-embed-text"
# base_url = "http://127.0.0.1:11434"

[llm]
provider = "openai"           # "openai" | "anthropic" | "ollama"
model = "gpt-4o-mini"
api_key_env = "OPENAI_API_KEY"
max_tokens = 2048
temperature = 0.7

[memory]
chunk_min_tokens = 300
chunk_max_tokens = 800
search_top_k = 5

[timeline]
date_formats = ["%Y-%m-%d", "%Y/%m/%d", "%Y%m%d"]

[tasks]
vault_root = "Tasks"
max_tree_depth = 20
max_nodes_per_document = 5000
default_page_size = 50
calendar_max_range_days = 366

[radar]
fetch_interval_hours = 6
relevance_threshold = 0.7
max_items_per_source = 20
readability_enabled = true    # 文章正文提取

[storage]
db_path = "./data/brain.db"
index_path = "./data/tantivy_index"

[logging]
level = "info"                # "trace" | "debug" | "info" | "warn" | "error"
file = "./data/obsidianbrain.log"
```

---

## 7. 错误处理策略

### 7.1 统一错误类型

```rust
enum BrainError {
    // 配置与启动
    ConfigError(String),

    // Vault 相关
    VaultNotFound(PathBuf),
    NoteNotFound(PathBuf),
    ParseError { path: PathBuf, detail: String },

    // 搜索相关
    SearchError(String),

    // 代码仓相关
    RepoNotFound(PathBuf),
    GitError { path: PathBuf, detail: String },

    // 外部服务
    ObsidianApiError(String),
    LlmApiError { provider: String, detail: String },
    FetchError { url: String, detail: String },

    // 通用
    IoError(std::io::Error),
    Internal(String),
}
```

### 7.2 错误响应格式

工具调用出错时返回结构化错误，由 LLM 决定如何向用户解释：

```json
{
  "tool": "get_repo_detail",
  "status": "error",
  "error": {
    "code": "REPO_NOT_FOUND",
    "message": "代码仓库 'my-app' 未找到",
    "suggestion": "请先使用 add_code_repo 注册仓库，或使用 list_code_repos 查看已注册仓库"
  }
}
```

### 7.3 容错与降级

| 场景 | 处理方式 |
|---|---|
| Obsidian API 不可用 | 返回错误，提示用户检查 Obsidian 插件是否启用 |
| LLM API 失败 | 返回错误 + 建议（"请稍后重试或使用更小的模型"） |
| 文件监控丢失 | 自动重连，全量扫描补偿 |
| Git 仓库路径失效 | 标记为 inactive，不删除注册信息 |

---

## 8. 性能设计

### 8.1 关键指标

| 操作 | 目标延迟 | 策略 |
|---|---|---|
| 笔记搜索 | < 200ms | 通过 Obsidian API 搜索 |
| 笔记读取 | < 50ms | 通过 Obsidian API 读取 |
| 仓库元信息 | < 100ms | SQLite 缓存 + 增量更新 |
| 文件变更处理 | < 500ms | 防抖 (300ms) + 事件过滤 |
| LLM 调用 | 取决于 API | 流式输出 + 超时控制 (30s) |

### 8.2 优化策略

- **连接池**：reqwest 连接池复用 HTTP 连接
- **缓存层**：仓库元信息、雷达结果等缓存在 SQLite，TTL 可控
- **懒加载**：雷达源仅在定时任务时拉取，不阻塞主流程
- **防抖**：文件变更事件防抖，避免频繁触发

---

## 9. Obsidian 插件协作策略

以下现有插件可以补充或替代部分功能。**建议策略**：先用插件覆盖不需定制的功能，把引擎精力聚焦在差异化价值上。

### 9.1 可直接复用的插件（无需自行实现）

| 功能 | 推荐插件 | 说明 |
|---|---|---|
| Vault 版本管理 | **Obsidian Git** | 自动 commit/push vault，不重复造轮子 |
| 网页剪藏 | **ReadItLater** | 手动剪藏网页到 vault |
| 随机笔记 | **Smart Random Note** | 快速随机打开笔记 |
| 结构化查询 | **Dataview** | 前端展示层，消费引擎的结构化 API |

### 9.2 可参考设计的插件

| 插件 | 参考价值 |
|---|---|
| **Omnisearch** | 全文 + 语义搜索的 UX 设计基线 |
| **Text Generator** | LLM 模板和命令的交互模式 |
| **Obsidian Memos** | 快速捕捉灵感的卡片式 UI |
| **Code Emitter** | 代码块执行与内联结果 |

### 9.3 本系统的差异化价值（插件无法替代）

- ✅ 统一的 Tool API 层（LLM 直接调用）
- ✅ 代码仓卡片信息与笔记双向关联
- ✅ 自动文档化（LLM 生成项目文档笔记）
- ✅ 灵感熔炉的个性化跨界组合
- ✅ 智识雷达的语义相关性排序
- ✅ 跨模块的技能编排

---

## 10. 实施路线图

### Phase 0: 基础设施搭建 ✅ 已完成

- [x] Task 1: 配置系统（config crate + TOML 解析 + 环境变量覆盖 + 校验）
- [x] Task 2: SQLite 元数据存储（rusqlite WAL + 迁移框架 + 7 张表）
- [x] Task 3: 文件监控（notify + 300ms 防抖 + .md 过滤）
- [x] Task 4: Obsidian Local REST API 客户端
- [x] Task 5: LLM Client（trait + OpenAI + Ollama + 流式 SSE）
- [x] Task 6: 集成到 AppContext + 健康检查

**注意**：当前架构使用 Obsidian Local REST API 进行笔记操作，不实现 Tantivy/Qdrant/Embedding 混合搜索架构。

### Phase 1: 核心引擎 MVP ✅ 已完成

- [x] 通过 Obsidian API 实现笔记操作
- [x] HTTP Tool API 基础协议
- [x] 核心工具：`search_notes`, `get_note`, `list_recent_notes`, `get_memory_stats`
- [x] Vue3 前端基础框架

**里程碑**：在 Claude 中通过 Tool API 搜索 Obsidian 笔记并获得结果 ✅

### Phase 2: 代码仓 + 时间线 ✅ 已完成

- [x] 代码仓注册与元信息提取 (git2)
- [x] 仓库卡片信息展示
- [x] 笔记 ↔ 仓库关联
- [x] VSCode 跳转
- [x] 时间线数据收集与查询
- [x] 工具：`add_code_repo`, `list_code_repos`, `get_repo_detail`, `link_note_to_repo`, `get_linked_notes`, `open_in_vscode`, `get_timeline`

### Phase 3: 灵感 + 雷达 ✅ 已完成

- [x] 灵感熔炉三种模式实现（LLM 生成）
- [x] 雷达外部源拉取（RSS、HN、arXiv、Reddit）
- [x] 基于标签的相关性排序
- [x] 文章纳藏到 vault
- [x] 工具：`get_inspiration`, `get_radar`, `add_to_vault`, `dismiss_radar_item`

### Phase 4: 打磨与增强（持续）

- [ ] 技能 YAML 扩展系统
- [ ] 用户兴趣漂移追踪
- [ ] 性能优化与内存调优
- [ ] 完善的文档与使用指南
- [ ] 考虑 Obsidian 插件端（可选，作为前端增强）

### Phase 5: 个人任务管理（MVP 已实现）

- [x] 任务领域模型、Markdown codec 与 SQLite 可重建投影
- [x] 短期待办创建、编辑、关闭、重开和归档
- [x] 长期任务多级拆解、移动、进展和状态管理
- [x] 任务视图（桌面与手机）
- [x] 日历视图（桌面与手机）
- [x] 文档版本冲突、同步恢复和异常文件处理
- [x] 11 个任务 Tool API

**设计文档**：[需求设计](requirement/09-task-management.md) · [开发设计](development/09-task-management.md)

---

## 附录 A：MCP 协议适配示例

作为 MCP Server 的工具定义格式：

```json
{
  "tools": [
    {
      "name": "search_notes",
      "description": "在 Obsidian vault 中搜索笔记。支持全文搜索和语义搜索，返回匹配的笔记片段及其来源链接。",
      "inputSchema": {
        "type": "object",
        "properties": {
          "query": {
            "type": "string",
            "description": "搜索查询词"
          },
          "top_k": {
            "type": "integer",
            "description": "返回结果数量，默认 5",
            "default": 5
          },
          "tags": {
            "type": "array",
            "items": {"type": "string"},
            "description": "按标签过滤"
          }
        },
        "required": ["query"]
      }
    }
  ]
}
```

## 附录 B：技术风险与对策

| 风险 | 影响 | 对策 |
|---|---|---|
| OpenAI Embedding API 费用 | 大量笔记首次索引费用较高 | 增量索引 + 本地 Embedding 备选方案 |
| Qdrant 容器依赖 | 增加部署复杂度 | 提供 `docker compose up` 一键启动；后续评估内嵌向量库 |
| 中文分词质量 | 影响搜索准确性 | jieba-rs + 自定义词典（从 vault 标签生成） |
| LLM 生成质量不稳定 | 文档化/灵感输出可能低质量 | 多 prompt 模板 + 用户反馈调整 + 输出后处理 |
| 大 vault 性能 | 数万篇笔记索引/搜索变慢 | 增量索引 + Tantivy 分片 + Qdrant 分区 |
