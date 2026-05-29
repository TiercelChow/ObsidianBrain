# 时间线模块 (Timeline) — 开发设计文档

> **文档编号**: DEV-04 | **版本**: v1.0 | **状态**: 设计中 | **最后更新**: 2026-05-29
>
> **上游依赖**: [顶层设计文档](../top_design.md) §5.2 时间线 | [需求设计文档](../requirement/04-timeline.md)

---

## 1. 技术架构详细设计

### 1.1 架构概览

时间线模块位于 `core` 层，对外通过 Tool API 暴露 `get_timeline` 工具，对内通过事件总线 (EventBus) 接收其他模块的事件推送。模块内部采用 **收集器 → 存储 → 查询 → 格式化** 的管道式架构。

```
┌────────────────────────────────────────────────────────────────────┐
│                       Timeline Service                              │
│                                                                    │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    事件收集层 (Collectors)                     │  │
│  │                                                              │  │
│  │  ┌──────────────┐ ┌──────────────┐ ┌─────────────────────┐  │  │
│  │  │ Frontmatter  │ │  Filename    │ │  Content Tag        │  │  │
│  │  │ Date         │ │  Date        │ │  Extractor          │  │  │
│  │  │ Extractor    │ │  Extractor   │ │                     │  │  │
│  │  └──────┬───────┘ └──────┬───────┘ └──────────┬──────────┘  │  │
│  │         │                │                    │             │  │
│  │  ┌──────┴───────┐ ┌─────┴────────┐ ┌────────┴──────────┐  │  │
│  │  │ FileWatcher  │ │ Git Commit   │ │ Module Callback   │  │  │
│  │  │ Collector    │ │ Collector    │ │ (Radar/Memory)    │  │  │
│  │  └──────┬───────┘ └──────┬───────┘ └────────┬──────────┘  │  │
│  └─────────┼────────────────┼──────────────────┼─────────────┘  │
│            │                │                  │                 │
│            ▼                ▼                  ▼                 │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              事件规范化层 (Event Normalizer)                   │  │
│  │         统一格式、去重、校验、批量写入                            │  │
│  └──────────────────────────┬───────────────────────────────────┘  │
│                             │                                      │
│                             ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              事件存储层 (SQLite Store)                         │  │
│  │                                                              │  │
│  │  timeline_events 表  │  timeline_monthly_summaries 表         │  │
│  │  索引：date + type   │  索引：year_month                      │  │  └──────────────────────────┬───────────────────────────────────┘  │
│                             │                                      │
│              ┌──────────────┼──────────────┐                       │
│              ▼              ▼              ▼                       │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────────┐           │
│  │ 查询引擎      │ │ 统计聚合器    │ │ 摘要生成器        │           │
│  │ (Query       │ │ (Statistics  │ │ (Summary         │           │
│  │  Engine)     │ │  Aggregator) │ │  Generator)      │           │
│  └──────┬───────┘ └──────┬───────┘ └────────┬─────────┘           │
│         │                │                  │                      │
│         ▼                ▼                  ▼                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              输出格式化层 (Response Formatter)                 │  │
│  │         按日分组、JSON 序列化、LLM Tool 响应封装                │  │
│  └──────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────┘
         │                                              ▲
         │ 写入事件                                      │ 查询请求
         ▼                                              │
┌─────────────────┐  ┌─────────────┐  ┌──────────────────────────┐
│    EventBus     │  │   SQLite    │  │     Tool API Handler     │
│  (事件总线)      │  │  (brain.db) │  │  get_timeline / get_stats│
└─────────────────┘  └─────────────┘  └──────────────────────────┘
```

### 1.2 模块间依赖关系

```
Timeline Service 依赖：
├── infra::sqlite_store    — SQLite 读写
├── infra::file_watcher    — 文件监控事件源
├── infra::llm_client      — LLM 摘要生成（可选）
├── core::code_repo        — 获取已注册仓库列表及 Git 信息
├── core::memory           — 接收 MemoryCreated 回调
└── core::radar            — 接收 RadarSaved 回调
```

---

## 2. 目录与文件组织

### 2.1 文件布局

```
src/
├── core/
│   ├── timeline/
│   │   ├── mod.rs                  # 模块入口：TimelineService 定义与初始化
│   │   ├── collector/
│   │   │   ├── mod.rs              # EventCollector trait 定义
│   │   │   ├── frontmatter.rs      # Frontmatter 日期提取器
│   │   │   ├── filename.rs         # 文件名日期提取器
│   │   │   ├── content_tag.rs      # 内容标签日期提取器
│   │   │   ├── file_watcher.rs     # 文件监控事件收集器
│   │   │   └── git_commit.rs       # Git commit 事件收集器
│   │   ├── store.rs                # 事件存储层（SQLite CRUD）
│   │   ├── query.rs                # 查询引擎（日期范围、过滤、分组）
│   │   ├── statistics.rs           # 统计聚合器
│   │   ├── summary.rs              # LLM 摘要生成器
│   │   ├── models.rs               # 数据结构定义
│   │   └── formatter.rs            # 响应格式化
│   └── ...
├── api/
│   └── handlers/
│       └── timeline.rs             # get_timeline 工具的 HTTP handler
└── models/
    └── timeline.rs                 # 共享的 TimelineEvent 模型（公共 API）
```

### 2.2 文件职责划分

| 文件 | 职责 | 估算行数 |
|---|---|---|
| `mod.rs` | `TimelineService` struct 定义、初始化、生命周期管理 | ~150 |
| `collector/mod.rs` | `EventCollector` trait、收集器注册表 | ~80 |
| `collector/frontmatter.rs` | Frontmatter 日期解析逻辑 | ~120 |
| `collector/filename.rs` | 文件名日期正则匹配 | ~100 |
| `collector/content_tag.rs` | 正文标签扫描 | ~80 |
| `collector/file_watcher.rs` | notify 事件 → TimelineEvent 转换 | ~100 |
| `collector/git_commit.rs` | git2 提交历史提取 | ~120 |
| `store.rs` | SQLite 事件读写、批量插入、过期清理 | ~200 |
| `query.rs` | 查询构建、日期范围过滤、分组 | ~150 |
| `statistics.rs` | 统计计算、趋势分析 | ~120 |
| `summary.rs` | LLM prompt 构建与摘要解析 | ~100 |
| `models.rs` | 所有内部数据结构 | ~200 |
| `formatter.rs` | JSON 响应组装 | ~80 |

---

## 3. 各子模块详细设计

### 3.1 事件收集器 (Event Collector)

#### 3.1.1 EventCollector Trait

所有收集器实现统一的 trait 接口：

```rust
use async_trait::async_trait;
use crate::core::timeline::models::TimelineEvent;

/// 事件收集器 trait —— 所有收集器实现此接口
#[async_trait]
pub trait EventCollector: Send + Sync {
    /// 收集器名称，用于日志和调试
    fn name(&self) -> &str;

    /// 从给定来源收集事件
    /// - `source`: 收集来源标识（如文件路径、仓库路径）
    /// - 返回收集到的事件列表
    async fn collect(&self, source: &CollectorSource) -> Result<Vec<TimelineEvent>>;

    /// 收集器是否支持增量收集
    /// 若支持，仅返回上次收集后的新事件
    fn supports_incremental(&self) -> bool;
}

/// 收集来源
pub enum CollectorSource {
    /// 单个笔记文件
    NoteFile {
        path: std::path::PathBuf,
        content: Option<String>,       // 可选：已读取的文件内容
        frontmatter: Option<serde_json::Value>, // 可选：已解析的 frontmatter
    },
    /// 已注册的代码仓库
    GitRepo {
        name: String,
        path: std::path::PathBuf,
        since: Option<chrono::DateTime<chrono::Utc>>, // 增量收集起始时间
    },
    /// 文件监控事件
    WatchEvent {
        event: notify::Event,
        vault_path: std::path::PathBuf,
    },
    /// 模块回调（雷达/记忆）
    ModuleCallback {
        module: String,
        payload: serde_json::Value,
    },
}
```

#### 3.1.2 Frontmatter 日期提取器 (`collector/frontmatter.rs`)

**功能**：解析笔记 YAML frontmatter 中的日期字段，生成 `NoteCreated` 或 `NoteModified` 事件。

```rust
use chrono::{NaiveDate, NaiveDateTime};
use gray_matter::Matter;
use serde_json::Value;

pub struct FrontmatterDateExtractor {
    /// 识别为"创建时间"的字段名列表
    created_fields: Vec<String>,
    /// 识别为"修改时间"的字段名列表
    modified_fields: Vec<String>,
    /// 支持的日期格式模式
    date_formats: Vec<String>,
}

impl FrontmatterDateExtractor {
    pub fn new() -> Self {
        Self {
            created_fields: vec![
                "date".into(), "created".into(), "created_at".into(),
            ],
            modified_fields: vec![
                "modified".into(), "updated".into(), "updated_at".into(), "lastmod".into(),
            ],
            date_formats: vec![
                "%Y-%m-%d".into(),
                "%Y-%m-%dT%H:%M:%S%#z".into(),  // ISO 8601 with timezone
                "%Y-%m-%dT%H:%M:%SZ".into(),
                "%Y-%m-%d %H:%M:%S".into(),
                "%Y/%m/%d".into(),
                "%Y/%m/%d %H:%M:%S".into(),
                "%b %d, %Y".into(),              // "May 28, 2026"
                "%d-%m-%Y".into(),               // "28-05-2026"
            ],
        }
    }

    /// 从 frontmatter JSON 中提取所有日期信息
    /// 返回 (字段名, 解析出的日期, 事件类型建议)
    pub fn extract_dates(
        &self,
        frontmatter: &Value,
    ) -> Vec<(String, NaiveDate, EventType)> {
        let mut results = Vec::new();

        if let Value::Object(map) = frontmatter {
            for (key, value) in map {
                let event_type = if self.created_fields.contains(key) {
                    Some(EventType::NoteCreated)
                } else if self.modified_fields.contains(key) {
                    Some(EventType::NoteModified)
                } else {
                    None
                };

                if let Some(event_type) = event_type {
                    if let Some(date) = self.parse_date_value(value) {
                        results.push((key.clone(), date, event_type));
                    }
                }
            }
        }

        results
    }

    /// 尝试用多种格式解析日期值
    fn parse_date_value(&self, value: &Value) -> Option<NaiveDate> {
        let date_str = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => {
                // 处理 Unix 时间戳（秒）
                if let Some(ts) = n.as_i64() {
                    return chrono::DateTime::from_timestamp(ts, 0)
                        .map(|dt| dt.date_naive());
                }
                return None;
            }
            _ => return None,
        };

        for format in &self.date_formats {
            // 先尝试解析为完整日期时间，再降级为仅日期
            if let Ok(dt) = NaiveDateTime::parse_from_str(&date_str, format) {
                return Some(dt.date());
            }
            if let Ok(d) = NaiveDate::parse_from_str(&date_str, format) {
                return Some(d);
            }
        }
        None
    }
}
```

**日期字段识别优先级**：
1. `date` → `NoteCreated`（最常见的 Obsidian 日记格式）
2. `created` / `created_at` → `NoteCreated`
3. `modified` / `updated` / `updated_at` / `lastmod` → `NoteModified`

#### 3.1.3 文件名日期提取器 (`collector/filename.rs`)

**功能**：从文件名中正则匹配日期模式。

```rust
use chrono::NaiveDate;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

/// 文件名日期提取器
pub struct FilenameDateExtractor {
    patterns: Vec<FilenameDatePattern>,
}

/// 文件名日期匹配模式
struct FilenameDatePattern {
    /// 正则表达式
    regex: Regex,
    /// 日期格式字符串（用于 chrono 解析）
    date_format: String,
    /// 描述
    description: String,
}

/// 预编译的正则模式列表
static FILENAME_PATTERNS: LazyLock<Vec<FilenameDatePattern>> = LazyLock::new(|| {
    vec![
        // 模式1: YYYY-MM-DD (最常见)
        // 匹配: "2026-05-28-meeting.md", "2026-05-28.md"
        FilenameDatePattern {
            regex: Regex::new(r"(\d{4}-\d{2}-\d{2})").unwrap(),
            date_format: "%Y-%m-%d".into(),
            description: "YYYY-MM-DD 格式".into(),
        },
        // 模式2: YYYYMMDD (紧凑格式)
        // 匹配: "20260528_daily.md", "meeting-notes-20260528.md"
        FilenameDatePattern {
            regex: Regex::new(r"(\d{8})").unwrap(),
            date_format: "%Y%m%d".into(),
            description: "YYYYMMDD 紧凑格式".into(),
        },
        // 模式3: YYYY_MM_DD (下划线分隔)
        // 匹配: "2026_05_28_notes.md"
        FilenameDatePattern {
            regex: Regex::new(r"(\d{4}_\d{2}_\d{2})").unwrap(),
            date_format: "%Y_%m_%d".into(),
            description: "YYYY_MM_DD 下划线格式".into(),
        },
        // 模式4: YYYY.MM.DD (点号分隔)
        // 匹配: "2026.05.28.md"
        FilenameDatePattern {
            regex: Regex::new(r"(\d{4}\.\d{2}\.\d{2})").unwrap(),
            date_format: "%Y.%m.%d".into(),
            description: "YYYY.MM.DD 点号格式".into(),
        },
    ]
});

impl FilenameDateExtractor {
    pub fn new() -> Self {
        Self {
            patterns: FILENAME_PATTERNS.clone(), // LazyLock 的 Vec 可 clone
        }
    }

    /// 从文件路径中提取日期
    /// 同时检查文件名和路径中的目录层级日期
    pub fn extract_date(&self, path: &Path) -> Option<NaiveDate> {
        // 优先从文件名提取
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            if let Some(date) = self.try_extract(stem) {
                return Some(date);
            }
        }

        // 降级：从路径的目录层级中提取
        // 匹配: "2026/05/28/note.md" 这种日记式目录结构
        let path_str = path.to_string_lossy();
        let dir_pattern = Regex::new(r"(\d{4})/(\d{2})/(\d{2})/").unwrap();
        if let Some(caps) = dir_pattern.captures(&path_str) {
            let date_str = format!("{}-{}-{}",
                &caps[1], &caps[2], &caps[3]);
            if let Ok(date) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                // 验证日期合理性（1970-2099）
                if date.year() >= 1970 && date.year() <= 2099 {
                    return Some(date);
                }
            }
        }

        None
    }

    /// 尝试用各模式匹配字符串中的日期
    fn try_extract(&self, text: &str) -> Option<NaiveDate> {
        for pattern in &*FILENAME_PATTERNS {
            if let Some(caps) = pattern.regex.captures(text) {
                if let Some(date_match) = caps.get(1) {
                    let date_str = date_match.as_str();
                    if let Ok(date) = NaiveDate::parse_from_str(date_str, &pattern.date_format) {
                        // 验证日期合理性
                        if date.year() >= 1970 && date.year() <= 2099 {
                            return Some(date);
                        }
                    }
                }
            }
        }
        None
    }
}
```

**正则模式汇总表**：

| 模式 | 正则 | 匹配示例 | 日期格式 |
|---|---|---|---|
| ISO 日期 | `(\d{4}-\d{2}-\d{2})` | `2026-05-28-meeting.md` | `%Y-%m-%d` |
| 紧凑日期 | `(\d{8})` | `20260528_daily.md` | `%Y%m%d` |
| 下划线日期 | `(\d{4}_\d{2}_\d{2})` | `2026_05_28_notes.md` | `%Y_%m_%d` |
| 点号日期 | `(\d{4}\.\d{2}\.\d{2})` | `2026.05.28.md` | `%Y.%m.%d` |
| 目录层级日期 | `(\d{4})/(\d{2})/(\d{2})/` | `2026/05/28/note.md` | 组合解析 |

**日期合理性校验**：提取出的日期年份必须在 1970-2099 范围内，避免将纯数字文件名（如 `001-intro.md`）误判为日期。

#### 3.1.4 内容标签提取器 (`collector/content_tag.rs`)

**功能**：扫描笔记正文中的 `#date/YYYY-MM-DD` 格式标签。

```rust
use chrono::NaiveDate;
use regex::Regex;
use std::sync::LazyLock;

/// 内容标签日期提取器
pub struct ContentTagExtractor;

/// 匹配 #date/YYYY-MM-DD 和 #date/YYYY-MM-DD/keyword
static DATE_TAG_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#date/(\d{4}-\d{2}-\d{2})(?:/([\w\-]+))?").unwrap()
});

/// 标签提取结果
pub struct DateTagMatch {
    /// 提取的日期
    pub date: NaiveDate,
    /// 可选的关键词
    pub keyword: Option<String>,
    /// 标签在文本中的字节偏移位置
    pub byte_offset: usize,
}

impl ContentTagExtractor {
    pub fn new() -> Self {
        Self
    }

    /// 从 Markdown 正文中提取所有 #date/ 标签
    pub fn extract_tags(&self, content: &str) -> Vec<DateTagMatch> {
        let mut results = Vec::new();

        for caps in DATE_TAG_PATTERN.captures_iter(content) {
            let full_match = caps.get(0).unwrap();
            let date_str = &caps[1];

            if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                // 验证日期合理性
                if date.year() >= 1970 && date.year() <= 2099 {
                    let keyword = caps.get(2).map(|m| m.as_str().to_string());
                    results.push(DateTagMatch {
                        date,
                        keyword,
                        byte_offset: full_match.start(),
                    });
                }
            }
        }

        results
    }

    /// 从内容中提取标签并生成 TimelineEvent
    pub fn extract_events(
        &self,
        content: &str,
        note_path: &std::path::Path,
        note_title: &str,
    ) -> Vec<TimelineEvent> {
        self.extract_tags(content)
            .into_iter()
            .map(|tag| {
                let title = if let Some(ref kw) = tag.keyword {
                    format!("[{}] {}", note_title, kw)
                } else {
                    format!("[{}] 日期标注", note_title)
                };

                let mut tags = vec!["date-tag".to_string()];
                if let Some(ref kw) = tag.keyword {
                    tags.push(kw.clone());
                }

                TimelineEvent {
                    id: uuid::Uuid::new_v4(),
                    date: tag.date,
                    timestamp: None,
                    event_type: EventType::NoteModified,
                    title,
                    summary: format!("笔记 '{}' 包含日期标注 #date/{}", note_title, tag.date),
                    tags,
                    related_paths: vec![note_path.to_path_buf()],
                    source: "content_tag".to_string(),
                    metadata: serde_json::json!({
                        "keyword": tag.keyword,
                        "byte_offset": tag.byte_offset,
                    }),
                }
            })
            .collect()
    }
}
```

**正则表达式详解**：

```
#date/(\d{4}-\d{2}-\d{2})(?:/([\w\-]+))?
```

| 部分 | 含义 | 示例 |
|---|---|---|
| `#date/` | 字面匹配标签前缀 | `#date/` |
| `(\d{4}-\d{2}-\d{2})` | 捕获组1：YYYY-MM-DD 格式日期 | `2026-05-28` |
| `(?:/([\w\-]+))?` | 可选的非捕获组 + 捕获组2：关键词 | `/meeting` |

**匹配示例**：
- `#date/2026-05-28` → date=2026-05-28, keyword=None
- `#date/2026-05-28/meeting` → date=2026-05-28, keyword="meeting"
- `#date/2026-05-28/rust-async` → date=2026-05-28, keyword="rust-async"

#### 3.1.5 文件监控事件收集器 (`collector/file_watcher.rs`)

**功能**：将 `notify` crate 的文件系统事件转化为 `TimelineEvent`。

```rust
use chrono::Utc;
use notify::Event;
use std::path::PathBuf;

/// 文件监控事件收集器
pub struct FileWatcherCollector {
    /// vault 根路径，用于计算相对路径
    vault_path: PathBuf,
    /// 排除的文件模式
    exclude_patterns: Vec<glob::Pattern>,
}

impl FileWatcherCollector {
    pub fn new(vault_path: PathBuf, exclude_patterns: Vec<String>) -> Self {
        let patterns = exclude_patterns
            .into_iter()
            .filter_map(|p| glob::Pattern::new(&p).ok())
            .collect();

        Self {
            vault_path,
            exclude_patterns: patterns,
        }
    }

    /// 将 notify::Event 转化为 TimelineEvent 列表
    /// 一次文件系统事件可能产生 0 或 1 条 TimelineEvent
    pub fn process_event(&self, event: &Event) -> Option<TimelineEvent> {
        // 仅处理 .md 文件
        let paths: Vec<&PathBuf> = event.paths.iter()
            .filter(|p| {
                p.extension().map_or(false, |ext| ext == "md")
                    && !self.is_excluded(p)
            })
            .collect();

        if paths.is_empty() {
            return None;
        }

        let now = Utc::now();
        let path = paths[0]; // 取第一个有效路径

        let (event_type, title_prefix) = match event.kind {
            notify::EventKind::Create(_) => {
                (EventType::NoteCreated, "新建笔记")
            }
            notify::EventKind::Modify(_) => {
                (EventType::NoteModified, "修改笔记")
            }
            notify::EventKind::Remove(_) => {
                (EventType::NoteModified, "删除笔记")
            }
            _ => return None,
        };

        let relative_path = path.strip_prefix(&self.vault_path)
            .unwrap_or(path);

        let file_stem = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");

        Some(TimelineEvent {
            id: uuid::Uuid::new_v4(),
            date: now.date_naive(),
            timestamp: Some(now),
            event_type,
            title: format!("{}: {}", title_prefix, file_stem),
            summary: format!("文件 '{}' 发生 {:?}", relative_path.display(), event.kind),
            tags: Vec::new(), // 标签由后续的 frontmatter 解析补充
            related_paths: vec![relative_path.to_path_buf()],
            source: "file_watcher".to_string(),
            metadata: serde_json::json!({
                "event_kind": format!("{:?}", event.kind),
                "absolute_path": path.to_string_lossy(),
            }),
        })
    }

    /// 检查路径是否匹配排除模式
    fn is_excluded(&self, path: &std::path::Path) -> bool {
        let relative = path.strip_prefix(&self.vault_path)
            .unwrap_or(path);
        let rel_str = relative.to_string_lossy();
        self.exclude_patterns.iter().any(|p| p.matches(&rel_str))
    }
}
```

**事件去重策略**：文件监控可能产生大量重复事件（如编辑器频繁自动保存），收集器内置防抖机制：

- 同一路径在 300ms 内的重复事件合并为一条
- 使用 `tokio::time::Instant` 追踪上次事件时间
- 合并时保留最新的事件类型和时间戳

#### 3.1.6 Git Commit 事件收集器 (`collector/git_commit.rs`)

**功能**：从已注册的代码仓库中提取提交历史。

```rust
use chrono::{DateTime, Utc};
use git2::{Repository, Sort};
use std::path::Path;

/// Git commit 事件收集器
pub struct GitCommitCollector {
    /// 每次最多提取的 commit 数量
    max_commits: usize,
}

/// 单个 commit 的摘要信息
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

impl GitCommitCollector {
    pub fn new(max_commits: usize) -> Self {
        Self {
            max_commits: max_commits.min(500), // 硬上限 500
        }
    }

    /// 从指定仓库提取最近的 commit 历史
    /// `since` 参数用于增量收集：仅返回该时间之后的提交
    pub fn collect_commits(
        &self,
        repo_path: &Path,
        repo_name: &str,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<TimelineEvent>> {
        let repo = Repository::open(repo_path)?;
        let mut revwalk = repo.revwalk()?;
        revwalk.set_sorting(Sort::TIME)?;
        revwalk.push_head()?;

        let mut events = Vec::new();
        let mut count = 0;

        for oid_result in revwalk {
            if count >= self.max_commits {
                break;
            }

            let oid = oid_result?;
            let commit = repo.find_commit(oid)?;

            let commit_time = DateTime::from_timestamp(commit.time().seconds(), 0)
                .unwrap_or_default();

            // 增量过滤：跳过 since 之前的提交
            if let Some(since_time) = since {
                if commit_time < since_time {
                    break; // revwalk 按时间排序，后续更早，直接跳出
                }
            }

            let message = commit.message()
                .unwrap_or("")
                .lines()
                .next() // 仅取首行
                .unwrap_or("")
                .to_string();

            let hash = oid.to_string();
            let short_hash = hash[..7.min(hash.len())].to_string();
            let author = commit.author().name()
                .unwrap_or("unknown")
                .to_string();

            // 从 commit message 提取标签（#xxx 格式）
            let tags = Self::extract_tags_from_message(&message);

            events.push(TimelineEvent {
                id: uuid::Uuid::new_v4(),
                date: commit_time.date_naive(),
                timestamp: Some(commit_time),
                event_type: EventType::RepoCommit,
                title: format!("[{}] {}", repo_name, message),
                summary: format!(
                    "commit {} by {} — {}",
                    short_hash, author, message
                ),
                tags,
                related_paths: Vec::new(),
                source: "git".to_string(),
                metadata: serde_json::json!({
                    "repo_name": repo_name,
                    "repo_path": repo_path.to_string_lossy(),
                    "commit_hash": hash,
                    "short_hash": short_hash,
                    "author": author,
                }),
            });

            count += 1;
        }

        Ok(events)
    }

    /// 从 commit message 中提取标签
    /// 识别格式：feat:, fix:, docs: 等 conventional commits 前缀
    fn extract_tags_from_message(message: &str) -> Vec<String> {
        let mut tags = Vec::new();

        // 识别 conventional commit 类型
        let prefixes = ["feat", "fix", "docs", "refactor", "test", "chore", "perf"];
        for prefix in &prefixes {
            if message.starts_with(&format!("{}:", prefix))
                || message.starts_with(&format!("{}(", prefix))
            {
                tags.push(prefix.to_string());
                break;
            }
        }

        // 提取 scope: feat(auth): ... → tags 增加 "auth"
        let scope_re = regex::Regex::new(r"^\w+\(([\w\-]+)\):").unwrap();
        if let Some(caps) = scope_re.captures(message) {
            if let Some(scope) = caps.get(1) {
                tags.push(scope.as_str().to_string());
            }
        }

        tags
    }
}
```

### 3.2 事件存储 (`store.rs`)

#### 3.2.1 SQLite Schema

```sql
-- ============================================================
-- 时间线事件主表
-- ============================================================
CREATE TABLE IF NOT EXISTS timeline_events (
    id              TEXT PRIMARY KEY,         -- UUID v4
    date            TEXT NOT NULL,            -- 事件日期 "YYYY-MM-DD"
    timestamp       TEXT,                     -- 精确时间戳 ISO 8601，可为 NULL
    event_type      TEXT NOT NULL,            -- "note_created" | "note_modified" | "repo_commit" | "radar_saved" | "memory_created"
    title           TEXT NOT NULL,            -- 事件标题（≤100 字符）
    summary         TEXT,                     -- 事件摘要（≤500 字符）
    tags            TEXT NOT NULL DEFAULT '[]',  -- JSON 数组 ["rust", "async"]
    related_paths   TEXT NOT NULL DEFAULT '[]',  -- JSON 数组 ["path/to/note.md"]
    source          TEXT NOT NULL,            -- "frontmatter" | "filename" | "content_tag" | "file_watcher" | "git" | "radar" | "memory"
    metadata        TEXT NOT NULL DEFAULT '{}',  -- JSON 扩展元数据
    title_hash      TEXT NOT NULL,            -- 标题 SHA-256 前 16 位 hex，用于去重
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 主查询索引：按日期范围查询（最常用场景）
CREATE INDEX IF NOT EXISTS idx_timeline_events_date
    ON timeline_events(date);

-- 复合索引：日期范围 + 事件类型过滤
CREATE INDEX IF NOT EXISTS idx_timeline_events_date_type
    ON timeline_events(date, event_type);

-- 来源索引：按来源清理或查询
CREATE INDEX IF NOT EXISTS idx_timeline_events_source
    ON timeline_events(source);

-- 去重唯一约束：防止重复采集同一事件（基于日期 + 类型 + 标题哈希）
CREATE UNIQUE INDEX IF NOT EXISTS idx_timeline_events_dedup
    ON timeline_events(date, event_type, title_hash);

-- ============================================================
-- 月度聚合摘要表（过期事件压缩后存储于此）
-- ============================================================
CREATE TABLE IF NOT EXISTS timeline_monthly_summaries (
    id              TEXT PRIMARY KEY,         -- UUID v4
    year            INTEGER NOT NULL,        -- 年份
    month           INTEGER NOT NULL,        -- 月份 1-12
    event_type      TEXT NOT NULL,            -- 原始事件类型
    count           INTEGER NOT NULL,        -- 事件总数
    summary         TEXT,                     -- 自然语言摘要
    tags_summary    TEXT NOT NULL DEFAULT '{}', -- 标签频率 JSON {"rust": 15, "ai": 8}
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(year, month, event_type)
);

-- 月度摘要查询索引
CREATE INDEX IF NOT EXISTS idx_monthly_summaries_ym
    ON timeline_monthly_summaries(year, month);

-- ============================================================
-- 收集器状态表（记录各收集器的增量收集进度）
-- ============================================================
CREATE TABLE IF NOT EXISTS timeline_collector_state (
    collector_name  TEXT PRIMARY KEY,         -- 收集器名称
    source_key      TEXT NOT NULL,            -- 来源标识（如仓库路径）
    last_collected  TEXT NOT NULL,            -- 上次收集的时间戳
    last_event_id   TEXT,                     -- 上次收集的最后事件 ID
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
```

#### 3.2.2 事件写入操作

```rust
use rusqlite::{params, Connection};
use uuid::Uuid;

/// 事件存储管理器
pub struct TimelineStore {
    db_path: std::path::PathBuf,
}

impl TimelineStore {
    pub fn new(db_path: std::path::PathBuf) -> Self {
        Self { db_path }
    }

    /// 初始化数据库表
    pub fn initialize(&self, conn: &Connection) -> Result<()> {
        conn.execute_batch(include_str!("../../migrations/004_timeline.sql"))?;
        Ok(())
    }

    /// 计算标题哈希（SHA-256 前 8 字节 → 16 位 hex，用于去重）
    fn title_hash(title: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(title.as_bytes());
        let result = hasher.finalize();
        hex::encode(&result[..8])
    }

    /// 插入单条事件（基于 date + event_type + title_hash 去重）
    ///
    /// 去重策略：
    /// - 唯一约束 idx_timeline_events_dedup 确保 (date, event_type, title_hash) 不重复
    /// - INSERT OR IGNORE 在冲突时静默跳过，不报错
    /// - 每条事件生成新 UUID，但相同标题+日期+类型视为同一事件
    pub fn insert_event(&self, conn: &Connection, event: &TimelineEvent) -> Result<()> {
        let hash = Self::title_hash(&event.title);
        conn.execute(
            "INSERT OR IGNORE INTO timeline_events
             (id, date, timestamp, event_type, title, summary, tags, related_paths, source, metadata, title_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                event.id.to_string(),
                event.date.format("%Y-%m-%d").to_string(),
                event.timestamp.map(|t| t.to_rfc3339()),
                event.event_type.as_str(),
                event.title,
                event.summary,
                serde_json::to_string(&event.tags)?,
                serde_json::to_string(&event.related_paths)?,
                event.source,
                event.metadata.to_string(),
                hash,
            ],
        )?;
        Ok(())
    }

    /// 批量插入事件（使用事务提升性能）
    ///
    /// 所有插入共享同一事务，任一失败不影响其他事件写入（仅记录告警）
    pub fn insert_events_batch(
        &self,
        conn: &Connection,
        events: &[TimelineEvent],
    ) -> Result<usize> {
        let tx = conn.unchecked_transaction()?;
        let mut count = 0;

        for event in events {
            match self.insert_event(&tx, event) {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::warn!(
                        "Failed to insert timeline event {}: {}",
                        event.id, e
                    );
                }
            }
        }

        tx.commit()?;
        Ok(count)
    }

    /// 删除过期事件（按月聚合后调用）
    pub fn delete_events_before(&self, conn: &Connection, before_date: NaiveDate) -> Result<usize> {
        let count = conn.execute(
            "DELETE FROM timeline_events WHERE date < ?1",
            params![before_date.format("%Y-%m-%d").to_string()],
        )?;
        Ok(count)
    }

    /// 插入月度聚合摘要
    pub fn insert_monthly_summary(
        &self,
        conn: &Connection,
        summary: &MonthlySummary,
    ) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO timeline_monthly_summaries
             (id, year, month, event_type, count, summary, tags_summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                Uuid::new_v4().to_string(),
                summary.year,
                summary.month,
                summary.event_type.as_str(),
                summary.count,
                summary.summary,
                serde_json::to_string(&summary.tags_summary)?,
            ],
        )?;
        Ok(())
    }
}
```

#### 3.2.3 索引策略说明

| 索引 | 覆盖场景 | 设计理由 |
|---|---|---|
| `idx_timeline_events_date` | `WHERE date BETWEEN ? AND ?` | 最常用查询模式，覆盖所有日期范围查询 |
| `idx_timeline_events_date_type` | `WHERE date BETWEEN ? AND ? AND event_type = ?` | 事件类型过滤 + 日期范围的复合查询 |
| `idx_timeline_events_source` | `WHERE source = ?` | 按来源清理数据或调试 |
| `idx_timeline_events_dedup` | `INSERT OR IGNORE` 去重 | 唯一约束：(date, event_type, title_hash)，防止重复采集同一事件 |
| `idx_monthly_summaries_ym` | `WHERE year = ? AND month = ?` | 月度摘要查询 |

### 3.3 事件查询引擎 (`query.rs`)

```rust
use chrono::NaiveDate;
use rusqlite::Connection;
use std::collections::HashMap;

/// 查询参数
pub struct TimelineQuery {
    /// 起始日期（含）
    pub start_date: NaiveDate,
    /// 结束日期（含）
    pub end_date: NaiveDate,
    /// 事件类型过滤（None = 全部）
    pub event_types: Option<Vec<EventType>>,
    /// 标签过滤（匹配任意一个即可）
    pub tags: Option<Vec<String>>,
    /// 最大返回数量
    pub limit: usize,
}

/// 查询引擎
pub struct QueryEngine;

impl QueryEngine {
    /// 执行时间线查询，返回按日分组的结果
    pub fn query(
        &self,
        conn: &Connection,
        query: &TimelineQuery,
    ) -> Result<Vec<DailyEvents>> {
        // 构建动态 SQL
        let (where_clause, params) = self.build_where_clause(query);
        let sql = format!(
            "SELECT id, date, timestamp, event_type, title, summary,
                    tags, related_paths, source, metadata
             FROM timeline_events
             {}
             ORDER BY date DESC, timestamp DESC
             LIMIT ?{}",
            where_clause,
            query.limit
        );

        let mut stmt = conn.prepare(&sql)?;
        let events = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            self.row_to_event(row)
        })?;

        let mut all_events: Vec<TimelineEvent> = Vec::new();
        for event in events {
            all_events.push(event?);
        }

        // 按日分组
        Ok(self.group_by_date(all_events))
    }

    /// 构建 WHERE 子句
    fn build_where_clause(
        &self,
        query: &TimelineQuery,
    ) -> (String, Vec<String>) {
        let mut conditions = vec![
            "date >= ?1".to_string(),
            "date <= ?2".to_string(),
        ];
        let mut params: Vec<String> = vec![
            query.start_date.format("%Y-%m-%d").to_string(),
            query.end_date.format("%Y-%m-%d").to_string(),
        ];
        let mut param_idx = 3;

        // 事件类型过滤
        if let Some(ref types) = query.event_types {
            let placeholders: Vec<String> = types.iter().map(|_| {
                let p = format!("?{}", param_idx);
                param_idx += 1;
                p
            }).collect();
            conditions.push(format!("event_type IN ({})", placeholders.join(",")));
            for t in types {
                params.push(t.as_str().to_string());
            }
        }

        // 标签过滤（JSON 数组包含匹配）
        // SQLite 使用 json_each 或 LIKE 匹配
        if let Some(ref tags) = query.tags {
            let tag_conditions: Vec<String> = tags.iter().map(|tag| {
                let p = format!("?{}", param_idx);
                param_idx += 1;
                params.push(format!("%{}%", tag));
                format!("tags LIKE {}", p)
            }).collect();
            conditions.push(format!("({})", tag_conditions.join(" OR ")));
        }

        let clause = format!("WHERE {}", conditions.join(" AND "));
        (clause, params)
    }

    /// 将查询结果按日期分组
    fn group_by_date(&self, events: Vec<TimelineEvent>) -> Vec<DailyEvents> {
        let mut grouped: HashMap<NaiveDate, Vec<TimelineEvent>> = HashMap::new();

        for event in events {
            grouped.entry(event.date).or_default().push(event);
        }

        // 按日期倒序排列
        let mut daily: Vec<DailyEvents> = grouped.into_iter()
            .map(|(date, events)| DailyEvents {
                date,
                event_count: events.len(),
                events,
            })
            .collect();

        daily.sort_by(|a, b| b.date.cmp(&a.date));
        daily
    }

    /// 将 SQLite 行映射为 TimelineEvent
    fn row_to_event(&self, row: &rusqlite::Row) -> rusqlite::Result<TimelineEvent> {
        let id_str: String = row.get(0)?;
        let date_str: String = row.get(1)?;
        let timestamp_str: Option<String> = row.get(2)?;
        let event_type_str: String = row.get(3)?;
        let title: String = row.get(4)?;
        let summary: Option<String> = row.get(5)?;
        let tags_json: String = row.get(6)?;
        let paths_json: String = row.get(7)?;
        let source: String = row.get(8)?;
        let metadata_json: String = row.get(9)?;

        Ok(TimelineEvent {
            id: uuid::Uuid::parse_str(&id_str)
                .unwrap_or_else(|_| uuid::Uuid::new_v4()),
            date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .unwrap_or_default(),
            timestamp: timestamp_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&chrono::Utc))
            }),
            event_type: EventType::from_str(&event_type_str),
            title,
            summary: summary.unwrap_or_default(),
            tags: serde_json::from_str(&tags_json).unwrap_or_default(),
            related_paths: serde_json::from_str::<Vec<String>>(&paths_json)
                .unwrap_or_default()
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect(),
            source,
            metadata: serde_json::from_str(&metadata_json)
                .unwrap_or(serde_json::Value::Null),
        })
    }

    /// "去年今日" 快捷查询
    pub fn query_on_this_day(
        &self,
        conn: &Connection,
        years_ago: i32,
    ) -> Result<Vec<TimelineEvent>> {
        let today = chrono::Local::now().date_naive();
        let target_year = today.year() - years_ago;
        let target_date = today.with_year(target_year);

        if let Some(date) = target_date {
            let query = TimelineQuery {
                start_date: date,
                end_date: date,
                event_types: None,
                tags: None,
                limit: 50,
            };
            let daily = self.query(conn, &query)?;
            Ok(daily.into_iter()
                .flat_map(|d| d.events)
                .collect())
        } else {
            // 闰年 2月29日 在非闰年不存在
            Ok(Vec::new())
        }
    }
}
```

### 3.4 统计聚合器 (`statistics.rs`)

```rust
use chrono::NaiveDate;
use std::collections::HashMap;

/// 统计聚合器
pub struct StatisticsAggregator;

/// 统计结果
#[derive(Debug, Serialize, Deserialize)]
pub struct TimelineStatistics {
    /// 总事件数
    pub total_events: usize,
    /// 按事件类型的分布
    pub by_type: HashMap<String, usize>,
    /// 有事件发生的天数
    pub active_days: usize,
    /// 高频标签 Top 10
    pub most_active_tags: Vec<String>,
    /// 日均事件数
    pub daily_average: f32,
    /// 与上一同等时段的对比
    pub period_comparison: Option<PeriodComparison>,
}

/// 时段对比
#[derive(Debug, Serialize, Deserialize)]
pub struct PeriodComparison {
    /// 当前时段事件数
    pub current_count: usize,
    /// 上一同等时段事件数
    pub previous_count: usize,
    /// 变化百分比（正=增长，负=下降）
    pub change_percent: f32,
}

impl StatisticsAggregator {
    /// 从已查询的事件列表计算统计信息
    pub fn compute(
        &self,
        daily_events: &[DailyEvents],
        start_date: NaiveDate,
        end_date: NaiveDate,
        previous_daily_events: Option<&[DailyEvents]>, // 上一时段数据（可选）
    ) -> TimelineStatistics {
        let total_events: usize = daily_events.iter()
            .map(|d| d.event_count)
            .sum();

        // 按类型统计
        let mut by_type: HashMap<String, usize> = HashMap::new();
        for daily in daily_events {
            for event in &daily.events {
                *by_type.entry(event.event_type.as_str().to_string())
                    .or_insert(0) += 1;
            }
        }

        let active_days = daily_events.len();

        // 标签频率统计
        let mut tag_freq: HashMap<String, usize> = HashMap::new();
        for daily in daily_events {
            for event in &daily.events {
                for tag in &event.tags {
                    *tag_freq.entry(tag.clone()).or_insert(0) += 1;
                }
            }
        }
        let mut tag_vec: Vec<(String, usize)> = tag_freq.into_iter().collect();
        tag_vec.sort_by(|a, b| b.1.cmp(&a.1));
        let most_active_tags: Vec<String> = tag_vec.iter()
            .take(10)
            .map(|(t, _)| t.clone())
            .collect();

        // 日均事件数
        let total_days = (end_date - start_date).num_days().max(1) as f32;
        let daily_average = total_events as f32 / total_days;

        // 时段对比
        let period_comparison = previous_daily_events.map(|prev| {
            let prev_total: usize = prev.iter().map(|d| d.event_count).sum();
            let change = if prev_total > 0 {
                ((total_events as f32 - prev_total as f32) / prev_total as f32) * 100.0
            } else {
                100.0 // 上一时段无数据，视为 100% 增长
            };
            PeriodComparison {
                current_count: total_events,
                previous_count: prev_total,
                change_percent: (change * 10.0).round() / 10.0,
            }
        });

        TimelineStatistics {
            total_events,
            by_type,
            active_days,
            most_active_tags,
            daily_average: (daily_average * 10.0).round() / 10.0,
            period_comparison,
        }
    }

    /// 标签趋势分析：比较两个时段的标签频率变化
    pub fn tag_trend(
        &self,
        current_daily: &[DailyEvents],
        previous_daily: &[DailyEvents],
    ) -> Vec<TagTrend> {
        let current_freq = self.compute_tag_freq(current_daily);
        let previous_freq = self.compute_tag_freq(previous_daily);

        let mut trends: Vec<TagTrend> = Vec::new();

        // 所有出现过的标签
        let all_tags: std::collections::HashSet<String> = current_freq.keys()
            .chain(previous_freq.keys())
            .cloned()
            .collect();

        for tag in all_tags {
            let curr = current_freq.get(&tag).copied().unwrap_or(0);
            let prev = previous_freq.get(&tag).copied().unwrap_or(0);

            let trend = if prev == 0 && curr > 0 {
                Trend::Emerging  // 新出现的标签
            } else if curr > prev {
                Trend::Rising
            } else if curr < prev {
                Trend::Declining
            } else {
                Trend::Stable
            };

            trends.push(TagTrend {
                tag,
                current_count: curr,
                previous_count: prev,
                trend,
            });
        }

        // 按变化幅度排序
        trends.sort_by(|a, b| {
            let a_change = a.current_count as i32 - a.previous_count as i32;
            let b_change = b.current_count as i32 - b.previous_count as i32;
            b_change.cmp(&a_change)
        });

        trends
    }

    /// 计算标签频率
    fn compute_tag_freq(&self, daily_events: &[DailyEvents]) -> HashMap<String, usize> {
        let mut freq: HashMap<String, usize> = HashMap::new();
        for daily in daily_events {
            for event in &daily.events {
                for tag in &event.tags {
                    *freq.entry(tag.clone()).or_insert(0) += 1;
                }
            }
        }
        freq
    }
}

/// 标签趋势条目
#[derive(Debug, Serialize, Deserialize)]
pub struct TagTrend {
    pub tag: String,
    pub current_count: usize,
    pub previous_count: usize,
    pub trend: Trend,
}

/// 趋势方向
#[derive(Debug, Serialize, Deserialize)]
pub enum Trend {
    Emerging,   // 新兴（之前不存在，现在有）
    Rising,     // 上升
    Stable,     // 稳定
    Declining,  // 下降
}
```

### 3.5 摘要生成器 (`summary.rs`)

```rust
use crate::infra::llm_client::LlmClient;

/// 摘要生成器
pub struct SummaryGenerator {
    llm_client: LlmClient,
}

impl SummaryGenerator {
    pub fn new(llm_client: LlmClient) -> Self {
        Self { llm_client }
    }

    /// 基于时间线数据生成自然语言摘要
    pub async fn generate_summary(
        &self,
        daily_events: &[DailyEvents],
        statistics: &TimelineStatistics,
        date_range: (NaiveDate, NaiveDate),
    ) -> Result<String> {
        let prompt = self.build_prompt(daily_events, statistics, date_range);
        let summary = self.llm_client.chat_completion(&prompt).await?;
        Ok(summary)
    }

    /// 构建 LLM prompt
    fn build_prompt(
        &self,
        daily_events: &[DailyEvents],
        statistics: &TimelineStatistics,
        date_range: (NaiveDate, NaiveDate),
    ) -> String {
        // 构建事件摘要文本（避免 token 过长，仅传关键信息）
        let events_text = self.summarize_events_for_prompt(daily_events, 30);

        format!(
r#"你是一个知识管理助手。请基于以下时间线数据，生成一段简洁的中文摘要。

## 时间范围
{} 至 {}

## 统计数据
- 总事件数: {}
- 活跃天数: {} 天
- 日均事件数: {} 条
- 高频标签: {}
- 事件类型分布: {}

## 事件摘要
{}

请生成一段 3-5 句话的摘要，包含：
1. 这段时间的主要活动概述
2. 重点关注的主题或领域
3. 值得注意的趋势或模式

语气友好自然，像在和朋友回顾近况。"#,
            date_range.0.format("%Y-%m-%d"),
            date_range.1.format("%Y-%m-%d"),
            statistics.total_events,
            statistics.active_days,
            statistics.daily_average,
            statistics.most_active_tags.join(", "),
            self.format_type_distribution(&statistics.by_type),
            events_text,
        )
    }

    /// 将事件列表压缩为 prompt 文本（控制 token 数量）
    fn summarize_events_for_prompt(
        &self,
        daily_events: &[DailyEvents],
        max_days: usize,
    ) -> String {
        let mut lines = Vec::new();
        for daily in daily_events.iter().take(max_days) {
            let events_summary: Vec<String> = daily.events.iter()
                .map(|e| format!("  - [{}] {}", e.event_type.as_str(), e.title))
                .collect();
            lines.push(format!(
                "{} ({} 条事件):\n{}",
                daily.date.format("%Y-%m-%d"),
                daily.event_count,
                events_summary.join("\n")
            ));
        }
        lines.join("\n")
    }

    /// 格式化事件类型分布
    fn format_type_distribution(&self, by_type: &HashMap<String, usize>) -> String {
        by_type.iter()
            .map(|(t, c)| format!("{}: {}", t, c))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
```

---

## 4. 数据流图

### 4.1 事件收集流程

```
┌─────────────┐     ┌──────────────┐     ┌───────────────┐     ┌──────────┐
│  notify     │     │  Git Repo    │     │  Module       │     │  Vault   │
│  FileWatcher│     │  (git2)      │     │  Callbacks    │     │  Scan    │
└──────┬──────┘     └──────┬───────┘     └───────┬───────┘     └────┬─────┘
       │                   │                     │                  │
       ▼                   ▼                     ▼                  ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                     Event Collectors (各收集器)                          │
│                                                                          │
│  FileWatcherCollector  GitCommitCollector  FrontmatterDateExtractor      │
│                        FilenameDateExtractor  ContentTagExtractor        │
└──────────────────────────────────┬───────────────────────────────────────┘
                                   │ Vec<TimelineEvent>
                                   ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                     Event Normalizer (事件规范化)                        │
│                                                                          │
│  1. 去重（同路径 + 同类型 + 300ms 内 → 合并）                              │
│  2. 校验（日期合理性、标题长度截断）                                        │
│  3. 补充标签（从 frontmatter/文件名提取标签）                               │
│  4. 批量分组（每 50 条或每 1 秒提交一次）                                   │
└──────────────────────────────────┬───────────────────────────────────────┘
                                   │ Vec<TimelineEvent>
                                   ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                     TimelineStore (SQLite 写入)                          │
│                                                                          │
│  BEGIN TRANSACTION                                                       │
│    INSERT INTO timeline_events ...                                       │
│    INSERT INTO timeline_events ...                                       │
│  COMMIT                                                                  │
│                                                                          │
│  发布事件: EventBus::publish("timeline.event_recorded")                   │
└──────────────────────────────────────────────────────────────────────────┘
```

### 4.2 查询响应流程

```
LLM 调用 get_timeline(start_date, end_date, event_types?, tags?)
    │
    ▼
┌──────────────────────────────────────────────────────────────┐
│  API Handler (api/handlers/timeline.rs)                       │
│  1. 反序列化请求参数                                           │
│  2. 参数校验（日期格式、范围合理性）                              │
└──────────────────────────┬───────────────────────────────────┘
                           │ TimelineQuery
                           ▼
┌──────────────────────────────────────────────────────────────┐
│  QueryEngine (query.rs)                                       │
│  1. 构建 SQL WHERE 子句                                        │
│  2. 执行查询                                                  │
│  3. 结果按日分组                                               │
└──────────────────────────┬───────────────────────────────────┘
                           │ Vec<DailyEvents>
                    ┌──────┴──────┐
                    ▼             ▼
┌──────────────────────┐  ┌──────────────────────┐
│  StatisticsAggregator│  │  SummaryGenerator    │
│  计算统计摘要         │  │  (可选) 调用 LLM     │
│                      │  │  生成自然语言摘要     │
└──────────┬───────────┘  └──────────┬───────────┘
           │                         │
           ▼                         ▼
┌──────────────────────────────────────────────────────────────┐
│  Response Formatter (formatter.rs)                            │
│  组装 TimelineResponse JSON                                   │
│  { date_range, daily_events, statistics, summary }            │
└──────────────────────────┬───────────────────────────────────┘
                           │ JSON
                           ▼
                    返回给 LLM 前端
```

### 4.3 过期清理流程

```
定时任务 (每日凌晨 3:00)
    │
    ▼
┌──────────────────────────────────────────────────────────────┐
│  Retention Policy Manager                                     │
│  1. 检查各事件类型的保留期限                                    │
│  2. 对超期事件按月聚合                                         │
│     - GROUP BY strftime('%Y-%m', date), event_type            │
│     - COUNT(*) as count                                       │
│     - 生成 tags_summary                                       │
│  3. 写入 timeline_monthly_summaries 表                         │
│  4. 删除已聚合的原始事件                                       │
└──────────────────────────────────────────────────────────────┘
```

---

## 5. 关键数据结构

### 5.1 核心数据结构 (models.rs)

```rust
use chrono::{NaiveDate, DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

// ============================================================
// 事件类型枚举
// ============================================================

/// 时间线事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// 笔记创建
    NoteCreated,
    /// 笔记修改
    NoteModified,
    /// 代码仓库提交
    RepoCommit,
    /// 雷达文章保存
    RadarSaved,
    /// 记忆单元创建
    MemoryCreated,
}

impl EventType {
    /// 转为字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoteCreated => "note_created",
            Self::NoteModified => "note_modified",
            Self::RepoCommit => "repo_commit",
            Self::RadarSaved => "radar_saved",
            Self::MemoryCreated => "memory_created",
        }
    }

    /// 从字符串解析
    pub fn from_str(s: &str) -> Self {
        match s {
            "note_created" => Self::NoteCreated,
            "note_modified" => Self::NoteModified,
            "repo_commit" => Self::RepoCommit,
            "radar_saved" => Self::RadarSaved,
            "memory_created" => Self::MemoryCreated,
            _ => Self::NoteModified, // 默认值
        }
    }

    /// 所有变体列表
    pub fn all() -> &'static [Self] {
        &[
            Self::NoteCreated,
            Self::NoteModified,
            Self::RepoCommit,
            Self::RadarSaved,
            Self::MemoryCreated,
        ]
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================
// 时间线事件
// ============================================================

/// 时间线事件 — 核心数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    /// 事件唯一标识 (UUID v4)
    pub id: Uuid,
    /// 事件发生日期 (YYYY-MM-DD)
    pub date: NaiveDate,
    /// 精确时间戳（部分事件仅有日期，如 frontmatter 日期）
    pub timestamp: Option<DateTime<Utc>>,
    /// 事件类型
    pub event_type: EventType,
    /// 事件标题（简短，≤100 字符）
    pub title: String,
    /// 事件摘要（≤500 字符）
    pub summary: String,
    /// 关联标签列表
    pub tags: Vec<String>,
    /// 关联的文件路径列表
    pub related_paths: Vec<PathBuf>,
    /// 事件来源标识
    /// "frontmatter" | "filename" | "content_tag" | "file_watcher" | "git" | "radar" | "memory"
    pub source: String,
    /// 扩展元数据 (JSON)
    pub metadata: serde_json::Value,
}

// ============================================================
// 按日分组事件
// ============================================================

/// 按日分组的事件集合
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyEvents {
    /// 日期
    pub date: NaiveDate,
    /// 当日事件数
    pub event_count: usize,
    /// 当日事件列表（按时间倒序排列）
    pub events: Vec<TimelineEvent>,
}

// ============================================================
// 查询请求
// ============================================================

/// get_timeline 工具的请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTimelineRequest {
    /// 起始日期，格式 "YYYY-MM-DD"
    pub start_date: String,
    /// 结束日期，格式 "YYYY-MM-DD"
    pub end_date: String,
    /// 事件类型过滤（可选）
    #[serde(default)]
    pub event_types: Option<Vec<String>>,
    /// 标签过滤（可选，匹配任意一个）
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// 最大返回事件数（默认 200）
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize { 200 }

// ============================================================
// 查询响应
// ============================================================

/// get_timeline 工具的完整响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineResponse {
    /// 查询的日期范围
    pub date_range: DateRange,
    /// 按日分组的事件列表
    pub daily_events: Vec<DailyEvents>,
    /// 统计摘要
    pub statistics: TimelineStatistics,
    /// LLM 生成的自然语言摘要（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// 日期范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

/// 统计摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineStatistics {
    /// 总事件数
    pub total_events: usize,
    /// 按事件类型的分布
    pub by_type: std::collections::HashMap<String, usize>,
    /// 有事件发生的天数
    pub active_days: usize,
    /// 高频标签 Top 10
    pub most_active_tags: Vec<String>,
    /// 日均事件数
    pub daily_average: f32,
    /// 与上一同等时段的对比
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_comparison: Option<PeriodComparison>,
}

/// 时段对比
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodComparison {
    pub current_count: usize,
    pub previous_count: usize,
    pub change_percent: f32,
}

// ============================================================
// 月度聚合摘要
// ============================================================

/// 月度聚合摘要（过期事件压缩后的存储形式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlySummary {
    pub year: i32,
    pub month: u32,
    pub event_type: EventType,
    pub count: usize,
    pub summary: Option<String>,
    pub tags_summary: std::collections::HashMap<String, usize>,
}
```

### 5.2 TimelineService 定义 (mod.rs)

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

/// 时间线服务 — 模块入口
pub struct TimelineService {
    /// 事件存储
    store: Arc<TimelineStore>,
    /// 查询引擎
    query_engine: Arc<QueryEngine>,
    /// 统计聚合器
    statistics: Arc<StatisticsAggregator>,
    /// 摘要生成器（可选，依赖 LLM Client）
    summary_generator: Option<Arc<SummaryGenerator>>,
    /// 收集器列表
    collectors: Vec<Arc<dyn EventCollector>>,
    /// 配置
    config: TimelineConfig,
    /// 数据库连接池
    db_pool: Arc<RwLock<rusqlite::Connection>>,
}

/// 时间线模块配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineConfig {
    /// 支持的日期格式列表
    pub date_formats: Vec<String>,
    /// 事件保留策略（天数）
    pub retention: RetentionPolicy,
    /// 是否启用 LLM 摘要
    pub summary_enabled: bool,
    /// Git commit 最大采集数量
    pub git_max_commits: usize,
    /// 文件监控防抖时间（毫秒）
    pub debounce_ms: u64,
}

/// 事件保留策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// NoteCreated 保留天数（0 = 永久）
    pub note_created_days: usize,
    /// NoteModified 保留天数
    pub note_modified_days: usize,
    /// RepoCommit 保留天数
    pub repo_commit_days: usize,
    /// RadarSaved 保留天数（0 = 永久）
    pub radar_saved_days: usize,
    /// MemoryCreated 保留天数
    pub memory_created_days: usize,
}

impl Default for TimelineConfig {
    fn default() -> Self {
        Self {
            date_formats: vec![
                "%Y-%m-%d".into(),
                "%Y/%m/%d".into(),
                "%Y%m%d".into(),
            ],
            retention: RetentionPolicy {
                note_created_days: 0,     // 永久
                note_modified_days: 730,  // 2 年
                repo_commit_days: 365,    // 1 年
                radar_saved_days: 0,      // 永久
                memory_created_days: 365, // 1 年
            },
            summary_enabled: true,
            git_max_commits: 100,
            debounce_ms: 300,
        }
    }
}

impl TimelineService {
    /// 初始化时间线服务
    pub async fn new(
        db_path: std::path::PathBuf,
        vault_path: std::path::PathBuf,
        llm_client: Option<LlmClient>,
        config: TimelineConfig,
    ) -> Result<Self> {
        // 1. 初始化 SQLite
        let conn = rusqlite::Connection::open(&db_path)?;
        let store = Arc::new(TimelineStore::new(db_path));
        store.initialize(&conn)?;

        // 2. 初始化收集器
        let collectors: Vec<Arc<dyn EventCollector>> = vec![
            Arc::new(FrontmatterDateExtractor::new()),
            Arc::new(FilenameDateExtractor::new()),
            Arc::new(ContentTagExtractor::new()),
            Arc::new(FileWatcherCollector::new(
                vault_path.clone(),
                vec![".obsidian/**".into(), "templates/**".into(), ".trash/**".into()],
            )),
            Arc::new(GitCommitCollector::new(config.git_max_commits)),
        ];

        // 3. 初始化查询引擎和统计聚合器
        let query_engine = Arc::new(QueryEngine);
        let statistics = Arc::new(StatisticsAggregator);

        // 4. 初始化摘要生成器（可选）
        let summary_generator = llm_client
            .filter(|_| config.summary_enabled)
            .map(|client| Arc::new(SummaryGenerator::new(client)));

        Ok(Self {
            store,
            query_engine,
            statistics,
            summary_generator,
            collectors,
            config,
            db_pool: Arc::new(RwLock::new(conn)),
        })
    }

    /// 处理 get_timeline 工具调用
    pub async fn get_timeline(
        &self,
        request: GetTimelineRequest,
    ) -> Result<TimelineResponse> {
        // 1. 解析日期参数
        let start_date = NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d")
            .map_err(|_| BrainError::InvalidParam("start_date 格式错误，应为 YYYY-MM-DD".into()))?;
        let end_date = NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d")
            .map_err(|_| BrainError::InvalidParam("end_date 格式错误，应为 YYYY-MM-DD".into()))?;

        // 2. 构建查询
        let query = TimelineQuery {
            start_date,
            end_date,
            event_types: request.event_types.map(|types| {
                types.iter().map(|t| EventType::from_str(t)).collect()
            }),
            tags: request.tags,
            limit: request.limit,
        };

        // 3. 执行查询
        let conn = self.db_pool.read().await;
        let daily_events = self.query_engine.query(&conn, &query)?;

        // 4. 计算统计（同时查询上一同等时段做对比）
        let period_days = (end_date - start_date).num_days();
        let prev_end = start_date - chrono::Duration::days(1);
        let prev_start = prev_end - chrono::Duration::days(period_days);
        let prev_query = TimelineQuery {
            start_date: prev_start,
            end_date: prev_end,
            event_types: query.event_types.clone(),
            tags: query.tags.clone(),
            limit: query.limit,
        };
        let prev_daily = self.query_engine.query(&conn, &prev_query).ok();

        let statistics = self.statistics.compute(
            &daily_events,
            start_date,
            end_date,
            prev_daily.as_deref(),
        );

        drop(conn); // 释放读锁

        // 5. 生成 LLM 摘要（异步，可选）
        let summary = if let Some(ref gen) = self.summary_generator {
            gen.generate_summary(&daily_events, &statistics, (start_date, end_date))
                .await
                .ok()
        } else {
            None
        };

        // 6. 组装响应
        Ok(TimelineResponse {
            date_range: DateRange {
                start: request.start_date,
                end: request.end_date,
            },
            daily_events,
            statistics,
            summary,
        })
    }

    /// 记录事件（由 EventBus 回调触发）
    pub async fn record_event(&self, event: TimelineEvent) -> Result<()> {
        let conn = self.db_pool.write().await;
        self.store.insert_event(&conn, &event)?;
        Ok(())
    }

    /// 批量记录事件
    pub async fn record_events(&self, events: Vec<TimelineEvent>) -> Result<usize> {
        let conn = self.db_pool.write().await;
        self.store.insert_events_batch(&conn, &events)
    }

    /// "去年今日" 查询
    pub async fn get_on_this_day(&self, years_ago: i32) -> Result<Vec<TimelineEvent>> {
        let conn = self.db_pool.read().await;
        self.query_engine.query_on_this_day(&conn, years_ago)
    }
}
```

---

## 6. 查询 API 设计

### 6.1 get_timeline 工具完整定义

**JSON Schema（MCP 工具定义格式）**：

```json
{
  "name": "get_timeline",
  "description": "查询时间线事件。返回指定日期范围内的所有知识活动事件，按日分组，附带统计摘要。可用于每日回顾、周报生成、知识演变追踪等场景。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "start_date": {
        "type": "string",
        "description": "起始日期（含），格式 YYYY-MM-DD",
        "pattern": "^\\d{4}-\\d{2}-\\d{2}$"
      },
      "end_date": {
        "type": "string",
        "description": "结束日期（含），格式 YYYY-MM-DD",
        "pattern": "^\\d{4}-\\d{2}-\\d{2}$"
      },
      "event_types": {
        "type": "array",
        "items": {
          "type": "string",
          "enum": [
            "note_created",
            "note_modified",
            "repo_commit",
            "radar_saved",
            "memory_created"
          ]
        },
        "description": "事件类型过滤。不指定则返回全部类型。"
      },
      "tags": {
        "type": "array",
        "items": { "type": "string" },
        "description": "标签过滤。匹配任意一个指定标签的事件都会被返回。"
      },
      "limit": {
        "type": "integer",
        "description": "最大返回事件数，默认 200",
        "default": 200,
        "minimum": 1,
        "maximum": 1000
      }
    },
    "required": ["start_date", "end_date"]
  }
}
```

### 6.2 请求/响应示例

#### 示例 1: 基础日期范围查询

**请求**：
```json
{
  "tool": "get_timeline",
  "arguments": {
    "start_date": "2026-05-01",
    "end_date": "2026-05-28"
  }
}
```

**响应**：
```json
{
  "tool": "get_timeline",
  "status": "success",
  "result": {
    "date_range": {
      "start": "2026-05-01",
      "end": "2026-05-28"
    },
    "daily_events": [
      {
        "date": "2026-05-28",
        "event_count": 3,
        "events": [
          {
            "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "date": "2026-05-28",
            "timestamp": "2026-05-28T14:30:00Z",
            "event_type": "note_modified",
            "title": "修改笔记: rust-async",
            "summary": "文件 'programming/rust-async.md' 发生内容修改",
            "tags": ["rust", "async"],
            "related_paths": ["programming/rust-async.md"],
            "source": "file_watcher",
            "metadata": {
              "event_kind": "Modify(Data(Content))",
              "absolute_path": "/Users/me/ObsidianVault/programming/rust-async.md"
            }
          },
          {
            "id": "b2c3d4e5-f6a7-8901-bcde-f12345678901",
            "date": "2026-05-28",
            "timestamp": "2026-05-28T10:15:00Z",
            "event_type": "repo_commit",
            "title": "[my-app] feat: add auth module",
            "summary": "commit a1b2c3d by TiercelChow — feat: add auth module",
            "tags": ["feat", "auth"],
            "related_paths": [],
            "source": "git",
            "metadata": {
              "repo_name": "my-app",
              "repo_path": "/Users/me/projects/my-app",
              "commit_hash": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0",
              "short_hash": "a1b2c3d",
              "author": "TiercelChow"
            }
          },
          {
            "id": "c3d4e5f6-a7b8-9012-cdef-123456789012",
            "date": "2026-05-28",
            "timestamp": "2026-05-28T14:35:00Z",
            "event_type": "memory_created",
            "title": "新记忆: tokio select! 模式",
            "summary": "从 rust-async.md 提取的关键知识点",
            "tags": ["rust", "async", "tokio"],
            "related_paths": ["programming/rust-async.md"],
            "source": "memory",
            "metadata": {}
          }
        ]
      },
      {
        "date": "2026-05-27",
        "event_count": 1,
        "events": [
          {
            "id": "d4e5f6a7-b8c9-0123-defa-234567890123",
            "date": "2026-05-27",
            "timestamp": "2026-05-27T09:00:00Z",
            "event_type": "note_created",
            "title": "新建笔记: rag-survey-notes",
            "summary": "阅读并整理了 RAG 技术综述论文的核心观点",
            "tags": ["ai", "rag", "reading"],
            "related_paths": ["ai/rag-survey-notes.md"],
            "source": "frontmatter",
            "metadata": {
              "frontmatter_field": "date",
              "frontmatter_value": "2026-05-27"
            }
          }
        ]
      }
    ],
    "statistics": {
      "total_events": 4,
      "by_type": {
        "note_created": 1,
        "note_modified": 1,
        "repo_commit": 1,
        "memory_created": 1
      },
      "active_days": 2,
      "most_active_tags": ["rust", "async", "ai", "rag", "reading", "feat", "auth", "tokio"],
      "daily_average": 0.1,
      "period_comparison": {
        "current_count": 4,
        "previous_count": 6,
        "change_percent": -33.3
      }
    },
    "summary": "5月1日至28日期间，你的知识活动主要集中在 Rust 异步编程和 AI RAG 技术领域。5月28日是最近最活跃的一天，既有笔记更新也有代码提交。与上月同期相比，活动量下降了 33.3%，不过质量上的积累同样重要。"
  }
}
```

#### 示例 2: 带过滤的查询

**请求**：
```json
{
  "tool": "get_timeline",
  "arguments": {
    "start_date": "2026-04-01",
    "end_date": "2026-04-30",
    "event_types": ["repo_commit"],
    "tags": ["feat"],
    "limit": 50
  }
}
```

#### 示例 3: 错误响应

**请求**（日期格式错误）：
```json
{
  "tool": "get_timeline",
  "arguments": {
    "start_date": "2026/05/01",
    "end_date": "2026-05-28"
  }
}
```

**响应**：
```json
{
  "tool": "get_timeline",
  "status": "error",
  "error": {
    "code": "INVALID_PARAM",
    "message": "start_date 格式错误，应为 YYYY-MM-DD",
    "suggestion": "请使用 ISO 8601 日期格式，例如 2026-05-01"
  }
}
```

---

## 7. LLM 摘要生成 Prompt 模板

### 7.1 时间线摘要 Prompt

```
你是一个知识管理助手。请基于以下时间线数据，生成一段简洁的中文摘要。

## 时间范围
{start_date} 至 {end_date}

## 统计数据
- 总事件数: {total_events}
- 活跃天数: {active_days} 天
- 日均事件数: {daily_average} 条
- 高频标签: {most_active_tags}
- 事件类型分布: {type_distribution}
- 与上一时段对比: {period_comparison}

## 事件摘要
{events_summary_text}

请生成一段 3-5 句话的摘要，包含：
1. 这段时间的主要活动概述
2. 重点关注的主题或领域
3. 值得注意的趋势或模式

语气友好自然，像在和朋友回顾近况。
```

### 7.2 周报生成 Prompt（供 weekly_review 技能使用）

```
你是一个个人知识管理助手，请基于本周的时间线数据生成一份周报。

## 本周时间范围
{start_date} 至 {end_date}

## 本周统计
- 新增笔记: {note_created_count} 篇
- 修改笔记: {note_modified_count} 次
- 代码提交: {repo_commit_count} 次
- 保存文章: {radar_saved_count} 篇
- 活跃天数: {active_days} / 7 天

## 高频标签
{most_active_tags}

## 每日事件概要
{daily_summary_text}

请生成结构化的周报，包含以下部分：
### 📊 本周概览
（一句话总结本周的知识活动情况）

### 📝 笔记动态
（新增和修改了哪些笔记，主要涉及什么主题）

### 💻 代码活动
（代码仓库的主要进展和提交摘要）

### 🏷️ 主题聚焦
（本周关注度最高的 2-3 个主题）

### 📈 趋势观察
（与上周对比有什么变化，有什么值得注意的模式）
```

---

## 8. 错误处理

### 8.1 错误类型定义

```rust
use thiserror::Error;

/// 时间线模块错误类型
#[derive(Error, Debug)]
pub enum TimelineError {
    #[error("日期格式错误: {0}，应为 YYYY-MM-DD")]
    InvalidDateFormat(String),

    #[error("日期范围无效: start_date ({0}) 晚于 end_date ({1})")]
    InvalidDateRange(String, String),

    #[error("查询超时: 日期范围过大 ({days} 天)，建议缩小范围")]
    QueryTimeout { days: i64 },

    #[error("事件类型无效: {0}")]
    InvalidEventType(String),

    #[error("SQLite 错误: {0}")]
    DatabaseError(#[from] rusqlite::Error),

    #[error("Git 仓库访问失败: {path} — {detail}")]
    GitError {
        path: String,
        detail: String,
    },

    #[error("LLM 摘要生成失败: {0}")]
    SummaryError(String),

    #[error("事件序列化失败: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("内部错误: {0}")]
    Internal(String),
}
```

### 8.2 错误处理策略

| 错误场景 | 处理方式 | 返回给 LLM 的信息 |
|---|---|---|
| 日期格式错误 | 返回参数校验错误 | 提示正确格式 |
| 日期范围为空 | 返回空结果（非错误） | `daily_events: [], total_events: 0` |
| start > end | 返回参数错误 | 提示日期顺序 |
| SQLite 读取失败 | 重试 1 次，失败则返回内部错误 | 通用错误消息 |
| Git 仓库不可访问 | 跳过该仓库，日志告警 | 不影响其他数据返回 |
| LLM 摘要失败 | 返回结果但不含 summary 字段 | `summary: null` |
| 标签 JSON 解析失败 | 返回空标签列表 | 不影响事件返回 |

### 8.3 降级策略

```rust
/// 安全查询：任何子步骤失败都不影响整体返回
impl TimelineService {
    pub async fn get_timeline_safe(
        &self,
        request: GetTimelineRequest,
    ) -> TimelineResponse {
        match self.get_timeline(request).await {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("Timeline query failed: {}", e);
                // 返回空结果而非错误，保证 LLM 不会因工具失败而中断
                TimelineResponse {
                    date_range: DateRange {
                        start: "unknown".into(),
                        end: "unknown".into(),
                    },
                    daily_events: Vec::new(),
                    statistics: TimelineStatistics::empty(),
                    summary: Some(format!("查询失败: {}", e)),
                }
            }
        }
    }
}
```

---

## 9. 性能优化

### 9.1 事件分区策略

对于大规模事件库（>10000 条），采用基于日期的逻辑分区：

```
分区键: date 字段的前 7 位 (YYYY-MM)

查询优化:
- 月度查询自动命中单分区
- 跨月查询按分区并行执行
- SQLite 本身不支持分区表，通过索引 + 查询条件模拟分区裁剪
```

### 9.2 查询索引优化

**已设计的索引**（见 §3.2.3）确保了以下查询路径的效率：

```
查询模式                              使用索引                         预期性能
─────────────────────────────────────────────────────────────────────────────
WHERE date BETWEEN ? AND ?          idx_timeline_events_date          O(log n)
WHERE date BETWEEN ? AND ?
  AND event_type = ?                idx_timeline_events_date_type     O(log n)
WHERE source = ?                    idx_timeline_events_source        O(log n)
ORDER BY date DESC, timestamp DESC  idx_timeline_events_date (覆盖)   O(log n)
```

### 9.3 缓存策略

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

/// 时间线查询结果缓存
pub struct TimelineCache {
    /// 缓存条目：(查询哈希, 结果, 过期时间)
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// 缓存 TTL
    ttl: Duration,
    /// 最大缓存条目数
    max_entries: usize,
}

struct CacheEntry {
    response: TimelineResponse,
    created_at: Instant,
}

impl TimelineCache {
    pub fn new(ttl_secs: u64, max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            ttl: Duration::from_secs(ttl_secs),
            max_entries,
        }
    }

    /// 生成缓存键
    pub fn cache_key(query: &TimelineQuery) -> String {
        format!(
            "{}:{}:{:?}:{:?}:{}",
            query.start_date,
            query.end_date,
            query.event_types,
            query.tags,
            query.limit,
        )
    }

    /// 获取缓存
    pub async fn get(&self, key: &str) -> Option<TimelineResponse> {
        let entries = self.entries.read().await;
        entries.get(key).and_then(|entry| {
            if entry.created_at.elapsed() < self.ttl {
                Some(entry.response.clone())
            } else {
                None
            }
        })
    }

    /// 写入缓存
    pub async fn put(&self, key: String, response: TimelineResponse) {
        let mut entries = self.entries.write().await;

        // 淘汰过期条目
        if entries.len() >= self.max_entries {
            entries.retain(|_, v| v.created_at.elapsed() < self.ttl);
        }

        // 如果仍超限，淘汰最旧的
        if entries.len() >= self.max_entries {
            if let Some(oldest_key) = entries.iter()
                .min_by_key(|(_, v)| v.created_at)
                .map(|(k, _)| k.clone())
            {
                entries.remove(&oldest_key);
            }
        }

        entries.insert(key, CacheEntry {
            response,
            created_at: Instant::now(),
        });
    }

    /// 清除所有缓存（事件写入时调用）
    pub async fn invalidate(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }
}
```

**缓存策略**：
- 查询结果缓存 60 秒（同一天内重复查询直接返回缓存）
- 新事件写入时清除全部缓存（保证数据一致性）
- 统计查询结果不缓存（计算成本低）
- LLM 摘要结果单独缓存 5 分钟（LLM 调用成本高）

### 9.4 批量写入优化

```rust
/// 事件写入缓冲器 — 收集事件后批量提交
pub struct EventWriteBuffer {
    buffer: Arc<RwLock<Vec<TimelineEvent>>>,
    /// 批量提交阈值
    batch_size: usize,
    /// 最大等待时间
    flush_interval: Duration,
}

impl EventWriteBuffer {
    /// 添加事件到缓冲区
    pub async fn push(&self, event: TimelineEvent) {
        let mut buf = self.buffer.write().await;
        buf.push(event);

        if buf.len() >= self.batch_size {
            let events: Vec<TimelineEvent> = buf.drain(..).collect();
            drop(buf);
            // 异步批量写入
            self.flush_events(events).await;
        }
    }

    /// 定时刷新：即使未达到 batch_size 也提交
    pub async fn flush(&self) {
        let mut buf = self.buffer.write().await;
        if !buf.is_empty() {
            let events: Vec<TimelineEvent> = buf.drain(..).collect();
            drop(buf);
            self.flush_events(events).await;
        }
    }

    async fn flush_events(&self, events: Vec<TimelineEvent>) {
        // 调用 TimelineStore::insert_events_batch
        // ...
    }
}
```

---

## 10. 测试策略

### 10.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    // ========== Frontmatter 日期提取测试 ==========

    #[test]
    fn test_frontmatter_iso_date() {
        let extractor = FrontmatterDateExtractor::new();
        let fm = serde_json::json!({"date": "2026-05-28"});
        let dates = extractor.extract_dates(&fm);
        assert_eq!(dates.len(), 1);
        assert_eq!(dates[0].1, NaiveDate::from_ymd_opt(2026, 5, 28).unwrap());
        assert_eq!(dates[0].2, EventType::NoteCreated);
    }

    #[test]
    fn test_frontmatter_datetime_with_timezone() {
        let extractor = FrontmatterDateExtractor::new();
        let fm = serde_json::json!({"created_at": "2026-05-28T14:30:00+08:00"});
        let dates = extractor.extract_dates(&fm);
        assert_eq!(dates.len(), 1);
        assert_eq!(dates[0].1, NaiveDate::from_ymd_opt(2026, 5, 28).unwrap());
    }

    #[test]
    fn test_frontmatter_multiple_fields() {
        let extractor = FrontmatterDateExtractor::new();
        let fm = serde_json::json!({
            "date": "2026-05-28",
            "modified": "2026-05-29"
        });
        let dates = extractor.extract_dates(&fm);
        assert_eq!(dates.len(), 2);
    }

    #[test]
    fn test_frontmatter_english_date() {
        let extractor = FrontmatterDateExtractor::new();
        let fm = serde_json::json!({"date": "May 28, 2026"});
        let dates = extractor.extract_dates(&fm);
        assert_eq!(dates.len(), 1);
        assert_eq!(dates[0].1, NaiveDate::from_ymd_opt(2026, 5, 28).unwrap());
    }

    #[test]
    fn test_frontmatter_invalid_date() {
        let extractor = FrontmatterDateExtractor::new();
        let fm = serde_json::json!({"date": "not-a-date"});
        let dates = extractor.extract_dates(&fm);
        assert_eq!(dates.len(), 0);
    }

    // ========== 文件名日期提取测试 ==========

    #[test]
    fn test_filename_iso_date() {
        let extractor = FilenameDateExtractor::new();
        let path = std::path::Path::new("2026-05-28-meeting.md");
        assert_eq!(
            extractor.extract_date(path),
            Some(NaiveDate::from_ymd_opt(2026, 5, 28).unwrap())
        );
    }

    #[test]
    fn test_filename_compact_date() {
        let extractor = FilenameDateExtractor::new();
        let path = std::path::Path::new("20260528_daily.md");
        assert_eq!(
            extractor.extract_date(path),
            Some(NaiveDate::from_ymd_opt(2026, 5, 28).unwrap())
        );
    }

    #[test]
    fn test_filename_no_date() {
        let extractor = FilenameDateExtractor::new();
        let path = std::path::Path::new("meeting-notes.md");
        assert_eq!(extractor.extract_date(path), None);
    }

    #[test]
    fn test_filename_directory_date() {
        let extractor = FilenameDateExtractor::new();
        let path = std::path::Path::new("2026/05/28/note.md");
        assert_eq!(
            extractor.extract_date(path),
            Some(NaiveDate::from_ymd_opt(2026, 5, 28).unwrap())
        );
    }

    #[test]
    fn test_filename_reject_invalid_year() {
        let extractor = FilenameDateExtractor::new();
        // 年份 0001 应该被拒绝
        let path = std::path::Path::new("0001-01-01-ancient.md");
        assert_eq!(extractor.extract_date(path), None);
    }

    // ========== 内容标签提取测试 ==========

    #[test]
    fn test_content_tag_basic() {
        let extractor = ContentTagExtractor::new();
        let content = "这是一篇笔记 #date/2026-05-28 包含日期标签";
        let tags = extractor.extract_tags(content);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].date, NaiveDate::from_ymd_opt(2026, 5, 28).unwrap());
        assert_eq!(tags[0].keyword, None);
    }

    #[test]
    fn test_content_tag_with_keyword() {
        let extractor = ContentTagExtractor::new();
        let content = "会议记录 #date/2026-05-28/meeting";
        let tags = extractor.extract_tags(content);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].keyword, Some("meeting".to_string()));
    }

    #[test]
    fn test_content_tag_multiple() {
        let extractor = ContentTagExtractor::new();
        let content = "#date/2026-05-28 第一次\n#date/2026-05-29/review 第二次";
        let tags = extractor.extract_tags(content);
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn test_content_tag_no_match() {
        let extractor = ContentTagExtractor::new();
        let content = "普通笔记没有日期标签 #tag/other";
        let tags = extractor.extract_tags(content);
        assert_eq!(tags.len(), 0);
    }

    // ========== 查询引擎测试 ==========

    #[test]
    fn test_group_by_date() {
        let engine = QueryEngine;
        let events = vec![
            make_test_event("2026-05-28", "event1"),
            make_test_event("2026-05-28", "event2"),
            make_test_event("2026-05-27", "event3"),
        ];
        let grouped = engine.group_by_date(events);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].date, NaiveDate::from_ymd_opt(2026, 5, 28).unwrap());
        assert_eq!(grouped[0].event_count, 2);
    }

    // ========== 统计聚合测试 ==========

    #[test]
    fn test_statistics_compute() {
        let agg = StatisticsAggregator;
        let daily = vec![
            DailyEvents {
                date: NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
                event_count: 3,
                events: vec![
                    make_test_event_with_tags("2026-05-28", vec!["rust"]),
                    make_test_event_with_tags("2026-05-28", vec!["rust", "async"]),
                    make_test_event_with_tags("2026-05-28", vec!["ai"]),
                ],
            },
        ];

        let stats = agg.compute(
            &daily,
            NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
            NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
            None,
        );

        assert_eq!(stats.total_events, 3);
        assert_eq!(stats.active_days, 1);
        assert_eq!(stats.most_active_tags[0], "rust"); // rust 出现 2 次
    }
}
```

### 10.2 集成测试

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;

    /// 端到端测试：从事件写入到查询
    #[tokio::test]
    async fn test_full_pipeline() {
        let tmp_dir = TempDir::new().unwrap();
        let db_path = tmp_dir.path().join("test_brain.db");

        // 1. 初始化服务
        let service = TimelineService::new(
            db_path,
            PathBuf::from("/tmp/test_vault"),
            None, // 不使用 LLM
            TimelineConfig::default(),
        ).await.unwrap();

        // 2. 写入测试事件
        let events = vec![
            TimelineEvent {
                id: Uuid::new_v4(),
                date: NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
                timestamp: Some(Utc::now()),
                event_type: EventType::NoteCreated,
                title: "测试笔记".into(),
                summary: "测试摘要".into(),
                tags: vec!["test".into()],
                related_paths: vec![PathBuf::from("test.md")],
                source: "test".into(),
                metadata: serde_json::json!({}),
            },
        ];
        let count = service.record_events(events).await.unwrap();
        assert_eq!(count, 1);

        // 3. 查询事件
        let response = service.get_timeline(GetTimelineRequest {
            start_date: "2026-05-28".into(),
            end_date: "2026-05-28".into(),
            event_types: None,
            tags: None,
            limit: 100,
        }).await.unwrap();

        assert_eq!(response.statistics.total_events, 1);
        assert_eq!(response.daily_events.len(), 1);
        assert_eq!(response.daily_events[0].events[0].title, "测试笔记");
    }

    /// 测试日期范围过滤
    #[tokio::test]
    async fn test_date_range_filter() {
        // ... 写入多天事件，验证日期过滤正确性
    }

    /// 测试事件类型过滤
    #[tokio::test]
    async fn test_event_type_filter() {
        // ... 写入多种类型事件，验证过滤正确性
    }

    /// 测试标签过滤
    #[tokio::test]
    async fn test_tag_filter() {
        // ... 写入不同标签事件，验证过滤正确性
    }

    /// 测试过期清理
    #[tokio::test]
    async fn test_retention_policy() {
        // ... 写入超期事件，执行清理，验证聚合正确
    }
}
```

### 10.3 测试覆盖目标

| 模块 | 目标覆盖率 | 关键测试点 |
|---|---|---|
| `frontmatter.rs` | > 90% | 各种日期格式、无效输入、多字段同时存在 |
| `filename.rs` | > 90% | 各种文件名模式、无日期文件名、不合理年份 |
| `content_tag.rs` | > 90% | 标签格式变体、多标签、无标签 |
| `file_watcher.rs` | > 80% | 创建/修改/删除事件、排除模式、非 .md 文件 |
| `git_commit.rs` | > 80% | 正常仓库、空仓库、增量收集、conventional commits |
| `store.rs` | > 85% | CRUD、批量写入、去重、过期清理 |
| `query.rs` | > 90% | 日期范围、类型过滤、标签过滤、分组正确性 |
| `statistics.rs` | > 95% | 统计计算、趋势分析、边界情况 |
| `summary.rs` | > 70% | prompt 构建（LLM 调用使用 mock） |

---

## 11. 依赖清单

### 11.1 Cargo.toml 新增依赖

```toml
[dependencies]
# ===== 时间线模块直接依赖 =====

# 日期时间处理
chrono = { version = "0.4", features = ["serde"] }

# UUID 生成
uuid = { version = "1.0", features = ["v4", "serde"] }

# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# SQLite 数据库
rusqlite = { version = "0.31", features = ["bundled"] }

# 正则表达式
regex = "1.10"

# 文件系统监控（已有依赖，时间线模块复用）
notify = "6.1"

# Git 操作（已有依赖，时间线模块复用）
git2 = "0.19"

# Markdown frontmatter 解析（已有依赖）
gray_matter = "0.2"

# Glob 模式匹配（排除文件）
glob = "0.3"

# 异步 trait
async-trait = "0.1"

# Tokio 异步运行时（已有依赖）
tokio = { version = "1.0", features = ["full"] }

# 日志
tracing = "0.1"

# 哈希计算（事件去重 title_hash）
sha2 = "0.10"
hex = "0.4"

# 错误处理
thiserror = "1.0"
anyhow = "1.0"

[dev-dependencies]
# 临时目录（集成测试）
tempfile = "3.10"
# Mock LLM client
mockall = "0.12"
```

### 11.2 依赖关系图

```
timeline/mod.rs
├── timeline/models.rs         (纯数据定义，无外部依赖)
├── timeline/collector/
│   ├── frontmatter.rs        → gray_matter, chrono, serde_json
│   ├── filename.rs           → regex, chrono
│   ├── content_tag.rs        → regex, chrono
│   ├── file_watcher.rs       → notify, chrono, glob
│   └── git_commit.rs         → git2, chrono, regex
├── timeline/store.rs          → rusqlite, uuid, serde_json, sha2, hex
├── timeline/query.rs          → rusqlite, chrono, serde_json
├── timeline/statistics.rs     → chrono (纯计算)
├── timeline/summary.rs        → crate::infra::llm_client
├── timeline/formatter.rs      → serde_json
└── api/handlers/timeline.rs   → axum, serde_json
```

### 11.3 与已有模块的集成点

| 集成点 | 集成方式 | 代码位置 |
|---|---|---|
| `infra::sqlite_store` | 共享 `rusqlite::Connection` | `TimelineStore` 接收连接池 |
| `infra::file_watcher` | 订阅 `notify::Event` | `FileWatcherCollector` 作为回调注册 |
| `infra::llm_client` | 调用 `chat_completion` | `SummaryGenerator` 依赖 |
| `core::code_repo` | 获取已注册仓库列表 | `GitCommitCollector` 查询仓库路径 |
| `EventBus` | 订阅/发布事件 | `TimelineService` 注册事件回调 |
| `tools::registry` | 注册 `get_timeline` 工具 | `api/handlers/timeline.rs` |

---

## 附录 A: 配置文件中的时间线配置段

对应 `config/default.toml` 中的 `[timeline]` 段：

```toml
[timeline]
# 支持的日期格式
date_formats = ["%Y-%m-%d", "%Y/%m/%d", "%Y%m%d"]

# 是否启用 LLM 摘要生成
summary_enabled = true

# Git commit 最大采集数量
git_max_commits = 100

# 文件监控防抖时间（毫秒）
debounce_ms = 300

# 事件保留策略（天数，0 = 永久）
[timeline.retention]
note_created_days = 0       # 永久保留
note_modified_days = 730    # 2 年
repo_commit_days = 365      # 1 年
radar_saved_days = 0        # 永久保留
memory_created_days = 365   # 1 年
```

## 附录 B: 完整 SQL 迁移脚本

文件：`migrations/004_timeline.sql`

```sql
-- Migration: 004_timeline
-- Description: 创建时间线模块所需的数据表
-- Created: 2026-05-29

-- 时间线事件主表
CREATE TABLE IF NOT EXISTS timeline_events (
    id              TEXT PRIMARY KEY,
    date            TEXT NOT NULL,
    timestamp       TEXT,
    event_type      TEXT NOT NULL,
    title           TEXT NOT NULL,
    summary         TEXT,
    tags            TEXT NOT NULL DEFAULT '[]',
    related_paths   TEXT NOT NULL DEFAULT '[]',
    source          TEXT NOT NULL,
    metadata        TEXT NOT NULL DEFAULT '{}',
    title_hash      TEXT NOT NULL,            -- 标题 SHA-256 前 16 位 hex
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_timeline_events_date
    ON timeline_events(date);

CREATE INDEX IF NOT EXISTS idx_timeline_events_date_type
    ON timeline_events(date, event_type);

CREATE INDEX IF NOT EXISTS idx_timeline_events_source
    ON timeline_events(source);

CREATE UNIQUE INDEX IF NOT EXISTS idx_timeline_events_dedup
    ON timeline_events(date, event_type, title_hash);

-- 月度聚合摘要表
CREATE TABLE IF NOT EXISTS timeline_monthly_summaries (
    id              TEXT PRIMARY KEY,
    year            INTEGER NOT NULL,
    month           INTEGER NOT NULL,
    event_type      TEXT NOT NULL,
    count           INTEGER NOT NULL,
    summary         TEXT,
    tags_summary    TEXT NOT NULL DEFAULT '{}',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(year, month, event_type)
);

CREATE INDEX IF NOT EXISTS idx_monthly_summaries_ym
    ON timeline_monthly_summaries(year, month);

-- 收集器状态表
CREATE TABLE IF NOT EXISTS timeline_collector_state (
    collector_name  TEXT PRIMARY KEY,
    source_key      TEXT NOT NULL,
    last_collected  TEXT NOT NULL,
    last_event_id   TEXT,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
```
