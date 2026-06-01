//! 智识雷达相关数据模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// 雷达条目状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RadarStatus {
    New,
    Read,
    Saved,
    Dismissed,
}

impl RadarStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RadarStatus::New => "new",
            RadarStatus::Read => "read",
            RadarStatus::Saved => "saved",
            RadarStatus::Dismissed => "dismissed",
        }
    }
}

impl std::fmt::Display for RadarStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 雷达条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarItem {
    pub id: Uuid,
    pub title: String,
    pub summary: String,
    pub source_name: String,
    pub url: String,
    pub status: RadarStatus,
    pub relevance_score: Option<f32>,
    pub published_at: Option<DateTime<Utc>>,
    pub saved_path: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

/// 雷达条目视图（返回给前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarItemView {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub url: String,
    pub relevance_score: f32,
    pub published_at: Option<String>,
    pub status: String,
}

/// 原始文章（抓取器输出）
#[derive(Debug, Clone)]
pub struct RawArticle {
    pub title: String,
    pub summary: Option<String>,
    pub url: String,
    pub source_name: String,
    pub author: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
}

/// 雷达源配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarSource {
    pub name: String,
    pub source_type: String,
    pub enabled: bool,
    pub description: String,
    pub max_items: usize,
    pub trust_weight: f32,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// 雷达配置
#[derive(Debug, Clone)]
pub struct RadarConfig {
    pub sources_path: PathBuf,
    pub fetch_interval_hours: u32,
    pub relevance_threshold: f32,
    pub max_items_per_source: usize,
    pub retention_days: u32,
}

impl Default for RadarConfig {
    fn default() -> Self {
        Self {
            sources_path: PathBuf::from("config/radar_sources.toml"),
            fetch_interval_hours: 6,
            relevance_threshold: 0.7,
            max_items_per_source: 20,
            retention_days: 90,
        }
    }
}

/// 拉取报告
#[derive(Debug, Clone, Serialize)]
pub struct FetchReport {
    pub successful_sources: usize,
    pub failed_sources: usize,
    pub new_items: usize,
    pub total_fetched: usize,
    pub errors: Vec<String>,
}

/// 保存结果
#[derive(Debug, Clone, Serialize)]
pub struct VaultSaveResult {
    pub note_path: String,
    pub obsidian_uri: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub word_count: usize,
}

/// SQLite 行模型
#[derive(Debug, Clone)]
pub struct RadarItemRow {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub source_name: String,
    pub url: String,
    pub status: String,
    pub relevance_score: Option<f32>,
    pub published_at: Option<String>,
    pub saved_path: Option<String>,
    pub fetched_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radar_status_display() {
        assert_eq!(RadarStatus::New.to_string(), "new");
        assert_eq!(RadarStatus::Read.to_string(), "read");
        assert_eq!(RadarStatus::Saved.to_string(), "saved");
        assert_eq!(RadarStatus::Dismissed.to_string(), "dismissed");
    }

    #[test]
    fn test_radar_item_roundtrip() {
        let item = RadarItem {
            id: Uuid::new_v4(),
            title: "Test Article".to_string(),
            summary: "A test article".to_string(),
            source_name: "hackernews".to_string(),
            url: "https://example.com".to_string(),
            status: RadarStatus::New,
            relevance_score: Some(0.85),
            published_at: Some(Utc::now()),
            saved_path: None,
            fetched_at: Utc::now(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: RadarItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.title, "Test Article");
        assert_eq!(parsed.status, RadarStatus::New);
    }
}
