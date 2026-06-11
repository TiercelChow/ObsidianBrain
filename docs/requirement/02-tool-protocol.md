# Tool Protocol & API 层 — 需求设计文档

> **版本**: v0.1-draft | **最后更新**: 2026-05-29 | **状态**: 设计中  
> **上游依赖**: [顶层设计文档](../top_design.md) §2.1 架构图、§5.1.3 工具协议  
> **对应开发文档**: [开发设计文档](../development/02-tool-protocol.md)

---

## 1. 模块概述

### 1.1 定位

Tool Protocol & API 层（以下简称"协议层"）是 ObsidianBrain 引擎的**统一对外接口层**，承担 LLM 前端（Claude Desktop、ChatGPT、自定义客户端）与后端核心服务之间的桥梁角色。

```
┌──────────────────────────────────────────────────────────┐
│                  LLM 前端 (Claude / ChatGPT / 自定义)      │
│              MCP (stdio/SSE)  │  HTTP REST API            │
└──────────────────┬────────────┴───────────┬──────────────┘
                   │                        │
                   ▼                        ▼
         ┌─────────────────────────────────────────┐
         │        Tool Protocol & API 层            │
         │  ┌──────────┐  ┌───────────┐            │
         │  │MCP Server│  │HTTP Handler│            │
         │  └────┬─────┘  └─────┬─────┘            │
         │       └──────┬───────┘                  │
         │              ▼                          │
         │       ┌──────────────┐                  │
         │       │ Tool Registry│ ← 工具注册表      │
         │       └──────┬───────┘                  │
         │              ▼                          │
         │       ┌──────────────┐                  │
         │       │Tool Handlers │ ← 工具执行器      │
         │       └──────────────┘                  │
         └─────────────────────────────────────────┘
                          │
                          ▼
              ┌───────────────────────┐
              │   核心服务层 (Services) │
              │  Memory / Timeline /  │
              │  CodeRepo / Radar /   │
              │  Inspiration          │
              └───────────────────────┘
```

### 1.2 核心职责

| 职责 | 说明 |
|---|---|
| **协议适配** | 同时支持 MCP（Model Context Protocol）和 HTTP REST 两种协议，屏蔽底层差异 |
| **工具发现** | 向 LLM 暴露所有可用工具的 JSON Schema 描述，使 LLM 能自主决定调用哪个工具 |
| **工具调用** | 接收 LLM 的工具调用请求，校验参数，路由到对应的 Handler，返回结构化结果 |
| **请求标准化** | 统一请求/响应 JSON 格式、统一错误码体系，不论协议入口如何 |
| **日志审计** | 记录每一次工具调用的完整生命周期（请求参数、执行耗时、响应结果、错误信息） |

### 1.3 设计原则

1. **协议无关性**：后端 Tool Handler 不感知请求来自 MCP 还是 HTTP，统一通过内部调用接口执行。
2. **Schema 驱动**：所有工具通过 JSON Schema 自描述，前端/LLM 可通过 API 动态发现工具。
3. **失败透明**：工具执行失败时返回结构化错误信息（含建议），不暴露内部堆栈，由 LLM 决定如何向用户解释。
4. **单一职责**：协议层只做"翻译 + 路由 + 格式化"，不包含业务逻辑。

---

## 2. 功能需求

### 2.1 MCP 协议支持

#### 2.1.1 MCP Server 实现

| 需求编号 | 需求描述 | 优先级 |
|---|---|---|
| FR-MCP-01 | 实现标准 MCP Server，兼容 [Model Context Protocol](https://modelcontextprotocol.io) 规范 | P0 |
| FR-MCP-02 | 支持 `initialize` 握手，返回 Server 能力声明（`capabilities.tools.listChanged: true`） | P0 |
| FR-MCP-03 | 支持 `tools/list` 方法，返回所有已注册工具的完整 JSON Schema 列表 | P0 |
| FR-MCP-04 | 支持 `tools/call` 方法，根据工具名路由到对应 Handler 并返回结果 | P0 |
| FR-MCP-05 | 支持 `ping` 方法，用于连接保活 | P1 |
| FR-MCP-06 | 支持 `notifications/tools/list_changed` 通知，当工具集动态变更时主动告知客户端 | P2 |

#### 2.1.2 传输层支持

| 需求编号 | 需求描述 | 优先级 |
|---|---|---|
| FR-MCP-07 | 支持 **stdio 传输模式**：通过 stdin/stdout 交换 JSON-RPC 2.0 消息，作为 Claude Desktop 的 MCP Server 运行 | P0 |
| FR-MCP-08 | 支持 **SSE 传输模式**（可选）：通过 HTTP Server-Sent Events + POST 端点交换消息，供 Web 前端或自定义客户端使用 | P2 |

#### 2.1.3 Claude Desktop 集成

| 需求编号 | 需求描述 | 优先级 |
|---|---|---|
| FR-MCP-09 | 支持通过命令行参数 `obsidianbrain serve --protocol mcp` 启动 stdio 模式的 MCP Server | P0 |
| FR-MCP-10 | Claude Desktop 配置示例可正常连接并发现所有工具 | P0 |

### 2.2 HTTP REST API 支持

#### 2.2.1 端点定义

| 需求编号 | 端点 | 方法 | 描述 | 优先级 |
|---|---|---|---|---|
| FR-HTTP-01 | `/v1/tools` | GET | 返回所有可用工具的 JSON Schema 列表（兼容 OpenAI function calling 格式） | P0 |
| FR-HTTP-02 | `/v1/tools/call` | POST | 调用指定工具，传入参数，返回执行结果 | P0 |
| FR-HTTP-03 | `/v1/health` | GET | 健康检查，返回服务状态与各组件连通性 | P0 |
| FR-HTTP-04 | `/v1/tools/{tool_name}` | GET | 返回单个工具的详细 Schema 定义 | P2 |

#### 2.2.2 OpenAI 兼容格式

| 需求编号 | 需求描述 | 优先级 |
|---|---|---|
| FR-HTTP-05 | `GET /v1/tools` 响应格式可被 OpenAI API 的 `tools` 参数直接消费（`type: "function"` 包装） | P1 |
| FR-HTTP-06 | `POST /v1/tools/call` 的请求体兼容 OpenAI `tool_calls` 中的参数格式 | P1 |

### 2.3 工具注册表 (Tool Registry)

| 需求编号 | 需求描述 | 优先级 |
|---|---|---|
| FR-REG-01 | 启动时自动扫描并注册所有内置工具，无需手动配置 | P0 |
| FR-REG-02 | 支持运行时动态注册新工具（为未来 YAML 技能扩展预留） | P1 |
| FR-REG-03 | 支持运行时注销工具 | P1 |
| FR-REG-04 | 每个工具注册时必须包含：名称（name）、描述（description）、输入参数 JSON Schema（inputSchema）、所属模块标签（module） | P0 |
| FR-REG-05 | 工具名称全局唯一，重复注册时以最新版本覆盖并记录告警日志 | P1 |
| FR-REG-06 | 支持通过模块标签（module）过滤工具列表（如仅返回"memory"模块的工具） | P2 |

### 2.4 请求路由与分发

| 需求编号 | 需求描述 | 优先级 |
|---|---|---|
| FR-ROUTE-01 | 根据请求中的工具名称（`tool` / `name` 字段）在注册表中查找并路由到对应 Handler | P0 |
| FR-ROUTE-02 | 工具未找到时返回标准化错误（`TOOL_NOT_FOUND`），附带可用工具列表提示 | P0 |
| FR-ROUTE-03 | 支持工具调用超时控制（默认 30 秒，可按工具配置） | P1 |
| FR-ROUTE-04 | 支持并发工具调用（多个工具调用请求可并行执行） | P1 |

### 2.5 请求/响应标准化

| 需求编号 | 需求描述 | 优先级 |
|---|---|---|
| FR-STD-01 | 所有工具调用响应遵循统一的 JSON 信封格式：`{ "tool": string, "status": "success" \| "error", "result"?: any, "error"?: ErrorObj }` | P0 |
| FR-STD-02 | 错误对象包含三个字段：`code`（机器可读错误码）、`message`（人类可读描述）、`suggestion`（修复建议，可选） | P0 |
| FR-STD-03 | 定义完整的错误码体系，覆盖所有可能的错误场景（见 §6） | P0 |
| FR-STD-04 | MCP 协议下的 `tools/call` 结果需转换为 MCP 规范的 `CallToolResult` 格式（`content` 数组 + `isError` 标志） | P0 |

### 2.6 工具调用日志与审计

| 需求编号 | 需求描述 | 优先级 |
|---|---|---|
| FR-LOG-01 | 每次工具调用记录结构化日志：工具名、请求参数（脱敏）、执行耗时、响应状态、错误信息 | P0 |
| FR-LOG-02 | 日志通过 `tracing` crate 输出，支持 `info` / `debug` / `trace` 级别控制 | P0 |
| FR-LOG-03 | `debug` 级别记录完整请求/响应 JSON；`info` 级别仅记录工具名、耗时、状态 | P0 |
| FR-LOG-04 | 支持将工具调用日志持久化到 SQLite（可选，用于后续分析和审计） | P2 |

---

## 3. 完整工具清单

> 参考顶层设计文档 §5.1.3 中的工具集定义。以下为本协议层需要暴露的全部工具。

### 3.1 笔记检索模块

| 工具名 | 参数 | 返回值 | 描述 | 所属模块 |
|---|---|---|---|---|
| `search_notes` | `query: string`（必填）, `top_k: int?`（默认5）, `tags: string[]?` | `{ notes: NoteResult[], total: int }` | 通过 Obsidian API 搜索 vault 中的笔记，返回匹配片段及来源链接 | `search` |
| `get_note` | `path: string`（必填） | `{ note: Note }` | 获取指定路径笔记的完整内容（含 frontmatter、标签、正文） | `search` |
| `list_recent_notes` | `days: int?`（默认7）, `limit: int?`（默认20） | `{ notes: NoteSummary[] }` | 列出最近修改/创建的笔记列表 | `search` |

### 3.2 记忆管理模块

| 工具名 | 参数 | 返回值 | 描述 | 所属模块 |
|---|---|---|---|---|
| `search_memory` | `query: string`（必填）, `top_k: int?`（默认5）, `tags: string[]?` | `{ memories: Memory[], total: int }` | 记忆搜索，通过 Obsidian API 搜索笔记内容 | `memory` |
| `add_memory` | `note_path: string`（必填）, `content: string`（必填）, `tags: string[]?` | `{ memory: Memory }` | 手动添加记忆单元（写入笔记内容） | `memory` |
| `update_memory` | `memory_id: string`（必填）, `content: string`（必填） | `{ memory: Memory }` | 更新记忆内容 | `memory` |
| `forget_memory` | `memory_id: string`（必填） | `{ deleted: bool }` | 删除指定记忆（从笔记中移除） | `memory` |
| `get_memory_stats` | 无 | `{ total: int, by_tag: object, recent_count: int }` | 获取记忆库统计信息 | `memory` |

### 3.3 代码仓管理模块

| 工具名 | 参数 | 返回值 | 描述 | 所属模块 |
|---|---|---|---|---|
| `add_code_repo` | `path: string`（必填）, `name: string`（必填） | `{ repo: CodeRepo }` | 注册本地代码仓库，提取元数据 | `code_repo` |
| `list_code_repos` | 无 | `{ repos: CodeRepoSummary[] }` | 列出所有已注册仓库的摘要信息 | `code_repo` |
| `get_repo_detail` | `name: string`（必填） | `{ repo: CodeRepo }` | 获取指定仓库的详细信息（分支、提交、语言统计等） | `code_repo` |
| `link_note_to_repo` | `note_path: string`（必填）, `repo_name: string`（必填） | `{ linked: bool }` | 将笔记与代码仓库关联（在笔记末尾插入关联块） | `code_repo` |
| `generate_docs` | `repo_name: string`（必填）, `target_path: string?` | `{ note_path: string, content_preview: string }` | 自动生成仓库文档笔记（LLM 生成） | `code_repo` |
| `open_in_vscode` | `repo_name: string`（必填） | `{ opened: bool, vscode_uri: string }` | 通过 VSCode 打开指定仓库 | `code_repo` |

### 3.4 时间线模块

| 工具名 | 参数 | 返回值 | 描述 | 所属模块 |
|---|---|---|---|---|
| `get_timeline` | `start_date: string`（必填）, `end_date: string`（必填） | `{ events: TimelineDay[], summary: string }` | 查询指定日期范围内的时间线事件 | `timeline` |

### 3.5 灵感熔炉模块

| 工具名 | 参数 | 返回值 | 描述 | 所属模块 |
|---|---|---|---|---|
| `get_inspiration` | `type: string?`（默认"concept_combo"，可选"reverse_question"/"counterpoint"）, `note_path: string?` | `{ type: string, inspiration: string, related_links: string[] }` | 触发灵感熔炉，生成跨界创意/反向提问/对立观点 | `inspiration` |

### 3.6 智识雷达模块

| 工具名 | 参数 | 返回值 | 描述 | 所属模块 |
|---|---|---|---|---|
| `get_radar` | `limit: int?`（默认10）, `query: string?` | `{ items: RadarItem[] }` | 获取外部信息个性化推荐列表 | `radar` |
| `add_to_vault` | `article_id: string`（必填）, `target_dir: string?` | `{ note_path: string, title: string }` | 将雷达文章保存到 Obsidian vault | `radar` |
| `dismiss_radar_item` | `article_id: string`（必填） | `{ dismissed: bool }` | 标记雷达条目为已忽略 | `radar` |

### 3.7 系统模块

| 工具名 | 参数 | 返回值 | 描述 | 所属模块 |
|---|---|---|---|---|
| `get_stats` | 无 | `{ vault: VaultStats, memory: MemoryStats, repos: RepoStats, radar: RadarStats, uptime: string }` | 获取系统整体统计信息 | `system` |

---

## 4. MCP 协议交互流程

### 4.1 连接建立流程（stdio 模式）

```
Claude Desktop                    ObsidianBrain (MCP Server)
     │                                       │
     │  ── 启动子进程 ──────────────────────→ │
     │                                       │ 初始化服务（加载配置、连接依赖）
     │                                       │
     │  ── initialize (JSON-RPC) ──────────→ │
     │     {                                 │
     │       "method": "initialize",         │ 校验协议版本
     │       "params": {                     │ 构建能力声明
     │         "protocolVersion": "2024-11-05",
     │         "capabilities": {},           │
     │         "clientInfo": {...}           │
     │       }                               │
     │     }                                 │
     │                                       │
     │  ←── initialize response ──────────── │
     │     {                                 │
     │       "protocolVersion": "2024-11-05",│
     │       "capabilities": {               │
     │         "tools": {                    │
     │           "listChanged": true         │
     │         }                             │
     │       },                              │
     │       "serverInfo": {                 │
     │         "name": "obsidian-brain",     │
     │         "version": "0.1.0"            │
     │       }                               │
     │     }                                 │
     │                                       │
     │  ── initialized notification ───────→ │
     │     { "method": "notifications/initialized" }
     │                                       │ 连接就绪
     │                                       │
```

### 4.2 工具发现流程

```
Claude Desktop                    ObsidianBrain (MCP Server)
     │                                       │
     │  ── tools/list ─────────────────────→ │
     │     {                                 │ 从 ToolRegistry 获取所有工具
     │       "method": "tools/list",         │ 序列化为 MCP Tool 格式
     │       "params": {}                    │
     │     }                                 │
     │                                       │
     │  ←── tools/list response ──────────── │
     │     {                                 │
     │       "tools": [                      │
     │         {                             │
     │           "name": "search_notes",     │
     │           "description": "在 Obsidian vault 中搜索笔记...",
     │           "inputSchema": {            │
     │             "type": "object",         │
     │             "properties": {...},      │
     │             "required": ["query"]     │
     │           }                           │
     │         },                            │
     │         ...                           │
     │       ]                               │
     │     }                                 │
     │                                       │
```

### 4.3 工具调用流程

```
Claude Desktop                    ObsidianBrain (MCP Server)
     │                                       │
     │  ── tools/call ─────────────────────→ │
     │     {                                 │
     │       "method": "tools/call",         │ 解析工具名
     │       "params": {                     │ 查找 ToolRegistry
     │         "name": "search_notes",       │ 校验参数 (JSON Schema)
     │         "arguments": {                │ 路由到 Handler
     │           "query": "Rust 异步编程",    │ Handler 调用核心服务
     │           "top_k": 5                  │ 组装结果
     │         }                             │
     │       }                               │
     │     }                                 │
     │                                       │
     │  ←── tools/call response ──────────── │
     │     {                                 │
     │       "content": [                    │
     │         {                             │
     │           "type": "text",             │
     │           "text": "{\"notes\":[...]}" │
     │         }                             │
     │       ],                              │
     │       "isError": false                │
     │     }                                 │
     │                                       │
```

### 4.4 错误处理流程

```
Claude Desktop                    ObsidianBrain (MCP Server)
     │                                       │
     │  ── tools/call ─────────────────────→ │
     │     {                                 │
     │       "method": "tools/call",         │ 解析工具名
     │       "params": {                     │ 查找 ToolRegistry → 未找到
     │         "name": "nonexistent_tool",   │
     │         "arguments": {}               │
     │       }                               │
     │     }                                 │
     │                                       │
     │  ←── tools/call error response ────── │
     │     {                                 │
     │       "content": [                    │
     │         {                             │
     │           "type": "text",             │
     │           "text": "{\"error\":{\"code\":\"TOOL_NOT_FOUND\",\"message\":\"...\",\"suggestion\":\"...\"}}"
     │         }                             │
     │       ],                              │
     │       "isError": true                 │
     │     }                                 │
     │                                       │
```

### 4.5 工具变更通知流程（可选）

```
Claude Desktop                    ObsidianBrain (MCP Server)
     │                                       │
     │                                       │ [运行时动态注册/注销了工具]
     │                                       │
     │  ←── tools/list_changed notification  │
     │     {                                 │
     │       "method": "notifications/tools/list_changed"
     │     }                                 │
     │                                       │
     │  ── tools/list (重新获取) ──────────→ │
     │                                       │
     │  ←── 更新后的工具列表 ──────────────── │
```

---

## 5. HTTP API 交互流程

### 5.1 工具发现

```
客户端                              ObsidianBrain (HTTP Server)
  │                                          │
  │  ── GET /v1/tools ─────────────────────→ │
  │                                          │ 从 ToolRegistry 获取工具列表
  │  ←── 200 OK ──────────────────────────── │
  │     {                                    │
  │       "tools": [                         │
  │         {                                │
  │           "name": "search_notes",        │
  │           "description": "...",          │
  │           "inputSchema": {...},          │
  │           "module": "search"             │
  │         },                               │
  │         ...                              │
  │       ]                                  │
  │     }                                    │
  │                                          │
```

### 5.2 OpenAI 兼容格式获取

```
客户端                              ObsidianBrain (HTTP Server)
  │                                          │
  │  ── GET /v1/tools?format=openai ───────→ │
  │                                          │
  │  ←── 200 OK ──────────────────────────── │
  │     {                                    │
  │       "tools": [                         │
  │         {                                │
  │           "type": "function",            │
  │           "function": {                  │
  │             "name": "search_notes",      │
  │             "description": "...",        │
  │             "parameters": {              │
  │               "type": "object",          │
  │               "properties": {...},       │
  │               "required": ["query"]      │
  │             }                            │
  │           }                              │
  │         },                               │
  │         ...                              │
  │       ]                                  │
  │     }                                    │
  │                                          │
```

### 5.3 工具调用

```
客户端                              ObsidianBrain (HTTP Server)
  │                                          │
  │  ── POST /v1/tools/call ───────────────→ │
  │     Content-Type: application/json       │
  │     {                                    │ 解析请求体
  │       "tool": "search_notes",            │ 查找 ToolRegistry
  │       "arguments": {                     │ 校验参数
  │         "query": "Rust 异步编程",         │ 路由到 Handler
  │         "top_k": 5                       │ 执行并组装结果
  │       }                                  │
  │     }                                    │
  │                                          │
  │  ←── 200 OK ──────────────────────────── │
  │     {                                    │
  │       "tool": "search_notes",            │
  │       "status": "success",               │
  │       "result": {                        │
  │         "notes": [...],                  │
  │         "total": 5                       │
  │       }                                  │
  │     }                                    │
  │                                          │
```

### 5.4 工具调用错误响应

```
客户端                              ObsidianBrain (HTTP Server)
  │                                          │
  │  ── POST /v1/tools/call ───────────────→ │
  │     {                                    │
  │       "tool": "get_note",                │
  │       "arguments": {                     │ 查找笔记 → 不存在
  │         "path": "nonexistent.md"         │
  │       }                                  │
  │     }                                    │
  │                                          │
  │  ←── 200 OK ──────────────────────────── │
  │     {                                    │
  │       "tool": "get_note",                │
  │       "status": "error",                 │
  │       "error": {                         │
  │         "code": "NOTE_NOT_FOUND",        │
  │         "message": "笔记 'nonexistent.md' 未找到",
  │         "suggestion": "请使用 search_notes 搜索笔记，或使用 list_recent_notes 查看最近笔记"
  │       }                                  │
  │     }                                    │
  │                                          │
```

> **注意**：即使工具执行失败，HTTP 状态码仍返回 200。错误信息通过响应体中的 `status: "error"` 和 `error` 字段传达。这样设计是因为工具调用失败是业务层面的预期行为，LLM 需要读取错误内容来决定下一步操作。仅当协议层面的错误（如请求格式错误、服务不可用）才返回非 200 HTTP 状态码。

### 5.5 健康检查

```
客户端                              ObsidianBrain (HTTP Server)
  │                                          │
  │  ── GET /v1/health ────────────────────→ │
  │                                          │ 检查各组件状态
  │  ←── 200 OK ──────────────────────────── │
  │     {                                    │
  │       "status": "healthy",               │
  │       "version": "0.1.0",                │
  │       "uptime_seconds": 86400,           │
  │       "components": {                    │
  │         "vault": "ok",                   │
  │         "obsidian": "ok",                │
  │         "sqlite": "ok"                   │
  │       },                                 │
  │       "tools_count": 20                  │
  │     }                                    │
  │                                          │
```

---

## 6. 错误码体系

### 6.1 错误码定义

| 错误码 | HTTP Status | 描述 | 默认建议 |
|---|---|---|---|
| `TOOL_NOT_FOUND` | 200 | 请求的工具名不存在 | 使用 GET /v1/tools 查看可用工具列表 |
| `INVALID_ARGUMENTS` | 200 | 工具参数校验失败 | 检查参数格式是否符合工具的 inputSchema 定义 |
| `MISSING_REQUIRED_PARAM` | 200 | 缺少必填参数 | 补充必填参数后重试 |
| `NOTE_NOT_FOUND` | 200 | 指定路径的笔记不存在 | 使用 search_notes 搜索笔记，或检查路径是否正确 |
| `MEMORY_NOT_FOUND` | 200 | 指定 ID 的记忆不存在 | 使用 search_memory 搜索记忆 |
| `REPO_NOT_FOUND` | 200 | 指定名称的代码仓库未注册 | 使用 add_code_repo 注册仓库，或使用 list_code_repos 查看已注册仓库 |
| `REPO_PATH_INVALID` | 200 | 代码仓库路径不存在或不是 Git 仓库 | 检查路径是否正确，确保路径下已初始化 Git 仓库 |
| `RADAR_ITEM_NOT_FOUND` | 200 | 指定 ID 的雷达条目不存在 | 使用 get_radar 获取最新推荐列表 |
| `SEARCH_ERROR` | 200 | 搜索执行失败 | 请稍后重试，或尝试简化搜索关键词 |
| `OBSIDIAN_API_ERROR` | 200 | Obsidian REST API 调用失败 | 请确保 Obsidian 正在运行且 Local REST API 插件已启用 |
| `LLM_API_ERROR` | 200 | LLM API 调用失败 | 请稍后重试，或检查 LLM 配置 |
| `GIT_ERROR` | 200 | Git 操作失败 | 检查仓库状态是否正常 |
| `FILE_WRITE_ERROR` | 200 | 文件写入失败 | 检查 vault 路径权限和磁盘空间 |
| `TOOL_TIMEOUT` | 200 | 工具执行超时 | 请稍后重试，或减小请求参数范围 |
| `INTERNAL_ERROR` | 200 | 未分类的内部错误 | 请查看服务端日志获取详细信息 |
| `BAD_REQUEST` | 400 | 请求格式错误（如 JSON 解析失败） | 检查请求体 JSON 格式是否正确 |
| `SERVICE_UNAVAILABLE` | 503 | 服务未就绪或正在关闭 | 请等待服务启动完成后重试 |

### 6.2 错误响应格式

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

---

## 7. 非功能需求

### 7.1 协议兼容性

| 需求编号 | 需求描述 |
|---|---|
| NFR-COMPAT-01 | MCP 协议兼容 `2024-11-05` 版本规范 |
| NFR-COMPAT-02 | HTTP API 的 JSON Schema 输出可被 OpenAI API `tools` 参数直接使用 |
| NFR-COMPAT-03 | HTTP API 支持 `Content-Type: application/json` 请求和响应 |
| NFR-COMPAT-04 | JSON 序列化使用 UTF-8 编码，正确处理中文及特殊字符 |

### 7.2 扩展性

| 需求编号 | 需求描述 |
|---|---|
| NFR-EXT-01 | 新增工具只需实现 Handler trait 并注册到 Registry，无需修改协议层代码 |
| NFR-EXT-02 | 工具注册表支持热插拔，运行时新增/移除工具不影响正在处理的请求 |
| NFR-EXT-03 | 预留 MCP Resources 和 Prompts 能力的扩展点（初期不实现） |
| NFR-EXT-04 | 传输层抽象为 trait，新增传输方式（如 WebSocket）只需实现传输 trait |

### 7.3 调试友好

| 需求编号 | 需求描述 |
|---|---|
| NFR-DEBUG-01 | 提供 `RUST_LOG=debug` 级别下完整的请求/响应 JSON 日志 |
| NFR-DEBUG-02 | 每次工具调用生成唯一 `request_id`（UUID），贯穿日志全链路 |
| NFR-DEBUG-03 | 健康检查端点返回各组件连通性状态，方便排查依赖问题 |
| NFR-DEBUG-04 | 工具调用超时时返回明确的超时错误码和已执行耗时 |
| NFR-DEBUG-05 | 支持 `--dry-run` 模式：仅做参数校验和路由查找，不实际执行工具 |

### 7.4 性能

| 需求编号 | 需求描述 |
|---|---|
| NFR-PERF-01 | 工具列表获取（`tools/list` 或 `GET /v1/tools`）响应延迟 < 10ms |
| NFR-PERF-02 | 工具调用请求的路由分发延迟 < 1ms（不含工具执行时间） |
| NFR-PERF-03 | 协议层自身内存占用 < 10MB（不含工具执行中的临时内存） |
| NFR-PERF-04 | 支持至少 10 个并发工具调用请求 |

---

## 8. 与其他模块的接口约定

### 8.1 与核心服务层的接口

协议层通过 **Tool Handler** 与核心服务层交互。每个 Handler 是一个异步函数，接收结构化参数，返回结构化结果。

```
协议层                    Tool Handler                核心服务层
   │                          │                          │
   │  ── ToolCallRequest ──→  │                          │
   │     { tool, arguments }  │                          │
   │                          │  ── 调用 Service ──────→ │
   │                          │     (MemoryService /     │
   │                          │      TimelineService /   │
   │                          │      CodeRepoService /   │
   │                          │      InspirationService / │
   │                          │      RadarService)       │
   │                          │                          │
   │                          │  ←── ServiceResult ───── │
   │                          │                          │
   │  ←── ToolCallResult ──── │                          │
   │     { status, result,    │                          │
   │       error }            │                          │
```

**接口约定**：

| 约定项 | 说明 |
|---|---|
| Handler 签名 | `async fn handle(args: serde_json::Value, ctx: &AppContext) -> Result<serde_json::Value, BrainError>` |
| 参数传递 | Handler 接收原始 JSON Value，自行反序列化为强类型参数结构体 |
| 结果返回 | Handler 返回 `serde_json::Value`，协议层负责包装为标准响应信封 |
| 错误传播 | Handler 返回 `BrainError`，协议层负责映射为标准错误码 |
| 上下文 | `AppContext` 包含所有 Service 的引用（Arc 包装），Handler 通过它访问后端服务 |

### 8.2 与配置模块的接口

| 配置项 | 来源 | 用途 |
|---|---|---|
| `server.host` | `config/default.toml` | HTTP 服务监听地址 |
| `server.port` | `config/default.toml` | HTTP 服务监听端口 |
| `server.protocol` | `config/default.toml` | 协议模式选择（`mcp` / `http` / `both`） |

### 8.3 与日志模块的接口

| 接口 | 说明 |
|---|---|
| `tracing::info!` | 工具调用概要（工具名、耗时、状态） |
| `tracing::debug!` | 完整请求/响应 JSON |
| `tracing::error!` | 工具执行异常详情 |
| `tracing::span` | 每次工具调用创建独立 span，携带 `request_id` |

### 8.4 与错误处理模块的接口

协议层消费 `error.rs` 中定义的 `BrainError` 枚举，将其映射为标准化错误响应：

| BrainError 变体 | 映射错误码 |
|---|---|
| `NoteNotFound` | `NOTE_NOT_FOUND` |
| `RepoNotFound` | `REPO_NOT_FOUND` |
| `GitError` | `GIT_ERROR` |
| `SearchError` | `SEARCH_ERROR` |
| `FetchError` | `OBSIDIAN_API_ERROR` |
| `LlmApiError` | `LLM_API_ERROR` |
| `IoError` | `FILE_WRITE_ERROR` |
| `Internal` | `INTERNAL_ERROR` |

---

## 9. 约束与假设

### 9.1 约束

| 约束 | 说明 |
|---|---|
| 仅本地监听 | HTTP 服务仅绑定 `127.0.0.1`，不对外网暴露 |
| 单进程架构 | 协议层与核心服务层运行在同一进程内，通过函数调用而非 RPC 通信 |
| 无认证 | 由于仅本地访问，不实现认证/鉴权机制 |
| 序列化格式 | 所有数据交换使用 JSON 格式（MCP 的 JSON-RPC 2.0 和 HTTP REST） |
| 运行时 | 基于 Tokio 异步运行时，所有 I/O 操作均为异步 |

### 9.2 假设

| 假设 | 说明 |
|---|---|
| LLM 前端能正确解析 JSON Schema | 假设 Claude / ChatGPT 等 LLM 前端能理解工具 Schema 并生成合法调用请求 |
| MCP 客户端遵循规范 | 假设 MCP 客户端（如 Claude Desktop）遵循 MCP 协议规范发送请求 |
| 工具数量可控 | 假设工具总数不超过 100 个，工具列表可一次性返回（无需分页） |
| 单用户使用场景 | 假设同一时间只有一个 LLM 客户端连接（个人使用场景） |
| 网络稳定 | 假设本地 `127.0.0.1` 通信不会出现网络问题 |

---

## 10. 验收标准

### 10.1 MCP 协议验收

| 验收项 | 验收条件 | 优先级 |
|---|---|---|
| AC-MCP-01 | ObsidianBrain 可作为 MCP Server 被 Claude Desktop 成功连接（通过 `claude_desktop_config.json` 配置） | P0 |
| AC-MCP-02 | Claude Desktop 的 `tools/list` 能获取到所有已注册工具，且 Schema 格式正确 | P0 |
| AC-MCP-03 | 在 Claude Desktop 中调用 `search_notes` 工具能返回正确的搜索结果 | P0 |
| AC-MCP-04 | 在 Claude Desktop 中调用 `add_memory` 工具能成功写入记忆 | P0 |
| AC-MCP-05 | 工具调用失败时，Claude Desktop 能正确显示错误信息（`isError: true`） | P0 |
| AC-MCP-06 | stdio 传输模式稳定运行，长时间连接不出现消息丢失或乱序 | P1 |

### 10.2 HTTP API 验收

| 验收项 | 验收条件 | 优先级 |
|---|---|---|
| AC-HTTP-01 | `GET /v1/tools` 返回所有工具的 JSON Schema 列表，响应时间 < 10ms | P0 |
| AC-HTTP-02 | `POST /v1/tools/call` 能正确调用任意已注册工具并返回结果 | P0 |
| AC-HTTP-03 | `GET /v1/health` 返回各组件健康状态 | P0 |
| AC-HTTP-04 | 参数校验失败时返回 `INVALID_ARGUMENTS` 错误码和详细信息 | P0 |
| AC-HTTP-05 | OpenAI 兼容格式输出可被 `openai` 库的 `tools` 参数直接传入 | P1 |
| AC-HTTP-06 | 并发 10 个工具调用请求，全部正确处理，无数据竞争 | P1 |

### 10.3 通用验收

| 验收项 | 验收条件 | 优先级 |
|---|---|---|
| AC-GEN-01 | 所有 20+ 个工具均已注册并可被发现 | P0 |
| AC-GEN-02 | 每次工具调用均有结构化日志记录（包含 request_id、工具名、耗时、状态） | P0 |
| AC-GEN-03 | 协议层自身不 panic——所有错误均被正确捕获和转换 | P0 |
| AC-GEN-04 | 协议层代码测试覆盖率 > 80%（路由分发、参数校验、错误映射） | P1 |
| AC-GEN-05 | 新增一个工具只需：① 实现 Handler 函数 ② 注册到 Registry ③ 无需修改协议层其他代码 | P1 |

---

> **下一步**：详细的代码结构、Rust 类型定义、协议实现细节请参阅 [开发设计文档](../development/02-tool-protocol.md)。
