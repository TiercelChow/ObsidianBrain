//! Wiki 页面写入器 — Karpathy LLM Wiki 模式
//!
//! LLM 编译完整文章，按主题组织。

use crate::error::BrainError;
use crate::infra::obsidian_client::{get_client, ObsidianProvider};

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

/// 确保 Wiki 目录结构存在
pub async fn ensure_wiki_structure(provider: &ObsidianProvider) -> Result<(), BrainError> {
    let client = get_client(provider)?;

    // 创建 Wiki 根目录占位
    let _ = client.write_file("Wiki/.gitkeep", "").await;
    let _ = client.write_file("Raw/.gitkeep", "").await;

    // 创建 index.md（如不存在）
    let index_path = "Wiki/index.md";
    if client.read_file(index_path).await.is_err() {
        let content = "# Knowledge Base Index\n";
        client.write_file(index_path, content).await?;
    }

    // 创建 log.md（如不存在）
    let log_path = "Wiki/log.md";
    if client.read_file(log_path).await.is_err() {
        let content = "# Wiki Log\n";
        client.write_file(log_path, content).await?;
    }

    Ok(())
}

/// 默认 Schema 文件内容
pub const DEFAULT_SCHEMA: &str = r#"# LLM Wiki 维护规则

## 目录结构
- Raw/<topic>/ — 原始资料（不可变）
- Wiki/<topic>/ — 编译后的知识文章（按主题一级子目录组织）

## 命名规范
- 主题目录：kebab-case，如 `machine-learning`
- 文章文件：kebab-case，如 `rmsnorm-and-layernorm.md`

## Ingest 流程
1. 读取原始资料
2. LLM 决定：合并已有文章 or 创建新文章
3. LLM 编译完整文章（综合改写，非逐字复制）
4. 级联更新同主题受影响的文章
5. 更新 index.md（表格格式）
6. 追加 log.md

## Query 流程
1. 读取 index.md 找相关文章
2. 读取文章内容
3. LLM 综合回答

## Lint 流程
1. 检查索引一致性（每个文件都在索引中）
2. 检查内部链接有效性
3. 检查孤岛页
4. LLM 生成改进建议
"#;
