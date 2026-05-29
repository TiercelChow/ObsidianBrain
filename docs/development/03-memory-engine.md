# 记忆引擎 (Memory Engine) — 开发设计文档

> **模块编号**: 03 | **版本**: v1.0 | **状态**: 设计中 | **关联**: [需求设计](../requirement/03-memory-engine.md) · [顶层设计](../top_design.md §5.1)

---

## 1. 技术架构详细设计

### 1.1 架构总览

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Memory Engine                                │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │                    Service 层 (memory.rs)                       │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  │  │
│  │  │  Parser  │ │ Chunker  │ │ Indexer  │ │ Search Engine    │  │  │
│  │  │  解析器   │ │  分块器   │ │  索引器   │ │  混合搜索引擎     │  │  │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘  │  │
│  └──────────────────────────┬─────────────────────────────────────┘  │
│                             │                                        │
│  ┌──────────────────────────┴─────────────────────────────────────┐  │
│  │                    Infrastructure 层                            │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  │  │
│  │  │ Tantivy  │ │ Qdrant   │ │ SQLite   │ │ Embedding        │  │  │
│  │  │ Index    │ │ Client   │ │ Store    │ │ Service          │  │  │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘  │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │                    Event 层                                     │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐ │  │
│  │  │ FileWatcher  │  │ EventBus     │  │ Timeline Publisher   │ │  │
│  │  │ (notify)     │──│ (broadcast)  │──│ (MemoryEvent)        │ │  │
│  │  └──────────────┘  └──────────────┘  └──────────────────────┘ │  │
│  └────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────┘
```

### 1.2 模块依赖关系

```
memory.rs (MemoryService)
├── parser.rs         (MarkdownParser)
│   ├── gray_matter    (frontmatter 解析)
│   └── pulldown-cmark (Markdown AST)
├── chunker.rs        (SmartChunker)
├── indexer.rs        (IndexManager)
│   ├── tantivy_index.rs  (全文索引)
│   │   └── jieba-rs      (中文分词)
│   ├── qdrant_client.rs  (向量索引)
│   └── embedding.rs      (Embedding 生成)
├── search.rs         (HybridSearchEngine)
│   ├── FullTextSearcher
│   └── SemanticSearcher
└── crud.rs           (MemoryCrud)
    └── sqlite_store.rs (元数据持久化)
```

### 1.3 并发模型

- **索引写入**：通过 `tokio::sync::mpsc` channel 接收文件变更事件，单 worker 顺序处理（避免索引竞争）
- **搜索请求**：多请求并行处理，每个搜索在独立 task 中执行
- **Embedding 批量**：索引 worker 内部聚合变更后批量调用 Embedding API
- **混合搜索**：全文检索与语义检索通过 `tokio::join!` 并行执行

---

## 2. 目录与文件组织

```
src/
├── core/
│   └── memory.rs              # MemoryService 主结构体，对外 API 入口
├── core/
│   └── memory/
│       ├── mod.rs             # 模块导出
│       ├── parser.rs          # Markdown 解析器
│       ├── chunker.rs         # 智能分块器
│       ├── indexer.rs         # 索引管理器（协调 Tantivy + Qdrant）
│       ├── search.rs          # 混合搜索引擎
│       └── crud.rs            # CRUD 操作实现
├── infra/
│   ├── tantivy_index.rs       # Tantivy 全文索引封装
│   ├── qdrant_client.rs       # Qdrant 向量操作封装
│   ├── sqlite_store.rs        # SQLite 元数据存储
│   ├── embedding.rs           # Embedding 生成服务
│   └── file_watcher.rs        # 文件监控
└── models/
    └── memory.rs              # Memory 相关数据模型
```

**设计说明**：`core/memory.rs` 作为门面（Facade），内部委托给 `memory/` 子模块。如果初期代码量不大，可将子模块合并到 `core/memory.rs` 单文件中，后续拆分。

---

## 3. 子模块详细设计

### 3.1 Markdown 解析器 (`parser.rs`)

#### 3.1.1 职责

将 Markdown 原始文本解析为结构化的 `ParsedDocument`，提取 frontmatter 元数据和正文的标题层级结构。

#### 3.1.2 核心数据结构

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use chrono::{DateTime, Utc};

/// 解析后的文档结构
#[derive(Debug, Clone)]
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

#### 3.1.3 解析流程

```rust
use gray_matter::engine::YAML;
use gray_matter::Matter;
use pulldown_cmark::{Parser, Event, Tag, TagEnd};

pub struct MarkdownParser {
    matter: Matter<YAML>,
}

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

    /// 使用 pulldown-cmark 解析正文为 Section 列表
    fn parse_sections(&self, body: &str) -> Result<Vec<Section>, BrainError> {
        let parser = Parser::new(body);
        let mut sections = Vec::new();
        let mut current_level: u8 = 0;
        let mut current_heading: Option<String> = None;
        let mut breadcrumb_stack: Vec<(u8, String)> = Vec::new(); // (level, text)
        let mut current_content = String::new();
        let mut in_code_block = false;
        let mut line_number = 1;

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    // 保存前一个 section
                    if !current_content.trim().is_empty() || current_heading.is_some() {
                        let breadcrumb = breadcrumb_stack
                            .iter()
                            .map(|(_, text)| text.clone())
                            .collect();
                        sections.push(Section {
                            level: current_level,
                            heading: current_heading.take(),
                            breadcrumb,
                            content: std::mem::take(&mut current_content),
                            code_blocks: Vec::new(), // 后续填充
                            line_start: line_number,
                            line_end: line_number,
                        });
                    }
                    current_level = level as u8;
                }
                Event::End(TagEnd::Heading(level)) => {
                    // 更新 breadcrumb 栈
                    let heading_text = current_content.trim().to_string();
                    // 弹出 >= 当前层级的旧条目
                    breadcrumb_stack.retain(|&(l, _)| l < level as u8);
                    breadcrumb_stack.push((level as u8, heading_text.clone()));
                    current_heading = Some(heading_text);
                    current_content.clear();
                }
                Event::Start(Tag::CodeBlock(_)) => {
                    in_code_block = true;
                }
                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;
                }
                Event::Text(text) => {
                    current_content.push_str(&text);
                }
                Event::SoftBreak | Event::HardBreak => {
                    current_content.push('\n');
                }
                _ => {}
            }
        }

        // 保存最后一个 section
        if !current_content.trim().is_empty() || current_heading.is_some() {
            let breadcrumb = breadcrumb_stack
                .iter()
                .map(|(_, text)| text.clone())
                .collect();
            sections.push(Section {
                level: current_level,
                heading: current_heading,
                breadcrumb,
                content: current_content,
                code_blocks: Vec::new(),
                line_start: line_number,
                line_end: line_number,
            });
        }

        Ok(sections)
    }

    fn extract_frontmatter(
        &self,
        result: &gray_matter::Pod<YAML>,
    ) -> HashMap<String, serde_json::Value> {
        // 从 result.data 中提取 frontmatter 键值对
        // ...
        HashMap::new() // placeholder
    }

    fn extract_tags(
        &self,
        frontmatter: &HashMap<String, serde_json::Value>,
        body: &str,
    ) -> Vec<String> {
        let mut tags = Vec::new();
        // 1. 从 frontmatter.tags 提取
        if let Some(serde_json::Value::Array(arr)) = frontmatter.get("tags") {
            for v in arr {
                if let Some(s) = v.as_str() {
                    tags.push(s.to_string());
                }
            }
        }
        // 2. 正则提取正文中的 #tag
        // let re = Regex::new(r"#([a-zA-Z一-鿿][\w一-鿿/-]*)").unwrap();
        // for cap in re.captures_iter(body) { ... }
        tags.sort();
        tags.dedup();
        tags
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

### 3.2 智能分块器 (`chunker.rs`)

#### 3.2.1 职责

将 `ParsedDocument` 的 `Section` 列表拆分为适合索引和检索的 `Chunk` 列表。

#### 3.2.2 Chunk 结构体

```rust
use uuid::Uuid;

/// 记忆单元 / 索引块
#[derive(Debug, Clone)]
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

#### 3.2.3 分块算法伪代码

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

#### 3.2.4 辅助函数

```rust
/// 估算文本的 token 数（中英文混合场景）
/// 粗略估算: 中文字符 ≈ 1 token, 英文单词 ≈ 1 token
pub fn estimate_tokens(text: &str) -> usize {
    let chinese_chars = text.chars()
        .filter(|c| c.is_ascii() == false)
        .count();
    let english_words = text
        .split_whitespace()
        .filter(|w| w.chars().all(|c| c.is_ascii()))
        .count();
    // 加上标点等
    chinese_chars + english_words
}

/// 从文本末尾提取最后 N 个句子（用于重叠）
pub fn get_last_sentences(text: &str, n: usize) -> String {
    let sentences: Vec<&str> = text
        .split(|c: char| c == '。' || c == '.' || c == '\n')
        .filter(|s| !s.trim().is_empty())
        .collect();
    let start = if sentences.len() > n {
        sentences.len() - n
    } else {
        0
    };
    sentences[start..].join("\n")
}

/// 按空行分割段落
pub fn split_by_paragraph(text: &str) -> Vec<&str> {
    text.split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .collect()
}

/// 检查文本是否包含围栏代码块
pub fn contains_code_block(text: &str) -> bool {
    text.contains("```")
}
```

### 3.3 全文索引管理 (`infra/tantivy_index.rs`)

#### 3.3.1 Tantivy Schema 定义

```rust
use tantivy::schema::*;
use tantivy_jieba::JiebaTokenizer;

/// 全文索引管理器
pub struct TantivyIndexManager {
    index: tantivy::Index,
    schema: Schema,
    // Schema 字段引用（避免重复查找）
    fields: TantivyFields,
    writer: std::sync::Mutex<IndexWriter>,
}

/// Tantivy Schema 中的字段定义
pub struct TantivyFields {
    /// Chunk 唯一标识（存储 Uuid 字符串）
    pub chunk_id: Field,
    /// 来源笔记路径
    pub note_path: Field,
    /// Chunk 文本内容（全文索引 + 存储）
    pub content: Field,
    /// 标题路径 breadcrumb（索引，用于展示）
    pub breadcrumb: Field,
    /// 标签（多值字段，用于过滤）
    pub tags: Field,
    /// 笔记标题（索引 + 存储）
    pub note_title: Field,
    /// Chunk 序号
    pub chunk_index: Field,
}

impl TantivyIndexManager {
    /// 创建 Tantivy Schema
    pub fn build_schema() -> (Schema, TantivyFields) {
        let mut schema_builder = Schema::builder();

        // chunk_id: 精确匹配，用于删除/更新
        let chunk_id = schema_builder.add_text_field(
            "chunk_id",
            STRING | STORED,
        );

        // note_path: 精确匹配 + 存储，用于按文件删除和溯源
        let note_path = schema_builder.add_text_field(
            "note_path",
            STRING | STORED,
        );

        // content: 全文索引（中英文分词） + 存储
        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("jieba")  // 中文分词器
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();
        let content = schema_builder.add_text_field("content", text_options);

        // breadcrumb: 存储用于展示，也可搜索
        let breadcrumb = schema_builder.add_text_field(
            "breadcrumb",
            TEXT | STORED,
        );

        // tags: 多值字符串字段，用于过滤
        let tags = schema_builder.add_text_field(
            "tags",
            STRING | STORED,
        );

        // note_title: 全文索引 + 存储
        let note_title = schema_builder.add_text_field(
            "note_title",
            TEXT | STORED,
        );

        // chunk_index: 数值字段，用于排序
        let chunk_index = schema_builder.add_u64_field(
            "chunk_index",
            INDEXED | STORED,
        );

        let schema = schema_builder.build();
        let fields = TantivyFields {
            chunk_id,
            note_path,
            content,
            breadcrumb,
            tags,
            note_title,
            chunk_index,
        };

        (schema, fields)
    }

    /// 初始化索引（创建或打开）
    pub fn open(index_path: &std::path::Path, heap_size: usize) -> Result<Self, BrainError> {
        let (schema, fields) = Self::build_schema();

        let index = if index_path.exists() {
            tantivy::Index::open_in_dir(index_path)
                .map_err(|e| BrainError::SearchError(e.to_string()))?
        } else {
            std::fs::create_dir_all(index_path)?;
            tantivy::Index::create_in_dir(index_path, schema.clone())
                .map_err(|e| BrainError::SearchError(e.to_string()))?
        };

        // 注册 jieba 中文分词器
        index.tokenizers()
            .register("jieba", JiebaTokenizer);

        let writer = index.writer(heap_size)
            .map_err(|e| BrainError::SearchError(e.to_string()))?;

        Ok(Self {
            index,
            schema,
            fields,
            writer: std::sync::Mutex::new(writer),
        })
    }
}
```

#### 3.3.2 Document 构建与索引更新

```rust
impl TantivyIndexManager {
    /// 将 Chunk 转为 Tantivy Document
    fn chunk_to_document(&self, chunk: &Chunk) -> tantivy::TantivyDocument {
        let mut doc = tantivy::TantivyDocument::new();

        doc.add_text(self.fields.chunk_id, &chunk.id.to_string());
        doc.add_text(
            self.fields.note_path,
            chunk.note_path.to_str().unwrap_or(""),
        );
        doc.add_text(self.fields.content, &chunk.content);
        doc.add_text(
            self.fields.breadcrumb,
            &chunk.breadcrumb.join(" > "),
        );
        for tag in &chunk.tags {
            doc.add_text(self.fields.tags, tag);
        }
        doc.add_text(self.fields.note_title, &chunk.note_title);
        doc.add_u64(self.fields.chunk_index, chunk.chunk_index as u64);

        doc
    }

    /// 索引一组 Chunk（对应一个文件的全部 Chunk）
    /// 策略: 先删除该文件旧文档，再插入新文档 (delete + insert)
    pub fn index_chunks(&self, note_path: &Path, chunks: &[Chunk]) -> Result<(), BrainError> {
        let mut writer = self.writer.lock()
            .map_err(|e| BrainError::Internal(e.to_string()))?;

        // 1. 删除该文件的所有旧文档
        let path_str = note_path.to_str().unwrap_or("");
        let delete_query = format!("note_path:\"{}\"", path_str);
        writer.delete_query(
            self.schema.parse_query(&delete_query)
                .map_err(|e| BrainError::SearchError(e.to_string()))?,
        ).map_err(|e| BrainError::SearchError(e.to_string()))?;

        // 2. 插入新文档
        for chunk in chunks {
            let doc = self.chunk_to_document(chunk);
            writer.add_document(doc)
                .map_err(|e| BrainError::SearchError(e.to_string()))?;
        }

        // 3. 提交
        writer.commit()
            .map_err(|e| BrainError::SearchError(e.to_string()))?;

        Ok(())
    }

    /// 删除指定文件的所有索引
    pub fn remove_by_path(&self, note_path: &Path) -> Result<usize, BrainError> {
        let mut writer = self.writer.lock()
            .map_err(|e| BrainError::Internal(e.to_string()))?;

        let path_str = note_path.to_str().unwrap_or("");
        let delete_query = format!("note_path:\"{}\"", path_str);
        writer.delete_query(
            self.schema.parse_query(&delete_query)
                .map_err(|e| BrainError::SearchError(e.to_string()))?,
        ).map_err(|e| BrainError::SearchError(e.to_string()))?;

        writer.commit()
            .map_err(|e| BrainError::SearchError(e.to_string()))?;

        Ok(0) // 删除数量后续可从查询结果获取
    }

    /// 删除指定 Chunk
    pub fn remove_chunk(&self, chunk_id: &Uuid) -> Result<(), BrainError> {
        let mut writer = self.writer.lock()
            .map_err(|e| BrainError::Internal(e.to_string()))?;

        let id_str = chunk_id.to_string();
        writer.delete_term(Term::from_field_text(self.fields.chunk_id, &id_str));
        writer.commit()
            .map_err(|e| BrainError::SearchError(e.to_string()))?;

        Ok(())
    }
}
```

#### 3.3.3 查询构建器

```rust
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;

/// 全文搜索结果
#[derive(Debug, Clone)]
pub struct FullTextSearchResult {
    pub chunk_id: Uuid,
    pub note_path: PathBuf,
    pub content: String,
    pub breadcrumb: String,
    pub note_title: String,
    pub chunk_index: usize,
    pub score: f32,  // BM25 评分
}

impl TantivyIndexManager {
    /// 执行全文搜索
    pub fn search(
        &self,
        query: &str,
        top_k: usize,
        tag_filter: Option<&[String]>,
    ) -> Result<Vec<FullTextSearchResult>, BrainError> {
        let reader = self.index.reader()
            .map_err(|e| BrainError::SearchError(e.to_string()))?;
        let searcher = reader.searcher();

        // 构建查询：在 content + note_title + breadcrumb 字段中搜索
        let query_parser = QueryParser::for_index(
            &self.index,
            vec![self.fields.content, self.fields.note_title, self.fields.breadcrumb],
        );
        let parsed_query = query_parser.parse_query(query)
            .map_err(|e| BrainError::SearchError(e.to_string()))?;

        // 如果有标签过滤，使用 BooleanQuery 组合
        let final_query: Box<dyn tantivy::query::Query> = if let Some(tags) = tag_filter {
            let mut sub_queries: Vec<Box<dyn tantivy::query::Query>> = vec![parsed_query];
            for tag in tags {
                let tag_query = Box::new(
                    tantivy::query::TermQuery::new(
                        Term::from_field_text(self.fields.tags, tag),
                        IndexRecordOption::Basic,
                    ),
                );
                sub_queries.push(tag_query);
            }
            Box::new(tantivy::query::BooleanQuery::intersection(
                sub_queries.into_iter().map(|q| (tantivy::query::Occur::Must, q)).collect(),
            ))
        } else {
            parsed_query
        };

        // 执行搜索
        let top_docs = searcher.search(
            &final_query,
            &TopDocs::with_limit(top_k),
        ).map_err(|e| BrainError::SearchError(e.to_string()))?;

        // 提取结果
        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: tantivy::TantivyDocument = searcher.doc(doc_address)
                .map_err(|e| BrainError::SearchError(e.to_string()))?;

            results.push(FullTextSearchResult {
                chunk_id: self.get_field_text(&doc, self.fields.chunk_id)
                    .parse().unwrap_or_default(),
                note_path: PathBuf::from(
                    self.get_field_text(&doc, self.fields.note_path),
                ),
                content: self.get_field_text(&doc, self.fields.content),
                breadcrumb: self.get_field_text(&doc, self.fields.breadcrumb),
                note_title: self.get_field_text(&doc, self.fields.note_title),
                chunk_index: self.get_field_u64(&doc, self.fields.chunk_index) as usize,
                score,
            });
        }

        Ok(results)
    }

    fn get_field_text(&self, doc: &tantivy::TantivyDocument, field: Field) -> String {
        doc.get_first(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn get_field_u64(&self, doc: &tantivy::TantivyDocument, field: Field) -> u64 {
        doc.get_first(field)
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    }
}
```

### 3.4 向量索引管理 (`infra/qdrant_client.rs`)

#### 3.4.1 Qdrant Collection 配置

```rust
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, VectorParamsBuilder,
    HnswConfigDiff, OptimizersConfigDiff,
};

/// Qdrant 向量索引管理器
pub struct QdrantIndexManager {
    client: Qdrant,
    collection_name: String,
    vector_size: u64,
}

/// Qdrant 配置
#[derive(Debug, Clone)]
pub struct QdrantConfig {
    pub url: String,
    pub collection_name: String,
    pub vector_size: u64,
}

impl QdrantIndexManager {
    pub async fn new(config: &QdrantConfig) -> Result<Self, BrainError> {
        let client = Qdrant::from_url(&config.url)
            .build()
            .map_err(|e| BrainError::QdrantError(e.to_string()))?;

        let manager = Self {
            client,
            collection_name: config.collection_name.clone(),
            vector_size: config.vector_size,
        };

        // 确保 collection 存在
        manager.ensure_collection().await?;

        Ok(manager)
    }

    /// 创建或检查 Qdrant collection
    async fn ensure_collection(&self) -> Result<(), BrainError> {
        let exists = self.client
            .collection_exists(&self.collection_name)
            .await
            .map_err(|e| BrainError::QdrantError(e.to_string()))?;

        if !exists {
            // HNSW 参数调优
            let hnsw_config = HnswConfigDiff {
                m: Some(16),                  // 每个节点最大连接数
                ef_construct: Some(100),      // 构建时搜索宽度
                full_scan_threshold: Some(10000), // 小于此值用暴力搜索
                ..Default::default()
            };

            // Optimizer 配置
            let optimizer_config = OptimizersConfigDiff {
                memmap_threshold: Some(20000), // 超过此量级使用 mmap
                indexing_threshold: Some(20000),
                ..Default::default()
            };

            self.client
                .create_collection(
                    CreateCollectionBuilder::new(&self.collection_name)
                        .vectors_config(
                            VectorParamsBuilder::new(self.vector_size, Distance::Cosine)
                                .build(),
                        )
                        .hnsw_config(hnsw_config)
                        .optimizers_config(optimizer_config)
                        .on_disk_payload(Some(false)), // payload 放内存，加速过滤
                )
                .await
                .map_err(|e| BrainError::QdrantError(e.to_string()))?;
        }

        Ok(())
    }
}
```

#### 3.4.2 PointStruct 定义与 Payload Schema

```rust
use qdrant_client::qdrant::{PointStruct, PointId, Value};

/// Qdrant 中每个向量点的 payload 结构
/// 与 Chunk 一一对应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChunkPayload {
    /// Chunk UUID
    pub chunk_id: String,
    /// 来源笔记路径
    pub note_path: String,
    /// 笔记标题
    pub note_title: String,
    /// 标题路径 breadcrumb
    pub breadcrumb: String,
    /// 标签（逗号分隔，用于 payload filter）
    pub tags: Vec<String>,
    /// Chunk 序号
    pub chunk_index: u64,
    /// 起始行号
    pub line_start: u64,
    /// 结束行号
    pub line_end: u64,
    /// 文本内容（存储在 payload 中，避免搜索后再次读取文件）
    pub content: String,
    /// 估算 token 数
    pub token_count: u64,
}

impl QdrantIndexManager {
    /// 将 Chunk + Embedding 向量转为 Qdrant PointStruct
    fn chunk_to_point(
        &self,
        chunk: &Chunk,
        vector: Vec<f32>,
    ) -> PointStruct {
        let payload = ChunkPayload {
            chunk_id: chunk.id.to_string(),
            note_path: chunk.note_path.to_str().unwrap_or("").to_string(),
            note_title: chunk.note_title.clone(),
            breadcrumb: chunk.breadcrumb.join(" > "),
            tags: chunk.tags.clone(),
            chunk_index: chunk.chunk_index as u64,
            line_start: chunk.line_start as u64,
            line_end: chunk.line_end as u64,
            content: chunk.content.clone(),
            token_count: chunk.token_count as u64,
        };

        // 使用 UUID 的哈希作为 point ID (u64)
        let point_id = chunk.id.as_u64_pair().0;

        PointStruct::new(
            point_id,
            vector,
            // payload 转为 HashMap<String, Value>
            serde_json::to_value(&payload)
                .unwrap_or_default()
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), Value::from(v.clone())))
                .collect(),
        )
    }

    /// 批量 upsert 向量点
    pub async fn upsert_chunks(
        &self,
        chunks: &[Chunk],
        vectors: &[Vec<f32>],
    ) -> Result<(), BrainError> {
        assert_eq!(chunks.len(), vectors.len());

        let points: Vec<PointStruct> = chunks
            .iter()
            .zip(vectors.iter())
            .map(|(chunk, vector)| self.chunk_to_point(chunk, vector.clone()))
            .collect();

        // 分批写入，每批 100 个
        for batch in points.chunks(100) {
            self.client
                .upsert_points(
                    qdrant_client::qdrant::UpsertPointsBuilder::new(
                        &self.collection_name,
                        batch.to_vec(),
                    ).wait(true),
                )
                .await
                .map_err(|e| BrainError::QdrantError(e.to_string()))?;
        }

        Ok(())
    }

    /// 按笔记路径删除所有向量点
    pub async fn remove_by_path(&self, note_path: &Path) -> Result<(), BrainError> {
        use qdrant_client::qdrant::{Filter, FieldCondition, MatchValue};

        let path_str = note_path.to_str().unwrap_or("").to_string();

        self.client
            .delete_points(
                qdrant_client::qdrant::DeletePointsBuilder::new(&self.collection_name)
                    .points(
                        qdrant_client::qdrant::PointsSelectorBuilder::new()
                            .filter(
                                Filter::must([
                                    FieldCondition::must_match(
                                        "note_path",
                                        MatchValue::from(path_str),
                                    ).into(),
                                ]),
                            ),
                    )
                    .wait(true)
                    .build(),
            )
            .await
            .map_err(|e| BrainError::QdrantError(e.to_string()))?;

        Ok(())
    }

    /// 按 chunk_id 删除单个向量点
    pub async fn remove_chunk(&self, chunk_id: &Uuid) -> Result<(), BrainError> {
        let point_id = chunk_id.as_u64_pair().0;
        self.client
            .delete_points(
                qdrant_client::qdrant::DeletePointsBuilder::new(&self.collection_name)
                    .points(vec![PointId::from(point_id)])
                    .wait(true)
                    .build(),
            )
            .await
            .map_err(|e| BrainError::QdrantError(e.to_string()))?;

        Ok(())
    }

    /// 语义搜索
    pub async fn search(
        &self,
        query_vector: &[f32],
        top_k: usize,
        tag_filter: Option<&[String]>,
    ) -> Result<Vec<SemanticSearchResult>, BrainError> {
        use qdrant_client::qdrant::{SearchPointsBuilder, Filter, FieldCondition, MatchValue};

        let mut builder = SearchPointsBuilder::new(
            &self.collection_name,
            query_vector.to_vec(),
            top_k as u64,
        )
        .with_payload(true)
        .params(qdrant_client::qdrant::SearchParams {
            hnsw_ef: Some(128),  // 搜索精度参数
            ..Default::default()
        });

        // 标签过滤
        if let Some(tags) = tag_filter {
            let conditions: Vec<_> = tags
                .iter()
                .map(|tag| {
                    FieldCondition::must_match("tags", MatchValue::from(tag.clone())).into()
                })
                .collect();
            builder = builder.filter(Filter::must(conditions));
        }

        let response = self.client
            .search_points(builder)
            .await
            .map_err(|e| BrainError::QdrantError(e.to_string()))?;

        let results = response.result.into_iter().map(|point| {
            let payload = &point.payload;
            SemanticSearchResult {
                chunk_id: payload.get("chunk_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .parse()
                    .unwrap_or_default(),
                note_path: PathBuf::from(
                    payload.get("note_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                ),
                content: payload.get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                breadcrumb: payload.get("breadcrumb")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                note_title: payload.get("note_title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                chunk_index: payload.get("chunk_index")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize,
                score: point.score,  // 余弦相似度
            }
        }).collect();

        Ok(results)
    }
}

/// 语义搜索结果
#[derive(Debug, Clone)]
pub struct SemanticSearchResult {
    pub chunk_id: Uuid,
    pub note_path: PathBuf,
    pub content: String,
    pub breadcrumb: String,
    pub note_title: String,
    pub chunk_index: usize,
    pub score: f32,  // 余弦相似度 (0.0 ~ 1.0)
}
```

### 3.5 Embedding 批处理 (`infra/embedding.rs`)

#### 3.5.1 核心设计

```rust
use reqwest::Client;

/// Embedding 服务
pub struct EmbeddingService {
    client: Client,
    config: EmbeddingConfig,
}

#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    pub provider: EmbeddingProvider,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub batch_size: usize,       // 单次 API 最大请求数
    pub max_retries: u32,
    pub retry_delay_ms: u64,
}

#[derive(Debug, Clone)]
pub enum EmbeddingProvider {
    OpenAI,
    Ollama,
}

impl EmbeddingService {
    /// 批量生成 Embedding 向量
    /// 自动按 batch_size 分批，支持错误重试
    pub async fn embed_batch(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, BrainError> {
        let mut all_vectors = Vec::with_capacity(texts.len());

        for batch in texts.chunks(self.config.batch_size) {
            let batch_texts: Vec<&str> = batch.iter().map(|s| s.as_str()).collect();

            let vectors = self.embed_with_retry(&batch_texts).await?;
            all_vectors.extend(vectors);
        }

        Ok(all_vectors)
    }

    /// 带重试的 Embedding 调用
    async fn embed_with_retry(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, BrainError> {
        let mut delay = self.config.retry_delay_ms;

        for attempt in 0..=self.config.max_retries {
            match self.call_embedding_api(texts).await {
                Ok(vectors) => return Ok(vectors),
                Err(e) => {
                    if attempt == self.config.max_retries {
                        tracing::error!(
                            "Embedding API 调用失败 (尝试 {}/{}): {}",
                            attempt + 1,
                            self.config.max_retries + 1,
                            e
                        );
                        return Err(e);
                    }
                    tracing::warn!(
                        "Embedding API 调用失败 (尝试 {}/{}): {}，{}ms 后重试",
                        attempt + 1,
                        self.config.max_retries + 1,
                        e,
                        delay,
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    delay *= 2; // 指数退避
                }
            }
        }

        Err(BrainError::EmbeddingError("重试次数耗尽".to_string()))
    }

    /// 调用具体的 Embedding API
    async fn call_embedding_api(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, BrainError> {
        match self.config.provider {
            EmbeddingProvider::OpenAI => self.call_openai_embedding(texts).await,
            EmbeddingProvider::Ollama => self.call_ollama_embedding(texts).await,
        }
    }

    async fn call_openai_embedding(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, BrainError> {
        let url = self.config.base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1/embeddings");

        let body = serde_json::json!({
            "model": self.config.model,
            "input": texts,
        });

        let mut request = self.client.post(url).json(&body);
        if let Some(ref api_key) = self.config.api_key {
            request = request.bearer_auth(api_key);
        }

        let response = request.send().await
            .map_err(|e| BrainError::EmbeddingError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BrainError::EmbeddingError(
                format!("API 返回 {}: {}", status, body),
            ));
        }

        let result: OpenAIEmbeddingResponse = response.json().await
            .map_err(|e| BrainError::EmbeddingError(e.to_string()))?;

        Ok(result.data.into_iter().map(|d| d.embedding).collect())
    }

    async fn call_ollama_embedding(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, BrainError> {
        // Ollama API 调用实现...
        unimplemented!()
    }
}

#[derive(serde::Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<OpenAIEmbeddingData>,
}

#[derive(serde::Deserialize)]
struct OpenAIEmbeddingData {
    embedding: Vec<f32>,
    index: usize,
}
```

### 3.6 混合搜索引擎 (`core/memory/search.rs`)

#### 3.6.1 并行执行与 RRF 融合

```rust
use tokio::join;

/// 混合搜索引擎
pub struct HybridSearchEngine {
    tantivy: Arc<TantivyIndexManager>,
    qdrant: Arc<QdrantIndexManager>,
    embedding: Arc<EmbeddingService>,
    config: HybridSearchConfig,
}

#[derive(Debug, Clone)]
pub struct HybridSearchConfig {
    /// RRF 常数 k
    pub rrf_k: f64,
    /// 全文检索候选数
    pub fulltext_top_k: usize,
    /// 语义检索候选数
    pub semantic_top_k: usize,
    /// 是否启用混合搜索（false 时仅全文搜索）
    pub hybrid_enabled: bool,
    /// importance 微调权重
    pub importance_weight: f64,
    /// access_count 微调权重
    pub access_count_weight: f64,
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self {
            rrf_k: 60.0,
            fulltext_top_k: 20,
            semantic_top_k: 20,
            hybrid_enabled: true,
            importance_weight: 0.05,
            access_count_weight: 0.02,
        }
    }
}

/// 混合搜索结果
#[derive(Debug, Clone)]
pub struct HybridSearchResult {
    pub chunk_id: Uuid,
    pub note_path: PathBuf,
    pub note_title: String,
    pub content: String,
    pub breadcrumb: String,
    pub chunk_index: usize,
    pub rrf_score: f64,
    pub fulltext_rank: Option<usize>,
    pub fulltext_score: Option<f32>,
    pub semantic_rank: Option<usize>,
    pub semantic_score: Option<f32>,
    pub obsidian_uri: String,
}

impl HybridSearchEngine {
    /// 执行混合搜索
    pub async fn search(
        &self,
        query: &str,
        top_k: usize,
        tag_filter: Option<&[String]>,
        vault_name: &str,
    ) -> Result<Vec<HybridSearchResult>, BrainError> {
        if !self.config.hybrid_enabled {
            // 降级：仅全文搜索
            return self.fulltext_only_search(query, top_k, tag_filter, vault_name).await;
        }

        // 并行执行全文检索和语义检索
        let (fulltext_result, semantic_result) = join!(
            // 全文检索
            async {
                self.tantivy.search(query, self.config.fulltext_top_k, tag_filter)
            },
            // 语义检索：先向量化查询，再搜索
            async {
                let embedding_result = self.embedding.embed_batch(&[query.to_string()]).await;
                match embedding_result {
                    Ok(vectors) if !vectors.is_empty() => {
                        self.qdrant.search(&vectors[0], self.config.semantic_top_k, tag_filter).await
                    }
                    Ok(_) => Ok(vec![]),
                    Err(e) => {
                        tracing::warn!("Embedding 生成失败，降级为纯全文搜索: {}", e);
                        Err(e)
                    }
                }
            }
        );

        let fulltext_results = fulltext_result?;
        let semantic_results = match semantic_result {
            Ok(results) => results,
            Err(e) => {
                tracing::warn!("语义搜索失败，降级为纯全文搜索: {}", e);
                vec![]
            }
        };

        // RRF 融合
        let merged = self.rrf_merge(
            &fulltext_results,
            &semantic_results,
            top_k,
            vault_name,
        );

        Ok(merged)
    }

    /// RRF (Reciprocal Rank Fusion) 融合算法
    ///
    /// 公式: RRF_score(d) = Σ 1 / (k + rank_i(d))
    /// 其中 k 为常数（默认 60），rank 从 1 开始
    fn rrf_merge(
        &self,
        fulltext_results: &[FullTextSearchResult],
        semantic_results: &[SemanticSearchResult],
        top_k: usize,
        vault_name: &str,
    ) -> Vec<HybridSearchResult> {
        use std::collections::HashMap;

        let k = self.config.rrf_k;

        // chunk_id -> (fulltext_rank, semantic_rank, fulltext_score, semantic_score, 元数据)
        let mut score_map: HashMap<Uuid, (Option<usize>, Option<usize>, Option<f32>, Option<f32>, HybridSearchResult)> = HashMap::new();

        // 处理全文检索结果
        for (rank, result) in fulltext_results.iter().enumerate() {
            let rank_1based = rank + 1;
            let obsidian_uri = format!(
                "obsidian://open?vault={}&file={}",
                urlencoding::encode(vault_name),
                urlencoding::encode(result.note_path.to_str().unwrap_or("")),
            );

            score_map.entry(result.chunk_id).or_insert_with(|| {
                (None, None, None, None, HybridSearchResult {
                    chunk_id: result.chunk_id,
                    note_path: result.note_path.clone(),
                    note_title: result.note_title.clone(),
                    content: result.content.clone(),
                    breadcrumb: result.breadcrumb.clone(),
                    chunk_index: result.chunk_index,
                    rrf_score: 0.0,
                    fulltext_rank: None,
                    fulltext_score: None,
                    semantic_rank: None,
                    semantic_score: None,
                    obsidian_uri,
                })
            });

            if let Some(entry) = score_map.get_mut(&result.chunk_id) {
                entry.0 = Some(rank_1based);
                entry.2 = Some(result.score);
            }
        }

        // 处理语义检索结果
        for (rank, result) in semantic_results.iter().enumerate() {
            let rank_1based = rank + 1;

            let entry = score_map.entry(result.chunk_id).or_insert_with(|| {
                let obsidian_uri = format!(
                    "obsidian://open?vault={}&file={}",
                    urlencoding::encode(vault_name),
                    urlencoding::encode(result.note_path.to_str().unwrap_or("")),
                );
                (None, None, None, None, HybridSearchResult {
                    chunk_id: result.chunk_id,
                    note_path: result.note_path.clone(),
                    note_title: result.note_title.clone(),
                    content: result.content.clone(),
                    breadcrumb: result.breadcrumb.clone(),
                    chunk_index: result.chunk_index,
                    rrf_score: 0.0,
                    fulltext_rank: None,
                    fulltext_score: None,
                    semantic_rank: None,
                    semantic_score: None,
                    obsidian_uri,
                })
            });

            entry.1 = Some(rank_1based);
            entry.3 = Some(result.score);
        }

        // 计算 RRF 评分
        let mut results: Vec<HybridSearchResult> = score_map.into_values().map(|(ft_rank, sem_rank, ft_score, sem_score, mut result)| {
            let mut rrf_score = 0.0_f64;

            if let Some(rank) = ft_rank {
                rrf_score += 1.0 / (k + rank as f64);
            }
            if let Some(rank) = sem_rank {
                rrf_score += 1.0 / (k + rank as f64);
            }

            result.rrf_score = rrf_score;
            result.fulltext_rank = ft_rank;
            result.fulltext_score = ft_score;
            result.semantic_rank = sem_rank;
            result.semantic_score = sem_score;

            result
        }).collect();

        // 按 RRF 评分降序排列
        results.sort_by(|a, b| b.rrf_score.partial_cmp(&a.rrf_score).unwrap());

        // 截取 top_k
        results.truncate(top_k);

        results
    }

    /// 降级：仅全文搜索
    async fn fulltext_only_search(
        &self,
        query: &str,
        top_k: usize,
        tag_filter: Option<&[String]>,
        vault_name: &str,
    ) -> Result<Vec<HybridSearchResult>, BrainError> {
        let fulltext_results = self.tantivy.search(query, top_k, tag_filter)?;

        let results = fulltext_results.into_iter().enumerate().map(|(rank, r)| {
            let obsidian_uri = format!(
                "obsidian://open?vault={}&file={}",
                urlencoding::encode(vault_name),
                urlencoding::encode(r.note_path.to_str().unwrap_or("")),
            );
            HybridSearchResult {
                chunk_id: r.chunk_id,
                note_path: r.note_path,
                note_title: r.note_title,
                content: r.content,
                breadcrumb: r.breadcrumb,
                chunk_index: r.chunk_index,
                rrf_score: 1.0 / (self.config.rrf_k + (rank + 1) as f64),
                fulltext_rank: Some(rank + 1),
                fulltext_score: Some(r.score),
                semantic_rank: None,
                semantic_score: None,
                obsidian_uri,
            }
        }).collect();

        Ok(results)
    }
}
```

### 3.7 记忆 CRUD 操作 (`core/memory/crud.rs`)

#### 3.7.1 完整 MemoryService 结构

```rust
use std::sync::Arc;
use tokio::sync::mpsc;

/// 记忆引擎主服务 — 对外暴露的所有操作的入口
pub struct MemoryService {
    parser: Arc<MarkdownParser>,
    chunker: Arc<SmartChunker>,
    indexer: Arc<IndexManager>,
    search_engine: Arc<HybridSearchEngine>,
    embedding: Arc<EmbeddingService>,
    sqlite: Arc<SqliteStore>,
    event_bus: Arc<EventBus>,
    config: MemoryConfig,
    vault_path: PathBuf,
    vault_name: String,
}

/// 记忆引擎配置
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    pub chunker: ChunkerConfig,
    pub search: HybridSearchConfig,
    pub debounce_ms: u64,
    pub exclude_patterns: Vec<String>,
}

impl MemoryService {
    // =================== 索引操作 ===================

    /// 处理文件变更事件（由 FileWatcher 调用）
    pub async fn on_file_event(&self, event: FileEvent) -> Result<(), BrainError> {
        match event {
            FileEvent::Created(path) | FileEvent::Modified(path) => {
                self.index_file(&path).await
            }
            FileEvent::Removed(path) => {
                self.remove_file_index(&path).await
            }
            FileEvent::Renamed { from, to } => {
                self.remove_file_index(&from).await?;
                self.index_file(&to).await
            }
        }
    }

    /// 索引单个文件：解析 → 分块 → 全文索引 → 向量化 → 向量索引
    async fn index_file(&self, path: &Path) -> Result<(), BrainError> {
        // 1. 检查是否应排除
        if self.should_exclude(path) {
            return Ok(());
        }

        // 2. 读取文件内容
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| BrainError::IoError(e))?;

        // 3. 解析 Markdown
        let relative_path = self.to_relative_path(path);
        let doc = self.parser.parse(relative_path.clone(), &content)?;

        // 4. 智能分块
        let chunks = self.chunker.chunk(&doc);

        // 5. 全文索引（Tantivy）
        self.indexer.tantivy.index_chunks(&relative_path, &chunks)?;

        // 6. 批量生成 Embedding
        let texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
        let vectors = match self.embedding.embed_batch(&texts).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Embedding 生成失败，仅保留全文索引: {}", e);
                // 发布事件
                self.event_bus.publish(MemoryEvent::Indexed {
                    path: relative_path,
                    chunk_count: chunks.len(),
                });
                return Ok(());
            }
        };

        // 7. 向量索引（Qdrant）
        self.indexer.qdrant.upsert_chunks(&chunks, &vectors).await?;

        // 8. 更新 SQLite 元数据
        self.sqlite.update_file_chunks(
            &relative_path,
            chunks.len(),
            Utc::now(),
        ).await?;

        // 9. 发布事件
        self.event_bus.publish(MemoryEvent::Indexed {
            path: relative_path,
            chunk_count: chunks.len(),
        });

        Ok(())
    }

    /// 删除文件的所有索引
    async fn remove_file_index(&self, path: &Path) -> Result<(), BrainError> {
        let relative_path = self.to_relative_path(path);

        // 从 Tantivy 删除
        self.indexer.tantivy.remove_by_path(&relative_path)?;

        // 从 Qdrant 删除
        self.indexer.qdrant.remove_by_path(&relative_path).await?;

        // 从 SQLite 删除
        self.sqlite.remove_file_chunks(&relative_path).await?;

        // 发布事件
        self.event_bus.publish(MemoryEvent::Removed {
            path: relative_path,
            chunk_count: 0,
        });

        Ok(())
    }

    // =================== 搜索操作 ===================

    /// search_memory: 混合检索记忆
    pub async fn search(
        &self,
        query: &str,
        top_k: Option<usize>,
        tags: Option<Vec<String>>,
    ) -> Result<Vec<MemorySearchResult>, BrainError> {
        let top_k = top_k.unwrap_or(5);
        let tag_refs = tags.as_deref();

        let results = self.search_engine
            .search(query, top_k, tag_refs, &self.vault_name)
            .await?;

        // 更新 access_count（异步，不阻塞搜索响应）
        let chunk_ids: Vec<Uuid> = results.iter().map(|r| r.chunk_id).collect();
        let sqlite = self.sqlite.clone();
        tokio::spawn(async move {
            for id in &chunk_ids {
                let _ = sqlite.increment_access_count(id).await;
            }
        });

        // 转为对外输出格式
        let memory_results = results.into_iter().map(|r| {
            MemorySearchResult {
                memory_id: r.chunk_id,
                note_path: r.note_path,
                note_title: r.note_title,
                content: r.content,
                breadcrumb: r.breadcrumb,
                chunk_index: r.chunk_index,
                score: r.rrf_score,
                obsidian_uri: r.obsidian_uri,
                fulltext_rank: r.fulltext_rank,
                semantic_rank: r.semantic_rank,
            }
        }).collect();

        Ok(memory_results)
    }

    /// search_notes: 按笔记聚合的搜索
    pub async fn search_notes(
        &self,
        query: &str,
        top_k: Option<usize>,
        tags: Option<Vec<String>>,
    ) -> Result<Vec<NoteSearchResult>, BrainError> {
        // 多取一些 Chunk，再按笔记聚合
        let chunk_results = self.search(query, top_k.map(|k| k * 3), tags).await?;

        // 按 note_path 分组
        let mut note_map: std::collections::HashMap<PathBuf, NoteSearchResult> =
            std::collections::HashMap::new();

        for chunk in chunk_results {
            let entry = note_map.entry(chunk.note_path.clone()).or_insert_with(|| {
                NoteSearchResult {
                    note_path: chunk.note_path.clone(),
                    note_title: chunk.note_title.clone(),
                    obsidian_uri: chunk.obsidian_uri.clone(),
                    best_match_content: chunk.content.clone(),
                    best_match_score: chunk.score,
                    match_count: 0,
                }
            });
            entry.match_count += 1;
            if chunk.score > entry.best_match_score {
                entry.best_match_content = chunk.content;
                entry.best_match_score = chunk.score;
            }
        }

        let mut results: Vec<NoteSearchResult> = note_map.into_values().collect();
        results.sort_by(|a, b| b.best_match_score.partial_cmp(&a.best_match_score).unwrap());
        results.truncate(top_k.unwrap_or(5));

        Ok(results)
    }

    // =================== CRUD 操作 ===================

    /// add_memory: 手动添加记忆
    pub async fn add(
        &self,
        note_path: &str,
        content: &str,
        tags: Option<Vec<String>>,
    ) -> Result<Memory, BrainError> {
        let path = PathBuf::from(note_path);
        let full_path = self.vault_path.join(&path);

        // 1. 写入笔记文件（追加）
        let separator = "\n\n<!-- memory-engine:managed -->\n";
        let formatted_content = format!("{}{}", separator, content);

        if full_path.exists() {
            let mut existing = tokio::fs::read_to_string(&full_path).await?;
            existing.push_str(&formatted_content);
            tokio::fs::write(&full_path, existing).await?;
        } else {
            // 创建新文件
            if let Some(parent) = full_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            let mut file_content = String::new();
            // 添加 frontmatter
            if let Some(ref tags) = tags {
                file_content.push_str("---\ntags:\n");
                for tag in tags {
                    file_content.push_str(&format!("  - {}\n", tag));
                }
                file_content.push_str("---\n\n");
            }
            file_content.push_str(content);
            tokio::fs::write(&full_path, file_content).await?;
        }

        // 2. 文件监控会自动触发 index_file，但也可以手动触发确保即时可用
        // 这里等待文件监控事件即可，不重复索引

        // 3. 发布事件
        let memory_id = Uuid::new_v4();
        self.event_bus.publish(MemoryEvent::MemoryCreated {
            memory_id,
            note_path: path,
        });

        // 返回创建的记忆（简化：实际应从索引结果中获取）
        Ok(Memory {
            id: memory_id,
            note_path: PathBuf::from(note_path),
            chunk_index: 0,
            content: content.to_string(),
            summary: None,
            tags: tags.unwrap_or_default(),
            embedding_id: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            access_count: 0,
            importance: 0.5,
        })
    }

    /// update_memory: 更新记忆内容
    pub async fn update(
        &self,
        memory_id: &Uuid,
        new_content: &str,
    ) -> Result<Memory, BrainError> {
        // 1. 查找记忆对应的文件和 Chunk
        let chunk_info = self.sqlite.get_chunk_info(memory_id).await?
            .ok_or(BrainError::NoteNotFound(PathBuf::from("memory not found")))?;

        // 2. 更新文件中的对应段落
        // （简化实现：重新索引整个文件）
        self.index_file(&self.vault_path.join(&chunk_info.note_path)).await?;

        // 3. 重新生成 Embedding 并更新 Qdrant
        let vectors = self.embedding.embed_batch(&[new_content.to_string()]).await?;
        if !vectors.is_empty() {
            // 更新 Qdrant 中的向量
            self.indexer.qdrant.upsert_chunks(
                &[Chunk {
                    id: *memory_id,
                    content: new_content.to_string(),
                    ..Default::default()
                }],
                &vectors,
            ).await?;
        }

        // 4. 更新 Tantivy
        // （已通过重新索引文件完成）

        Ok(Memory {
            id: *memory_id,
            note_path: chunk_info.note_path,
            content: new_content.to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        })
    }

    /// forget_memory: 删除记忆
    pub async fn forget(
        &self,
        memory_id: &Uuid,
    ) -> Result<bool, BrainError> {
        // 1. 从 Tantivy 删除
        self.indexer.tantivy.remove_chunk(memory_id)?;

        // 2. 从 Qdrant 删除
        self.indexer.qdrant.remove_chunk(memory_id).await?;

        // 3. 从 SQLite 删除元数据
        self.sqlite.remove_chunk_info(memory_id).await?;

        Ok(true)
    }

    /// get_memory_stats: 获取记忆库统计
    pub async fn stats(&self) -> Result<MemoryStats, BrainError> {
        let total = self.sqlite.get_total_chunk_count().await?;
        let by_tag = self.sqlite.get_tag_distribution().await?;
        let recent = self.sqlite.get_recent_chunks(10).await?;
        let index_size_mb = self.get_index_size_mb().await?;

        Ok(MemoryStats {
            total,
            by_tag,
            recent,
            index_size_mb,
        })
    }

    // =================== 笔记操作 ===================

    /// get_note: 获取笔记完整内容
    pub async fn get_note(&self, path: &str) -> Result<Note, BrainError> {
        let full_path = self.vault_path.join(path);
        if !full_path.exists() {
            return Err(BrainError::NoteNotFound(PathBuf::from(path)));
        }

        let content = tokio::fs::read_to_string(&full_path).await?;
        let doc = self.parser.parse(PathBuf::from(path), &content)?;

        Ok(Note {
            path: PathBuf::from(path),
            title: doc.title,
            content,
            frontmatter: doc.frontmatter,
            tags: doc.tags,
            created_at: doc.created_at,
            updated_at: doc.updated_at,
            word_count: doc.sections.iter().map(|s| s.content.len()).sum(),
        })
    }

    /// list_recent_notes: 列出最近修改的笔记
    pub async fn list_recent(
        &self,
        days: Option<u32>,
        limit: Option<usize>,
    ) -> Result<Vec<NoteSummary>, BrainError> {
        let days = days.unwrap_or(7);
        let limit = limit.unwrap_or(20);

        // 扫描 vault 目录，按修改时间排序
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);
        let mut notes = Vec::new();

        let mut entries = tokio::fs::read_dir(&self.vault_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            if self.should_exclude(&path) {
                continue;
            }

            let metadata = entry.metadata().await?;
            let modified: DateTime<Utc> = metadata.modified()?.into();
            if modified >= cutoff {
                let relative = self.to_relative_path(&path);
                let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
                let doc = self.parser.parse(relative.clone(), &content).ok();

                notes.push(NoteSummary {
                    path: relative,
                    title: doc.as_ref().map(|d| d.title.clone()).unwrap_or_default(),
                    tags: doc.as_ref().map(|d| d.tags.clone()).unwrap_or_default(),
                    modified_at: modified,
                });
            }
        }

        notes.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        notes.truncate(limit);

        Ok(notes)
    }

    // =================== 全量索引 ===================

    /// 首次启动或手动触发全量索引
    pub async fn full_index(&self) -> Result<FullIndexReport, BrainError> {
        tracing::info!("开始全量索引...");
        let start = std::time::Instant::now();

        let mut total_files = 0;
        let mut total_chunks = 0;
        let mut errors = Vec::new();

        // 递归扫描 vault 目录
        let files = self.scan_vault_files(&self.vault_path).await?;

        for file_path in &files {
            match self.index_file(file_path).await {
                Ok(_) => {
                    total_files += 1;
                }
                Err(e) => {
                    tracing::warn!("索引文件失败 {:?}: {}", file_path, e);
                    errors.push((file_path.clone(), e.to_string()));
                }
            }
        }

        total_chunks = self.sqlite.get_total_chunk_count().await?;
        let duration = start.elapsed();

        tracing::info!(
            "全量索引完成: {} 个文件, {} 个 Chunk, 耗时 {:?}, {} 个错误",
            total_files, total_chunks, duration, errors.len()
        );

        Ok(FullIndexReport {
            total_files,
            total_chunks,
            duration,
            errors,
        })
    }
}
```

---

## 4. 数据流图

### 4.1 文件变更 → 索引更新 完整流程

```
┌─────────────┐
│ 用户编辑笔记  │  保存 .md 文件
└──────┬──────┘
       │
       ▼
┌──────────────────┐
│ notify FileWatcher │  检测文件 Create/Modify/Remove
└──────┬───────────┘
       │  FileEvent (via EventBus)
       ▼
┌──────────────────┐
│ 防抖聚合 (300ms)   │  合并同文件多次变更
└──────┬───────────┘
       │
       ▼
┌──────────────────┐    读取文件内容
│ MarkdownParser   │────────────────────┐
│ (parser.rs)      │                    │
└──────┬───────────┘                    ▼
       │  ParsedDocument          ┌────────────┐
       ▼                          │ gray_matter │  frontmatter 提取
┌──────────────────┐              │pulldown-cmark│ 正文解析
│ SmartChunker     │              └────────────┘
│ (chunker.rs)     │
└──────┬───────────┘
       │  Vec<Chunk>
       ├─────────────────────────────┐
       ▼                             ▼
┌──────────────────┐       ┌──────────────────┐
│ TantivyIndex     │       │ EmbeddingService  │
│ (全文索引)         │       │ (批量向量化)       │
│                  │       └──────┬───────────┘
│ - 删除旧文档      │              │  Vec<Vec<f32>>
│ - 插入新文档      │              ▼
│ - commit         │       ┌──────────────────┐
└──────────────────┘       │ QdrantIndex      │
                           │ (向量索引)         │
                           │                  │
                           │ - upsert points  │
                           └──────────────────┘
                                    │
                                    ▼
                           ┌──────────────────┐
                           │ SQLite 元数据更新  │
                           │ EventBus 发布事件  │
                           │ → Timeline 记录   │
                           └──────────────────┘
```

### 4.2 `search_memory` 查询流程

```
┌──────────────────┐
│ LLM 调用          │  search_memory(query, top_k, tags)
│ search_memory    │
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│ HybridSearch     │
│ Engine           │
└──────┬───────────┘
       │
       ├──────────────────────┐
       │                      │
       ▼                      ▼
┌──────────────────┐  ┌──────────────────┐
│ FullText Search  │  │ Semantic Search   │
│ (Tantivy)        │  │                   │
│                  │  │  1. Embedding     │
│  query → BM25    │  │     query → vec   │
│  top-20 结果      │  │  2. Qdrant ANN   │
│                  │  │     top-20 结果    │
└──────┬───────────┘  └──────┬───────────┘
       │                      │
       └──────────┬───────────┘
                  │
                  ▼
       ┌──────────────────┐
       │  RRF 融合          │
       │                    │
       │  score(d) =        │
       │    1/(k+rank_ft)   │
       │  + 1/(k+rank_sem)  │
       │                    │
       │  → 去重             │
       │  → 排序             │
       │  → top_k           │
       └──────┬───────────┘
              │
              ▼
       ┌──────────────────┐
       │ 后处理             │
       │                    │
       │ - 附加 obsidian_uri│
       │ - access_count +=1 │
       │ - 格式化输出        │
       └──────┬───────────┘
              │
              ▼
       ┌──────────────────┐
       │ 返回结果给 LLM    │  Vec<MemorySearchResult>
       └──────────────────┘
```

### 4.3 混合检索融合流程（RRF 详细）

```
输入:
  fulltext_results = [A(rank=1), B(rank=2), C(rank=3), D(rank=4), ...]
  semantic_results = [B(rank=1), E(rank=2), A(rank=3), F(rank=4), ...]
  k = 60

Step 1: 计算每个文档的 RRF 分
  A: 1/(60+1) + 1/(60+3) = 0.01639 + 0.01587 = 0.03226
  B: 1/(60+2) + 1/(60+1) = 0.01613 + 0.01639 = 0.03252
  C: 1/(60+3)            = 0.01587
  D: 1/(60+4)            = 0.01563
  E: 1/(60+2)            = 0.01613
  F: 1/(60+4)            = 0.01563

Step 2: 按 RRF 分降序排列
  B (0.03252) > A (0.03226) > E (0.01613) > C (0.01587) > D (0.01563) = F (0.01563)

Step 3: 取 top_k 返回
  → [B, A, E, C, D]  (top_k=5)

说明:
  - B 在两路中都排名靠前，RRF 分最高
  - A 在全文中排第1、语义中排第3，综合排第2
  - 仅出现在一路中的文档，RRF 分较低
```

---

## 5. 关键数据结构

### 5.1 对外输出模型

```rust
/// 记忆搜索结果（对外输出）
#[derive(Debug, Clone, serde::Serialize)]
pub struct MemorySearchResult {
    /// 记忆 ID (Chunk UUID)
    pub memory_id: Uuid,
    /// 来源笔记路径
    pub note_path: PathBuf,
    /// 笔记标题
    pub note_title: String,
    /// 记忆内容
    pub content: String,
    /// 标题路径
    pub breadcrumb: String,
    /// Chunk 序号
    pub chunk_index: usize,
    /// 综合评分 (RRF)
    pub score: f64,
    /// Obsidian URI
    pub obsidian_uri: String,
    /// 全文检索排名（如果在全文结果中出现）
    pub fulltext_rank: Option<usize>,
    /// 语义检索排名（如果在语义结果中出现）
    pub semantic_rank: Option<usize>,
}

/// 笔记搜索结果（按笔记聚合）
#[derive(Debug, Clone, serde::Serialize)]
pub struct NoteSearchResult {
    pub note_path: PathBuf,
    pub note_title: String,
    pub obsidian_uri: String,
    pub best_match_content: String,
    pub best_match_score: f64,
    pub match_count: usize,
}

/// 笔记摘要
#[derive(Debug, Clone, serde::Serialize)]
pub struct NoteSummary {
    pub path: PathBuf,
    pub title: String,
    pub tags: Vec<String>,
    pub modified_at: DateTime<Utc>,
}

/// 记忆库统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryStats {
    pub total: usize,
    pub by_tag: HashMap<String, usize>,
    pub recent: Vec<MemorySummary>,
    pub index_size_mb: f64,
}

/// 记忆摘要（用于统计展示）
#[derive(Debug, Clone, serde::Serialize)]
pub struct MemorySummary {
    pub memory_id: Uuid,
    pub note_path: PathBuf,
    pub note_title: String,
    pub content_preview: String,  // 前 100 字符
    pub created_at: DateTime<Utc>,
}

/// 全量索引报告
#[derive(Debug, Clone, serde::Serialize)]
pub struct FullIndexReport {
    pub total_files: usize,
    pub total_chunks: usize,
    pub duration: std::time::Duration,
    pub errors: Vec<(PathBuf, String)>,
}
```

### 5.2 内部事件

```rust
/// 文件变更事件（FileWatcher → MemoryService）
#[derive(Debug, Clone)]
pub enum FileEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Removed(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

/// 记忆引擎事件（MemoryService → Timeline）
#[derive(Debug, Clone)]
pub enum MemoryEvent {
    Indexed {
        path: PathBuf,
        chunk_count: usize,
    },
    Updated {
        path: PathBuf,
        chunk_count: usize,
    },
    Removed {
        path: PathBuf,
        chunk_count: usize,
    },
    MemoryCreated {
        memory_id: Uuid,
        note_path: PathBuf,
    },
}
```

---

## 6. 算法详述

### 6.1 分块算法伪代码

已在 §3.2.3 中详细描述。核心逻辑摘要：

```
1. 按标题层级 (H1 > H2 > H3) 将文档切分为 Section
2. 对每个 Section:
   a. 如果 section_tokens ≤ max_tokens:
      - 尝试合并到当前 buffer
      - buffer 满时输出为一个 Chunk，保留重叠句
   b. 如果 section_tokens > max_tokens:
      - 先输出当前 buffer
      - 按段落（空行）边界二次分割
      - 代码块作为不可分割单元保护
3. 最终将剩余 buffer 合并到最后一个 Chunk（或独立成 Chunk）
```

### 6.2 RRF 融合算法实现

已在 §3.6.1 的 `rrf_merge` 方法中完整实现。核心公式：

```
RRF_score(d) = Σ_{i ∈ {fulltext, semantic}} 1 / (k + rank_i(d))

其中:
  k = 60 (默认，可配置)
  rank_i(d) = 文档 d 在第 i 路结果中的排名（从 1 开始）
  如果文档 d 不在第 i 路结果中，则该项为 0
```

**算法特性**：
- 无需对两路评分做归一化（BM25 与余弦相似度量纲不同，RRF 仅使用排名）
- 同时出现在两路结果中的文档会获得更高的 RRF 分
- 对异常评分不敏感（仅依赖排名顺序）

### 6.3 BM25 + 余弦相似度评分融合说明

本系统**不直接融合** BM25 分和余弦相似度分，原因如下：

| 评分方式 | 值域 | 特点 |
|---|---|---|
| BM25 | [0, +∞) | 无固定上界，受文档长度影响 |
| 余弦相似度 | [0, 1] | 归一化，值域固定 |

两种评分量纲不同，直接加权需要复杂的归一化处理。RRF 仅基于排名融合，天然规避了量纲问题。

**保留原始评分的目的**：
- `fulltext_score`（BM25）和 `semantic_score`（余弦）作为附加信息返回给调用方
- 便于调试和评估搜索质量
- 未来可尝试其他融合策略（如归一化加权）

---

## 7. 错误处理

### 7.1 错误分类与处理策略

```rust
/// 记忆引擎相关的错误处理
impl MemoryService {
    /// 索引文件时的错误处理
    async fn index_file_safe(&self, path: &Path) -> Result<(), BrainError> {
        match self.index_file(path).await {
            Ok(_) => Ok(()),
            Err(BrainError::EmbeddingError(e)) => {
                // Embedding 失败：全文索引已建立，降级运行
                tracing::warn!("文件 {:?} 仅建立全文索引，向量化失败: {}", path, e);
                Ok(())  // 不返回错误，允许降级
            }
            Err(BrainError::QdrantError(e)) => {
                // Qdrant 不可用：记录待同步队列
                tracing::warn!("Qdrant 写入失败 {:?}: {}，加入待同步队列", path, e);
                self.sqlite.add_pending_sync(path).await?;
                Ok(())
            }
            Err(e) => {
                // 其他错误：记录日志，不中断监控循环
                tracing::error!("索引文件 {:?} 失败: {}", path, e);
                Err(e)
            }
        }
    }
}
```

### 7.2 降级策略矩阵

| 组件 | 故障类型 | 降级行为 | 恢复策略 |
|---|---|---|---|
| Embedding API | 超时/限流/不可用 | 仅建立全文索引，跳过向量化 | 指数退避重试，3 次后降级 |
| Qdrant | 连接失败/服务不可用 | 全文搜索可用，语义搜索不可用 | 待同步队列 + 定期重试 |
| Tantivy | 索引损坏 | 全量重建索引 | 删除索引目录 + `full_index()` |
| SQLite | 写入失败 | 内存状态可用，持久化延迟 | 重试写入 |
| 文件读取 | 权限错误/文件损坏 | 跳过该文件，记录错误 | 用户修复后重新触发 |

### 7.3 待同步队列

```rust
/// 当 Qdrant 不可用时，记录待同步的文件路径
/// 存储在 SQLite 的 pending_syncs 表中
/// 后台任务定期检查 Qdrant 是否恢复，恢复后批量同步

impl SqliteStore {
    pub async fn add_pending_sync(&self, path: &Path) -> Result<(), BrainError> {
        // INSERT OR REPLACE INTO pending_syncs (path, created_at)
        // VALUES (?, CURRENT_TIMESTAMP)
        todo!()
    }

    pub async fn get_pending_syncs(&self, limit: usize) -> Result<Vec<PathBuf>, BrainError> {
        // SELECT path FROM pending_syncs ORDER BY created_at LIMIT ?
        todo!()
    }

    pub async fn remove_pending_sync(&self, path: &Path) -> Result<(), BrainError> {
        // DELETE FROM pending_syncs WHERE path = ?
        todo!()
    }
}

/// 后台同步任务
async fn sync_pending_task(memory: Arc<MemoryService>, interval_secs: u64) {
    let mut interval = tokio::time::interval(
        std::time::Duration::from_secs(interval_secs),
    );

    loop {
        interval.tick().await;

        // 检查 Qdrant 是否可用
        if memory.qdrant.is_healthy().await {
            let pending = memory.sqlite.get_pending_syncs(50).await.unwrap_or_default();
            for path in pending {
                match memory.index_file(&path).await {
                    Ok(_) => {
                        let _ = memory.sqlite.remove_pending_sync(&path).await;
                        tracing::info!("待同步文件已处理: {:?}", path);
                    }
                    Err(e) => {
                        tracing::warn!("同步待处理文件失败 {:?}: {}", path, e);
                    }
                }
            }
        }
    }
}
```

---

## 8. 性能优化

### 8.1 批量 Embedding

```rust
/// 索引 worker 内部的变更聚合
/// 收到多个文件变更事件后，统一批量调用 Embedding API

struct IndexWorker {
    receiver: mpsc::Receiver<FileEvent>,
    memory: Arc<MemoryService>,
    debounce_ms: u64,
}

impl IndexWorker {
    async fn run(mut self) {
        let mut pending_events: Vec<FileEvent> = Vec::new();
        let mut debounce_timer: Option<tokio::time::Sleep> = None;

        loop {
            tokio::select! {
                Some(event) = self.receiver.recv() => {
                    pending_events.push(event);
                    // 重置防抖计时器
                    debounce_timer = Some(tokio::time::sleep(
                        std::time::Duration::from_millis(self.debounce_ms),
                    ));
                }
                _ = async {
                    if let Some(ref mut timer) = debounce_timer {
                        timer.as_mut().await;
                    } else {
                        // 永远不会完成
                        std::future::pending::<()>().await;
                    }
                } => {
                    // 防抖超时，批量处理所有待处理事件
                    self.process_batch(&mut pending_events).await;
                    debounce_timer = None;
                }
            }
        }
    }

    async fn process_batch(&self, events: &mut Vec<FileEvent>) {
        // 1. 去重：同文件多次变更只处理最后一次
        let deduped = self.deduplicate_events(events.drain(..).collect());

        // 2. 批量解析和分块
        let mut all_chunks: Vec<Chunk> = Vec::new();
        let mut chunk_file_map: Vec<(usize, usize, PathBuf)> = Vec::new();

        for (path, content) in &deduped {
            // 先做全文索引（不依赖 Embedding）
            if let Ok(doc) = self.memory.parser.parse(path.clone(), content) {
                let chunks = self.memory.chunker.chunk(&doc);
                let start = all_chunks.len();
                let _ = self.memory.indexer.tantivy.index_chunks(path, &chunks);
                all_chunks.extend(chunks.clone());
                chunk_file_map.push((start, all_chunks.len(), path.clone()));
            }
        }

        // 3. 批量 Embedding（一次 API 调用处理所有 Chunk）
        if !all_chunks.is_empty() {
            let texts: Vec<String> = all_chunks.iter().map(|c| c.content.clone()).collect();
            match self.memory.embedding.embed_batch(&texts).await {
                Ok(vectors) => {
                    // 4. 批量写入 Qdrant
                    let _ = self.memory.indexer.qdrant.upsert_chunks(&all_chunks, &vectors).await;
                }
                Err(e) => {
                    tracing::warn!("批量 Embedding 失败: {}，文件已建立全文索引", e);
                }
            }
        }

        tracing::info!(
            "批量索引完成: {} 个文件, {} 个 Chunk",
            deduped.len(),
            all_chunks.len(),
        );
    }
}
```

### 8.2 增量索引

- **文件级别增量**：只重新索引变更的文件，未变更的文件保留原有索引
- **Chunk 级别优化**（可选，后续迭代）：
  - 对比新旧 Chunk 内容，仅对内容变化的 Chunk 重新生成 Embedding
  - 通过 Chunk 内容的 hash 判断是否变化
  - 未变化的 Chunk 保留原向量 ID 和内容

### 8.3 搜索并行化

```rust
// 全文检索和语义检索通过 tokio::join! 并行执行
let (fulltext_result, semantic_result) = join!(
    self.tantivy.search(query, top_k, tag_filter),
    async {
        let vec = self.embedding.embed_batch(&[query]).await?;
        self.qdrant.search(&vec[0], top_k, tag_filter).await
    }
);
// 总延迟 ≈ max(全文延迟, 语义延迟) 而非两者之和
```

### 8.4 索引缓存

- **Tantivy 索引**：常驻内存，通过 `IndexReader` 自动 reload
- **Qdrant**：服务端维护 HNSW 索引，客户端无需缓存
- **SQLite 元数据**：热点查询（文件→Chunk 映射）缓存在内存 HashMap 中
- **Embedding 缓存**（可选，后续迭代）：对相同文本的 Embedding 结果做 LRU 缓存

---

## 9. 测试策略

### 9.1 单元测试

#### 分块正确性测试

```rust
#[cfg(test)]
mod chunker_tests {
    use super::*;

    #[test]
    fn test_basic_chunking() {
        let doc = create_test_document(
            "# Title\n\nIntro paragraph.\n\n## Section 1\n\nContent 1.\n\n## Section 2\n\nContent 2."
        );
        let chunker = SmartChunker::new(ChunkerConfig::default());
        let chunks = chunker.chunk(&doc);

        assert!(chunks.len() >= 1);
        for chunk in &chunks {
            assert!(chunk.token_count <= 800);
        }
    }

    #[test]
    fn test_code_block_preservation() {
        let content = format!(
            "# Test\n\nSome text.\n\n```rust\n{}\n```\n\nMore text.",
            "fn main() {{}}\n".repeat(60) // 60 行代码
        );
        let doc = create_test_document(&content);
        let chunker = SmartChunker::new(ChunkerConfig::default());
        let chunks = chunker.chunk(&doc);

        // 验证代码块完整出现在某个 Chunk 中
        let code_block_complete = chunks.iter().any(|c| {
            c.content.contains("```rust")
                && c.content.contains("fn main()")
                && c.content.matches("```").count() >= 2
        });
        assert!(code_block_complete, "代码块应在同一 Chunk 中保持完整");
    }

    #[test]
    fn test_breadcrumb_preservation() {
        let doc = create_test_document(
            "# H1\n\n## H2\n\n### H3\n\nDeep content here."
        );
        let chunker = SmartChunker::new(ChunkerConfig::default());
        let chunks = chunker.chunk(&doc);

        let deep_chunk = chunks.iter().find(|c| c.content.contains("Deep content")).unwrap();
        assert!(deep_chunk.breadcrumb.contains(&"H1".to_string()));
        assert!(deep_chunk.breadcrumb.contains(&"H2".to_string()));
        assert!(deep_chunk.breadcrumb.contains(&"H3".to_string()));
    }

    #[test]
    fn test_long_paragraph_split() {
        // 构造一个超长段落（>800 tokens）
        let long_content = "这是一段很长的内容。".repeat(500);
        let doc = create_test_document(&format!("# Title\n\n{}", long_content));
        let chunker = SmartChunker::new(ChunkerConfig::default());
        let chunks = chunker.chunk(&doc);

        assert!(chunks.len() > 1, "超长段落应被分割为多个 Chunk");
    }

    #[test]
    fn test_empty_document() {
        let doc = create_test_document("");
        let chunker = SmartChunker::new(ChunkerConfig::default());
        let chunks = chunker.chunk(&doc);

        assert!(chunks.is_empty() || chunks.iter().all(|c| c.content.trim().is_empty()));
    }
}
```

### 9.2 集成测试

#### 搜索准确性测试

```rust
#[cfg(test)]
mod search_tests {
    use super::*;

    #[tokio::test]
    async fn test_hybrid_search_chinese() {
        let memory = setup_test_memory().await;

        // 插入测试笔记
        memory.add("test/raft.md", "Raft 是一种共识算法，用于分布式系统中保证数据一致性。", Some(vec!["distributed".to_string()])).await.unwrap();
        memory.add("test/paxos.md", "Paxos consensus protocol 是经典的分布式共识协议。", Some(vec!["distributed".to_string()])).await.unwrap();
        memory.add("test/rust-async.md", "Rust 的异步编程模型基于 Future 和 async/await 语法。", Some(vec!["rust".to_string()])).await.unwrap();

        // 等待索引完成
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // 搜索"分布式一致性"应返回 Raft 和 Paxos 相关结果
        let results = memory.search("分布式一致性", Some(5), None).await.unwrap();

        assert!(!results.is_empty());
        // 前两条应与分布式/共识相关
        let relevant_count = results.iter().take(2).filter(|r| {
            r.note_path.to_str().unwrap().contains("raft") ||
            r.note_path.to_str().unwrap().contains("paxos")
        }).count();
        assert!(relevant_count >= 1, "应至少返回一条共识相关结果");
    }

    #[tokio::test]
    async fn test_search_with_tag_filter() {
        let memory = setup_test_memory().await;
        // ... 插入数据 ...

        let results = memory.search("算法", Some(5), Some(vec!["rust".to_string()])).await.unwrap();

        // 所有结果应包含 "rust" 标签
        for r in &results {
            // 验证标签过滤生效
        }
    }

    #[tokio::test]
    async fn test_crud_lifecycle() {
        let memory = setup_test_memory().await;

        // Add
        let mem = memory.add("test/note.md", "测试内容", None).await.unwrap();
        let mem_id = mem.id;

        // Search (验证可搜索)
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let results = memory.search("测试内容", None, None).await.unwrap();
        assert!(!results.is_empty());

        // Update
        memory.update(&mem_id, "更新后的内容").await.unwrap();

        // Forget
        let deleted = memory.forget(&mem_id).await.unwrap();
        assert!(deleted);

        // 验证已删除
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let results = memory.search("更新后的内容", None, None).await.unwrap();
        assert!(results.iter().all(|r| r.memory_id != mem_id));
    }
}
```

### 9.3 性能基准测试

```rust
#[cfg(test)]
mod bench_tests {
    use super::*;
    use std::hint::black_box;

    #[tokio::test]
    async fn bench_fulltext_search_latency() {
        let memory = setup_test_memory_with_data(1000).await; // 1000 条 Chunk

        let start = std::time::Instant::now();
        for _ in 0..100 {
            let results = memory.search("Rust 异步编程", Some(5), None).await.unwrap();
            black_box(results);
        }
        let avg_latency = start.elapsed() / 100;

        println!("全文搜索平均延迟: {:?}", avg_latency);
        assert!(avg_latency < std::time::Duration::from_millis(50));
    }

    #[tokio::test]
    async fn bench_hybrid_search_latency() {
        let memory = setup_test_memory_with_data(1000).await;

        let start = std::time::Instant::now();
        for _ in 0..100 {
            let results = memory.search("Rust 异步编程", Some(5), None).await.unwrap();
            black_box(results);
        }
        let avg_latency = start.elapsed() / 100;

        println!("混合搜索平均延迟: {:?}", avg_latency);
        assert!(avg_latency < std::time::Duration::from_millis(300));
    }

    #[tokio::test]
    async fn bench_chunking_throughput() {
        let content = generate_large_markdown(100); // 100 个段落
        let doc = create_test_document(&content);
        let chunker = SmartChunker::new(ChunkerConfig::default());

        let start = std::time::Instant::now();
        for _ in 0..100 {
            let chunks = chunker.chunk(&doc);
            black_box(chunks);
        }
        let avg_latency = start.elapsed() / 100;

        println!("分块平均延迟: {:?}", avg_latency);
        assert!(avg_latency < std::time::Duration::from_millis(10));
    }
}
```

---

## 10. 依赖清单

### 10.1 直接依赖

| Crate | 版本 | 用途 |
|---|---|---|
| `tantivy` | 0.22+ | 全文搜索引擎 |
| `tantivy-jieba` | 0.11+ | Tantivy 的 jieba 中文分词器 |
| `qdrant-client` | 1.12+ | Qdrant 向量数据库客户端 |
| `pulldown-cmark` | 0.12+ | Markdown 解析 |
| `gray_matter` | 0.2+ | YAML frontmatter 提取 |
| `tokio` | 1.38+ | 异步运行时 |
| `serde` + `serde_json` | 1.0+ | 序列化 |
| `uuid` | 1.10+ | 唯一 ID 生成 |
| `chrono` | 0.4+ | 时间处理 |
| `reqwest` | 0.12+ | HTTP 客户端（Embedding API） |
| `tracing` | 0.1+ | 结构化日志 |
| `notify` | 6.1+ | 文件系统监控 |
| `rusqlite` | 0.31+ | SQLite 数据库 |
| `urlencoding` | 2.1+ | URI 编码（Obsidian URI） |
| `regex` | 1.10+ | 正则表达式（标签提取等） |
| `thiserror` | 1.0+ | 错误类型派生 |

### 10.2 开发依赖

| Crate | 用途 |
|---|---|
| `tokio-test` | 异步测试辅助 |
| `tempfile` | 临时目录（测试用 Tantivy 索引和 Vault） |
| `mockall` | Mock 对象（模拟 Embedding API、Qdrant） |
| `criterion` | 性能基准测试框架（可选） |

### 10.3 外部服务依赖

| 服务 | 必需性 | 说明 |
|---|---|---|
| **Qdrant** (Docker) | 必需（可降级） | 向量存储与搜索，不可用时降级为纯全文搜索 |
| **OpenAI API** | 必需（可替换） | Embedding 生成，可切换为 Ollama/本地 ONNX |
| **文件系统** | 必需 | Vault 文件存储 |

---

> **关联文档**：
> - [需求设计](../requirement/03-memory-engine.md) — 功能需求、用户故事、验收标准
> - [顶层设计](../top_design.md) — 系统架构、技术栈、数据模型
