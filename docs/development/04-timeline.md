# 时光机模块 (Time Machine) — 开发设计文档

> **文档编号**: DEV-04 | **版本**: v2.0 | **状态**: 设计中 | **最后更新**: 2026-06-02
>
> **上游依赖**: [顶层设计文档](../top_design.md) §5.2 时光机 | [需求设计文档](../requirement/04-timeline.md)

---

## 1. 技术架构详细设计

### 1.1 架构概览

时光机模块位于 `core` 层，对外通过 Tool API 暴露 `create_memo`、`browse_timeline`、`search_memos` 三个工具，对内通过 Obsidian Local REST API 进行文件操作，通过 SQLite 存储元数据。模块内部采用 **创建 → 存储 → 查询** 的简洁架构。

```
┌────────────────────────────────────────────────────────────────────┐
│                       Time Machine Service                          │
│                                                                    │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    小记管理层 (Memo Manager)                   │  │
│  │                                                              │  │
│  │  ┌──────────────┐ ┌──────────────┐ ┌─────────────────────┐  │  │
│  │  │ Create Memo  │ │ Browse       │ │ Search Memos        │  │  │
│  │  │              │ │ Timeline     │ │                     │  │  │
│  │  └──────┬───────┘ └──────┬───────┘ └──────────┬──────────┘  │  │
│  └─────────┼────────────────┼──────────────────┼─────────────┘  │
│            │                │                  │                 │
│            ▼                ▼                  ▼                 │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              存储层 (Storage Layer)                            │  │
│  │                                                              │  │
│  │  ┌──────────────────┐ ┌──────────────────┐                  │  │
│  │  │ SQLite Store     │ │ Obsidian API     │                  │  │
│  │  │ (memos 表)       │ │ (Timeline 文件夹) │                  │  │
│  │  └──────────────────┘ └──────────────────┘                  │  │
│  └──────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────┘
         │                                              ▲
         │ 创建/查询                                     │ Tool API 调用
         ▼                                              │
┌─────────────────┐  ┌─────────────┐  ┌──────────────────────────┐
│    SQLite       │  │  Obsidian   │  │     Tool API Handler     │
│  (brain.db)    │  │  Local API  │  │  create_memo / browse   │
└─────────────────┘  └─────────────┘  └──────────────────────────┘
```

### 1.2 模块间依赖关系

```
Time Machine Service 依赖：
├── infra::sqlite_store    — SQLite 读写（元数据存储）
├── infra::obsidian_client — Obsidian API（文件操作）
└── infra::llm_client      — LLM 摘要生成（可选，用于未来扩展）
```

---

## 2. 目录与文件组织

### 2.1 文件布局

```
src/
├── core/
│   ├── timeline/
│   │   ├── mod.rs                  # 模块入口：TimeMachineService 定义
│   │   ├── memo_manager.rs         # 小记管理器（创建、浏览、搜索）
│   │   └── store.rs                # 元数据存储层（SQLite CRUD）
├── models/
│   └── timeline.rs                 # 数据模型（Memo、MemoQuery 等）
└── tools/
    └── handlers/
        └── timeline_handlers.rs    # Tool API Handler
```

### 2.2 文件职责

| 文件 | 职责 |
|---|---|
| `core/timeline/mod.rs` | 模块入口，TimeMachineService 结构定义与初始化 |
| `core/timeline/memo_manager.rs` | 小记管理器，处理创建、浏览、搜索逻辑 |
| `core/timeline/store.rs` | SQLite 元数据存储，提供 CRUD 接口 |
| `models/timeline.rs` | 数据模型定义（Memo、MemoQuery 等） |
| `tools/handlers/timeline_handlers.rs` | Tool API Handler，暴露 3 个工具 |

---

## 3. 数据模型详细设计

### 3.1 Memo（小记）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memo {
    pub id: String,               // UUID
    pub timestamp: DateTime<Utc>, // 精确时间戳
    pub date: String,             // YYYY-MM-DD
    pub content: String,          // 小记内容（Markdown）
    pub images: Vec<String>,      // 图片路径列表
    pub tags: Vec<String>,        // 标签列表
    pub file_path: String,        // 月份文件路径
    pub created_at: DateTime<Utc>,
}
```

### 3.2 MemoQuery（查询参数）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoQuery {
    pub query: Option<String>,      // 搜索关键词
    pub start_date: Option<String>, // YYYY-MM-DD
    pub end_date: Option<String>,   // YYYY-MM-DD
    pub tags: Option<Vec<String>>,  // 标签筛选
    pub limit: usize,               // 每页数量
    pub offset: usize,              // 偏移量
}
```

### 3.3 MemoCreateRequest（创建请求）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoCreateRequest {
    pub content: String,            // 小记内容
    pub images: Vec<String>,        // 图片路径列表
    pub tags: Vec<String>,          // 标签列表
}
```

### 3.4 SQLite 表结构

```sql
CREATE TABLE IF NOT EXISTS memos (
    id          TEXT PRIMARY KEY,
    timestamp   DATETIME NOT NULL,
    date        TEXT NOT NULL,
    content     TEXT NOT NULL,
    images      TEXT,               -- JSON 数组
    tags        TEXT,               -- JSON 数组
    file_path   TEXT NOT NULL,
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_memos_timestamp ON memos(timestamp DESC);
CREATE INDEX idx_memos_date ON memos(date);
CREATE INDEX idx_memos_tags ON memos(tags);
```

---

## 4. 核心功能实现

### 4.1 创建小记 (create_memo)

**实现步骤**：

1. **生成小记 ID**：使用 UUID v4
2. **记录时间戳**：精确到秒
3. **生成文件路径**：`Timeline/YYYY-MM.md`
4. **格式化 Markdown 内容**：
   ```markdown
   ### HH:MM:SS
   小记内容...
   
   ![[Timeline/images/image1.png]]
   
   #tag1 #tag2
   
   ---
   ```
5. **写入 Obsidian 文件**：使用 Obsidian API `PUT /vault/Timeline/YYYY-MM.md`（追加模式）
6. **写入 SQLite 元数据**：存储元数据用于快速查询

**代码示例**：
```rust
pub async fn create_memo(&self, request: MemoCreateRequest) -> Result<Memo, BrainError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M:%S").to_string();
    
    // 生成文件路径
    let file_path = format!("Timeline/{}.md", now.format("%Y-%m"));
    
    // 格式化 Markdown 内容
    let mut md_content = format!("### {}\n{}\n\n", time, request.content);
    for img in &request.images {
        md_content.push_str(&format!("![[{}]]\n", img));
    }
    if !request.tags.is_empty() {
        md_content.push_str(&format!("\n{}\n", request.tags.iter()
            .map(|t| format!("#{}", t))
            .collect::<Vec<_>>()
            .join(" ")));
    }
    md_content.push_str("\n---\n\n");
    
    // 写入 Obsidian 文件（追加模式）
    self.obsidian.append_to_file(&file_path, &md_content).await?;
    
    // 写入 SQLite 元数据
    let memo = Memo {
        id: id.clone(),
        timestamp: now,
        date: date.clone(),
        content: request.content,
        images: request.images,
        tags: request.tags,
        file_path: file_path.clone(),
        created_at: now,
    };
    self.store.insert_memo(&memo)?;
    
    Ok(memo)
}
```

### 4.2 浏览时间线 (browse_timeline)

**实现步骤**：

1. **构建查询**：根据 `start_date`、`end_date`、`limit`、`offset` 构建 SQL 查询
2. **查询 SQLite**：从 `memos` 表查询
3. **格式化响应**：按时间倒序排列

**代码示例**：
```rust
pub async fn browse_timeline(&self, query: MemoQuery) -> Result<Vec<Memo>, BrainError> {
    let mut sql = String::from("SELECT * FROM memos WHERE 1=1");
    let mut params = Vec::new();
    
    if let Some(ref start) = query.start_date {
        sql.push_str(" AND date >= ?");
        params.push(start.clone());
    }
    if let Some(ref end) = query.end_date {
        sql.push_str(" AND date <= ?");
        params.push(end.clone());
    }
    if let Some(ref tags) = query.tags {
        for tag in tags {
            sql.push_str(" AND tags LIKE ?");
            params.push(format!("%{}%", tag));
        }
    }
    
    sql.push_str(" ORDER BY timestamp DESC LIMIT ? OFFSET ?");
    params.push(query.limit.to_string());
    params.push(query.offset.to_string());
    
    self.store.query_memos(&sql, &params)
}
```

### 4.3 搜索小记 (search_memos)

**实现步骤**：

1. **构建全文搜索查询**：使用 SQLite 全文搜索（FTS5）
2. **支持组合搜索**：关键词 + 时间范围 + 标签
3. **相关性排序**：按相关性 + 时间排序

**代码示例**：
```rust
pub async fn search_memos(&self, query: MemoQuery) -> Result<Vec<Memo>, BrainError> {
    let mut sql = String::from("SELECT * FROM memos WHERE content MATCH ?");
    let mut params = vec![query.query.clone().unwrap_or_default()];
    
    if let Some(ref start) = query.start_date {
        sql.push_str(" AND date >= ?");
        params.push(start.clone());
    }
    if let Some(ref end) = query.end_date {
        sql.push_str(" AND date <= ?");
        params.push(end.clone());
    }
    if let Some(ref tags) = query.tags {
        for tag in tags {
            sql.push_str(" AND tags LIKE ?");
            params.push(format!("%{}%", tag));
        }
    }
    
    sql.push_str(" ORDER BY timestamp DESC LIMIT ? OFFSET ?");
    params.push(query.limit.to_string());
    params.push(query.offset.to_string());
    
    self.store.query_memos(&sql, &params)
}
```

---

## 5. 工具接口实现

### 5.1 create_memo

**工具定义**：
```json
{
  "name": "create_memo",
  "description": "创建一条小记，支持文本和图片",
  "inputSchema": {
    "type": "object",
    "properties": {
      "content": {
        "type": "string",
        "description": "小记内容（支持 Markdown）"
      },
      "images": {
        "type": "array",
        "items": {"type": "string"},
        "description": "图片路径列表"
      },
      "tags": {
        "type": "array",
        "items": {"type": "string"},
        "description": "标签列表"
      }
    },
    "required": ["content"]
  }
}
```

### 5.2 browse_timeline

**工具定义**：
```json
{
  "name": "browse_timeline",
  "description": "浏览时间线，支持按时间范围筛选",
  "inputSchema": {
    "type": "object",
    "properties": {
      "start_date": {
        "type": "string",
        "description": "起始日期（YYYY-MM-DD）"
      },
      "end_date": {
        "type": "string",
        "description": "结束日期（YYYY-MM-DD）"
      },
      "limit": {
        "type": "integer",
        "default": 20
      },
      "offset": {
        "type": "integer",
        "default": 0
      }
    }
  }
}
```

### 5.3 search_memos

**工具定义**：
```json
{
  "name": "search_memos",
  "description": "搜索小记内容",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "搜索关键词"
      },
      "start_date": {
        "type": "string",
        "description": "起始日期（YYYY-MM-DD）"
      },
      "end_date": {
        "type": "string",
        "description": "结束日期（YYYY-MM-DD）"
      },
      "tags": {
        "type": "array",
        "items": {"type": "string"}
      },
      "limit": {
        "type": "integer",
        "default": 20
      }
    },
    "required": ["query"]
  }
}
```

---

## 6. 前端 UI 设计

### 6.1 页面布局

```
┌─────────────────────────────────────────────────────┐
│  [创建小记按钮]                    [搜索框] [筛选]  │
├──────────┬──────────────────────────────────────────┤
│ 时间线   │                                          │
│ (200px)  │  内容区                                  │
│          │                                          │
│ 2026-06  │  ┌─────────────────────────────────────┐│
│  ├─ 02   │  │ 14:30:25                            ││
│  ├─ 01   │  │ 这是一条小记...                     ││
│          │  │ ![[image.png]]                      ││
│ 2026-05  │  │ #灵感 #想法                         ││
│  ├─ 28   │  └─────────────────────────────────────┘│
│  ...     │                                          │
└──────────┴──────────────────────────────────────────┘
```

### 6.2 组件设计

**TimeMachine.vue**：
- 左侧时间线：年月日树形结构
- 右侧内容区：小记列表（无限滚动）
- 顶部工具栏：创建按钮、搜索框、筛选器

**CreateMemoDialog.vue**：
- 多行文本框（支持 Markdown）
- 图片上传（拖拽、粘贴）
- 标签输入（自动补全）

**TimeFilter.vue**：
- 预设范围按钮
- 自定义日期选择器

---

## 7. 错误处理

| 错误场景 | 处理方式 |
|---|---|
| Timeline 文件夹不存在 | 自动创建 |
| 月份文件不存在 | 自动创建 |
| 图片上传失败 | 返回错误，小记不创建 |
| SQLite 写入失败 | 返回错误，提示用户 |
| Obsidian API 不可用 | 返回错误，提示用户检查插件 |

---

## 8. 测试策略

### 8.1 单元测试

| 模块 | 测试内容 |
|---|---|
| `memo_manager` | 创建小记、浏览时间线、搜索小记 |
| `store` | SQLite CRUD、查询构建 |
| `timeline_handlers` | Tool API 调用 |

### 8.2 集成测试

- 创建小记 → 验证文件写入
- 浏览时间线 → 验证查询结果
- 搜索小记 → 验证搜索结果

---

## 9. 性能优化

| 操作 | 优化策略 |
|---|---|
| 创建小记 | 异步写入文件，立即返回 |
| 浏览时间线 | SQLite 索引查询，分页加载 |
| 搜索小记 | SQLite FTS5 全文搜索 |
| 时间筛选 | SQLite 索引查询 |

---

## 10. 未来扩展

### 10.1 可能的扩展

- **小记编辑**：支持编辑已发布的小记
- **小记删除**：支持删除小记
- **小记关联**：将小记关联到正式笔记
- **小记导出**：导出小记为 PDF
- **小记统计**：统计小记数量、频率

### 10.2 与其他模块的集成

- **灵感熔炉**：从小记中提取灵感素材
- **智识雷达**：基于小记内容推荐相关内容
- **时间线回顾**：与其他模块的时间线事件融合

---

## 11. 修订历史

| 版本 | 日期 | 修订内容 |
|---|---|---|
| v1.0 | 2026-05-29 | 初始版本（自动事件采集） |
| v2.0 | 2026-06-02 | 重新设计为用户主动记录模式，更名为"时光机" |
