//! 时间线工具处理器

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::error::BrainError;
use crate::models::timeline::GetTimelineRequest;
use crate::tools::definitions;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

/// 获取时间线
pub struct GetTimelineHandler;

#[async_trait]
impl ToolHandler for GetTimelineHandler {
    fn name(&self) -> &str { "get_timeline" }
    fn description(&self) -> &str { "获取时间线事件，支持日期范围和类型过滤" }
    fn input_schema(&self) -> Value { definitions::get_timeline_schema() }
    fn module(&self) -> &str { "timeline" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let start_date = args.get("start_date")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'start_date'".to_string()))?
            .to_string();
        let end_date = args.get("end_date")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'end_date'".to_string()))?
            .to_string();

        let request = GetTimelineRequest {
            start_date,
            end_date,
            event_types: None,
            tags: None,
            limit: 200,
        };

        tracing::debug!(start = %request.start_date, end = %request.end_date, "get_timeline 调用");
        let response = ctx.timeline_service.get_timeline(request).await?;

        let events_json: Vec<Value> = response.daily_events.iter().map(|de| {
            let events: Vec<Value> = de.events.iter().map(|e| {
                json!({
                    "id": e.id.to_string(),
                    "event_type": e.event_type.as_str(),
                    "title": e.title,
                    "summary": e.summary,
                    "tags": e.tags,
                    "related_paths": e.related_paths,
                })
            }).collect();
            json!({
                "date": de.date.to_string(),
                "event_count": de.event_count,
                "events": events,
            })
        }).collect();

        Ok(json!({
            "date_range": {
                "start": response.date_range.start,
                "end": response.date_range.end,
            },
            "daily_events": events_json,
            "statistics": {
                "total_events": response.statistics.total_events,
                "by_type": response.statistics.by_type,
                "active_days": response.statistics.active_days,
                "most_active_tags": response.statistics.most_active_tags,
                "daily_average": response.statistics.daily_average,
            },
        }))
    }
}
