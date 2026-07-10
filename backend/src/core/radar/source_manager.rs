//! 雷达源管理器

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::BrainError;

/// 源类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Rss,
    Arxiv,
    Hackernews,
    Reddit,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::Rss => write!(f, "rss"),
            SourceType::Arxiv => write!(f, "arxiv"),
            SourceType::Hackernews => write!(f, "hackernews"),
            SourceType::Reddit => write!(f, "reddit"),
        }
    }
}

/// 雷达源配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarSource {
    pub name: String,
    #[serde(rename = "type")]
    pub source_type: SourceType,
    pub enabled: bool,
    pub description: String,
    pub max_items: usize,
    pub trust_weight: f32,
    // RSS 源
    #[serde(default)]
    pub feeds: Vec<String>,
    // arXiv 源
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub query: String,
    // HN 源
    #[serde(default)]
    pub min_score: i64,
    // Reddit 源
    #[serde(default)]
    pub subreddits: Vec<String>,
}

/// 雷达源配置文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarSourcesConfig {
    pub sources: Vec<RadarSource>,
}

/// 源管理器
pub struct SourceManager {
    sources: Vec<RadarSource>,
}

impl SourceManager {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// 从 TOML 文件加载源配置
    pub fn load_from_file(path: &Path) -> Result<Self, BrainError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| BrainError::ConfigError(format!("读取雷达配置失败: {e}")))?;

        let config: RadarSourcesConfig = toml::from_str(&content)
            .map_err(|e| BrainError::ConfigError(format!("解析雷达配置失败: {e}")))?;

        Ok(Self {
            sources: config.sources,
        })
    }

    /// 列出所有源
    pub fn list_sources(&self) -> &[RadarSource] {
        &self.sources
    }

    /// 切换源的启用状态
    pub fn toggle_source(&mut self, name: &str, enabled: bool) -> Result<(), BrainError> {
        let source = self
            .sources
            .iter_mut()
            .find(|s| s.name == name)
            .ok_or_else(|| BrainError::Internal(format!("源 '{}' 不存在", name)))?;
        source.enabled = enabled;
        Ok(())
    }

    /// 获取源的数量
    pub fn count(&self) -> usize {
        self.sources.len()
    }

    /// 获取已启用源的数量
    pub fn enabled_count(&self) -> usize {
        self.sources.iter().filter(|s| s.enabled).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_manager_basic() {
        let mut manager = SourceManager::new();
        manager.sources = vec![
            RadarSource {
                name: "hn".to_string(),
                source_type: SourceType::Hackernews,
                enabled: true,
                description: "HackerNews".to_string(),
                max_items: 20,
                trust_weight: 0.9,
                feeds: vec![],
                categories: vec![],
                query: String::new(),
                min_score: 50,
                subreddits: vec![],
            },
            RadarSource {
                name: "rss".to_string(),
                source_type: SourceType::Rss,
                enabled: false,
                description: "RSS".to_string(),
                max_items: 10,
                trust_weight: 0.8,
                feeds: vec!["https://example.com/feed.xml".to_string()],
                categories: vec![],
                query: String::new(),
                min_score: 0,
                subreddits: vec![],
            },
        ];

        assert_eq!(manager.count(), 2);
        assert_eq!(manager.enabled_count(), 1);

        manager.toggle_source("rss", true).unwrap();
        assert_eq!(manager.enabled_count(), 2);
    }

    #[test]
    fn test_source_type_display() {
        assert_eq!(SourceType::Rss.to_string(), "rss");
        assert_eq!(SourceType::Hackernews.to_string(), "hackernews");
    }
}
