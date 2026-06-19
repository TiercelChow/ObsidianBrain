//! 时间线工具处理器

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::error::BrainError;
use crate::models::timeline::{BrowseTimelineRequest, GetTimelineRequest, MemoCreateRequest, MemoQuery};
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

/// 创建小记
pub struct CreateMemoHandler;

#[async_trait]
impl ToolHandler for CreateMemoHandler {
    fn name(&self) -> &str { "create_memo" }
    fn description(&self) -> &str { "创建一条小记，支持文本和图片" }
    fn input_schema(&self) -> Value { definitions::create_memo_schema() }
    fn module(&self) -> &str { "timeline" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let content = args.get("content")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'content'".to_string()))?
            .to_string();

        let images: Vec<String> = args.get("images")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let tags: Vec<String> = args.get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let request = MemoCreateRequest {
            content,
            images,
            tags,
        };

        tracing::debug!("create_memo 调用");
        let memo = ctx.memo_manager.create_memo(request).await?;

        Ok(json!({
            "id": memo.id,
            "timestamp": memo.timestamp.to_rfc3339(),
            "file_path": memo.file_path,
        }))
    }
}

/// 浏览时间线
pub struct BrowseTimelineHandler;

#[async_trait]
impl ToolHandler for BrowseTimelineHandler {
    fn name(&self) -> &str { "browse_timeline" }
    fn description(&self) -> &str { "浏览时间线，支持按时间范围筛选" }
    fn input_schema(&self) -> Value { definitions::browse_timeline_schema() }
    fn module(&self) -> &str { "timeline" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let start_date = args.get("start_date").and_then(|v| v.as_str()).map(String::from);
        let end_date = args.get("end_date").and_then(|v| v.as_str()).map(String::from);
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        let request = BrowseTimelineRequest {
            start_date,
            end_date,
            limit,
            offset,
        };

        tracing::debug!("browse_timeline 调用");
        let memos = ctx.memo_manager.browse_timeline(request).await?;

        let memos_json: Vec<Value> = memos.iter().map(|m| {
            json!({
                "id": m.id,
                "timestamp": m.timestamp.to_rfc3339(),
                "content": m.content,
                "images": m.images,
                "tags": m.tags,
            })
        }).collect();

        Ok(json!({
            "memos": memos_json,
            "total": memos.len(),
            "has_more": memos.len() == 20,
        }))
    }
}

/// 搜索小记
pub struct SearchMemosHandler;

#[async_trait]
impl ToolHandler for SearchMemosHandler {
    fn name(&self) -> &str { "search_memos" }
    fn description(&self) -> &str { "搜索小记内容" }
    fn input_schema(&self) -> Value { definitions::search_memos_schema() }
    fn module(&self) -> &str { "timeline" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'query'".to_string()))?
            .to_string();

        let start_date = args.get("start_date").and_then(|v| v.as_str()).map(String::from);
        let end_date = args.get("end_date").and_then(|v| v.as_str()).map(String::from);
        let tags: Option<Vec<String>> = args.get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

        let query_obj = MemoQuery {
            query: Some(query),
            start_date,
            end_date,
            tags,
            limit,
            offset: 0,
        };

        tracing::debug!("search_memos 调用");
        let memos = ctx.memo_manager.search_memos(query_obj).await?;

        let memos_json: Vec<Value> = memos.iter().map(|m| {
            json!({
                "id": m.id,
                "timestamp": m.timestamp.to_rfc3339(),
                "content": m.content,
                "images": m.images,
                "tags": m.tags,
                "score": 1.0, // TODO: implement relevance scoring
            })
        }).collect();

        Ok(json!({
            "memos": memos_json,
            "total": memos.len(),
        }))
    }
}

/// 从 Obsidian 同步小记
pub struct SyncMemosHandler;

#[async_trait]
impl ToolHandler for SyncMemosHandler {
    fn name(&self) -> &str { "sync_memos" }
    fn description(&self) -> &str { "从 Obsidian Timeline 文件夹同步小记到数据库" }
    fn input_schema(&self) -> Value { definitions::sync_memos_schema() }
    fn module(&self) -> &str { "timeline" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let months = args.get("months").and_then(|v| v.as_u64()).unwrap_or(3) as u32;

        tracing::info!(months = months, "sync_memos 调用");
        let (synced, deleted) = ctx.memo_manager.sync_from_obsidian(months).await?;

        Ok(json!({
            "synced": synced,
            "deleted": deleted,
            "months": months,
        }))
    }
}

/// 获取小记统计信息
pub struct GetMemoStatsHandler;

#[async_trait]
impl ToolHandler for GetMemoStatsHandler {
    fn name(&self) -> &str { "get_memo_stats" }
    fn description(&self) -> &str { "获取小记统计信息（总数等）" }
    fn input_schema(&self) -> Value { definitions::get_memo_stats_schema() }
    fn module(&self) -> &str { "timeline" }

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let total = ctx.memo_manager.count_memos()?;
        Ok(json!({
            "total_memos": total,
        }))
    }
}