//! 时间线事件存储

use chrono::{NaiveDate, Utc};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::BrainError;
use crate::infra::sqlite_store::SqliteStore;
use crate::models::timeline::*;

/// 时间线事件存储
pub struct TimelineStore {
    db: Arc<SqliteStore>,
}

impl TimelineStore {
    /// 创建新的时间线存储
    pub fn new(db: Arc<SqliteStore>) -> Self {
        Self { db }
    }

    /// 插入事件
    pub fn insert_event(&self, event: &TimelineEvent) -> Result<(), BrainError> {
        let tags_json = serde_json::to_string(&event.tags)
            .map_err(|e| BrainError::Internal(format!("序列化 tags 失败: {e}")))?;
        let paths_json = serde_json::to_string(&event.related_paths)
            .map_err(|e| BrainError::Internal(format!("序列化 paths 失败: {e}")))?;

        self.db.insert_timeline_event(
            &event.id.to_string(),
            &event.date.to_string(),
            event.event_type.as_str(),
            &event.title,
            &event.summary,
            &tags_json,
            &paths_json,
            &event.source,
        )?;
        Ok(())
    }

    /// 批量插入事件
    pub fn insert_events(&self, events: &[TimelineEvent]) -> Result<usize, BrainError> {
        let mut count = 0;
        for event in events {
            self.insert_event(event)?;
            count += 1;
        }
        Ok(count)
    }

    /// 按日期范围查询事件
    pub fn get_events(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<TimelineEvent>, BrainError> {
        let rows = self.db.get_timeline_events(start_date, end_date)?;
        let mut events = Vec::new();

        for (id, date, event_type, title, summary, tags_json, paths_json, source) in rows {
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            let related_paths: Vec<String> = serde_json::from_str(&paths_json).unwrap_or_default();

            events.push(TimelineEvent {
                id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
                date: NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                    .unwrap_or_else(|_| Utc::now().date_naive()),
                timestamp: None,
                event_type: EventType::from_str(&event_type),
                title,
                summary,
                tags,
                related_paths,
                source,
                metadata: serde_json::json!({}),
            });
        }

        Ok(events)
    }

    /// 按日期范围查询并按天分组
    pub fn get_daily_events(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<DailyEvents>, BrainError> {
        let events = self.get_events(start_date, end_date)?;
        let mut daily_map: std::collections::BTreeMap<String, Vec<TimelineEvent>> =
            std::collections::BTreeMap::new();

        for event in events {
            let date_str = event.date.to_string();
            daily_map.entry(date_str).or_default().push(event);
        }

        let daily_events: Vec<DailyEvents> = daily_map
            .into_iter()
            .map(|(date_str, events)| {
                let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                    .unwrap_or_else(|_| Utc::now().date_naive());
                DailyEvents {
                    date,
                    event_count: events.len(),
                    events,
                }
            })
            .collect();

        Ok(daily_events)
    }

    /// 删除指定日期之前的事件
    pub fn delete_before(&self, before_date: &str) -> Result<usize, BrainError> {
        self.db.delete_timeline_events_before(before_date)
    }

    /// 计算统计数据
    pub fn get_statistics(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<TimelineStatistics, BrainError> {
        let events = self.get_events(start_date, end_date)?;
        let total_events = events.len();

        // 按类型统计
        let mut by_type: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for event in &events {
            *by_type.entry(event.event_type.to_string()).or_insert(0) += 1;
        }

        // 活跃天数
        let mut dates: std::collections::HashSet<String> = std::collections::HashSet::new();
        for event in &events {
            dates.insert(event.date.to_string());
        }
        let active_days = dates.len();

        // 高频标签
        let mut tag_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for event in &events {
            for tag in &event.tags {
                *tag_counts.entry(tag.clone()).or_insert(0) += 1;
            }
        }
        let mut most_active_tags: Vec<(String, usize)> = tag_counts.into_iter().collect();
        most_active_tags.sort_by(|a, b| b.1.cmp(&a.1));
        let most_active_tags: Vec<String> = most_active_tags
            .into_iter()
            .take(10)
            .map(|(tag, _)| tag)
            .collect();

        // 日均事件数
        let daily_average = if active_days > 0 {
            total_events as f32 / active_days as f32
        } else {
            0.0
        };

        Ok(TimelineStatistics {
            total_events,
            by_type,
            active_days,
            most_active_tags,
            daily_average,
            period_comparison: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_store() -> (TempDir, TimelineStore) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Arc::new(SqliteStore::new(&db_path).unwrap());
        let store = TimelineStore::new(db);
        (dir, store)
    }

    fn create_test_event(date: &str, title: &str, event_type: EventType) -> TimelineEvent {
        TimelineEvent {
            id: Uuid::new_v4(),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            timestamp: Some(Utc::now()),
            event_type,
            title: title.to_string(),
            summary: format!("Summary for {}", title),
            tags: vec!["test".to_string()],
            related_paths: vec![format!("{}.md", title.to_lowercase())],
            source: "test".to_string(),
            metadata: serde_json::json!({}),
        }
    }

    #[test]
    fn test_insert_and_query() {
        let (_dir, store) = create_store();
        let event = create_test_event("2026-05-31", "Test Note", EventType::NoteCreated);
        store.insert_event(&event).unwrap();

        let events = store.get_events("2026-05-01", "2026-06-01").unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Test Note");
        assert_eq!(events[0].event_type, EventType::NoteCreated);
    }

    #[test]
    fn test_batch_insert() {
        let (_dir, store) = create_store();
        let events = vec![
            create_test_event("2026-05-31", "Note 1", EventType::NoteCreated),
            create_test_event("2026-05-31", "Note 2", EventType::NoteModified),
            create_test_event("2026-06-01", "Note 3", EventType::NoteCreated),
        ];

        let count = store.insert_events(&events).unwrap();
        assert_eq!(count, 3);

        let all = store.get_events("2026-05-01", "2026-06-30").unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_date_range_filter() {
        let (_dir, store) = create_store();
        store
            .insert_event(&create_test_event(
                "2026-05-15",
                "May",
                EventType::NoteCreated,
            ))
            .unwrap();
        store
            .insert_event(&create_test_event(
                "2026-06-15",
                "June",
                EventType::NoteCreated,
            ))
            .unwrap();

        let may_events = store.get_events("2026-05-01", "2026-05-31").unwrap();
        assert_eq!(may_events.len(), 1);
        assert_eq!(may_events[0].title, "May");

        let june_events = store.get_events("2026-06-01", "2026-06-30").unwrap();
        assert_eq!(june_events.len(), 1);
        assert_eq!(june_events[0].title, "June");
    }

    #[test]
    fn test_daily_events_grouping() {
        let (_dir, store) = create_store();
        store
            .insert_event(&create_test_event(
                "2026-05-31",
                "A",
                EventType::NoteCreated,
            ))
            .unwrap();
        store
            .insert_event(&create_test_event(
                "2026-05-31",
                "B",
                EventType::NoteModified,
            ))
            .unwrap();
        store
            .insert_event(&create_test_event(
                "2026-06-01",
                "C",
                EventType::NoteCreated,
            ))
            .unwrap();

        let daily = store.get_daily_events("2026-05-01", "2026-06-30").unwrap();
        assert_eq!(daily.len(), 2);
        assert_eq!(daily[0].event_count, 2);
        assert_eq!(daily[1].event_count, 1);
    }

    #[test]
    fn test_statistics() {
        let (_dir, store) = create_store();
        store
            .insert_event(&create_test_event(
                "2026-05-31",
                "A",
                EventType::NoteCreated,
            ))
            .unwrap();
        store
            .insert_event(&create_test_event(
                "2026-05-31",
                "B",
                EventType::NoteModified,
            ))
            .unwrap();
        store
            .insert_event(&create_test_event("2026-06-01", "C", EventType::RepoCommit))
            .unwrap();

        let stats = store.get_statistics("2026-05-01", "2026-06-30").unwrap();
        assert_eq!(stats.total_events, 3);
        assert_eq!(stats.active_days, 2);
        assert!(stats.by_type.contains_key("note_created"));
        assert!(stats.most_active_tags.contains(&"test".to_string()));
    }

    #[test]
    fn test_delete_before() {
        let (_dir, store) = create_store();
        store
            .insert_event(&create_test_event(
                "2026-05-01",
                "Old",
                EventType::NoteCreated,
            ))
            .unwrap();
        store
            .insert_event(&create_test_event(
                "2026-06-01",
                "New",
                EventType::NoteCreated,
            ))
            .unwrap();

        let deleted = store.delete_before("2026-05-15").unwrap();
        assert_eq!(deleted, 1);

        let remaining = store.get_events("2026-01-01", "2026-12-31").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].title, "New");
    }
}
