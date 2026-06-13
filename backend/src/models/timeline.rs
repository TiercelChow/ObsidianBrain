//! 时间线相关数据模型

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// 事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    NoteCreated,
    NoteModified,
    RepoCommit,
    RadarSaved,
    MemoryCreated,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::NoteCreated => "note_created",
            EventType::NoteModified => "note_modified",
            EventType::RepoCommit => "repo_commit",
            EventType::RadarSaved => "radar_saved",
            EventType::MemoryCreated => "memory_created",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "note_created" => EventType::NoteCreated,
            "note_modified" => EventType::NoteModified,
            "repo_commit" => EventType::RepoCommit,
            "radar_saved" => EventType::RadarSaved,
            "memory_created" => EventType::MemoryCreated,
            _ => EventType::NoteModified,
        }
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 时间线事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: Uuid,
    pub date: NaiveDate,
    pub timestamp: Option<DateTime<Utc>>,
    pub event_type: EventType,
    pub title: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub related_paths: Vec<String>,
    pub source: String,
    pub metadata: serde_json::Value,
}

/// 每日事件集合
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyEvents {
    pub date: NaiveDate,
    pub event_count: usize,
    pub events: Vec<TimelineEvent>,
}

/// 时间线查询请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTimelineRequest {
    pub start_date: String,
    pub end_date: String,
    #[serde(default)]
    pub event_types: Option<Vec<String>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    200
}

/// 日期范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

/// 时段对比
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodComparison {
    pub current_count: usize,
    pub previous_count: usize,
    pub change_percent: f32,
}

/// 时间线统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineStatistics {
    pub total_events: usize,
    pub by_type: HashMap<String, usize>,
    pub active_days: usize,
    pub most_active_tags: Vec<String>,
    pub daily_average: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_comparison: Option<PeriodComparison>,
}

/// 时间线响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineResponse {
    pub date_range: DateRange,
    pub daily_events: Vec<DailyEvents>,
    pub statistics: TimelineStatistics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

// ─── 时光机（Time Machine）模型 ───

/// 小记（Memo）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memo {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub date: String,
    pub content: String,
    pub images: Vec<String>,
    pub tags: Vec<String>,
    pub file_path: String,
    pub created_at: DateTime<Utc>,
}

/// 小记查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoQuery {
    pub query: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub tags: Option<Vec<String>>,
    pub limit: usize,
    pub offset: usize,
}

/// 创建小记请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoCreateRequest {
    pub content: String,
    #[serde(default)]
    pub images: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// 时间线浏览请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowseTimelineRequest {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_display() {
        assert_eq!(EventType::NoteCreated.to_string(), "note_created");
        assert_eq!(EventType::RepoCommit.to_string(), "repo_commit");
    }

    #[test]
    fn test_event_type_from_str() {
        assert_eq!(EventType::from_str("note_created"), EventType::NoteCreated);
        assert_eq!(EventType::from_str("unknown"), EventType::NoteModified);
    }

    #[test]
    fn test_timeline_event_roundtrip() {
        let event = TimelineEvent {
            id: Uuid::new_v4(),
            date: NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
            timestamp: Some(Utc::now()),
            event_type: EventType::NoteCreated,
            title: "Test Event".to_string(),
            summary: "A test event".to_string(),
            tags: vec!["test".to_string()],
            related_paths: vec!["test.md".to_string()],
            source: "test".to_string(),
            metadata: serde_json::json!({}),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: TimelineEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.title, "Test Event");
        assert_eq!(parsed.event_type, EventType::NoteCreated);
    }
}
