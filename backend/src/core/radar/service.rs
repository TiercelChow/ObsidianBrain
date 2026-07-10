//! 雷达服务

use chrono::Utc;
use std::sync::Arc;

use crate::core::radar::source_manager::SourceManager;
use crate::error::BrainError;
use crate::infra::obsidian_client::ObsidianProvider;
use crate::infra::sqlite_store::SqliteStore;
use crate::models::radar::*;

/// 雷达服务
pub struct RadarService {
    source_manager: SourceManager,
    db: Arc<SqliteStore>,
    obsidian: ObsidianProvider,
    config: RadarConfig,
}

impl RadarService {
    pub fn new(
        db: Arc<SqliteStore>,
        obsidian: ObsidianProvider,
        config: RadarConfig,
    ) -> Result<Self, BrainError> {
        let source_manager = if config.sources_path.exists() {
            SourceManager::load_from_file(&config.sources_path)?
        } else {
            tracing::warn!(
                "雷达源配置文件不存在: {:?}，使用默认配置",
                config.sources_path
            );
            SourceManager::new()
        };

        tracing::info!(
            "雷达服务初始化完成: {} 个源，{} 个已启用",
            source_manager.count(),
            source_manager.enabled_count()
        );

        Ok(Self {
            source_manager,
            db,
            obsidian,
            config,
        })
    }

    /// 获取推荐列表
    pub async fn get_radar(&self, limit: usize) -> Result<Vec<RadarItemView>, BrainError> {
        let rows = self.db.get_radar_items("new", limit as i64)?;

        let items = rows
            .into_iter()
            .map(
                |(id, title, summary, source_name, url, status)| RadarItemView {
                    id,
                    title,
                    summary,
                    source: source_name,
                    url,
                    relevance_score: 0.0,
                    published_at: None,
                    status,
                },
            )
            .collect();

        Ok(items)
    }

    /// 忽略雷达条目
    pub async fn dismiss_radar_item(&self, article_id: &str) -> Result<bool, BrainError> {
        self.db.update_radar_status(article_id, "dismissed")
    }

    /// 保存到 Vault
    pub async fn add_to_vault(
        &self,
        article_id: &str,
        target_dir: Option<&str>,
    ) -> Result<VaultSaveResult, BrainError> {
        let obsidian = match crate::infra::obsidian_client::get_client(&self.obsidian) {
            Ok(c) => c,
            Err(_) => {
                return Err(BrainError::ConfigError("Obsidian API 未启用".to_string()));
            }
        };

        // 获取雷达条目信息
        let rows = self.db.get_radar_items("new", 1000)?;
        let item = rows
            .iter()
            .find(|(id, _, _, _, _, _)| id == article_id)
            .ok_or_else(|| BrainError::Internal(format!("雷达条目 '{}' 不存在", article_id)))?;

        let (_, title, summary, source_name, url, _) = item;

        // 生成笔记内容
        let dir = target_dir.unwrap_or("radar");
        let slug = slugify(title);
        let filename = format!("{}-{}.md", Utc::now().format("%Y-%m-%d"), slug);
        let note_path = format!("{}/{}", dir, filename);

        let content = format!(
            "---\ntitle: \"{}\"\nsource: \"{}\"\nurl: \"{}\"\ntags: [radar]\ndate_fetched: {}\nstatus: saved\n---\n\n# {}\n\n## 摘要\n{}\n\n## 原始链接\n- [{}]({})\n\n## 来源\n{}\n",
            title, source_name, url, Utc::now().format("%Y-%m-%d"),
            title, summary, url, url, source_name
        );

        // 写入 vault
        obsidian.write_file(&note_path, &content).await?;

        // 更新状态
        self.db.update_radar_status(article_id, "saved")?;

        let obsidian_uri = format!(
            "obsidian://open?vault={}&file={}",
            self.config
                .sources_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("brain"),
            urlencoding::encode(&note_path)
        );

        tracing::info!(title = %title, path = %note_path, "文章已保存到 Vault");

        Ok(VaultSaveResult {
            note_path,
            obsidian_uri,
            summary: summary.clone(),
            tags: vec!["radar".to_string()],
            word_count: content.split_whitespace().count(),
        })
    }

    /// 列出所有源
    pub fn list_sources(&self) -> Vec<RadarSourceStatus> {
        self.source_manager
            .list_sources()
            .iter()
            .map(|s| RadarSourceStatus {
                name: s.name.clone(),
                source_type: s.source_type.to_string(),
                enabled: s.enabled,
                description: s.description.clone(),
                last_fetch_at: None,
                last_success_at: None,
                total_items_fetched: 0,
                health: if s.enabled {
                    "healthy".to_string()
                } else {
                    "disabled".to_string()
                },
            })
            .collect()
    }

    /// 切换源状态
    pub fn toggle_source(&mut self, name: &str, enabled: bool) -> Result<(), BrainError> {
        self.source_manager.toggle_source(name, enabled)
    }
}

/// 生成 URL 友好的 slug
fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// 雷达源状态（返回给前端）
#[derive(Debug, Clone, serde::Serialize)]
pub struct RadarSourceStatus {
    pub name: String,
    pub source_type: String,
    pub enabled: bool,
    pub description: String,
    pub last_fetch_at: Option<String>,
    pub last_success_at: Option<String>,
    pub total_items_fetched: u64,
    pub health: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World!"), "hello-world");
        assert_eq!(slugify("Rust 异步编程"), "rust");
        assert_eq!(slugify("test--double--dash"), "test-double-dash");
    }

    #[test]
    fn test_radar_item_view_structure() {
        let item = RadarItemView {
            id: "test-1".to_string(),
            title: "Test Article".to_string(),
            summary: "A test article".to_string(),
            source: "hackernews".to_string(),
            url: "https://example.com".to_string(),
            relevance_score: 0.85,
            published_at: Some("2026-06-01".to_string()),
            status: "new".to_string(),
        };
        assert_eq!(item.title, "Test Article");
        assert_eq!(item.status, "new");
    }
}
