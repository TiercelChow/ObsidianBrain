//! Wiki 页面写入器 — Karpathy LLM Wiki 模式
//!
//! LLM 编译完整文章，按主题组织。

use crate::error::BrainError;
use crate::infra::obsidian_client::{get_client, ObsidianProvider};

/// 检查 Wiki 页面是否存在
pub async fn page_exists(provider: &ObsidianProvider, path: &str) -> bool {
    let client = match get_client(provider) {
        Ok(c) => c,
        Err(_) => return false,
    };
    client.read_file(path).await.is_ok()
}

/// 读取 Wiki 页面内容
pub async fn read_page(provider: &ObsidianProvider, path: &str) -> Result<String, BrainError> {
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
