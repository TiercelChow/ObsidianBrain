//! Wiki 页面写入器
//!
//! 通过 ObsidianClient 创建和更新 Wiki 中的 Markdown 页面。

use crate::error::BrainError;
use crate::infra::obsidian_client::{get_client, ObsidianProvider};
use serde::{Deserialize, Serialize};

/// Wiki 页面类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PageType {
    Entity,
    Concept,
    Source,
    Synthesis,
}

impl PageType {
    pub fn dir(&self) -> &'static str {
        match self {
            PageType::Entity => "Wiki/entities",
            PageType::Concept => "Wiki/concepts",
            PageType::Source => "Wiki/sources",
            PageType::Synthesis => "Wiki/synthesis",
        }
    }
}

/// Wiki 页面的 frontmatter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageFrontmatter {
    #[serde(rename = "type")]
    pub page_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub sources: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingested: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub entities: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub concepts: Vec<String>,
}

/// 将名称转为 kebab-case 文件名
pub fn to_filename(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .replace(' ', "-")
        .replace('/', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '.')
        .collect()
}

/// 构建页面路径
pub fn page_path(page_type: &PageType, name: &str) -> String {
    format!("{}/{}.md", page_type.dir(), to_filename(name))
}

/// 检查 Wiki 页面是否存在
pub async fn page_exists(
    provider: &ObsidianProvider,
    path: &str,
) -> bool {
    let client = match get_client(provider) {
        Ok(c) => c,
        Err(_) => return false,
    };
    client.read_file(path).await.is_ok()
}

/// 读取 Wiki 页面内容
pub async fn read_page(
    provider: &ObsidianProvider,
    path: &str,
) -> Result<String, BrainError> {
    let client = get_client(provider)?;
    client.read_file(path).await
}

/// 创建或更新 Wiki 页面
pub async fn write_page(
    provider: &ObsidianProvider,
    path: &str,
    content: &str,
) -> Result<(), BrainError> {
    let client = get_client(provider)?;
    client.write_file(path, content).await
}

/// 确保 Wiki 目录结构存在（创建占位文件）
pub async fn ensure_wiki_structure(provider: &ObsidianProvider) -> Result<(), BrainError> {
    let client = get_client(provider)?;

    // 创建各子目录的占位
    for dir in &["Wiki/entities", "Wiki/concepts", "Wiki/sources", "Wiki/synthesis"] {
        let placeholder = format!("{}/.gitkeep", dir);
        let _ = client.write_file(&placeholder, "").await;
    }

    // 创建 index.md（如不存在）
    let index_path = "Wiki/index.md";
    if client.read_file(index_path).await.is_err() {
        let content = "# Wiki 索引\n\n最后更新：-\n总页数：0\n\n## 实体\n\n## 概念\n\n## 源摘要\n\n## 综合论述\n";
        client.write_file(index_path, content).await?;
    }

    // 创建 log.md（如不存在）
    let log_path = "Wiki/log.md";
    if client.read_file(log_path).await.is_err() {
        let content = "# Wiki 操作日志\n";
        client.write_file(log_path, content).await?;
    }

    // 创建 schema.md（如不存在）
    let schema_path = "Wiki/schema.md";
    if client.read_file(schema_path).await.is_err() {
        let content = DEFAULT_SCHEMA;
        client.write_file(schema_path, content).await?;
    }

    Ok(())
}

/// 默认 Schema 文件内容
pub const DEFAULT_SCHEMA: &str = r#"# LLM Wiki 维护规则

## 目录结构
- Wiki/entities/ — 实体页（人物、项目、工具）
- Wiki/concepts/ — 概念页（技术主题、理论）
- Wiki/sources/ — 源摘要页（每篇原始资料的摘要）
- Wiki/synthesis/ — 综合论述页（跨源分析）

## 命名规范
- 文件名：kebab-case，如 `andrej-karpathy.md`
- 源摘要：`YYYY-MM-DD-{简短描述}.md`

## Ingest 流程
1. 读取原始资料全文
2. 生成摘要（200-500 字）
3. 提取实体和概念
4. 创建或更新对应页面
5. 更新交叉引用（[[wikilink]]）
6. 更新 index.md
7. 追加 log.md

## Query 流程
1. 先读 index.md 找相关页面
2. 读取相关页面内容
3. 综合回答，附带 [[引用]]
4. 如果回答有价值，归档为 synthesis/ 新页面

## Lint 流程
1. 检查所有页面的 frontmatter 完整性
2. 检查交叉引用双向性
3. 检查孤岛页（无入链的页面）
4. 检查矛盾声明
5. 建议新探索方向
"#;
