# 记忆引擎 (Memory Engine) — 需求设计文档

> **模块编号**: 03 | **版本**: v1.1 | **状态**: 设计中 | **关联**: [顶层设计文档](../top_design.md §5.1)

---

## 1. 模块概述

记忆引擎是 ObsidianBrain 的**核心中枢**，承担知识的检索、管理与操作职责。它通过 Obsidian Local REST API 直接与用户的 Obsidian Vault 交互，将 Vault 从被动的文件存储升级为可被 LLM 主动查询和操作的**活体知识库**。

### 1.1 核心定位

```
         用户写作/编辑笔记
               │
               ▼
        ┌─────────────┐
        │ Obsidian 应用 │ ← 用户直接操作的笔记环境
        └──────┬──────┘
               │ Local REST API
               ▼
        ┌─────────────┐
        │  记忆引擎    │ ← 通过 Obsidian API 搜索与操作笔记
        │  (本模块)    │
        └──────┬──────┘
               │
     ┌─────────┼─────────┐
     ▼         ▼         ▼
  笔记搜索   笔记 CRUD   元数据查询
 (Obsidian)  (Obsidian)   (SQLite)
```

### 1.2 核心价值

| 能力 | 说明 |
|---|---|
| **笔记搜索** | 通过 Obsidian 搜索 API 按关键词搜索 Vault 中的笔记，返回匹配上下文 |
| **笔记管理** | 完整 CRUD 操作，LLM 可读取、创建、修改、删除笔记 |
| **记忆管理** | 向指定笔记追加结构化记忆段落，支持查询和删除 |
| **引用溯源** | 每条搜索结果均附带笔记路径和 Obsidian URI，支持一键跳转 |
| **标签组织** | 利用 Obsidian frontmatter 的 tags 字段组织和过滤记忆 |

### 1.3 与其他模块的关系

- **Obsidian REST API** (`infra/obsidian_client.rs`)：提供笔记搜索与 CRUD 的底层能力
- **时间线模块** (`core/timeline.rs`)：记忆引擎的操作事件（新增/修改/删除）作为时间线事件源
- **智识雷达** (`core/radar.rs`)：雷达纳藏的文章写入 Vault 后，可直接通过记忆引擎搜索
- **灵感熔炉** (`core/inspiration.rs`)：灵感生成时通过记忆引擎搜索相关笔记片段
- **API 层** (`api/handlers/`)：将记忆操作暴露为标准 Tool API

---

## 2. 功能需求

### 2.1 笔记搜索

**需求描述**：通过 Obsidian Local REST API 的搜索端点，按关键词搜索 Vault 中的笔记内容。

**搜索流程**：

1. **构建查询**：将用户查询关键词转换为 JsonLogic 查询格式
2. **调用 API**：通过 `POST /search/` 端点发送搜索请求
3. **结果整理**：解析搜索结果，提取笔记路径和匹配上下文
4. **标签过滤**：如指定了 tags 参数，在结果中按 frontmatter tags 过滤
5. **排序返回**：按匹配度排序，返回 top_k 条结果

**查询格式**：

```json
{
  "in": ["查询关键词", {"var": "content"}]
}
```

**功能要求**：

1. **关键词搜索**：支持按关键词在笔记内容中搜索
2. **标签过滤**：支持按标签过滤搜索结果
3. **结果限制**：支持 top_k 参数控制返回结果数量
4. **上下文展示**：每条结果附带匹配片段上下文

**性能要求**：

- 搜索延迟 < 500ms（取决于 Obsidian API 响应速度）
- 搜索结果即时反映 Vault 最新内容（Obsidian 实时维护搜索索引）

### 2.2 笔记读取

**需求描述**：通过 Obsidian API 获取指定笔记的完整内容。

**功能要求**：

1. **读取内容**：通过 `GET /vault/{path}` 获取笔记 Markdown 原文
2. **解析 frontmatter**：提取 YAML frontmatter 中的元数据（标题、标签、创建时间等）
3. **Obsidian URI**：生成 `obsidian://open?vault=<vault_name>&file=<encoded_path>` 链接

### 2.3 笔记列表

**需求描述**：获取 Vault 中最近修改或创建的笔记列表。

**功能要求**：

1. **文件遍历**：通过 Obsidian API 列出所有文件
2. **排序**：按修改时间倒序排列
3. **过滤**：支持按天数范围过滤（默认最近 7 天）
4. **限制**：支持 limit 参数控制返回数量（默认 20 条）
5. **摘要信息**：每条记录包含标题、路径、修改时间、标签

### 2.4 记忆 CRUD 操作

记忆引擎对外暴露以下 Tool API，供 LLM 直接调用：

#### 2.4.1 `search_memory`

- **输入**：`query: string, top_k?: int (默认5), tags?: string[]`
- **输出**：`Memory[]` — 匹配的记忆单元列表
- **说明**：通过 Obsidian 搜索 API 搜索笔记内容。每条结果附带来源笔记路径和 Obsidian URI
- **实现**：构建 JsonLogic 查询，调用 `POST /search/` 端点

#### 2.4.2 `add_memory`

- **输入**：`note_path: string, content: string, tags?: string[]`
- **输出**：`Memory` — 新创建的记忆单元
- **说明**：
  1. 在指定笔记末尾追加内容（以分隔符标记为记忆段落）
  2. 如果笔记不存在，通过 Obsidian API 自动创建
  3. 新内容立即可被搜索到（Obsidian 实时更新搜索索引）
- **副作用**：触发时间线 `MemoryCreated` 事件

#### 2.4.3 `update_memory`

- **输入**：`memory_id: string, content: string`
- **输出**：`Memory` — 更新后的记忆单元
- **说明**：
  1. 根据 memory_id 查找对应记忆（通过笔记路径和段落标记定位）
  2. 更新来源笔记中的对应段落
  3. 更新后的内容立即可被搜索到

#### 2.4.4 `forget_memory`

- **输入**：`memory_id: string`
- **输出**：`bool` — 是否删除成功
- **说明**：
  1. 根据 memory_id 定位对应记忆段落
  2. 从来源笔记中移除对应段落

#### 2.4.5 `get_memory_stats`

- **输入**：无
- **输出**：`{ total: int, by_tag: HashMap<String, int>, recent: Memory[] }`
- **说明**：返回记忆库的统计概览——通过读取 Vault 中的笔记样本，统计标签分布和最近创建的记忆

### 2.5 引用溯源 (Citation & Provenance)

**需求描述**：每条搜索结果都必须携带完整的来源信息，让用户可以一键跳转到原始笔记。

**溯源信息包含**：

1. **笔记路径**：Vault 内相对路径（如 `programming/rust-async.md`）
2. **Obsidian URI**：`obsidian://open?vault=<vault_name>&file=<encoded_path>`
3. **标题路径**：匹配内容所在的标题层级（如 `## 异步编程 > ### tokio select!`），通过解析笔记内容获取
4. **行号范围**：匹配内容在原文中的大致行号范围

### 2.6 笔记搜索工具

除记忆级别的搜索外，还提供笔记级别的搜索工具：

#### 2.6.1 `search_notes`

- **输入**：`query: string, top_k?: int, tags?: string[]`
- **输出**：`NoteSearchResult[]` — 匹配的笔记列表，每篇附带最佳匹配片段
- **说明**：与 `search_memory` 共享搜索能力，但结果按笔记聚合（同一笔记的多个匹配合并展示）

#### 2.6.2 `get_note`

- **输入**：`path: string`
- **输出**：`Note` — 笔记完整内容（含 frontmatter、正文、标签）
- **说明**：通过 Obsidian API 读取笔记原文

#### 2.6.3 `list_recent_notes`

- **输入**：`days?: int (默认7), limit?: int (默认20)`
- **输出**：`NoteSummary[]` — 最近修改的笔记摘要列表
- **说明**：基于文件修改时间排序，返回标题、路径、修改时间、标签

---

## 3. 用户故事

### US-01：通过 LLM 搜索历史知识

**角色**：小明在与 Claude 对话
**场景**：小明问 Claude："我之前记过关于 Rust 生命周期的一些笔记，能帮我找一下吗？"
**期望**：
- Claude 调用 `search_memory(query="Rust 生命周期", top_k=5)`
- 返回包含"生命周期"关键词的笔记片段
- 每条结果附带 Obsidian URI，小明可以直接点击跳转到原文

**验收条件**：
- 搜索结果包含匹配"生命周期"关键词的内容
- 每条结果的 `obsidian_uri` 字段非空，URI 格式正确
- 搜索延迟 < 500ms

### US-02：让 LLM 添加新记忆

**角色**：小明在对话中产生了一个新想法
**场景**：小明对 Claude 说："帮我记一下：在 Rust 中使用 Arc<Mutex<T>> 时，要注意避免死锁，尤其是在异步上下文中。把它加到我的 Rust 并发笔记里。"
**期望**：
- Claude 调用 `add_memory(note_path="programming/rust-concurrency.md", content="...", tags=["rust", "concurrency"])`
- 笔记文件被追加了新内容
- 新内容可立即被搜索到
- 时间线记录了此事件

**验收条件**：
- `get_note("programming/rust-concurrency.md")` 可以看到追加的内容
- `search_memory("Arc Mutex 死锁")` 能返回该记忆
- `get_timeline` 中可以看到 `MemoryCreated` 事件

### US-03：删除过时记忆

**角色**：小明发现一篇旧笔记中的信息已过时
**场景**：小明对 Claude 说："我之前记的那条关于 Python 2 的 print 语句的记忆已经过时了，帮我删掉。"
**期望**：
- Claude 先搜索找到对应记忆，获取 memory_id
- 调用 `forget_memory(memory_id="xxx")`
- 该记忆从笔记中移除
- 后续搜索不再返回该记忆

**验收条件**：
- 删除后 `search_memory` 不再返回该 memory_id 对应的结果
- `get_memory_stats` 的 total 计数减少

### US-04：查看记忆库状态

**角色**：小明想了解系统当前管理了多少知识
**场景**：小明对 Claude 说："我的知识库里有多少条记忆？按标签分布是怎样的？"
**期望**：
- Claude 调用 `get_memory_stats()`
- 返回总数、标签分布（如 "rust: 45, ai: 32, 随笔: 18"）、最近添加的记忆
- Claude 基于统计数据生成自然语言摘要

**验收条件**：
- 统计数据与实际笔记内容一致
- 标签分布覆盖所有已索引标签
- 返回的 recent 列表按时间倒序排列

### US-05：搜索笔记内容

**角色**：小明在 Obsidian 中有大量笔记
**场景**：小明问 Claude："帮我找一下关于 Rust 异步编程的笔记"
**期望**：
- Claude 调用 `search_notes(query="Rust 异步编程", top_k=5)`
- 返回匹配的笔记列表，每篇附带最佳匹配片段
- 每条结果附带 Obsidian URI，可直接打开笔记

**验收条件**：
- 搜索结果包含匹配的笔记及其上下文片段
- 结果按笔记聚合，同一笔记不会重复出现
- Obsidian URI 可正确打开对应笔记

### US-06：查看最近修改的笔记

**角色**：小明想回顾最近的工作
**场景**：小明问 Claude："这周我修改了哪些笔记？"
**期望**：
- Claude 调用 `list_recent_notes(days=7, limit=20)`
- 返回最近 7 天修改的笔记列表
- 每条包含标题、路径、修改时间

**验收条件**：
- 列表按修改时间倒序排列
- 每条记录包含准确的标题和路径信息
- 修改时间与 Obsidian 中显示一致

### US-07：外部文章纳藏后可搜索

**角色**：小明通过智识雷达发现了一篇好文章
**场景**：小明调用 `add_to_vault(article_id="xxx")` 将文章保存到 Vault 的 `radar/` 目录。
**期望**：
- 文章保存为 Markdown 文件后，立即可被搜索到
- 后续可通过 `search_memory` 搜索到该文章的内容

**验收条件**：
- 纳藏文章后，`search_memory` 可搜索到该文章的相关内容
- 文章的来源信息（URL、标题）保留在 frontmatter 中

---

## 4. 非功能需求

### 4.1 性能需求

| 指标 | 目标值 | 说明 |
|---|---|---|
| 笔记搜索延迟 | P95 < 500ms | 通过 Obsidian API 搜索 |
| 笔记读取延迟 | P95 < 200ms | 读取单篇笔记内容 |
| 记忆追加延迟 | < 300ms | 追加内容到笔记 |
| 文件列表延迟 | P95 < 1s | 列出 Vault 所有文件 |

### 4.2 存储效率

| 指标 | 约束 |
|---|---|
| 内存占用（空闲） | < 50MB（仅核心服务运行） |
| 内存占用（搜索） | < 100MB 峰值 |
| SQLite 数据库 | 仅存储元数据，笔记内容由 Obsidian 管理 |

### 4.3 可靠性

- 笔记操作通过 Obsidian API 执行，由 Obsidian 处理文件锁和冲突
- Obsidian API 调用失败时自动重试（3 次，指数退避）
- 所有操作结果可通过后续搜索验证一致性

### 4.4 可扩展性

- 搜索查询格式可配置（支持不同的 JsonLogic 查询模式）
- 笔记解析和 frontmatter 提取逻辑可复用

---

## 5. 与其他模块的接口

### 5.1 记忆引擎 → 时间线模块

```
事件类型: MemoryEvent
  - MemoryCreated { memory_id: Uuid, note_path: PathBuf }
  - MemoryUpdated { memory_id: Uuid, note_path: PathBuf }
  - MemoryRemoved { memory_id: Uuid, note_path: PathBuf }

接口: EventBus::publish(MemoryEvent)
```

**说明**：记忆引擎完成操作后发布事件，时间线模块订阅并记录。

### 5.2 智识雷达 → 记忆引擎

```
场景: add_to_vault 纳藏文章
流程:
  1. 雷达模块通过 Obsidian API 将文章写入 Vault（radar/xxx.md）
  2. 文章立即可被 Obsidian 搜索索引
  3. 记忆引擎通过 Obsidian API 可搜索到新文章

无需直接接口调用，通过 Obsidian 的实时索引间接触发。
```

### 5.3 灵感熔炉 → 记忆引擎

```
接口: MemoryService::search(query: &str, top_k: usize, tags: &[String]) -> Result<Vec<MemorySearchResult>>
```

**说明**：灵感熔炉需要获取相关笔记片段作为创意素材时，调用记忆引擎的搜索接口。

### 5.4 API 层 → 记忆引擎

```
Tool API 映射:
  search_memory   → MemoryService::search()
  add_memory      → MemoryService::add()
  update_memory   → MemoryService::update()
  forget_memory   → MemoryService::forget()
  get_memory_stats → MemoryService::stats()
  search_notes    → MemoryService::search_notes()
  get_note        → MemoryService::get_note()
  list_recent_notes → MemoryService::list_recent()
```

---

## 6. 约束与假设

### 6.1 约束

1. **Obsidian 依赖**：笔记搜索和 CRUD 操作依赖 Obsidian 应用运行及 Local REST API 插件启用
2. **搜索能力**：搜索质量取决于 Obsidian 内置搜索引擎的能力（关键词匹配为主）
3. **文件格式**：仅支持 `.md` (Markdown) 文件，其他格式（PDF、图片等）不在处理范围内
4. **Vault 大小**：初期设计目标为 ≤ 5000 篇笔记，更大规模需后续优化
5. **单机部署**：不支持多实例并发写入同一 Vault（单用户场景）

### 6.2 假设

1. 用户的 Obsidian Vault 路径在配置中指定，运行期间不变
2. Obsidian 应用持续运行，Local REST API 插件保持可用
3. 笔记文件使用 UTF-8 编码
4. 笔记内容以 Markdown 格式编写，frontmatter 使用 YAML 格式
5. Obsidian REST API 使用自签名 TLS 证书，客户端需配置信任

---

## 7. 验收标准

### 7.1 功能验收

| 编号 | 验收条件 | 验证方法 |
|---|---|---|
| AC-01 | `search_memory` 返回包含查询关键词的笔记内容 | 集成测试 |
| AC-02 | `search_notes` 返回按笔记聚合的搜索结果 | 集成测试 |
| AC-03 | `get_note` 返回笔记完整内容（含 frontmatter） | 集成测试 |
| AC-04 | `list_recent_notes` 返回按时间排序的笔记列表 | 集成测试 |
| AC-05 | `add_memory` 后新内容写入笔记并可搜索 | 集成测试 |
| AC-06 | `update_memory` 后搜索结果反映更新内容 | 集成测试 |
| AC-07 | `forget_memory` 后该记忆不再出现在搜索结果中 | 集成测试 |
| AC-08 | `get_memory_stats` 返回的统计数据与实际一致 | 集成测试 |
| AC-09 | 搜索结果包含 Obsidian URI，URI 可正确打开对应笔记 | 集成测试 |
| AC-10 | 标签过滤正确生效 | 集成测试 |

### 7.2 性能验收

| 编号 | 验收条件 | 验证方法 |
|---|---|---|
| PA-01 | 笔记搜索 P95 延迟 < 500ms | 基准测试 |
| PA-02 | 笔记读取 P95 延迟 < 200ms | 基准测试 |
| PA-03 | 空闲时内存占用 < 50MB | 运行时监控 |

### 7.3 可靠性验收

| 编号 | 验收条件 | 验证方法 |
|---|---|---|
| RA-01 | Obsidian API 不可用时返回清晰的错误信息 | 故障注入测试 |
| RA-02 | Obsidian API 调用失败时自动重试 | 故障注入测试 |
| RA-03 | 网络恢复后搜索功能自动恢复 | 端到端测试 |

---

## 8. 术语表

| 术语 | 说明 |
|---|---|
| **Obsidian Local REST API** | Obsidian 社区插件，提供 HTTP API 访问 Vault 的能力 |
| **JsonLogic** | 一种 JSON 格式的查询语言，Obsidian REST API 搜索端点使用此格式 |
| **Frontmatter** | Markdown 文件顶部的 YAML 元数据块，包含标题、标签等信息 |
| **Obsidian URI** | `obsidian://` 协议链接，可在 Obsidian 中直接打开指定笔记 |
| **Breadcrumb** | 匹配内容所在的标题层级路径 |
| **防抖 (Debounce)** | 将短时间内的多次操作合并为一次处理 |

---

> **下一步**：详见 [开发设计文档](../development/03-memory-engine.md)，包含技术架构、数据结构与代码组织。
