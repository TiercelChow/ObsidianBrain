//! Wiki 索引和日志管理器
//!
//! 维护 Wiki/index.md（内容目录）和 Wiki/log.md（操作日志）。

use crate::error::BrainError;
use crate::infra::obsidian_client::ObsidianProvider;

use super::page_writer::{read_page, write_page};

/// 追加日志条目
pub async fn append_log(
    provider: &ObsidianProvider,
    entry_type: &str,
    summary: &str,
    affected_pages: &[String],
) -> Result<(), BrainError> {
    let log_path = "Wiki/log.md";
    let existing = read_page(provider, log_path).await.unwrap_or_default();

    let now = chrono::Local::now().format("%Y-%m-%d").to_string();
    let pages_str = if affected_pages.is_empty() {
        String::new()
    } else {
        format!("- 影响页面：{}", affected_pages.join(", "))
    };

    let entry = format!(
        "\n## [{}] {} | {}\n{}\n",
        now, entry_type, summary, pages_str
    );

    let new_content = format!("{}{}", existing, entry);
    write_page(provider, log_path, &new_content).await
}

/// 更新索引文件
pub async fn update_index(
    provider: &ObsidianProvider,
    entities: &[String],
    concepts: &[String],
    sources: &[String],
    synthesis: &[String],
) -> Result<(), BrainError> {
    let index_path = "Wiki/index.md";
    let now = chrono::Local::now().format("%Y-%m-%d").to_string();
    let total = entities.len() + concepts.len() + sources.len() + synthesis.len();

    let mut content = format!(
        "# Wiki 索引\n\n最后更新：{}\n总页数：{} · 实体：{} · 概念：{} · 源摘要：{} · 综合论述：{}\n\n",
        now, total, entities.len(), concepts.len(), sources.len(), synthesis.len()
    );

    content.push_str("## 实体\n\n");
    for e in entities {
        content.push_str(&format!("- [[{}]]\n", e));
    }

    content.push_str("\n## 概念\n\n");
    for c in concepts {
        content.push_str(&format!("- [[{}]]\n", c));
    }

    content.push_str("\n## 源摘要\n\n");
    for s in sources {
        content.push_str(&format!("- [[{}]]\n", s));
    }

    content.push_str("\n## 综合论述\n\n");
    for s in synthesis {
        content.push_str(&format!("- [[{}]]\n", s));
    }

    write_page(provider, index_path, &content).await
}

/// 列出 Wiki 中某类型的所有页面名（不含路径和 .md）
pub async fn list_pages(
    provider: &ObsidianProvider,
    dir: &str,
) -> Result<Vec<String>, BrainError> {
    let client = crate::infra::obsidian_client::get_client(provider)?;
    let files = client.list_all_files().await?;

    let pages: Vec<String> = files
        .into_iter()
        .filter(|f| f.starts_with(&format!("{}/", dir)) && f.ends_with(".md") && !f.ends_with(".gitkeep"))
        .map(|f| {
            // "Wiki/entities/andrej-karpathy.md" → "andrej-karpathy"
            f.rsplit('/').next().unwrap_or(&f).trim_end_matches(".md").to_string()
        })
        .collect();

    Ok(pages)
}
