# 智识雷达 (Knowledge Radar) — 开发设计文档

> **模块编号**: 07 | **版本**: v1.0-draft | **最后更新**: 2026-05-29 | **状态**: 设计中
> **关联文档**: [顶层设计文档](../top_design.md) | [需求设计文档](../requirement/07-radar.md)

---

## 1. 技术架构详细设计

### 1.1 架构总览

智识雷达采用 **管道式架构**，数据从外部源经过抓取、处理、排序，最终呈现给用户或纳藏到 vault。整体分为五个核心子模块：

```
┌─────────────────────────────────────────────────────────────────┐
│                     Knowledge Radar Service                      │
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐   │
│  │ SourceManager│───▶│   Fetcher    │───▶│    Processor     │   │
│  │  (源管理器)   │    │  (内容抓取器) │    │   (内容处理器)    │   │
│  └──────────────┘    └──────────────┘    └────────┬─────────┘   │
│                                                    │             │
│                                                    ▼             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐   │
│  │  VaultSaver  │◀───│StateMachine  │◀───│ RelevanceEngine  │   │
│  │   (纳藏器)    │    │  (状态管理器) │    │   (相关性引擎)    │   │
│  └──────────────┘    └──────────────┘    └──────────────────┘   │
│         │                  │                       │             │
│         ▼                  ▼                       ▼             │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                   SQLite + Qdrant                         │   │
│  │          (radar_items 表 + embedding 向量)                 │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
         │                                        │
         ▼                                        ▼
  ┌──────────────┐                      ┌──────────────────┐
  │  记忆引擎     │                      │  Embedding 基础设施│
  │ (Memory)     │                      │  (infra/embedding)│
  └──────────────┘                      └──────────────────┘
```

### 1.2 核心设计原则

1. **异步优先**：所有网络 IO 操作使用 `tokio` 异步执行，不阻塞主事件循环
2. **容错隔离**：每个源的抓取独立执行，单个源失败不影响其他源
3. **增量处理**：已抓取的 URL 不重复处理，embedding 缓存复用
4. **可配置性**：所有关键参数（拉取间隔、阈值、排序权重等）可通过配置文件调整
5. **可观测性**：关键操作通过 `tracing` 输出结构化日志

---

## 2. 目录与文件组织

```
src/
├── core/
│   └── radar.rs                    # 雷达主服务，RadarService struct + 对外工具接口
├── infra/
│   ├── embedding.rs                # (已有) Embedding 生成基础设施
│   ├── qdrant_client.rs            # (已有) Qdrant 向量操作
│   ├── sqlite_store.rs             # (已有) SQLite 操作，扩展 radar_items 表
│   └── llm_client.rs               # (已有) LLM 调用封装
├── models/
│   └── radar.rs                    # 雷达相关数据模型定义
└── tools/
    └── handlers/
        └── radar_handlers.rs       # 雷达工具的 HTTP Handler
```

**`src/core/radar.rs` 内部组织**（单文件，通过子模块 `mod` 划分逻辑区块）：

```rust
// src/core/radar.rs

mod source_manager;   // 源管理器
mod fetcher;          // 内容抓取器
mod processor;        // 内容处理器
mod relevance;        // 相关性引擎
mod vault_saver;      // 纳藏器
mod state;            // 状态管理器
mod scheduler;        // 定时调度

pub use self::source_manager::SourceManager;
pub use self::fetcher::Fetcher;
pub use self::processor::Processor;
pub use self::relevance::RelevanceEngine;
pub use self::vault_saver::VaultSaver;
pub use self::state::StateManager;
pub use self::scheduler::RadarScheduler;

/// 智识雷达主服务
pub struct RadarService {
    source_manager: SourceManager,
    fetcher: Fetcher,
    processor: Processor,
    relevance_engine: RelevanceEngine,
    vault_saver: VaultSaver,
    state_manager: StateManager,
    scheduler: RadarScheduler,
    // 基础设施依赖
    embedding_client: Arc<EmbeddingClient>,
    qdrant_client: Arc<QdrantClient>,
    sqlite_store: Arc<SqliteStore>,
    llm_client: Arc<LlmClient>,
    vault_path: PathBuf,
    config: RadarConfig,
}

impl RadarService {
    /// 启动雷达服务：加载配置、初始化定时任务
    pub async fn start(config: RadarConfig, /* 其他依赖 */) -> Result<Self, BrainError> { ... }

    /// 手动触发一次全量拉取
    pub async fn fetch_now(&self) -> Result<FetchReport, BrainError> { ... }

    // ── LLM Tool API ──

    /// get_radar(limit?, query?) -> 获取推荐列表
    pub async fn get_radar(&self, limit: Option<usize>, query: Option<String>)
        -> Result<Vec<RadarItemView>, BrainError> { ... }

    /// add_to_vault(article_id, target_dir?) -> 纳藏文章
    pub async fn add_to_vault(&self, article_id: Uuid, target_dir: Option<String>)
        -> Result<VaultSaveResult, BrainError> { ... }

    /// dismiss_radar_item(article_id) -> 忽略文章
    pub async fn dismiss_radar_item(&self, article_id: Uuid)
        -> Result<bool, BrainError> { ... }

    /// add_radar_source(type, name, config) -> 添加源
    pub async fn add_radar_source(&self, source_type: SourceType, name: String, config: SourceConfig)
        -> Result<RadarSource, BrainError> { ... }

    /// remove_radar_source(name) -> 删除源
    pub async fn remove_radar_source(&self, name: &str)
        -> Result<bool, BrainError> { ... }

    /// toggle_radar_source(name, enabled) -> 启用/禁用源
    pub async fn toggle_radar_source(&self, name: &str, enabled: bool)
        -> Result<bool, BrainError> { ... }

    /// list_radar_sources() -> 列出所有源
    pub async fn list_radar_sources(&self)
        -> Result<Vec<RadarSourceStatus>, BrainError> { ... }
}
```

---

## 3. 各子模块详细设计

### 3.1 源管理器 (SourceManager)

负责管理所有外部信息源的配置、生命周期和 CRUD 操作。

#### 3.1.1 radar_sources.toml 完整结构

```toml
# config/radar_sources.toml
# 智识雷达 — 外部信息源配置文件

# 全局默认配置
[defaults]
max_items_per_source = 20          # 每个源每次最大拉取条数
trust_weight = 0.8                 # 默认来源可信度 (0.0 - 1.0)
request_timeout_secs = 30          # 单个源请求超时

# ── HackerNews 源 ──
[[sources]]
name = "hackernews"
type = "hackernews"
enabled = true
description = "HackerNews 高分技术文章"
# HN 特定配置
min_score = 50                     # 最低分数阈值
min_comments = 10                  # 最低评论数
query = ""                         # 搜索关键词（空 = 热门）
max_items = 20
trust_weight = 0.85

# ── arXiv 源 ──
[[sources]]
name = "arxiv-cs"
type = "arxiv"
enabled = true
description = "arXiv CS 领域最新论文"
# arXiv 特定配置
categories = ["cs.AI", "cs.CL", "cs.SE", "cs.IR"]
query = "LLM OR large language model OR retrieval OR RAG"
max_results = 15
sort_by = "submittedDate"          # "submittedDate" | "relevance"
trust_weight = 0.95                # 学术论文可信度高

# ── RSS 源 ──
[[sources]]
name = "tech-rss"
type = "rss"
enabled = true
description = "技术博客 RSS 聚合"
# RSS 特定配置
feeds = [
    "https://blog.rust-lang.org/feed.xml",
    "https://simonwillison.net/atom/everything/",
    "https://lucumr.pocoo.org/feed.atom",
    "https://www.paulgraham.com/rss.html",
]
max_items = 25                     # 每个 feed 最大条目数
trust_weight = 0.80

# ── Reddit 源 ──
[[sources]]
name = "reddit-programming"
type = "reddit"
enabled = false
description = "Reddit 编程相关 subreddit"
# Reddit 特定配置
subreddits = ["programming", "rust", "MachineLearning"]
min_upvotes = 100                  # 最低 upvotes
sort = "hot"                       # "hot" | "new" | "top"
time_range = "day"                 # "hour" | "day" | "week"
max_items = 15
trust_weight = 0.70

# ── 第二个 RSS 源示例 ──
[[sources]]
name = "ai-news"
type = "rss"
enabled = true
description = "AI 领域新闻与博客"
feeds = [
    "https://openai.com/blog/rss.xml",
    "https://blog.google/technology/ai/rss/",
    "https://huggingface.co/blog/feed.xml",
]
max_items = 20
trust_weight = 0.85
```

#### 3.1.2 源配置 Rust 数据结构

```rust
/// 源配置文件的顶层结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarSourcesConfig {
    pub defaults: SourceDefaults,
    pub sources: Vec<SourceEntry>,
}

/// 全局默认配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDefaults {
    pub max_items_per_source: usize,
    pub trust_weight: f32,
    pub request_timeout_secs: u64,
}

/// 单个源配置条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub source_type: SourceType,
    pub enabled: bool,
    #[serde(default)]
    pub description: String,
    #[serde(flatten)]
    pub config: SourceTypeConfig,
    #[serde(default)]
    pub max_items: Option<usize>,
    #[serde(default)]
    pub trust_weight: Option<f32>,
}

/// 源类型枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Rss,
    Arxiv,
    Hackernews,
    Reddit,
}

/// 各类型源的特定配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SourceTypeConfig {
    Rss(RssConfig),
    Arxiv(ArxivConfig),
    Hackernews(HackernewsConfig),
    Reddit(RedditConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssConfig {
    pub feeds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArxivConfig {
    pub categories: Vec<String>,
    #[serde(default)]
    pub query: String,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HackernewsConfig {
    #[serde(default = "default_min_score")]
    pub min_score: i64,
    #[serde(default)]
    pub min_comments: i64,
    #[serde(default)]
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedditConfig {
    pub subreddits: Vec<String>,
    #[serde(default = "default_min_upvotes")]
    pub min_upvotes: i64,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default = "default_time_range")]
    pub time_range: String,
}

fn default_min_score() -> i64 { 50 }
fn default_min_upvotes() -> i64 { 100 }
fn default_max_results() -> usize { 15 }
fn default_sort_by() -> String { "submittedDate".to_string() }
fn default_sort() -> String { "hot".to_string() }
fn default_time_range() -> String { "day".to_string() }
```

#### 3.1.3 源 CRUD 操作

```rust
impl SourceManager {
    /// 从配置文件加载源列表
    pub fn load_from_config(config_path: &Path) -> Result<Self, BrainError> { ... }

    /// 获取所有源配置
    pub fn list_sources(&self) -> Vec<SourceEntry> { ... }

    /// 获取指定源配置
    pub fn get_source(&self, name: &str) -> Option<&SourceEntry> { ... }

    /// 获取所有已启用的源
    pub fn enabled_sources(&self) -> Vec<&SourceEntry> { ... }

    /// 添加新源（同时持久化到配置文件）
    pub fn add_source(&mut self, entry: SourceEntry) -> Result<(), BrainError> {
        // 1. 校验 name 唯一性
        // 2. 校验配置合法性（如 feeds URL 格式）
        // 3. 添加到内存列表
        // 4. 回写到 radar_sources.toml
        ...
    }

    /// 删除源（同时持久化）
    pub fn remove_source(&mut self, name: &str) -> Result<bool, BrainError> {
        // 1. 从内存列表移除
        // 2. 回写到 radar_sources.toml
        // 3. 可选：清理该源的历史 radar_items
        ...
    }

    /// 启用/禁用源
    pub fn toggle_source(&mut self, name: &str, enabled: bool) -> Result<bool, BrainError> {
        // 1. 修改 enabled 字段
        // 2. 回写到 radar_sources.toml
        ...
    }

    /// 持久化当前配置到 TOML 文件
    fn persist_config(&self) -> Result<(), BrainError> {
        let toml_str = toml::to_string_pretty(&self.config)
            .map_err(|e| BrainError::ConfigError(format!("TOML序列化失败: {}", e)))?;
        fs::write(&self.config_path, toml_str)?;
        Ok(())
    }

    /// 获取源的有效可信度权重
    pub fn trust_weight(&self, source_name: &str) -> f32 {
        self.get_source(source_name)
            .and_then(|s| s.trust_weight)
            .unwrap_or(self.config.defaults.trust_weight)
    }

    /// 获取源的最大拉取条数
    pub fn max_items(&self, source_name: &str) -> usize {
        self.get_source(source_name)
            .and_then(|s| s.max_items)
            .unwrap_or(self.config.defaults.max_items_per_source)
    }
}
```

#### 3.1.4 源健康检查

```rust
/// 源健康状态
#[derive(Debug, Clone)]
pub struct SourceHealth {
    pub name: String,
    pub status: HealthStatus,
    pub last_fetch_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    pub total_items_fetched: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,     // 连续失败 1-2 次
    Unhealthy,    // 连续失败 3 次以上
    Disabled,
}

impl SourceManager {
    /// 对指定源执行健康检查（发送 HEAD 或轻量请求）
    pub async fn health_check(&self, name: &str) -> Result<SourceHealth, BrainError> { ... }

    /// 对所有源执行健康检查
    pub async fn health_check_all(&self) -> Vec<SourceHealth> { ... }

    /// 记录拉取成功
    pub fn record_fetch_success(&mut self, name: &str, items_count: usize) { ... }

    /// 记录拉取失败
    pub fn record_fetch_failure(&mut self, name: &str, error: &str) { ... }
}
```

---

### 3.2 内容抓取器 (Fetcher)

抓取器负责从各类型外部源拉取原始内容，返回统一的 `RawArticle` 结构。

#### 3.2.1 通用 Fetcher trait

```rust
/// 原始文章（抓取后的统一格式）
#[derive(Debug, Clone)]
pub struct RawArticle {
    pub title: String,
    pub summary: Option<String>,
    pub url: String,
    pub source_name: String,        // 来源名称（如 "arxiv-cs"）
    pub source_type: SourceType,
    pub author: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub content_html: Option<String>, // 原始 HTML（如果可用）
    pub extra: HashMap<String, serde_json::Value>, // 源特定的额外数据
}

/// Fetcher trait — 所有源类型的抓取器需实现此 trait
#[async_trait]
pub trait ArticleFetcher: Send + Sync {
    /// 执行一次抓取，返回原始文章列表
    async fn fetch(&self, source: &SourceEntry) -> Result<Vec<RawArticle>, BrainError>;

    /// 返回此 Fetcher 支持的源类型
    fn supported_type(&self) -> SourceType;
}
```

#### 3.2.2 RSS 抓取器

```rust
pub struct RssFetcher {
    http_client: reqwest::Client,
    request_timeout: Duration,
}

impl RssFetcher {
    pub fn new(timeout: Duration) -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(timeout)
                .user_agent("ObsidianBrain/0.1 (Knowledge Radar)")
                .build()
                .expect("HTTP client 构建失败"),
            request_timeout: timeout,
        }
    }

    /// 解析单个 RSS/Atom feed
    async fn fetch_feed(&self, feed_url: &str) -> Result<Vec<RawArticle>, BrainError> {
        let response = self.http_client.get(feed_url)
            .send()
            .await
            .map_err(|e| BrainError::FetchError {
                url: feed_url.to_string(),
                detail: format!("请求失败: {}", e),
            })?;

        let body = response.bytes().await
            .map_err(|e| BrainError::FetchError {
                url: feed_url.to_string(),
                detail: format!("读取响应失败: {}", e),
            })?;

        // 使用 feed-rs 解析 RSS 2.0 / Atom
        let feed = feed_rs::parser::parse(&body[..])
            .map_err(|e| BrainError::FetchError {
                url: feed_url.to_string(),
                detail: format!("Feed 解析失败: {}", e),
            })?;

        let articles = feed.entries.into_iter().map(|entry| {
            RawArticle {
                title: entry.title.map(|t| t.content).unwrap_or_default(),
                summary: entry.summary.map(|s| s.content),
                url: entry.links.into_iter()
                    .next()
                    .map(|l| l.href)
                    .unwrap_or_default(),
                source_name: String::new(), // 由调用者填充
                source_type: SourceType::Rss,
                author: entry.authors.into_iter().next().map(|a| a.name),
                published_at: entry.published.or(entry.updated),
                content_html: entry.content.and_then(|c| c.body),
                extra: HashMap::new(),
            }
        }).collect();

        Ok(articles)
    }
}

#[async_trait]
impl ArticleFetcher for RssFetcher {
    async fn fetch(&self, source: &SourceEntry) -> Result<Vec<RawArticle>, BrainError> {
        let config = match &source.config {
            SourceTypeConfig::Rss(c) => c,
            _ => return Err(BrainError::Internal("源类型不匹配".into())),
        };

        // 并行抓取所有 feed
        let futures: Vec<_> = config.feeds.iter().map(|url| {
            self.fetch_feed(url)
        }).collect();

        let results = futures::future::join_all(futures).await;

        let mut all_articles = Vec::new();
        for result in results {
            match result {
                Ok(articles) => all_articles.extend(articles),
                Err(e) => {
                    tracing::warn!("RSS feed 抓取失败: {}", e);
                    // 单个 feed 失败不影响其他
                }
            }
        }

        // 填充 source_name 并限制条数
        let max_items = source.max_items.unwrap_or(20);
        all_articles.truncate(max_items);
        for article in &mut all_articles {
            article.source_name = source.name.clone();
        }

        Ok(all_articles)
    }

    fn supported_type(&self) -> SourceType {
        SourceType::Rss
    }
}
```

#### 3.2.3 arXiv 抓取器

```rust
pub struct ArxivFetcher {
    http_client: reqwest::Client,
    request_timeout: Duration,
}

/// arXiv API 响应中的条目（XML 解析中间结构）
#[derive(Debug, Deserialize)]
struct ArxivEntry {
    id: String,           // "http://arxiv.org/abs/2401.12345v1"
    title: String,
    summary: String,
    published: String,    // ISO 8601
    updated: String,
    author: Vec<ArxivAuthor>,
    link: Vec<ArxivLink>,
    #[serde(rename = "category")]
    categories: Vec<ArxivCategory>,
}

#[derive(Debug, Deserialize)]
struct ArxivAuthor {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ArxivLink {
    #[serde(rename = "@href")]
    href: String,
    #[serde(rename = "@type")]
    link_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ArxivCategory {
    #[serde(rename = "@term")]
    term: String,
}

impl ArxivFetcher {
    pub fn new(timeout: Duration) -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .expect("HTTP client 构建失败"),
            request_timeout: timeout,
        }
    }

    /// 构建 arXiv API 查询 URL
    fn build_query_url(config: &ArxivConfig) -> String {
        let mut query_parts = Vec::new();

        // 分类过滤
        if !config.categories.is_empty() {
            let cat_query = config.categories.iter()
                .map(|c| format!("cat:{}", c))
                .collect::<Vec<_>>()
                .join(" OR ");
            query_parts.push(format!("({})", cat_query));
        }

        // 关键词查询
        if !config.query.is_empty() {
            query_parts.push(format!("(all:{})", config.query));
        }

        let search_query = query_parts.join(" AND ");
        let encoded = urlencoding::encode(&search_query);

        format!(
            "http://export.arxiv.org/api/query?search_query={}&sortBy={}&sortOrder=descending&max_results={}",
            encoded, config.sort_by, config.max_results
        )
    }

    /// 解析 arXiv Atom XML 响应
    fn parse_response(xml: &str) -> Result<Vec<RawArticle>, BrainError> {
        // 使用 quick-xml + serde 解析 Atom feed
        let feed: ArxivFeed = quick_xml::de::from_str(xml)
            .map_err(|e| BrainError::FetchError {
                url: "arxiv-api".into(),
                detail: format!("arXiv XML 解析失败: {}", e),
            })?;

        let articles = feed.entries.into_iter().map(|entry| {
            let arxiv_id = entry.id.rsplit('/').next().unwrap_or(&entry.id);
            let pdf_url = entry.link.iter()
                .find(|l| l.link_type.as_deref() == Some("application/pdf"))
                .map(|l| l.href.clone())
                .unwrap_or_else(|| format!("https://arxiv.org/pdf/{}", arxiv_id));

            let mut extra = HashMap::new();
            extra.insert("arxiv_id".into(), serde_json::Value::String(arxiv_id.to_string()));
            extra.insert("pdf_url".into(), serde_json::Value::String(pdf_url));
            extra.insert("categories".into(), serde_json::json!(
                entry.categories.iter().map(|c| &c.term).collect::<Vec<_>>()
            ));

            RawArticle {
                title: entry.title.replace('\n', " ").trim().to_string(),
                summary: Some(entry.summary.replace('\n', " ").trim().to_string()),
                url: entry.id,
                source_name: String::new(),
                source_type: SourceType::Arxiv,
                author: Some(entry.author.into_iter().map(|a| a.name).collect::<Vec<_>>().join(", ")),
                published_at: DateTime::parse_from_rfc3339(&entry.published)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc)),
                content_html: None,
                extra,
            }
        }).collect();

        Ok(articles)
    }
}

#[async_trait]
impl ArticleFetcher for ArxivFetcher {
    async fn fetch(&self, source: &SourceEntry) -> Result<Vec<RawArticle>, BrainError> {
        let config = match &source.config {
            SourceTypeConfig::Arxiv(c) => c,
            _ => return Err(BrainError::Internal("源类型不匹配".into())),
        };

        // arXiv API 礼貌使用：请求间至少间隔 3 秒
        let url = Self::build_query_url(config);
        let response = self.http_client.get(&url)
            .header("User-Agent", "ObsidianBrain/0.1 (Knowledge Radar; mailto:user@example.com)")
            .send()
            .await
            .map_err(|e| BrainError::FetchError {
                url: url.clone(),
                detail: format!("arXiv API 请求失败: {}", e),
            })?;

        let body = response.text().await
            .map_err(|e| BrainError::FetchError {
                url,
                detail: format!("读取响应失败: {}", e),
            })?;

        let mut articles = Self::parse_response(&body)?;
        for article in &mut articles {
            article.source_name = source.name.clone();
        }

        Ok(articles)
    }

    fn supported_type(&self) -> SourceType {
        SourceType::Arxiv
    }
}
```

#### 3.2.4 HackerNews 抓取器

```rust
pub struct HackernewsFetcher {
    http_client: reqwest::Client,
}

/// Algolia HN Search API 响应
#[derive(Debug, Deserialize)]
struct HnSearchResponse {
    hits: Vec<HnHit>,
    #[serde(rename = "nbHits")]
    total_hits: u64,
}

#[derive(Debug, Deserialize)]
struct HnHit {
    #[serde(rename = "objectID")]
    object_id: String,
    title: Option<String>,
    url: Option<String>,
    author: Option<String>,
    points: Option<i64>,
    #[serde(rename = "num_comments")]
    num_comments: Option<i64>,
    #[serde(rename = "created_at")]
    created_at: String,        // ISO 8601
    story_text: Option<String>, // 正文 HTML（自发帖）
}

impl HackernewsFetcher {
    pub fn new(timeout: Duration) -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .expect("HTTP client 构建失败"),
        }
    }

    /// 使用 Algolia HN Search API 获取高分文章
    async fn fetch_by_score(&self, config: &HackernewsConfig, max_items: usize)
        -> Result<Vec<RawArticle>, BrainError>
    {
        // 使用 search_by_date 端点，按时间排序，通过 numericFilters 过滤分数
        let url = if config.query.is_empty() {
            format!(
                "https://hn.algolia.com/api/v1/search?tags=story&numericFilters=points>{},num_comments>{}&hitsPerPage={}",
                config.min_score, config.min_comments, max_items
            )
        } else {
            format!(
                "https://hn.algolia.com/api/v1/search?query={}&tags=story&numericFilters=points>{},num_comments>{}&hitsPerPage={}",
                urlencoding::encode(&config.query),
                config.min_score, config.min_comments, max_items
            )
        };

        let response = self.http_client.get(&url).send().await
            .map_err(|e| BrainError::FetchError {
                url: url.clone(),
                detail: format!("HN API 请求失败: {}", e),
            })?;

        let search_result: HnSearchResponse = response.json().await
            .map_err(|e| BrainError::FetchError {
                url,
                detail: format!("HN API 响应解析失败: {}", e),
            })?;

        let articles = search_result.hits.into_iter().filter_map(|hit| {
            // 过滤掉没有外部链接的 Ask HN / Show HN（可选）
            let article_url = hit.url.unwrap_or_else(|| {
                format!("https://news.ycombinator.com/item?id={}", hit.object_id)
            });

            Some(RawArticle {
                title: hit.title?,
                summary: hit.story_text.clone(),
                url: article_url,
                source_name: String::new(),
                source_type: SourceType::Hackernews,
                author: hit.author,
                published_at: DateTime::parse_from_rfc3339(&hit.created_at)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc)),
                content_html: hit.story_text,
                extra: {
                    let mut map = HashMap::new();
                    if let Some(points) = hit.points {
                        map.insert("points".into(), serde_json::json!(points));
                    }
                    if let Some(comments) = hit.num_comments {
                        map.insert("num_comments".into(), serde_json::json!(comments));
                    }
                    map.insert("hn_id".into(), serde_json::Value::String(hit.object_id));
                    map
                },
            })
        }).collect();

        Ok(articles)
    }
}

#[async_trait]
impl ArticleFetcher for HackernewsFetcher {
    async fn fetch(&self, source: &SourceEntry) -> Result<Vec<RawArticle>, BrainError> {
        let config = match &source.config {
            SourceTypeConfig::Hackernews(c) => c,
            _ => return Err(BrainError::Internal("源类型不匹配".into())),
        };

        let max_items = source.max_items.unwrap_or(20);
        let mut articles = self.fetch_by_score(config, max_items).await?;

        for article in &mut articles {
            article.source_name = source.name.clone();
        }

        Ok(articles)
    }

    fn supported_type(&self) -> SourceType {
        SourceType::Hackernews
    }
}
```

#### 3.2.5 Reddit 抓取器

```rust
pub struct RedditFetcher {
    http_client: reqwest::Client,
}

/// Reddit JSON API 响应（Listing 格式）
#[derive(Debug, Deserialize)]
struct RedditListing {
    data: RedditListingData,
}

#[derive(Debug, Deserialize)]
struct RedditListingData {
    children: Vec<RedditPost>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RedditPost {
    data: RedditPostData,
}

#[derive(Debug, Deserialize)]
struct RedditPostData {
    title: String,
    url: String,
    selftext: Option<String>,      // 帖子正文（自发帖）
    selftext_html: Option<String>, // 帖子正文 HTML
    author: String,
    subreddit: String,
    score: i64,                    // upvotes
    #[serde(rename = "num_comments")]
    num_comments: i64,
    created_utc: f64,              // Unix timestamp
    permalink: String,
    #[serde(rename = "link_flair_text")]
    flair: Option<String>,
}

impl RedditFetcher {
    pub fn new(timeout: Duration) -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(timeout)
                .user_agent("ObsidianBrain/0.1 (by /u/ObsidianBrain)")
                .build()
                .expect("HTTP client 构建失败"),
        }
    }

    /// 抓取单个 subreddit
    async fn fetch_subreddit(&self, subreddit: &str, config: &RedditConfig, max_items: usize)
        -> Result<Vec<RawArticle>, BrainError>
    {
        let url = format!(
            "https://www.reddit.com/r/{}/{}.json?limit={}&t={}",
            subreddit, config.sort, max_items, config.time_range
        );

        let response = self.http_client.get(&url).send().await
            .map_err(|e| BrainError::FetchError {
                url: url.clone(),
                detail: format!("Reddit API 请求失败: {}", e),
            })?;

        let listing: RedditListing = response.json().await
            .map_err(|e| BrainError::FetchError {
                url,
                detail: format!("Reddit API 响应解析失败: {}", e),
            })?;

        let articles = listing.data.children.into_iter()
            .filter(|post| post.data.score >= config.min_upvotes)
            .map(|post| {
                let post_url = if post.data.url.starts_with("https://www.reddit.com") {
                    format!("https://www.reddit.com{}", post.data.permalink)
                } else {
                    post.data.url.clone()
                };

                let summary = post.data.selftext.clone()
                    .filter(|t| !t.is_empty())
                    .map(|t| if t.len() > 500 { format!("{}...", &t[..500]) } else { t });

                let mut extra = HashMap::new();
                extra.insert("subreddit".into(), serde_json::Value::String(post.data.subreddit.clone()));
                extra.insert("upvotes".into(), serde_json::json!(post.data.score));
                extra.insert("num_comments".into(), serde_json::json!(post.data.num_comments));
                if let Some(flair) = &post.data.flair {
                    extra.insert("flair".into(), serde_json::Value::String(flair.clone()));
                }

                let published_at = DateTime::from_timestamp(post.data.created_utc as i64, 0);

                RawArticle {
                    title: post.data.title,
                    summary,
                    url: post_url,
                    source_name: String::new(),
                    source_type: SourceType::Reddit,
                    author: Some(post.data.author),
                    published_at,
                    content_html: post.data.selftext_html,
                    extra,
                }
            })
            .collect();

        Ok(articles)
    }
}

#[async_trait]
impl ArticleFetcher for RedditFetcher {
    async fn fetch(&self, source: &SourceEntry) -> Result<Vec<RawArticle>, BrainError> {
        let config = match &source.config {
            SourceTypeConfig::Reddit(c) => c,
            _ => return Err(BrainError::Internal("源类型不匹配".into())),
        };

        let max_items = source.max_items.unwrap_or(15);

        // 并行抓取所有 subreddit
        let futures: Vec<_> = config.subreddits.iter().map(|sub| {
            self.fetch_subreddit(sub, config, max_items)
        }).collect();

        let results = futures::future::join_all(futures).await;

        let mut all_articles = Vec::new();
        for result in results {
            match result {
                Ok(articles) => all_articles.extend(articles),
                Err(e) => {
                    tracing::warn!("Reddit subreddit 抓取失败: {}", e);
                }
            }
        }

        // 按 upvotes 降序排序，截取 max_items
        all_articles.sort_by(|a, b| {
            let score_a = a.extra.get("upvotes").and_then(|v| v.as_i64()).unwrap_or(0);
            let score_b = b.extra.get("upvotes").and_then(|v| v.as_i64()).unwrap_or(0);
            score_b.cmp(&score_a)
        });
        all_articles.truncate(max_items);

        for article in &mut all_articles {
            article.source_name = source.name.clone();
        }

        Ok(all_articles)
    }

    fn supported_type(&self) -> SourceType {
        SourceType::Reddit
    }
}
```

#### 3.2.6 抓取调度器

```rust
use tokio_cron_scheduler::{JobScheduler, Job};

pub struct RadarScheduler {
    scheduler: JobScheduler,
}

impl RadarScheduler {
    pub async fn new() -> Result<Self, BrainError> {
        let scheduler = JobScheduler::new().await
            .map_err(|e| BrainError::Internal(format!("调度器初始化失败: {}", e)))?;
        Ok(Self { scheduler })
    }

    /// 注册定时拉取任务
    pub async fn register_fetch_job(
        &self,
        interval_hours: u32,
        radar_service: Arc<RadarService>,
    ) -> Result<(), BrainError> {
        // 将小时转为 cron 表达式：每 N 小时执行一次
        // 例如每 6 小时 = "0 0 */6 * * *"
        let cron_expr = format!("0 0 */{} * * *", interval_hours);

        let job = Job::new_async(cron_expr.as_str(), move |_uuid, _lock| {
            let service = radar_service.clone();
            Box::pin(async move {
                tracing::info!("定时拉取开始");
                match service.fetch_now().await {
                    Ok(report) => {
                        tracing::info!(
                            "定时拉取完成: 成功 {} 源, 新增 {} 篇, 失败 {} 源",
                            report.successful_sources,
                            report.new_items,
                            report.failed_sources,
                        );
                    }
                    Err(e) => {
                        tracing::error!("定时拉取失败: {}", e);
                    }
                }
            })
        }).map_err(|e| BrainError::Internal(format!("定时任务创建失败: {}", e)))?;

        self.scheduler.add(job).await
            .map_err(|e| BrainError::Internal(format!("定时任务添加失败: {}", e)))?;

        Ok(())
    }

    /// 启动调度器
    pub async fn start(&self) -> Result<(), BrainError> {
        self.scheduler.start().await
            .map_err(|e| BrainError::Internal(format!("调度器启动失败: {}", e)))?;
        Ok(())
    }

    /// 停止调度器
    pub async fn shutdown(&self) -> Result<(), BrainError> {
        self.scheduler.shutdown().await
            .map_err(|e| BrainError::Internal(format!("调度器停止失败: {}", e)))?;
        Ok(())
    }
}
```

---

### 3.3 内容处理器 (Processor)

对抓取到的原始文章进行清洗、去重和标准化处理。

```rust
pub struct Processor {
    sqlite_store: Arc<SqliteStore>,
    embedding_client: Arc<EmbeddingClient>,
}

impl Processor {
    pub fn new(sqlite_store: Arc<SqliteStore>, embedding_client: Arc<EmbeddingClient>) -> Self {
        Self { sqlite_store, embedding_client }
    }

    /// 处理一批原始文章：清洗 → 去重 → 标准化
    pub async fn process(&self, articles: Vec<RawArticle>) -> Result<Vec<ProcessedArticle>, BrainError> {
        let mut processed = Vec::new();

        for article in articles {
            // 1. URL 去重：检查是否已存在
            if self.sqlite_store.radar_item_exists_by_url(&article.url).await? {
                tracing::debug!("URL 已存在，跳过: {}", article.url);
                continue;
            }

            // 2. 标题清洗
            let clean_title = self.clean_title(&article.title);

            // 3. 摘要清洗/生成
            let summary = self.extract_or_generate_summary(&article);

            // 4. 构建处理后的文章
            processed.push(ProcessedArticle {
                title: clean_title,
                summary,
                url: article.url,
                source_name: article.source_name,
                source_type: article.source_type,
                author: article.author,
                published_at: article.published_at,
                content_html: article.content_html,
                extra: article.extra,
            });
        }

        // 5. 标题去重：编辑距离 < 3 的合并
        processed = self.deduplicate_by_title(processed);

        Ok(processed)
    }

    /// 清洗标题：去除多余空白、HTML 实体、特殊字符
    fn clean_title(&self, title: &str) -> String {
        let decoded = html_escape::decode_html_entities(title);
        let cleaned = decoded
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        // 合并连续空白
        let re = regex::Regex::new(r"\s+").unwrap();
        re.replace_all(&cleaned, " ").trim().to_string()
    }

    /// 提取或生成摘要
    fn extract_or_generate_summary(&self, article: &RawArticle) -> String {
        if let Some(ref summary) = article.summary {
            let cleaned = self.strip_html(summary);
            if cleaned.len() > 300 {
                format!("{}...", &cleaned[..300])
            } else {
                cleaned
            }
        } else if let Some(ref html) = article.content_html {
            let text = self.strip_html(html);
            if text.len() > 300 {
                format!("{}...", &text[..300])
            } else {
                text
            }
        } else {
            "暂无摘要".to_string()
        }
    }

    /// 去除 HTML 标签
    fn strip_html(&self, html: &str) -> String {
        let re = regex::Regex::new(r"<[^>]+>").unwrap();
        re.replace_all(html, "").to_string()
    }

    /// 基于标题编辑距离去重
    fn deduplicate_by_title(&self, articles: Vec<ProcessedArticle>) -> Vec<ProcessedArticle> {
        let mut seen = Vec::new();
        let mut result = Vec::new();

        for article in articles {
            let is_duplicate = seen.iter().any(|existing: &String| {
                edit_distance(&article.title, existing) < 3
            });

            if !is_duplicate {
                seen.push(article.title.clone());
                result.push(article);
            }
        }

        result
    }
}

/// 处理后的文章（去重 + 清洗后）
#[derive(Debug, Clone)]
pub struct ProcessedArticle {
    pub title: String,
    pub summary: String,
    pub url: String,
    pub source_name: String,
    pub source_type: SourceType,
    pub author: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub content_html: Option<String>,
    pub extra: HashMap<String, serde_json::Value>,
}
```

---

### 3.4 相关性引擎 (RelevanceEngine)

核心的个性化推荐引擎，负责构建用户兴趣向量、计算文章相关性并执行多因子排序。

#### 3.4.1 用户兴趣向量构建算法

```rust
pub struct RelevanceEngine {
    embedding_client: Arc<EmbeddingClient>,
    qdrant_client: Arc<QdrantClient>,
    sqlite_store: Arc<SqliteStore>,
    config: RelevanceConfig,
}

#[derive(Debug, Clone)]
pub struct RelevanceConfig {
    pub relevance_threshold: f32,     // 默认 0.7
    pub interest_window_days: u32,    // 默认 30
    pub content_dedup_threshold: f32, // 默认 0.92
    // 多因子排序权重
    pub alpha: f32,  // 语义相似度权重，默认 0.6
    pub beta: f32,   // 来源可信度权重，默认 0.15
    pub gamma: f32,  // 时效性权重，默认 0.15
    pub delta: f32,  // 内容重复度降权，默认 0.1
    // 时间衰减参数
    pub time_decay_halflife_days: f32, // 时间衰减半衰期（天），默认 7
}

impl Default for RelevanceConfig {
    fn default() -> Self {
        Self {
            relevance_threshold: 0.7,
            interest_window_days: 30,
            content_dedup_threshold: 0.92,
            alpha: 0.6,
            beta: 0.15,
            gamma: 0.15,
            delta: 0.1,
            time_decay_halflife_days: 7.0,
        }
    }
}
```

**用户兴趣向量构建算法**：

```
输入：最近 N 天活跃笔记集合 Notes = {n₁, n₂, ..., nₖ}
输出：用户兴趣向量 V_user (维度 = embedding 维度，如 1536)

算法步骤：

1. 筛选活跃笔记：
   选取 updated_at 在最近 interest_window_days 天内的笔记

2. 对每篇笔记 nᵢ 计算权重 wᵢ：

   wᵢ = w_time(i) × w_access(i) × w_importance(i)

   其中：
   - 时间衰减权重：
     w_time(i) = exp(-λ × Δt_i)
     λ = ln(2) / time_decay_halflife_days
     Δt_i = (now - nᵢ.updated_at).days()   // 距离现在的天数

   - 访问频次权重：
     w_access(i) = 1.0 + log₂(1 + nᵢ.access_count)

   - 重要度权重：
     w_importance(i) = 0.5 + nᵢ.importance   // importance ∈ [0, 1]，故权重 ∈ [0.5, 1.5]

3. 归一化权重：
   w̃ᵢ = wᵢ / Σⱼ wⱼ

4. 计算加权平均向量：
   V_user = Σᵢ (w̃ᵢ × embedding(nᵢ))

5. 归一化 V_user 为单位向量：
   V_user = V_user / ‖V_user‖₂
```

```rust
impl RelevanceEngine {
    /// 构建用户兴趣向量
    pub async fn build_user_interest_vector(&self) -> Result<Vec<f32>, BrainError> {
        let now = Utc::now();
        let window_start = now - Duration::days(self.config.interest_window_days as i64);

        // 从记忆引擎获取最近 30 天活跃笔记的 embedding 和元数据
        let active_memories = self.qdrant_client
            .scroll_with_filter(
                "obsidian_brain",
                QdrantFilter::gte("updated_at", window_start.to_rfc3339()),
                None, // 不设 limit，获取所有活跃笔记
            )
            .await?;

        if active_memories.is_empty() {
            tracing::warn!("无活跃笔记，返回零向量作为默认兴趣向量");
            return Ok(vec![0.0; 1536]); // 降级：返回零向量
        }

        let lambda = (2.0_f32).ln() / self.config.time_decay_halflife_days;

        let mut weighted_sum = vec![0.0f32; 1536];
        let mut total_weight = 0.0f32;

        for memory in &active_memories {
            // 时间衰减
            let updated_at = DateTime::parse_from_rfc3339(
                memory.payload.get("updated_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
            ).unwrap_or_else(|_| now.into());

            let delta_days = (now - updated_at).num_days().max(0) as f32;
            let w_time = (-lambda * delta_days).exp();

            // 访问频次
            let access_count = memory.payload.get("access_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as f32;
            let w_access = 1.0 + (1.0 + access_count).log2();

            // 重要度
            let importance = memory.payload.get("importance")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5) as f32;
            let w_importance = 0.5 + importance;

            // 综合权重
            let weight = w_time * w_access * w_importance;

            // 加权累加
            for (i, &val) in memory.vector.iter().enumerate() {
                weighted_sum[i] += weight * val;
            }
            total_weight += weight;
        }

        // 归一化
        if total_weight > 0.0 {
            for val in &mut weighted_sum {
                *val /= total_weight;
            }
        }

        // L2 归一化为单位向量
        let norm: f32 = weighted_sum.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut weighted_sum {
                *val /= norm;
            }
        }

        Ok(weighted_sum)
    }

    /// 计算两个向量的余弦相似度
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }

    /// 对一批文章计算相关性得分并排序
    pub async fn rank_articles(
        &self,
        articles: &[ScoredArticle],
        user_interest: &[f32],
        source_manager: &SourceManager,
    ) -> Result<Vec<RankedArticle>, BrainError> {
        let now = Utc::now();
        let mut ranked = Vec::new();

        // 获取已有笔记的 embedding（用于内容去重）
        let existing_embeddings = self.get_recent_note_embeddings().await?;

        for article in articles {
            // 1. 计算语义相似度
            let similarity = Self::cosine_similarity(&article.embedding, user_interest);

            // 2. 阈值过滤
            if similarity < self.config.relevance_threshold {
                continue;
            }

            // 3. 检查已读/已忽略状态
            let status = self.sqlite_store.get_radar_item_status_by_url(&article.url).await?;
            match status {
                Some(RadarStatus::Read) | Some(RadarStatus::Saved) | Some(RadarStatus::Dismissed) => {
                    continue;
                }
                _ => {}
            }

            // 4. 计算来源可信度
            let trust = source_manager.trust_weight(&article.source_name);

            // 5. 计算时效性得分
            let recency = self.calculate_recency_score(article.published_at, now);

            // 6. 计算内容重复度
            let duplication = self.calculate_duplication_score(&article.embedding, &existing_embeddings);

            // 7. 多因子排序得分
            let final_score = self.config.alpha * similarity
                + self.config.beta * trust
                + self.config.gamma * recency
                - self.config.delta * duplication;

            ranked.push(RankedArticle {
                article: article.clone(),
                relevance_score: final_score,
                similarity,
                trust_score: trust,
                recency_score: recency,
                duplication_score: duplication,
            });
        }

        // 按最终得分降序排序
        ranked.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());

        Ok(ranked)
    }

    /// 时效性得分：线性衰减，30 天内的文章从 1.0 衰减到 0.0
    fn calculate_recency_score(&self, published_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> f32 {
        match published_at {
            Some(pub_time) => {
                let days_ago = (now - pub_time).num_days().max(0) as f32;
                let max_days = 30.0;
                (1.0 - days_ago / max_days).max(0.0)
            }
            None => 0.5, // 无发布时间信息，给中间值
        }
    }

    /// 内容重复度：与已有笔记 embedding 的最大相似度
    fn calculate_duplication_score(
        &self,
        article_embedding: &[f32],
        existing_embeddings: &[Vec<f32>],
    ) -> f32 {
        if existing_embeddings.is_empty() {
            return 0.0;
        }

        let max_similarity = existing_embeddings.iter()
            .map(|emb| Self::cosine_similarity(article_embedding, emb))
            .fold(0.0f32, f32::max);

        // 超过去重阈值才返回实际相似度，否则返回 0（不降权）
        if max_similarity > self.config.content_dedup_threshold {
            max_similarity
        } else {
            0.0
        }
    }

    /// 获取最近笔记的 embedding 向量（用于去重计算）
    async fn get_recent_note_embeddings(&self) -> Result<Vec<Vec<f32>>, BrainError> {
        // 从 Qdrant 获取最近 30 天笔记的 embedding，限制 100 条避免开销过大
        let results = self.qdrant_client
            .scroll_with_filter(
                "obsidian_brain",
                QdrantFilter::gte("updated_at",
                    (Utc::now() - Duration::days(30)).to_rfc3339()),
                Some(100),
            )
            .await?;

        Ok(results.into_iter().map(|r| r.vector).collect())
    }
}
```

#### 3.4.2 多因子排序公式

```
最终得分 S = α × sim(V_article, V_user) + β × trust(source) + γ × recency(t) - δ × dup(V_article, E_existing)

其中：
  sim(V_article, V_user) = cosine_similarity(article_embedding, user_interest_vector)
                          ∈ [0, 1]

  trust(source)           = source_manager.trust_weight(source_name)
                          ∈ [0, 1], 默认 0.8

  recency(t)              = max(0, 1 - days_since_published / 30)
                          ∈ [0, 1]

  dup(V_article, E)       = max_{e ∈ E} cosine_similarity(V_article, e)  (仅当 > 0.92 时非零)
                          ∈ {0} ∪ (0.92, 1.0]

默认权重：α=0.6, β=0.15, γ=0.15, δ=0.1

得分范围：约 [-0.1, 1.0]
  - 最高：1.0 × 0.6 + 1.0 × 0.15 + 1.0 × 0.15 - 0 = 0.9
  - 最低：0 × 0.6 + 0 × 0.15 + 0 × 0.15 - 1.0 × 0.1 = -0.1
```

#### 3.4.3 过滤链完整流程

```
新文章列表
    │
    ▼
[1] URL 去重（SQLite 中已存在的 URL 跳过）
    │
    ▼
[2] Embedding 生成（复用 infra/embedding.rs，批量调用）
    │
    ▼
[3] 阈值过滤（cosine_similarity < 0.7 → 丢弃）
    │
    ▼
[4] 状态过滤（status ∈ {Read, Saved, Dismissed} → 丢弃）
    │
    ▼
[5] 内容去重（与已有笔记 cosine > 0.92 → 降权 50%）
    │
    ▼
[6] 多因子排序
    │
    ▼
[7] 取 top-N 返回
```

---

### 3.5 纳藏器 (VaultSaver)

负责将雷达文章保存到 Obsidian vault 并触发后续流程。

```rust
pub struct VaultSaver {
    vault_path: PathBuf,
    sqlite_store: Arc<SqliteStore>,
    llm_client: Arc<LlmClient>,
    embedding_client: Arc<EmbeddingClient>,
    http_client: reqwest::Client,
}

/// 纳藏结果
#[derive(Debug, Clone, Serialize)]
pub struct VaultSaveResult {
    pub note_path: String,          // vault 内相对路径
    pub obsidian_uri: String,       // Obsidian URI
    pub summary: String,            // LLM 生成的摘要
    pub tags: Vec<String>,          // 自动提取的标签
    pub related_notes: Vec<String>, // 关联笔记路径
    pub word_count: usize,          // 正文字数
}
```

#### 3.5.1 正文提取

```rust
impl VaultSaver {
    /// 从 URL 提取文章正文
    async fn extract_article_content(&self, url: &str, html: Option<&str>)
        -> Result<String, BrainError>
    {
        // 如果已有 HTML 内容，直接使用 readability 提取
        if let Some(html_content) = html {
            return self.readability_extract(html_content, url);
        }

        // 否则从 URL 抓取页面内容
        let response = self.http_client.get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; ObsidianBrain/0.1)")
            .send()
            .await
            .map_err(|e| BrainError::FetchError {
                url: url.to_string(),
                detail: format!("页面抓取失败: {}", e),
            })?;

        let html_content = response.text().await
            .map_err(|e| BrainError::FetchError {
                url: url.to_string(),
                detail: format!("读取页面内容失败: {}", e),
            })?;

        self.readability_extract(&html_content, url)
    }

    /// Readability 正文提取
    fn readability_extract(&self, html: &str, url: &str) -> Result<String, BrainError> {
        // 使用 readability-rs (readable-readability crate)
        let product = readability::extractor::extract(html, url)
            .map_err(|e| BrainError::Internal(format!("Readability 提取失败: {}", e)))?;

        // product.content 是清洗后的 HTML
        // product.title 是提取的标题
        // 转换为 Markdown
        let markdown = html2md::parse_html(&product.content);

        Ok(markdown)
    }
}
```

#### 3.5.2 Obsidian 笔记模板

```rust
impl VaultSaver {
    /// 生成 Obsidian Markdown 笔记内容
    async fn generate_note_content(
        &self,
        article: &RadarItem,
        body_markdown: &str,
    ) -> Result<NoteContent, BrainError> {
        // 1. 调用 LLM 生成中文摘要
        let summary = self.generate_summary(article, body_markdown).await?;

        // 2. 调用 LLM 提取标签
        let tags = self.extract_tags(article, body_markdown).await?;

        // 3. 搜索相关笔记
        let related_notes = self.find_related_notes(article).await?;

        // 4. 组装笔记内容
        let content = format!(
            r#"---
title: "{title}"
source: "{source}"
source_type: "{source_type}"
url: "{url}"
author: "{author}"
date_fetched: {date_fetched}
date_published: {date_published}
relevance_score: {relevance_score}
tags:
{tags_yaml}
status: saved
---

# {title}

## 📋 摘要

{summary}

## 📄 正文

{body}

---

## 🔗 相关链接

- **原文**: [{url}]({url})
- **来源**: {source}
{author_line}

## 📌 相关笔记

{related_notes_links}
"#,
            title = article.title,
            source = article.source_name,
            source_type = format!("{:?}", article.source_type).to_lowercase(),
            url = article.url,
            author = article.author.as_deref().unwrap_or("未知"),
            date_fetched = Utc::now().format("%Y-%m-%d %H:%M"),
            date_published = article.published_at
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "未知".to_string()),
            relevance_score = format!("{:.2}", article.relevance_score),
            tags_yaml = tags.iter().map(|t| format!("  - {}", t)).collect::<Vec<_>>().join("\n"),
            summary = summary,
            body = body_markdown,
            author_line = article.author.as_ref()
                .map(|a| format!("- **作者**: {}", a))
                .unwrap_or_default(),
            related_notes_links = if related_notes.is_empty() {
                "暂无相关笔记".to_string()
            } else {
                related_notes.iter()
                    .map(|p| format!("- [[{}]]", p))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
        );

        Ok(NoteContent {
            content,
            summary,
            tags,
            related_notes,
        })
    }

    /// 调用 LLM 生成中文摘要
    async fn generate_summary(&self, article: &RadarItem, body: &str) -> Result<String, BrainError> {
        let prompt = format!(
            "请用中文为以下文章生成一段简洁的摘要（150-250字）：\n\n标题：{}\n来源：{}\n\n正文（截取前2000字）：\n{}",
            article.title,
            article.source_name,
            &body[..body.len().min(2000)]
        );

        self.llm_client.complete(&prompt, LlmParams {
            max_tokens: 500,
            temperature: 0.3,
            ..Default::default()
        }).await
    }

    /// 调用 LLM 提取标签
    async fn extract_tags(&self, article: &RadarItem, body: &str) -> Result<Vec<String>, BrainError> {
        let prompt = format!(
            "请从以下文章中提取 3-5 个关键词标签，用 JSON 数组格式返回：\n\n标题：{}\n摘要：{}",
            article.title, article.summary
        );

        let response = self.llm_client.complete(&prompt, LlmParams {
            max_tokens: 200,
            temperature: 0.1,
            ..Default::default()
        }).await?;

        // 解析 JSON 数组
        serde_json::from_str::<Vec<String>>(&response)
            .or_else(|_| {
                // 降级：尝试从响应中提取引号内的词
                let tags: Vec<String> = response.matches(r#""([^"]+)""#)
                    .map(|m| m.to_string())
                    .collect();
                if tags.is_empty() {
                    Ok(vec!["radar".to_string()])
                } else {
                    Ok(tags)
                }
            })
    }

    /// 搜索与新文章语义相近的已有笔记
    async fn find_related_notes(&self, article: &RadarItem) -> Result<Vec<String>, BrainError> {
        let results = self.qdrant_client
            .search(
                "obsidian_brain",
                &article.embedding,
                3, // top 3
            )
            .await?;

        Ok(results.into_iter()
            .filter_map(|r| r.payload.get("note_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()))
            .collect())
    }
}
```

#### 3.5.3 纳藏完整流程

```rust
impl VaultSaver {
    /// 将文章纳藏到 vault
    pub async fn save_to_vault(
        &self,
        article: &RadarItem,
        target_dir: Option<&str>,
    ) -> Result<VaultSaveResult, BrainError> {
        let target_dir = target_dir.unwrap_or("radar");

        // 1. 提取正文
        let body_markdown = self.extract_article_content(
            &article.url,
            article.content_html.as_deref(),
        ).await?;

        // 2. 生成笔记内容
        let note_content = self.generate_note_content(article, &body_markdown).await?;

        // 3. 生成文件名
        let date_prefix = Utc::now().format("%Y-%m-%d");
        let slug = slugify(&article.title, 60); // 将标题转为文件名安全格式
        let filename = format!("{}-{}.md", date_prefix, slug);

        // 4. 确保目标目录存在
        let target_path = self.vault_path.join(target_dir);
        tokio::fs::create_dir_all(&target_path).await?;

        // 5. 写入文件
        let file_path = target_path.join(&filename);
        tokio::fs::write(&file_path, &note_content.content).await?;

        // 6. 计算 vault 内相对路径
        let relative_path = format!("{}/{}", target_dir, filename);

        // 7. 生成 Obsidian URI
        let obsidian_uri = format!(
            "obsidian://open?vault={}&file={}",
            "brain", // 从配置获取 vault name
            urlencoding::encode(&relative_path)
        );

        // 8. 更新状态为 Saved
        self.sqlite_store.update_radar_item_status(
            article.id,
            RadarStatus::Saved,
        ).await?;

        // 9. 触发记忆系统索引（异步，不等待完成）
        let vault_path = self.vault_path.clone();
        let relative_path_clone = relative_path.clone();
        tokio::spawn(async move {
            // 通知记忆引擎对新笔记进行索引
            // 通过事件总线或直接调用 MemoryService
            tracing::info!("触发记忆系统索引: {}", relative_path_clone);
        });

        // 10. 记录时间线事件
        // 通过事件总线发送 RadarSaved 事件
        tracing::info!(
            event_type = "RadarSaved",
            title = %article.title,
            path = %relative_path,
            "文章纳藏完成"
        );

        let word_count = body_markdown.chars().count();

        Ok(VaultSaveResult {
            note_path: relative_path,
            obsidian_uri,
            summary: note_content.summary,
            tags: note_content.tags,
            related_notes: note_content.related_notes,
            word_count,
        })
    }
}

/// 将字符串转为文件名安全的 slug
fn slugify(s: &str, max_len: usize) -> String {
    let slug: String = s
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() { c } else { '-' }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if slug.len() > max_len {
        slug[..max_len].trim_end_matches('-').to_string()
    } else {
        slug
    }
}
```

#### 3.5.4 纳藏后生成的 Markdown 笔记示例

```markdown
---
title: "Retrieval-Augmented Generation for Large Language Models: A Survey"
source: "arxiv-cs"
source_type: "arxiv"
url: "https://arxiv.org/abs/2401.12345"
author: "Zhang, Li, Wang et al."
date_fetched: 2026-05-29 14:30
date_published: 2026-05-25
relevance_score: 0.89
tags:
  - RAG
  - LLM
  - 检索增强生成
  - 知识检索
  - NLP
status: saved
---

# Retrieval-Augmented Generation for Large Language Models: A Survey

## 📋 摘要

本文系统综述了检索增强生成（RAG）技术在大语言模型中的应用与发展。RAG 通过在生成过程中引入外部知识检索，有效缓解了 LLM 的幻觉问题和知识过时问题。文章从检索策略、生成架构、训练方法三个维度对现有工作进行了分类梳理，并讨论了 RAG 在企业级应用中的实践经验和未来研究方向。

## 📄 正文

## 1. Introduction

Large Language Models (LLMs) have demonstrated remarkable capabilities...

[正文内容，由 readability 提取并转为 Markdown]

## 2. Related Work

### 2.1 Retrieval-Augmented Generation

The concept of augmenting generation with retrieval was first proposed by...

[更多正文内容]

---

## 🔗 相关链接

- **原文**: [https://arxiv.org/abs/2401.12345](https://arxiv.org/abs/2401.12345)
- **来源**: arxiv-cs
- **作者**: Zhang, Li, Wang et al.

## 📌 相关笔记

- [[ai/rag-notes.md]]
- [[ai/llm-architecture.md]]
- [[nlp/embedding-techniques.md]]
```

---

### 3.6 状态管理器 (StateManager)

基于 SQLite 的雷达条目状态管理。

#### 3.6.1 SQLite 表结构（扩展）

```sql
-- 雷达条目主表（在顶层设计文档基础上扩展）
CREATE TABLE IF NOT EXISTS radar_items (
    id              TEXT PRIMARY KEY,          -- UUID
    title           TEXT NOT NULL,
    summary         TEXT,
    source          TEXT NOT NULL,             -- 源名称（如 "arxiv-cs"）
    source_type     TEXT NOT NULL,             -- 源类型（rss/arxiv/hackernews/reddit）
    url             TEXT NOT NULL UNIQUE,
    embedding_id    TEXT,                      -- Qdrant 中的向量 ID
    status          TEXT NOT NULL DEFAULT 'new', -- new/read/saved/dismissed
    relevance_score REAL,                      -- 最终排序得分
    similarity_score REAL,                     -- 语义相似度原始分
    author          TEXT,
    content_html    TEXT,                      -- 原始 HTML（可选，用于纳藏时正文提取）
    extra_data      JSON,                      -- 源特定的额外数据
    related_notes   JSON,                      -- 关联笔记路径列表
    fetched_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    published_at    DATETIME,
    read_at         DATETIME,                  -- 首次查看时间
    saved_at        DATETIME,                  -- 纳藏时间
    dismissed_at    DATETIME,                  -- 忽略时间
    saved_path      TEXT                       -- 纳藏后的 vault 路径
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_radar_items_status ON radar_items(status);
CREATE INDEX IF NOT EXISTS idx_radar_items_source ON radar_items(source);
CREATE INDEX IF NOT EXISTS idx_radar_items_fetched_at ON radar_items(fetched_at DESC);
CREATE INDEX IF NOT EXISTS idx_radar_items_relevance ON radar_items(relevance_score DESC);
CREATE INDEX IF NOT EXISTS idx_radar_items_url ON radar_items(url);
```

#### 3.6.2 状态管理器实现

```rust
pub struct StateManager {
    sqlite_store: Arc<SqliteStore>,
}

impl StateManager {
    pub fn new(sqlite_store: Arc<SqliteStore>) -> Self {
        Self { sqlite_store }
    }

    /// 批量插入新的雷达条目
    pub async fn insert_articles(&self, articles: &[RankedArticle]) -> Result<usize, BrainError> {
        let mut inserted = 0;
        for article in articles {
            match self.sqlite_store.insert_radar_item(article).await {
                Ok(_) => inserted += 1,
                Err(BrainError::SqliteUniqueViolation) => {
                    tracing::debug!("跳过已存在的文章: {}", article.url);
                }
                Err(e) => {
                    tracing::warn!("插入雷达条目失败: {}", e);
                }
            }
        }
        Ok(inserted)
    }

    /// 获取推荐列表（status = 'new'，按 relevance_score 降序）
    pub async fn get_recommendations(&self, limit: usize) -> Result<Vec<RadarItem>, BrainError> {
        self.sqlite_store.query_radar_items(
            "SELECT * FROM radar_items WHERE status = 'new' ORDER BY relevance_score DESC LIMIT ?1",
            &[&limit.to_string()],
        ).await
    }

    /// 更新条目状态
    pub async fn update_status(
        &self,
        id: Uuid,
        new_status: RadarStatus,
    ) -> Result<bool, BrainError> {
        let timestamp_field = match new_status {
            RadarStatus::Read => "read_at",
            RadarStatus::Saved => "saved_at",
            RadarStatus::Dismissed => "dismissed_at",
            RadarStatus::New => return Err(BrainError::Internal("不能手动设为 New".into())),
        };

        let sql = format!(
            "UPDATE radar_items SET status = ?1, {} = datetime('now') WHERE id = ?2",
            timestamp_field
        );

        self.sqlite_store.execute(&sql, &[
            &new_status.to_string(),
            &id.to_string(),
        ]).await
    }

    /// 标记条目为已读（批量）
    pub async fn mark_as_read(&self, ids: &[Uuid]) -> Result<usize, BrainError> {
        let id_list = ids.iter().map(|id| format!("'{}'", id)).collect::<Vec<_>>().join(",");
        let sql = format!(
            "UPDATE radar_items SET status = 'read', read_at = datetime('now') WHERE id IN ({}) AND status = 'new'",
            id_list
        );
        self.sqlite_store.execute_raw(&sql).await
    }

    /// 清理旧条目（保留 Saved 状态的，清理超过 1000 条的旧条目）
    pub async fn cleanup_old_items(&self, max_items: usize) -> Result<usize, BrainError> {
        let sql = "
            DELETE FROM radar_items
            WHERE status IN ('new', 'read')
            AND id NOT IN (
                SELECT id FROM radar_items
                ORDER BY
                    CASE status WHEN 'saved' THEN 0 ELSE 1 END,
                    fetched_at DESC
                LIMIT ?1
            )
        ";
        self.sqlite_store.execute(sql, &[&max_items.to_string()]).await
    }

    /// 按 query 过滤（语义搜索）
    pub async fn search_in_radar(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<RadarItem>, BrainError> {
        // 通过 Qdrant 在雷达条目的 embedding 中搜索
        // 然后与 SQLite 中的 status 过滤结合
        let qdrant_results = self.qdrant_client
            .search_with_filter(
                "obsidian_brain",
                query_embedding,
                limit,
                QdrantFilter::eq("item_type", "radar"),
            )
            .await?;

        // 从 SQLite 获取完整信息
        let ids: Vec<String> = qdrant_results.iter()
            .filter_map(|r| r.payload.get("radar_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()))
            .collect();

        self.sqlite_store.get_radar_items_by_ids(&ids).await
    }
}
```

---

## 4. 数据流图

### 4.1 定时拉取流程

```
tokio-cron-scheduler (每 6 小时触发)
    │
    ▼
RadarService::fetch_now()
    │
    ├─→ [1] RelevanceEngine::build_user_interest_vector()
    │       └─→ Qdrant: 获取最近 30 天活跃笔记 embedding
    │       └─→ 计算加权平均 → V_user
    │
    ├─→ [2] 对每个已启用的源并行抓取：
    │       │
    │       ├─→ RssFetcher::fetch(source)
    │       │       └─→ 并行请求各 feed URL → feed-rs 解析 → Vec<RawArticle>
    │       │
    │       ├─→ ArxivFetcher::fetch(source)
    │       │       └─→ 构建 API URL → HTTP GET → XML 解析 → Vec<RawArticle>
    │       │
    │       ├─→ HackernewsFetcher::fetch(source)
    │       │       └─→ Algolia API → JSON 解析 → 分数过滤 → Vec<RawArticle>
    │       │
    │       └─→ RedditFetcher::fetch(source)
    │               └─→ 并行请求各 subreddit → JSON 解析 → upvotes 过滤 → Vec<RawArticle>
    │
    ├─→ [3] Processor::process(all_raw_articles)
    │       └─→ URL 去重 → 标题清洗 → 摘要提取 → 标题去重 → Vec<ProcessedArticle>
    │
    ├─→ [4] 批量生成 embedding
    │       └─→ EmbeddingClient::batch_embed(titles + summaries) → Vec<Vec<f32>>
    │
    ├─→ [5] RelevanceEngine::rank_articles(articles, V_user)
    │       └─→ 余弦相似度 → 阈值过滤 → 状态过滤 → 内容去重 → 多因子排序
    │       → Vec<RankedArticle>
    │
    ├─→ [6] StateManager::insert_articles(ranked_articles)
    │       └─→ 写入 SQLite radar_items 表
    │       └─→ 写入 Qdrant 向量（用于后续 query 搜索）
    │
    └─→ [7] StateManager::cleanup_old_items(1000)
            └─→ 清理超限的旧条目

返回 FetchReport { successful_sources, failed_sources, new_items }
```

### 4.2 相关性排序流程

```
输入：Vec<ProcessedArticle> + V_user (用户兴趣向量)
    │
    ▼
┌─────────────────────────────────┐
│ Step 1: 批量生成文章 embedding   │
│   调用 EmbeddingClient::batch()  │
│   输入：[title + " " + summary]  │
│   输出：Vec<Vec<f32>>           │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ Step 2: 逐篇计算                │
│   for each article:             │
│     similarity = cosine(V, V_u) │
│     if similarity < 0.7:        │
│       → 丢弃                    │
│     if url in existing:         │
│       → 丢弃                    │
│     if status != New:           │
│       → 丢弃                    │
│     trust = source.trust_weight │
│     recency = f(published_at)   │
│     dup = max_sim(existing)     │
│     score = α·sim + β·trust    │
│           + γ·recency - δ·dup  │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ Step 3: 排序                    │
│   按 score 降序排列             │
│   返回 Vec<RankedArticle>       │
└─────────────────────────────────┘
```

### 4.3 纳藏流程

```
用户: "保存这篇文章"
    │
    ▼
RadarService::add_to_vault(article_id, target_dir?)
    │
    ▼
[1] SQLite: 查询文章详情
    │
    ▼
[2] VaultSaver::extract_article_content()
    ├── 有 content_html？ → 直接 readability 提取
    └── 无 content_html？ → HTTP GET URL → readability 提取
    │
    ▼
[3] html2md: HTML → Markdown 转换
    │
    ▼
[4] VaultSaver::generate_note_content()
    ├── LLM: 生成中文摘要 (150-250字)
    ├── LLM: 提取标签 (3-5个)
    └── Qdrant: 搜索相关笔记 (top 3)
    │
    ▼
[5] 组装 Obsidian 笔记 Markdown
    ├── YAML frontmatter
    ├── 摘要区
    ├── 正文区
    ├── 链接区
    └── 相关笔记区
    │
    ▼
[6] 写入文件
    └── vault/<target_dir>/YYYY-MM-DD-<slug>.md
    │
    ▼
[7] 后续操作（异步并行）
    ├── StateManager: 状态 → Saved
    ├── MemoryService: 触发新笔记索引（分块 → embedding → Qdrant）
    ├── TimelineService: 记录 RadarSaved 事件
    └── 返回 VaultSaveResult
```

---

## 5. 关键数据结构

### 5.1 RadarItem（完整定义）

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::collections::HashMap;

/// 雷达条目 — SQLite 持久化的核心数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarItem {
    pub id: Uuid,
    pub title: String,
    pub summary: String,
    pub source_name: String,          // "arxiv-cs" | "hackernews" | "tech-rss" | "reddit-programming"
    pub source_type: SourceType,      // rss / arxiv / hackernews / reddit
    pub url: String,
    pub embedding_id: Option<String>, // Qdrant 向量 ID
    pub status: RadarStatus,
    pub relevance_score: Option<f32>,
    pub similarity_score: Option<f32>,
    pub author: Option<String>,
    pub content_html: Option<String>, // 原始 HTML（用于延迟正文提取）
    pub extra_data: Option<serde_json::Value>, // 源特定额外数据
    pub related_notes: Vec<String>,   // 关联笔记路径
    pub fetched_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub read_at: Option<DateTime<Utc>>,
    pub saved_at: Option<DateTime<Utc>>,
    pub dismissed_at: Option<DateTime<Utc>>,
    pub saved_path: Option<String>,   // 纳藏后的 vault 路径
}
```

### 5.2 RadarStatus

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RadarStatus {
    New,       // 新抓取，未查看
    Read,      // 已通过 get_radar 查看
    Saved,     // 已纳藏到 vault
    Dismissed, // 已忽略
}

impl std::fmt::Display for RadarStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RadarStatus::New => write!(f, "new"),
            RadarStatus::Read => write!(f, "read"),
            RadarStatus::Saved => write!(f, "saved"),
            RadarStatus::Dismissed => write!(f, "dismissed"),
        }
    }
}

impl std::str::FromStr for RadarStatus {
    type Err = BrainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "new" => Ok(RadarStatus::New),
            "read" => Ok(RadarStatus::Read),
            "saved" => Ok(RadarStatus::Saved),
            "dismissed" => Ok(RadarStatus::Dismissed),
            _ => Err(BrainError::Internal(format!("无效的雷达状态: {}", s))),
        }
    }
}
```

### 5.3 RadarItemView（get_radar 返回视图）

```rust
/// get_radar 工具返回的视图结构
#[derive(Debug, Clone, Serialize)]
pub struct RadarItemView {
    pub id: String,                   // UUID 字符串
    pub title: String,
    pub summary: String,
    pub source: String,               // 来源标识
    pub url: String,
    pub relevance_score: f32,         // 相关性得分 (0-1)
    pub related_notes: Vec<String>,   // 关联笔记路径
    pub published_at: Option<String>, // ISO 8601
    pub status: String,               // "new" | "read" | "saved" | "dismissed"
}

impl From<RadarItem> for RadarItemView {
    fn from(item: RadarItem) -> Self {
        Self {
            id: item.id.to_string(),
            title: item.title,
            summary: item.summary,
            source: item.source_name,
            url: item.url,
            relevance_score: item.relevance_score.unwrap_or(0.0),
            related_notes: item.related_notes,
            published_at: item.published_at.map(|d| d.to_rfc3339()),
            status: item.status.to_string(),
        }
    }
}
```

### 5.4 ScoredArticle / RankedArticle（内部中间结构）

```rust
/// 已生成 embedding 的文章（相关性计算前）
#[derive(Debug, Clone)]
pub struct ScoredArticle {
    pub title: String,
    pub summary: String,
    pub url: String,
    pub source_name: String,
    pub source_type: SourceType,
    pub author: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub content_html: Option<String>,
    pub extra: HashMap<String, serde_json::Value>,
    pub embedding: Vec<f32>,          // 文章 embedding 向量
}

/// 排序后的文章（相关性计算后）
#[derive(Debug, Clone)]
pub struct RankedArticle {
    pub article: ScoredArticle,
    pub relevance_score: f32,         // 最终综合得分
    pub similarity: f32,              // 语义相似度原始分
    pub trust_score: f32,             // 来源可信度
    pub recency_score: f32,           // 时效性得分
    pub duplication_score: f32,       // 内容重复度

    // 便捷访问
    pub url: String,
    pub source_name: String,
}
```

### 5.5 RadarConfig（雷达配置）

```rust
/// 雷达模块运行时配置（从 config/default.toml 的 [radar] 段加载）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarConfig {
    pub fetch_interval_hours: u32,     // 拉取间隔（小时），默认 6
    pub relevance_threshold: f32,      // 相关性阈值，默认 0.7
    pub max_items_per_source: usize,   // 每源最大条目数，默认 20
    pub readability_enabled: bool,     // 是否启用 readability 正文提取
    pub max_total_items: usize,        // 总存储上限，默认 1000
    pub sources_config_path: String,   // radar_sources.toml 路径
}

impl Default for RadarConfig {
    fn default() -> Self {
        Self {
            fetch_interval_hours: 6,
            relevance_threshold: 0.7,
            max_items_per_source: 20,
            readability_enabled: true,
            max_total_items: 1000,
            sources_config_path: "config/radar_sources.toml".to_string(),
        }
    }
}
```

### 5.6 FetchReport（拉取报告）

```rust
/// 一次拉取操作的报告
#[derive(Debug, Clone, Serialize)]
pub struct FetchReport {
    pub successful_sources: usize,
    pub failed_sources: usize,
    pub new_items: usize,
    pub total_fetched: usize,
    pub duration_secs: f64,
    pub errors: Vec<FetchError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FetchError {
    pub source_name: String,
    pub error: String,
}
```

### 5.7 RadarSourceStatus（源状态视图）

```rust
/// list_radar_sources 返回的源状态
#[derive(Debug, Clone, Serialize)]
pub struct RadarSourceStatus {
    pub name: String,
    pub source_type: String,
    pub enabled: bool,
    pub description: String,
    pub last_fetch_at: Option<String>,     // ISO 8601
    pub last_success_at: Option<String>,
    pub total_items_fetched: u64,
    pub health: String,                    // "healthy" | "degraded" | "unhealthy" | "disabled"
}
```

---

## 6. 完整请求/响应示例

### 6.1 get_radar 示例

**请求**（MCP / HTTP Tool API）：

```json
{
  "tool": "get_radar",
  "arguments": {
    "limit": 5
  }
}
```

**响应**：

```json
{
  "tool": "get_radar",
  "status": "success",
  "result": {
    "items": [
      {
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "title": "Retrieval-Augmented Generation for Large Language Models: A Survey",
        "summary": "本文系统综述了 RAG 技术的最新进展，涵盖检索策略、生成架构和训练方法三个维度...",
        "source": "arxiv-cs",
        "url": "https://arxiv.org/abs/2401.12345",
        "relevance_score": 0.89,
        "related_notes": ["ai/rag-notes.md", "ai/llm-architecture.md"],
        "published_at": "2026-05-25T00:00:00+00:00",
        "status": "new"
      },
      {
        "id": "550e8400-e29b-41d4-a716-446655440001",
        "title": "Building a High-Performance Search Engine in Rust",
        "summary": "A deep dive into building a search engine from scratch using Rust, covering inverted indices, BM25 scoring, and query optimization...",
        "source": "hackernews",
        "url": "https://example.com/rust-search-engine",
        "relevance_score": 0.85,
        "related_notes": ["cs/search-engine.md"],
        "published_at": "2026-05-28T10:30:00+00:00",
        "status": "new"
      },
      {
        "id": "550e8400-e29b-41d4-a716-446655440002",
        "title": "Tokio 2.0: What's New in Rust's Async Runtime",
        "summary": "Tokio 2.0 带来了重大更新，包括改进的任务调度器、更低的内存占用和新的 io_uring 支持...",
        "source": "rss:tech-rss",
        "url": "https://blog.rust-lang.org/2026/05/27/tokio-2.html",
        "relevance_score": 0.82,
        "related_notes": ["programming/rust-async.md", "programming/tokio-notes.md"],
        "published_at": "2026-05-27T08:00:00+00:00",
        "status": "new"
      },
      {
        "id": "550e8400-e29b-41d4-a716-446655440003",
        "title": "Understanding Vector Databases: A Practical Guide",
        "summary": "向量数据库的实用指南，涵盖 HNSW 索引、量化技术和生产环境部署最佳实践...",
        "source": "hackernews",
        "url": "https://example.com/vector-db-guide",
        "relevance_score": 0.79,
        "related_notes": ["cs/vector-databases.md"],
        "published_at": "2026-05-26T15:00:00+00:00",
        "status": "new"
      },
      {
        "id": "550e8400-e29b-41d4-a716-446655440004",
        "title": "Prompt Engineering for Better RAG Systems",
        "summary": "探讨如何通过优化 prompt 来提升 RAG 系统的检索和生成质量...",
        "source": "reddit:MachineLearning",
        "url": "https://reddit.com/r/MachineLearning/comments/abc123",
        "relevance_score": 0.76,
        "related_notes": ["ai/prompt-engineering.md"],
        "published_at": "2026-05-25T20:00:00+00:00",
        "status": "new"
      }
    ],
    "total": 5,
    "interest_profile": {
      "active_notes_count": 42,
      "top_tags": ["rust", "ai", "llm", "search", "async"],
      "last_updated": "2026-05-29T06:00:00+00:00"
    }
  }
}
```

**带 query 参数的请求**：

```json
{
  "tool": "get_radar",
  "arguments": {
    "limit": 5,
    "query": "WebAssembly WASM"
  }
}
```

**响应**：

```json
{
  "tool": "get_radar",
  "status": "success",
  "result": {
    "items": [
      {
        "id": "550e8400-e29b-41d4-a716-446655440010",
        "title": "WASM in Edge Computing: A Practical Guide",
        "summary": "WebAssembly 在边缘计算中的实践，包括 WASI 接口、性能基准测试和部署策略...",
        "source": "hackernews",
        "url": "https://example.com/wasm-edge",
        "relevance_score": 0.85,
        "related_notes": ["programming/wasm-intro.md"],
        "published_at": "2026-05-28T12:00:00+00:00",
        "status": "new"
      }
    ],
    "total": 1,
    "query": "WebAssembly WASM"
  }
}
```

### 6.2 add_to_vault 示例

**请求**：

```json
{
  "tool": "add_to_vault",
  "arguments": {
    "article_id": "550e8400-e29b-41d4-a716-446655440000",
    "target_dir": "radar/"
  }
}
```

**响应**：

```json
{
  "tool": "add_to_vault",
  "status": "success",
  "result": {
    "note_path": "radar/2026-05-29-retrieval-augmented-generation-for-large-language-models-a-survey.md",
    "obsidian_uri": "obsidian://open?vault=brain&file=radar%2F2026-05-29-retrieval-augmented-generation-for-large-language-models-a-survey.md",
    "summary": "本文系统综述了检索增强生成（RAG）技术在大语言模型中的应用与发展。RAG 通过在生成过程中引入外部知识检索，有效缓解了 LLM 的幻觉问题和知识过时问题。文章从检索策略、生成架构、训练方法三个维度对现有工作进行了分类梳理，并讨论了 RAG 在企业级应用中的实践经验和未来研究方向。",
    "tags": ["RAG", "LLM", "检索增强生成", "知识检索", "NLP"],
    "related_notes": [
      "ai/rag-notes.md",
      "ai/llm-architecture.md",
      "nlp/embedding-techniques.md"
    ],
    "word_count": 4523
  }
}
```

### 6.3 dismiss_radar_item 示例

**请求**：

```json
{
  "tool": "dismiss_radar_item",
  "arguments": {
    "article_id": "550e8400-e29b-41d4-a716-446655440004"
  }
}
```

**响应**：

```json
{
  "tool": "dismiss_radar_item",
  "status": "success",
  "result": {
    "dismissed": true,
    "article_title": "Prompt Engineering for Better RAG Systems"
  }
}
```

### 6.4 错误响应示例

**文章不存在**：

```json
{
  "tool": "add_to_vault",
  "status": "error",
  "error": {
    "code": "RADAR_ITEM_NOT_FOUND",
    "message": "雷达条目 '550e8400-xxxx-xxxx-xxxx-xxxxxxxxxxxx' 未找到",
    "suggestion": "请通过 get_radar 获取可用的文章列表"
  }
}
```

**源不可达**：

```json
{
  "tool": "get_radar",
  "status": "success",
  "result": {
    "items": [],
    "total": 0,
    "warnings": [
      "arxiv-cs 源拉取失败: API 请求超时",
      "当前无新的推荐内容，请稍后再试"
    ]
  }
}
```

---

## 7. 错误处理

### 7.1 错误分类与处理策略

| 错误类型 | 具体场景 | 处理方式 |
|---|---|---|
| **源不可达** | DNS 解析失败、连接超时、HTTP 5xx | 记录错误，跳过该源，下次拉取重试。连续失败 3 次标记为 Unhealthy |
| **API 限流** | HTTP 429、arXiv 3 秒间隔 | 指数退避重试（1s → 2s → 4s），最多重试 3 次 |
| **解析失败** | Feed 格式不标准、XML/JSON 结构异常 | 记录错误日志，跳过该条目，不中断整体流程 |
| **Embedding 失败** | API 超时、额度不足 | 重试 3 次后降级为按可信度+时效性排序（无语义排序） |
| **Readability 失败** | SPA 页面、JS 渲染内容 | 降级为仅保存摘要和链接，不保存正文 |
| **文件写入失败** | 磁盘满、权限不足 | 返回错误给用户，建议检查 vault 路径和磁盘空间 |
| **SQLite 错误** | 数据库锁定、磁盘 IO 错误 | 重试 3 次，若仍失败返回 Internal 错误 |
| **Qdrant 不可用** | 容器未启动、网络不通 | 降级为纯 SQLite 存储，不做向量搜索，日志告警 |
| **LLM 调用失败** | API 超时、模型不可用 | 摘要生成降级为截取正文前 200 字，标签降级为 "radar" |

### 7.2 错误类型扩展

在 `BrainError` 枚举中新增雷达相关的错误变体（参见[顶层设计文档 §7](../top_design.md)）：

```rust
// 在 BrainError 中新增的变体：
enum BrainError {
    // ... 已有变体 ...

    // 雷达相关
    RadarItemNotFound(Uuid),
    RadarSourceNotFound(String),
    RadarSourceDuplicate(String),
    FetchError { url: String, detail: String },
    RadarConfigError(String),
}
```

---

## 8. 性能优化

### 8.1 增量拉取

```rust
/// 增量拉取策略：只处理未见过的 URL
impl Processor {
    /// 检查 URL 是否已存在于 radar_items 表
    async fn is_url_seen(&self, url: &str) -> Result<bool, BrainError> {
        self.sqlite_store.radar_item_exists_by_url(url).await
    }

    /// 对于 arXiv：记录上次拉取的最新 ID，下次只拉更新的
    async fn get_last_arxiv_id(&self, source_name: &str) -> Option<String> {
        self.sqlite_store.get_app_state(
            &format!("radar_arxiv_last_id_{}", source_name)
        ).await
    }
}
```

### 8.2 Embedding 缓存

```rust
/// Embedding 缓存策略
pub struct EmbeddingCache {
    cache: DashMap<String, Vec<f32>>,  // URL/文本 hash → embedding
    max_size: usize,
}

impl EmbeddingCache {
    /// 查找缓存
    pub fn get(&self, key: &str) -> Option<Vec<f32>> {
        self.cache.get(key).map(|v| v.clone())
    }

    /// 写入缓存
    pub fn insert(&self, key: String, embedding: Vec<f32>) {
        if self.cache.len() >= self.max_size {
            // LRU 淘汰（简化实现：随机淘汰 10%）
            let keys_to_remove: Vec<String> = self.cache.iter()
                .take(self.max_size / 10)
                .map(|e| e.key().clone())
                .collect();
            for key in keys_to_remove {
                self.cache.remove(&key);
            }
        }
        self.cache.insert(key, embedding);
    }
}
```

### 8.3 批量处理

```rust
/// 批量生成 embedding，减少 API 调用次数
impl RelevanceEngine {
    pub async fn batch_generate_embeddings(
        &self,
        articles: &[ProcessedArticle],
    ) -> Result<Vec<Vec<f32>>, BrainError> {
        // 将标题 + 摘要拼接为文本列表
        let texts: Vec<String> = articles.iter()
            .map(|a| format!("{} {}", a.title, a.summary))
            .collect();

        // 检查缓存
        let mut results = vec![vec![]; texts.len()];
        let mut uncached_indices = Vec::new();
        let mut uncached_texts = Vec::new();

        for (i, text) in texts.iter().enumerate() {
            let cache_key = format!("radar:{}", md5::compute(text));
            if let Some(cached) = self.embedding_cache.get(&cache_key) {
                results[i] = cached;
            } else {
                uncached_indices.push(i);
                uncached_texts.push(text.clone());
            }
        }

        if !uncached_texts.is_empty() {
            // 批量调用 Embedding API（OpenAI 支持单次最多 2048 条）
            let batch_embeddings = self.embedding_client
                .batch_embed(&uncached_texts)
                .await?;

            for (idx, embedding) in uncached_indices.into_iter().zip(batch_embeddings) {
                let cache_key = format!("radar:{}", md5::compute(&texts[idx]));
                self.embedding_cache.insert(cache_key, embedding.clone());
                results[idx] = embedding;
            }
        }

        Ok(results)
    }
}
```

### 8.4 性能指标

| 操作 | 目标耗时 | 优化手段 |
|---|---|---|
| 全量拉取（4 个源） | < 5 min | 源并行抓取，单源超时 30s |
| 100 篇文章 embedding 生成 | < 30s | 批量 API 调用（一次请求 100 条） |
| 100 篇文章相关性排序 | < 5s | 内存中向量计算，无 IO |
| 单篇文章纳藏 | < 15s | 正文提取 + LLM 摘要 + 文件写入 |
| get_radar 查询 | < 500ms | SQLite 查询 + 内存排序 |
| 用户兴趣向量构建 | < 10s | Qdrant scroll + 内存计算 |

---

## 9. 测试策略

### 9.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ── SourceManager 测试 ──

    #[test]
    fn test_parse_radar_sources_toml() {
        let toml_str = r#"
            [defaults]
            max_items_per_source = 20
            trust_weight = 0.8
            request_timeout_secs = 30

            [[sources]]
            name = "test-hn"
            type = "hackernews"
            enabled = true
            min_score = 50
            min_comments = 10
            query = ""
        "#;
        let config: RadarSourcesConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.sources.len(), 1);
        assert_eq!(config.sources[0].name, "test-hn");
    }

    #[test]
    fn test_source_crud() {
        let mut manager = SourceManager::new_for_test();
        let entry = SourceEntry { /* ... */ };
        manager.add_source(entry).unwrap();
        assert!(manager.get_source("test").is_some());
        manager.toggle_source("test", false).unwrap();
        assert!(!manager.get_source("test").unwrap().enabled);
        manager.remove_source("test").unwrap();
        assert!(manager.get_source("test").is_none());
    }

    // ── Processor 测试 ──

    #[test]
    fn test_clean_title() {
        let processor = Processor::new_for_test();
        assert_eq!(
            processor.clean_title("  Hello   World  \n  Foo  "),
            "Hello World Foo"
        );
        assert_eq!(
            processor.clean_title("Title with &amp; entities"),
            "Title with & entities"
        );
    }

    #[test]
    fn test_deduplicate_by_title() {
        let processor = Processor::new_for_test();
        let articles = vec![
            make_article("Rust Async Programming"),
            make_article("Rust Async Programing"), // 编辑距离 < 3
            make_article("Python Machine Learning"),
        ];
        let result = processor.deduplicate_by_title(articles);
        assert_eq!(result.len(), 2); // 第二个被去重
    }

    // ── RelevanceEngine 测试 ──

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((RelevanceEngine::cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0];
        assert!(RelevanceEngine::cosine_similarity(&a, &c).abs() < 1e-6);
    }

    #[test]
    fn test_recency_score() {
        let engine = RelevanceEngine::new_for_test();
        let now = Utc::now();

        // 今天的文章得分接近 1.0
        let score = engine.calculate_recency_score(Some(now), now);
        assert!(score > 0.95);

        // 30 天前的文章得分接近 0
        let old = now - Duration::days(30);
        let score = engine.calculate_recency_score(Some(old), now);
        assert!(score < 0.05);

        // 无发布时间
        let score = engine.calculate_recency_score(None, now);
        assert!((score - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_user_interest_vector_normalization() {
        let engine = RelevanceEngine::new_for_test();
        // 测试加权平均和归一化逻辑
        // ...
    }

    // ── VaultSaver 测试 ──

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World!", 60), "hello-world");
        assert_eq!(slugify("Rust 异步编程", 60), "rust-异步编程");
        assert_eq!(slugify("A Very Long Title That Should Be Truncated", 20),
                   "a-very-long-title-th");
    }

    #[test]
    fn test_note_template_generation() {
        // 验证生成的 Markdown 笔记格式正确
        // 包含正确的 frontmatter、摘要、正文、链接区
    }

    // ── StateManager 测试 ──

    #[tokio::test]
    async fn test_state_transitions() {
        let state = StateManager::new_for_test();
        let id = Uuid::new_v4();

        // 插入新条目
        state.insert_test_item(id, "new").await;
        assert_eq!(state.get_status(id).await, RadarStatus::New);

        // New → Read
        state.update_status(id, RadarStatus::Read).await.unwrap();
        assert_eq!(state.get_status(id).await, RadarStatus::Read);

        // Read → Saved
        state.update_status(id, RadarStatus::Saved).await.unwrap();
        assert_eq!(state.get_status(id).await, RadarStatus::Saved);
    }

    #[tokio::test]
    async fn test_cleanup_old_items() {
        let state = StateManager::new_for_test();
        // 插入 1100 个条目
        for i in 0..1100 {
            state.insert_test_item(Uuid::new_v4(), "new").await;
        }
        state.cleanup_old_items(1000).await.unwrap();
        let count = state.count_items().await;
        assert!(count <= 1000);
    }
}
```

### 9.2 集成测试

```rust
#[cfg(test)]
mod integration_tests {
    // 测试完整的拉取 → 处理 → 排序 → 存储流程
    #[tokio::test]
    async fn test_full_fetch_pipeline() {
        // 使用 mock HTTP server 模拟外部源
        let mock_server = MockServer::start().await;
        mock_server.register(
            Mock::given(method("GET"))
                .and(path("/feed.xml"))
                .respond_with(ResponseTemplate::new(200)
                    .set_body_string(MOCK_RSS_FEED))
        ).await;

        // 配置使用 mock server 的 RSS 源
        let config = test_radar_config(&mock_server.uri());
        let service = RadarService::start(config, /* mock deps */).await.unwrap();

        // 执行拉取
        let report = service.fetch_now().await.unwrap();
        assert!(report.new_items > 0);
        assert_eq!(report.failed_sources, 0);

        // 验证 SQLite 中有新条目
        let items = service.state_manager.get_recommendations(10).await.unwrap();
        assert!(!items.is_empty());
    }

    // 测试纳藏流程
    #[tokio::test]
    async fn test_vault_save_flow() {
        let temp_dir = TempDir::new().unwrap();
        let vault_path = temp_dir.path().to_path_buf();

        // 创建测试文章
        let article = create_test_radar_item();

        // 执行纳藏
        let saver = VaultSaver::new_for_test(vault_path.clone());
        let result = saver.save_to_vault(&article, Some("radar")).await.unwrap();

        // 验证文件已写入
        let saved_path = vault_path.join(&result.note_path);
        assert!(saved_path.exists());

        // 验证文件内容包含 frontmatter
        let content = std::fs::read_to_string(&saved_path).unwrap();
        assert!(content.starts_with("---"));
        assert!(content.contains("source:"));
        assert!(content.contains("url:"));
    }
}
```

### 9.3 Mock 数据

```rust
const MOCK_RSS_FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Test Blog</title>
    <item>
      <title>Understanding Rust Ownership</title>
      <link>https://example.com/rust-ownership</link>
      <description>A deep dive into Rust's ownership model and borrow checker...</description>
      <pubDate>Wed, 28 May 2026 10:00:00 GMT</pubDate>
    </item>
    <item>
      <title>Building AI Agents with Tool Use</title>
      <link>https://example.com/ai-agents</link>
      <description>How to build AI agents that can use tools effectively...</description>
      <pubDate>Tue, 27 May 2026 08:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>"#;
```

---

## 10. 依赖清单

### 10.1 新增外部依赖

以下依赖需添加到 `Cargo.toml`：

```toml
[dependencies]
# ── 已有（复用） ──
# tokio, axum, serde, serde_json, reqwest, tracing
# rusqlite, qdrant-client, uuid, chrono

# ── 雷达模块新增 ──

# RSS/Atom 解析
feed-rs = "2"                    # 纯 Rust RSS/Atom 解析器

# XML 解析（arXiv API 响应）
quick-xml = { version = "0.36", features = ["serialize"] }

# HTML → Markdown 转换
html2md = "0.2"                  # HTML 转 Markdown

# Readability 正文提取
readability = "0.3"              # Readability 算法 Rust 实现

# HTML 实体解码
html-escape = "0.2"

# URL 编码
urlencoding = "2"

# 正则表达式
regex = "1"

# 编辑距离（标题去重）
edit-distance = "2"

# 文件名 slug 生成
slug = "0.1"

# 定时任务调度
tokio-cron-scheduler = "0.13"

# 异步 trait
async-trait = "0.1"

# futures 工具
futures = "0.3"

# 哈希（embedding 缓存 key）
md5 = "0.7"

# TOML 序列化（配置文件回写）
toml = "0.8"

# 并发 HashMap（embedding 缓存）
dashmap = "6"

# 临时目录（测试用）
[dev-dependencies]
tempfile = "3"
wiremock = "0.6"                 # HTTP mock server
```

### 10.2 依赖关系图

```
feed-rs ─────────────────────┐
quick-xml ──────────────────┤
reqwest ────────────────────┤
                            ▼
                    ┌───────────────┐
                    │   Fetcher     │
                    │ (各源抓取器)   │
                    └───────┬───────┘
                            │
html-escape ────────────────┤
regex ──────────────────────┤
edit-distance ──────────────┤
                            ▼
                    ┌───────────────┐
                    │  Processor    │
                    │ (内容处理器)   │
                    └───────┬───────┘
                            │
infra/embedding.rs ─────────┤
qdrant-client ──────────────┤
                            ▼
                    ┌───────────────┐
                    │ Relevance     │
                    │ Engine        │
                    └───────┬───────┘
                            │
readability ────────────────┤
html2md ────────────────────┤
infra/llm_client.rs ────────┤
                            ▼
                    ┌───────────────┐
                    │  VaultSaver   │
                    │  (纳藏器)      │
                    └───────┬───────┘
                            │
rusqlite ───────────────────┤
tokio-cron-scheduler ───────┤
                            ▼
                    ┌───────────────┐
                    │ StateManager  │
                    │ + Scheduler   │
                    └───────────────┘
```

### 10.3 可选替代方案

| 依赖 | 替代方案 | 切换条件 |
|---|---|---|
| `readability` | `trafilatura` (Python 桥接) 或自实现简化版 | 如果 readability crate 维护不活跃 |
| `quick-xml` | `roxmltree` (只读 XML 解析) | 如果需要更轻量的 XML 解析 |
| `feed-rs` | `rss` + `atom` (分别处理 RSS 和 Atom) | 如果需要更精细的格式控制 |
| `dashmap` | `std::sync::Mutex<HashMap>` | 如果不需要高并发缓存访问 |

---

> **相关文档**：
> - [顶层设计文档](../top_design.md) — 系统整体架构和数据模型
> - [需求设计文档](../requirement/07-radar.md) — 功能需求和用户故事
