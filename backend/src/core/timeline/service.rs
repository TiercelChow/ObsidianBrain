//! 时间线服务

use chrono::Utc;
use std::sync::Arc;

use crate::core::timeline::store::TimelineStore;
use crate::error::BrainError;
use crate::models::timeline::*;

/// 时间线服务配置
#[derive(Debug, Clone)]
pub struct TimelineConfig {
    pub date_formats: Vec<String>,
    pub retention_days: u32,
}

impl Default for TimelineConfig {
    fn default() -> Self {
        Self {
            date_formats: vec![
                "%Y-%m-%d".to_string(),
                "%Y/%m/%d".to_string(),
                "%Y%m%d".to_string(),
            ],
            retention_days: 365,
        }
    }
}

/// 时间线服务
pub struct TimelineService {
    store: Arc<TimelineStore>,
    config: TimelineConfig,
}

impl TimelineService {
    pub fn new(store: Arc<TimelineStore>, config: TimelineConfig) -> Self {
        Self { store, config }
    }

    /// 记录事件
    pub async fn record_event(&self, event: TimelineEvent) -> Result<(), BrainError> {
        self.store.insert_event(&event)
    }

    /// 记录多个事件
    pub async fn record_events(&self, events: Vec<TimelineEvent>) -> Result<usize, BrainError> {
        self.store.insert_events(&events)
    }

    /// 获取时间线
    pub async fn get_timeline(
        &self,
        request: GetTimelineRequest,
    ) -> Result<TimelineResponse, BrainError> {
        let daily_events = self
            .store
            .get_daily_events(&request.start_date, &request.end_date)?;
        let statistics = self
            .store
            .get_statistics(&request.start_date, &request.end_date)?;

        Ok(TimelineResponse {
            date_range: DateRange {
                start: request.start_date,
                end: request.end_date,
            },
            daily_events,
            statistics,
            summary: None,
        })
    }

    /// 获取事件列表
    pub async fn get_events(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Vec<TimelineEvent>, BrainError> {
        self.store.get_events(start, end)
    }

    /// 获取统计信息
    pub async fn get_statistics(
        &self,
        start: &str,
        end: &str,
    ) -> Result<TimelineStatistics, BrainError> {
        self.store.get_statistics(start, end)
    }

    /// 清理旧事件
    pub async fn cleanup_old_events(&self) -> Result<usize, BrainError> {
        let cutoff = Utc::now()
            .checked_sub_signed(chrono::Duration::days(self.config.retention_days as i64))
            .unwrap_or_else(Utc::now)
            .format("%Y-%m-%d")
            .to_string();
        self.store.delete_before(&cutoff)
    }
}
