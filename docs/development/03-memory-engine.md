# 记忆引擎 (Memory Engine) — 开发设计文档

> **模块编号**: 03 | **版本**: v2.0 | **状态**: 设计中 | **关联**: [需求设计](../requirement/03-memory-engine.md) · [顶层设计](../top_design.md §5.1)
>
> **架构说明**：项目已从混合搜索架构（Tantivy + Qdrant + Embedding）简化为直接使用 Obsidian Local REST API。不再需要本地索引、向量存储或 Embedding 服务。

---

## 1. 技术架构详细设计

### 1.1 架构总览

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Memory Engine                                │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │                    Service 层 (memory_service.rs)               │  │
│  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────────────┐   │  │
│  │  │ SearchProxy  │ │ NoteReader   │ │ MemoryStatsCollector │   │  │
│  │  │ (搜索代理)    │ │ (笔记读取)    │ │ (统计收集)           │   │  │
│  │  └──────┬───────┘ └──────┬───────┘ └──────────────────────┘   │  │
│  └─────────┼────────────────┼────────────────────────────────────┘  │
│            │                │                                        │
│  ┌─────────┴────────────────┴────────────────────────────────────┐  │
│  │                    Infrastructure 层                            │  │
│  │  ┌──────────────────┐  ┌──────────────────┐                   │  │
│  │  │ ObsidianClient   │  │ SQLite Store     │                   │  │
│  │  │ (REST API)       │  │ (元数据缓存)     │                   │  │
│  │  └──────────────────┘  └──────────────────┘                   │  │
│  └────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────┘
```

**设计说明**：本引擎已简化为直接使用 Obsidian Local REST API 进行所有搜索和文件操作，移除了本地 Tantivy 全文索引、Qdrant 向量存储和 Embedding 生成服务。

### 1.2 模块依赖关系

```
memory_service.rs (MemoryService)
├── obsidian_client.rs  (ObsidianClient)
│   └── reqwest          (HTTP 客户端)
└── sqlite_store.rs     (SqliteStore)
    └── rusqlite         (SQLite 数据库)
```

### 1.3 并发模型

- **搜索请求**：多请求并行处理，每个搜索在独立 task 中执行
- **SQLite 访问**：通过连接池（`Arc<SqliteStore>`）支持并发读取
- **无本地索引写入**：无需 channel 队列、批量处理或并行搜索

---

## 2. 目录与文件组织

```
src/
├── core/
│   └── memory_service.rs    # MemoryService 主结构体，对外 API 入口
├── infra/
│   ├── obsidian_client.rs   # Obsidian REST API 客户端
│   └── sqlite_store.rs      # SQLite 元数据存储
└── models/
    ├── memory.rs             # MemoryStats
    └── note.rs               # ParsedDocument, NoteSummary
```

**设计说明**：架构已大幅简化，移除了所有本地索引（Tantivy）、向量存储（Qdrant）、Embedding 生成及混合搜索相关模块。`markdown_parser.rs` 和 `chunker.rs` 作为预留模块保留在代码库中（标记为 `#[allow(dead_code)]`），待未来迭代启用。

---

## 3. 子模块详细设计

### 3.1 ObsidianClient 搜索代理

#### 3.1.1 职责

通过 Obsidian Local REST API 的 `/search/` 端点执行笔记内容搜索，使用 JsonLogic 查询格式。

#### 3.1.2 ObsidianClient 结构

```rust
/// Obsidian REST API 客户端
pub struct ObsidianClient {
    base_url: String,
    api_key: String,
    vault_name: String,
    client: reqwest::Client,
}

impl ObsidianClient {
    pub fn new(base_url: &str, api_key: &str, vault_name: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            vault_name: vault_name.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// 搜索笔记内容
    ///
    /// 使用 Obsidian Local REST API 的 /search/ 端点
    /// 查询格式为 JsonLogic: { "in": [query, {"var": "content"}] }
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResultItem>, BrainError> {
        let body = serde_json::json!({
            "in": [query, {"var": "content"}]
        });

        let response = self.client
            .post(format!("{}/search/", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/vnd.olrapi.jsonlogic+json")
            .json(&body)
            .send()
            .await
            .map_err(|e| BrainError::ObsidianApiError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BrainError::ObsidianApiError(
                format!("搜索 API 返回 {}: {}", status, body),
            ));
        }

        let mut results: Vec<SearchResultItem> = response.json().await
            .map_err(|e| BrainError::ObsidianApiError(e.to_string()))?;

        results.truncate(limit);
        Ok(results)
    }
}
```

#### 3.1.3 JsonLogic 查询格式

Obsidian API 使用 JsonLogic 格式进行查询：

```json
{
  "in": ["关键词", {"var": "content"}]
}
```

- `in` 操作符：检查第一个参数是否在第二个参数中
- `{"var": "content"}`：引用笔记内容字段
- 支持更复杂的 JsonLogic 表达式（如 `and`、`or`、`contains` 等）

### 3.2 笔记读取与写入

#### 3.2.1 读取笔记

```rust
impl ObsidianClient {
    /// 读取笔记文件内容
    pub async fn read_file(&self, path: &str) -> Result<String, BrainError> {
        let response = self.client
            .get(format!("{}/vault/{}", self.base_url, path))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "text/markdown")
            .send()
            .await
            .map_err(|e| BrainError::ObsidianApiError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(BrainError::NoteNotFound(path.to_string()));
        }

        response.text().await
            .map_err(|e| BrainError::ObsidianApiError(e.to_string()))
    }

    /// 写入笔记内容
    pub async fn write_file(&self, path: &str, content: &str) -> Result<(), BrainError> {
        let response = self.client
            .put(format!("{}/vault/{}", self.base_url, path))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "text/markdown")
            .body(content.to_string())
            .send()
            .await
            .map_err(|e| BrainError::ObsidianApiError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(BrainError::ObsidianApiError(
                format!("写入文件失败: {}", status),
            ));
        }

        Ok(())
    }
}
```

#### 3.2.2 列出最近修改的笔记

```rust
impl ObsidianClient {
    /// 列出最近修改的笔记
    pub async fn list_recent_notes(
        &self,
        days: Option<u32>,
        limit: Option<usize>,
    ) -> Result<Vec<NoteSummary>, BrainError> {
        // 通过 /vault/ 端点列出文件
        let response = self.client
            .get(format!("{}/vault/", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| BrainError::ObsidianApiError(e.to_string()))?;

        let files: Vec<FileMetadata> = response.json().await
            .map_err(|e| BrainError::ObsidianApiError(e.to_string()))?;

        // 按修改时间排序，过滤时间范围
        let cutoff = days.map(|d| Utc::now() - Duration::days(d as i64));
        
        let mut notes: Vec<NoteSummary> = files
            .into_iter()
            .filter(|f| f.path.ends_with(".md"))
            .filter(|f| {
                cutoff.map_or(true, |c| f.modified_at >= c)
            })
            .map(|f| NoteSummary {
                path: f.path,
                title: f.path.trim_end_matches(".md").to_string(),
                tags: Vec::new(),
                modified_at: f.modified_at,
            })
            .collect();

        // 按修改时间降序排序
        notes.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

        // 应用 limit
        if let Some(lim) = limit {
            notes.truncate(lim);
        }

        Ok(notes)
    }
}
```

### 3.3 记忆统计

#### 3.3.1 get_memory_stats 实现

```rust
impl MemoryService {
    /// 获取记忆库统计信息
    pub async fn stats(&self) -> Result<MemoryStats, BrainError> {
        let total = self.obsidian_client.count_files().await?;
        let tags = self.obsidian_client.get_all_tags().await?;
        let index_size_mb = 0.0; // 无本地索引

        Ok(MemoryStats {
            total,
            by_tag: tags,
            index_size_mb,
        })
    }
}

impl ObsidianClient {
    /// 统计 vault 中的笔记文件数
    pub async fn count_files(&self) -> Result<usize, BrainError> {
        let response = self.client
            .get(format!("{}/vault/", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| BrainError::ObsidianApiError(e.to_string()))?;

        let files: Vec<FileMetadata> = response.json().await
            .map_err(|e| BrainError::ObsidianApiError(e.to_string()))?;

        Ok(files.iter().filter(|f| f.path.ends_with(".md")).count())
    }

    /// 获取所有标签及其计数
    pub async fn get_all_tags(&self) -> Result<HashMap<String, usize>, BrainError> {
        // 遍历文件 frontmatter 收集标签
        // 可通过 SQLite 缓存优化性能
        let response = self.client
            .get(format!("{}/vault/", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| BrainError::ObsidianApiError(e.to_string()))?;

        let files: Vec<FileMetadata> = response.json().await
            .map_err(|e| BrainError::ObsidianApiError(e.to_string()))?;

        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        
        for file in files {
            if let Some(tags) = file.tags {
                for tag in tags {
                    *tag_counts.entry(tag).or_insert(0) += 1;
                }
            }
        }

        Ok(tag_counts)
    }
}
```

### 3.4 Markdown 解析器（预留模块）

> **注意**：此模块当前标记为 `#[allow(dead_code)]`，仅在后续需要本地解析时使用。当前所有笔记操作通过 Obsidian REST API 完成。

#### 3.4.1 职责

将 Markdown 原始文本解析为结构化的 `ParsedDocument`，提取 frontmatter 元数据和正文的标题层级结构。

#### 3.4.2 核心数据结构

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use chrono::{DateTime, Utc};

/// 解析后的文档结构
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ParsedDocument {
    /// 文件路径（vault 内相对路径）
    pub path: PathBuf,
    /// YAML frontmatter 键值对
    pub frontmatter: HashMap<String, serde_json::Value>,
    /// 文档标题（frontmatter.title > 文件名）
    pub title: String,
    /// 标签列表（frontmatter.tags + 正文 #tag 合并去重）
    pub tags: Vec<String>,
    /// 正文段落结构（按标题层级组织）
    pub sections: Vec<Section>,
    /// 文件创建时间
    pub created_at: DateTime<Utc>,
    /// 文件修改时间
    pub updated_at: DateTime<Utc>,
}

/// 文档中的一个段落（由标题界定）
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Section {
    /// 标题层级 (0=文档开头无标题部分, 1=H1, 2=H2, 3=H3, ...)
    pub level: u8,
    /// 标题文本
    pub heading: Option<String>,
    /// 标题路径（breadcrumb），如 ["项目概述", "架构设计", "后端"]
    pub breadcrumb: Vec<String>,
    /// 该段落的纯文本内容（不含子标题，但含子标题下的内容）
    pub content: String,
    /// 该段落中的代码块（保持完整）
    pub code_blocks: Vec<CodeBlock>,
    /// 该段落在原文中的起始行号
    pub line_start: usize,
    /// 该段落在原文中的结束行号
    pub line_end: usize,
}

/// 代码块
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CodeBlock {
    /// 代码语言标识（如 "rust", "python"）
    pub language: Option<String>,
    /// 代码内容
    pub code: String,
    /// 在原文中的起始行号
    pub line_start: usize,
    /// 在原文中的结束行号
    pub line_end: usize,
}
```

#### 3.4.3 解析流程

```rust
use gray_matter::engine::YAML;
use gray_matter::Matter;
use pulldown_cmark::{Parser, Event, Tag, TagEnd};

#[allow(dead_code)]
pub struct MarkdownParser {
    matter: Matter<YAML>,
}

#[allow(dead_code)]
impl MarkdownParser {
    pub fn new() -> Self {
        Self {
            matter: Matter::new(),
        }
    }

    /// 解析 Markdown 文件内容为结构化文档
    pub fn parse(&self, path: PathBuf, content: &str) -> Result<ParsedDocument, BrainError> {
        // 1. 提取 frontmatter
        let result = self.matter.parse(content);
        let frontmatter = self.extract_frontmatter(&result);
        let body = result.content; // frontmatter 之后的正文

        // 2. 解析正文标题层级
        let sections = self.parse_sections(body)?;

        // 3. 提取标签（frontmatter.tags + 正文 #tag）
        let tags = self.extract_tags(&frontmatter, body);

        // 4. 确定标题
        let title = self.extract_title(&frontmatter, &path, &sections);

        Ok(ParsedDocument {
            path,
            frontmatter,
            title,
            tags,
            sections,
            created_at: Utc::now(), // 后续从文件元数据获取
            updated_at: Utc::now(),
        })
    }

    fn extract_title(
        &self,
        frontmatter: &HashMap<String, serde_json::Value>,
        path: &PathBuf,
        sections: &[Section],
    ) -> String {
        // 优先级: frontmatter.title > 第一个 H1 > 文件名
        if let Some(serde_json::Value::String(t)) = frontmatter.get("title") {
            return t.clone();
        }
        for s in sections {
            if s.level == 1 {
                if let Some(ref h) = s.heading {
                    return h.clone();
                }
            }
        }
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string()
    }
}
```

### 3.5 智能分块器（预留模块）

> **注意**：此模块当前标记为 `#[allow(dead_code)]`，仅在后续需要本地索引时使用。

#### 3.5.1 职责

将 `ParsedDocument` 的 `Section` 列表拆分为适合索引和检索的 `Chunk` 列表。

#### 3.5.2 Chunk 结构体

```rust
use uuid::Uuid;

/// 记忆单元 / 索引块
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Chunk {
    /// 唯一标识
    pub id: Uuid,
    /// 来源笔记路径
    pub note_path: PathBuf,
    /// 在笔记中的段落序号 (0-based)
    pub chunk_index: usize,
    /// Chunk 文本内容
    pub content: String,
    /// 标题路径 breadcrumb
    pub breadcrumb: Vec<String>,
    /// 标签（继承自笔记）
    pub tags: Vec<String>,
    /// 在原文中的起始行号
    pub line_start: usize,
    /// 在原文中的结束行号
    pub line_end: usize,
    /// 来源笔记标题
    pub note_title: String,
    /// 估算的 token 数
    pub token_count: usize,
    /// 是否包含代码块
    pub has_code_block: bool,
}

/// Chunk 配置
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ChunkerConfig {
    /// 最小 token 数
    pub min_tokens: usize,
    /// 最大 token 数
    pub max_tokens: usize,
    /// 相邻 Chunk 重叠句数
    pub overlap_sentences: usize,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            min_tokens: 300,
            max_tokens: 800,
            overlap_sentences: 1,
        }
    }
}
```

#### 3.5.3 分块算法伪代码

```
算法: SmartChunk
输入: sections: [Section], config: ChunkerConfig
输出: chunks: [Chunk]

初始化:
    chunks = []
    chunk_index = 0
    buffer = ""          // 当前累积的文本
    buffer_tokens = 0
    current_breadcrumb = []
    current_line_start = 0

对于 sections 中的每个 section:
    section_tokens = estimate_tokens(section.content)

    // 情况1: 当前 section 本身就在 max_tokens 范围内
    如果 section_tokens <= config.max_tokens:
        // 尝试将 section 追加到 buffer
        如果 buffer_tokens + section_tokens <= config.max_tokens:
            buffer += section.content
            buffer_tokens += section_tokens
            current_breadcrumb = section.breadcrumb
        否则:
            // buffer 已满，将 buffer 输出为一个 Chunk
            如果 buffer_tokens >= config.min_tokens:
                chunks.push(make_chunk(buffer, chunk_index, ...))
                chunk_index += 1
                // 保留重叠句
                buffer = get_last_sentences(buffer, config.overlap_sentences)
                         + section.content
                buffer_tokens = estimate_tokens(buffer)
            否则:
                buffer += section.content
                buffer_tokens += section_tokens

    // 情况2: section 超过 max_tokens，需要二次分割
    否则:
        // 先将 buffer 输出为一个 Chunk（如果有足够内容）
        如果 buffer_tokens >= config.min_tokens:
            chunks.push(make_chunk(buffer, chunk_index, ...))
            chunk_index += 1
            buffer = get_last_sentences(buffer, config.overlap_sentences)
            buffer_tokens = estimate_tokens(buffer)

        // 对 section 按段落边界分割
        paragraphs = split_by_paragraph(section.content)
        对于 paragraphs 中的每个 paragraph:
            para_tokens = estimate_tokens(paragraph)

            // 代码块保护: 如果段落包含代码块且整体超过 max_tokens
            如果 contains_code_block(paragraph) 且 para_tokens > config.max_tokens:
                // 先输出 buffer
                如果 buffer_tokens > 0:
                    chunks.push(make_chunk(buffer, chunk_index, ...))
                    chunk_index += 1
                    buffer = ""
                    buffer_tokens = 0
                // 代码块单独成一个 Chunk（允许超过 max_tokens）
                chunks.push(make_chunk(paragraph, chunk_index, ...))
                chunk_index += 1
                继续下一个 paragraph

            如果 buffer_tokens + para_tokens <= config.max_tokens:
                buffer += "\n\n" + paragraph
                buffer_tokens += para_tokens
            否则:
                chunks.push(make_chunk(buffer, chunk_index, ...))
                chunk_index += 1
                buffer = get_last_sentences(buffer, config.overlap_sentences)
                         + paragraph
                buffer_tokens = estimate_tokens(buffer)

// 处理剩余的 buffer
如果 buffer_tokens > 0:
    如果 buffer_tokens >= config.min_tokens 或 chunks 为空:
        chunks.push(make_chunk(buffer, chunk_index, ...))
    否则:
        // 剩余内容太少，合并到最后一个 Chunk
        chunks.last_mut().content += "\n\n" + buffer

返回 chunks
```

---

## 4. 数据流图

### 4.1 search_notes 查询流程

```
LLM 调用 search_notes(query)
    │
    ▼
MemoryService::search(query, limit)
    │
    ▼
ObsidianClient::search(query, limit)
    │  POST /search/
    │  Content-Type: application/vnd.olrapi.jsonlogic+json
    │  Body: { "in": [query, {"var": "content"}] }
    ▼
Obsidian REST API
    │
    ▼
返回搜索结果 (JSON Array)
    │
    ▼
附加 Obsidian URI → 返回 SearchResultItem 列表给 LLM
```

### 4.2 get_note 读取流程

```
LLM 调用 get_note(path)
    │
    ▼
MemoryService::get_note(path)
    │
    ▼
ObsidianClient::read_file(path)
    │  GET /vault/{path}
    │  Accept: text/markdown
    ▼
Obsidian REST API
    │
    ▼
返回 Markdown 原文
    │
    ▼
解析 frontmatter、标签
    │
    ▼
返回 Note 结构给 LLM（含 obsidian_uri）
```

### 4.3 list_recent_notes 流程

```
LLM 调用 list_recent_notes(days, limit)
    │
    ▼
MemoryService::list_recent_notes(days, limit)
    │
    ▼
ObsidianClient::list_recent_notes(days, limit)
    │  GET /vault/
    │  获取文件列表及元数据
    ▼
Obsidian REST API
    │
    ▼
过滤时间范围、按修改时间排序
    │
    ▼
返回 NoteSummary 列表给 LLM
```

---

## 5. 关键数据结构

### 5.1 对外输出模型

```rust
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// 搜索结果项
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResultItem {
    /// 笔记路径（vault 内相对路径）
    pub path: String,
    /// 笔记标题
    pub title: String,
    /// 匹配片段摘要
    pub snippet: String,
    /// 相关性评分（Obsidian API 返回）
    pub score: f32,
    /// 标签列表
    pub tags: Vec<String>,
    /// Obsidian URI（可直接在 Obsidian 中打开）
    pub obsidian_uri: String,
}

/// 笔记搜索结果（按笔记聚合）
#[derive(Debug, Clone, serde::Serialize)]
pub struct NoteSearchResult {
    pub note_path: String,
    pub note_title: String,
    pub obsidian_uri: String,
    pub best_match_content: String,
    pub best_match_score: f32,
    pub match_count: usize,
}

/// 笔记摘要
#[derive(Debug, Clone, serde::Serialize)]
pub struct NoteSummary {
    pub path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub modified_at: DateTime<Utc>,
}

/// 笔记完整内容
#[derive(Debug, Clone, serde::Serialize)]
pub struct Note {
    pub path: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub obsidian_uri: String,
}

/// 记忆库统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryStats {
    /// vault 中的笔记总数
    pub total: usize,
    /// 标签分布
    pub by_tag: HashMap<String, usize>,
    /// 索引大小（MB），简化架构下为 0.0
    pub index_size_mb: f64,
}
```

### 5.2 内部事件

```rust
use std::path::PathBuf;
use chrono::{DateTime, Utc};

/// 文件变更事件（来自 file_watcher）
#[derive(Debug, Clone)]
pub enum FileEvent {
    /// 文件创建
    Created {
        path: PathBuf,
        timestamp: DateTime<Utc>,
    },
    /// 文件修改
    Modified {
        path: PathBuf,
        timestamp: DateTime<Utc>,
    },
    /// 文件删除
    Deleted {
        path: PathBuf,
        timestamp: DateTime<Utc>,
    },
}

/// 记忆引擎内部事件
#[derive(Debug, Clone)]
pub enum MemoryEvent {
    /// 搜索请求
    SearchRequested {
        query: String,
        limit: usize,
        timestamp: DateTime<Utc>,
    },
    /// 笔记读取
    NoteAccessed {
        path: String,
        timestamp: DateTime<Utc>,
    },
    /// 统计信息请求
    StatsRequested {
        timestamp: DateTime<Utc>,
    },
}
```

---

## 6. 错误处理

### 6.1 错误分类与处理策略

```rust
/// 记忆引擎相关的错误处理
impl MemoryService {
    /// 搜索时的错误处理
    pub async fn search_safe(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResultItem>, BrainError> {
        match self.search(query, limit).await {
            Ok(results) => Ok(results),
            Err(BrainError::ObsidianApiError(e)) => {
                tracing::error!("Obsidian API 搜索失败: {}", e);
                Err(BrainError::ObsidianApiError(e))
            }
            Err(e) => {
                tracing::error!("搜索未知错误: {}", e);
                Err(e)
            }
        }
    }
}
```

### 6.2 降级策略矩阵

| 组件 | 故障类型 | 降级行为 | 恢复策略 |
|---|---|---|---|
| Obsidian REST API | 连接失败/超时 | 搜索和文件操作均不可用，返回错误给 LLM | 等待 Obsidian 恢复后自动重试 |
| SQLite | 写入失败 | 元数据缓存不可用，搜索仍可通过 API 直接进行 | 重试写入 |
| 文件读取 | 权限错误/文件不存在 | 返回 `NoteNotFound` 错误 | 用户检查文件路径和权限 |

---

## 7. 测试策略

### 7.1 单元测试

#### ObsidianClient Mock 测试

```rust
#[cfg(test)]
mod obsidian_client_tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate, matchers};

    #[tokio::test]
    async fn test_search_success() {
        let mock_server = MockServer::start().await;

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/search/"))
            .and(matchers::header("Content-Type", "application/vnd.olrapi.jsonlogic+json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![
                SearchResultItem {
                    path: "test/note.md".to_string(),
                    title: "note".to_string(),
                    snippet: "匹配内容".to_string(),
                    score: 0.95,
                    tags: vec![],
                    obsidian_uri: "obsidian://open?vault=test&file=test/note.md".to_string(),
                },
            ]))
            .mount(&mock_server)
            .await;

        let client = ObsidianClient::new(&mock_server.uri(), "test-key", "test");
        let results = client.search("测试", 5).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "test/note.md");
        assert!(results[0].obsidian_uri.contains("test/note.md"));
    }

    #[tokio::test]
    async fn test_search_api_error() {
        let mock_server = MockServer::start().await;

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/search/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = ObsidianClient::new(&mock_server.uri(), "test-key", "test");
        let result = client.search("测试", 5).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_file_success() {
        let mock_server = MockServer::start().await;

        Mock::given(matchers::method("GET"))
            .and(matchers::path("/vault/test/note.md"))
            .respond_with(ResponseTemplate::new(200).set_body_string("# 标题\n\n内容"))
            .mount(&mock_server)
            .await;

        let client = ObsidianClient::new(&mock_server.uri(), "test-key", "test");
        let content = client.read_file("test/note.md").await.unwrap();

        assert!(content.contains("# 标题"));
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(matchers::method("GET"))
            .and(matchers::path("/vault/missing.md"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = ObsidianClient::new(&mock_server.uri(), "test-key", "test");
        let result = client.read_file("missing.md").await;

        assert!(result.is_err());
    }
}
```

#### MemoryService 集成测试

```rust
#[cfg(test)]
mod memory_service_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_delegates_to_obsidian() {
        let (mock_server, client) = setup_mock_obsidian_client().await;
        let memory = MemoryService::new(client, sqlite_store, vault_path, "test-vault".to_string());

        let results = memory.search("关键词", 5).await.unwrap();
        assert!(!results.is_empty());
        // 验证 obsidian_uri 已正确附加
        for r in &results {
            assert!(r.obsidian_uri.starts_with("obsidian://open"));
        }
    }

    #[tokio::test]
    async fn test_get_note_returns_structured_data() {
        let (mock_server, client) = setup_mock_obsidian_client().await;
        let memory = MemoryService::new(client, sqlite_store, vault_path, "test-vault".to_string());

        let note = memory.get_note("test/note.md").await.unwrap();
        assert_eq!(note.path, "test/note.md");
        assert!(!note.content.is_empty());
    }

    #[tokio::test]
    async fn test_stats_returns_correct_totals() {
        let memory = setup_test_memory_service().await;
        let stats = memory.stats().await.unwrap();

        assert!(stats.total > 0);
    }
}
```

### 7.2 集成测试

#### 端到端搜索流程测试

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_end_to_end() {
        // 使用 wiremock 模拟完整的 Obsidian API
        let mock_server = MockServer::start().await;

        // 模拟搜索 API
        Mock::given(matchers::method("POST"))
            .and(matchers::path("/search/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![
                SearchResultItem {
                    path: "notes/raft.md".to_string(),
                    title: "raft".to_string(),
                    snippet: "Raft 共识算法".to_string(),
                    score: 0.9,
                    tags: vec![],
                    obsidian_uri: "obsidian://open?vault=test&file=notes/raft.md".to_string(),
                },
                SearchResultItem {
                    path: "notes/paxos.md".to_string(),
                    title: "paxos".to_string(),
                    snippet: "Paxos 协议".to_string(),
                    score: 0.8,
                    tags: vec![],
                    obsidian_uri: "obsidian://open?vault=test&file=notes/paxos.md".to_string(),
                },
            ]))
            .mount(&mock_server)
            .await;

        let client = ObsidianClient::new(&mock_server.uri(), "test-key", "test");
        let memory = MemoryService::new(
            Arc::new(client),
            sqlite_store,
            PathBuf::from("/tmp/vault"),
            "test".to_string(),
        );

        let results = memory.search("共识算法", 5).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "notes/raft.md");
        assert!(results[0].obsidian_uri.contains("raft.md"));
    }

    #[tokio::test]
    async fn test_obsidian_api_unavailable_returns_error() {
        // 模拟 Obsidian 不可用
        let client = ObsidianClient::new("http://localhost:99999", "test-key", "test");
        let result = client.search("test", 5).await;

        assert!(result.is_err());
        // 验证错误类型
        assert!(matches!(result, Err(BrainError::ObsidianApiError(_))));
    }
}
```

### 7.3 性能基准测试

```rust
#[cfg(test)]
mod bench_tests {
    use super::*;
    use std::hint::black_box;

    #[tokio::test]
    async fn bench_search_latency() {
        let memory = setup_test_memory_service().await;

        let start = std::time::Instant::now();
        for _ in 0..100 {
            let results = memory.search("Rust 异步编程", 5).await.unwrap();
            black_box(results);
        }
        let avg_latency = start.elapsed() / 100;

        println!("搜索平均延迟: {:?}", avg_latency);
        // Obsidian API 搜索延迟通常 < 500ms
        assert!(avg_latency < std::time::Duration::from_millis(1000));
    }
}
```

---

## 8. 依赖清单

### 8.1 直接依赖

| Crate | 版本 | 用途 |
|---|---|---|
| `reqwest` | 0.12+ | HTTP 客户端（Obsidian REST API） |
| `tokio` | 1.38+ | 异步运行时 |
| `serde` + `serde_json` | 1.0+ | 序列化 |
| `chrono` | 0.4+ | 时间处理 |
| `tracing` | 0.1+ | 结构化日志 |
| `rusqlite` | 0.31+ | SQLite 数据库（元数据缓存） |
| `urlencoding` | 2.1+ | URI 编码（Obsidian URI） |
| `thiserror` | 1.0+ | 错误类型派生 |
| `pulldown-cmark` | 0.12+ | Markdown 解析（预留，当前未使用） |
| `gray_matter` | 0.2+ | YAML frontmatter 提取（预留，当前未使用） |
| `uuid` | 1.10+ | 唯一 ID 生成（预留，当前未使用） |

### 8.2 开发依赖

| Crate | 用途 |
|---|---|
| `tokio-test` | 异步测试辅助 |
| `tempfile` | 临时目录（测试用 SQLite 数据库） |
| `wiremock` | Mock HTTP 服务（模拟 Obsidian REST API） |

### 8.3 外部服务依赖

| 服务 | 必需性 | 说明 |
|---|---|---|
| **Obsidian Local REST API** | 必需 | 所有搜索和文件操作均通过此 API 完成 |
| **文件系统** | 间接 | Obsidian 管理的 Vault 文件存储 |

---

> **关联文档**：
> - [需求设计](../requirement/03-memory-engine.md) — 功能需求、用户故事、验收标准
> - [顶层设计](../top_design.md) — 系统架构、技术栈、数据模型
