use crate::{core::home::overview_date, error::BrainError, infra::home_store, AppContext};
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct HomeQuery {
    today: Option<String>,
}

pub async fn home_overview(
    State(ctx): State<Arc<AppContext>>,
    Query(query): Query<HomeQuery>,
) -> Result<Json<Value>, BrainError> {
    let day = overview_date(query.today.as_deref())?;
    let db = ctx.db.clone();
    let overview = tokio::task::spawn_blocking(move || home_store::overview(&db, day))
        .await
        .map_err(|error| BrainError::Internal(format!("首页读取任务失败: {error}")))??;
    let components = ctx
        .components
        .lock()
        .map_err(|_| BrainError::Internal("系统状态暂不可用".into()))?;
    let mut value =
        serde_json::to_value(overview).map_err(|e| BrainError::Internal(e.to_string()))?;
    value["system"] = json!({"version":env!("CARGO_PKG_VERSION"),"uptime_seconds":(chrono::Utc::now()-ctx.start_time).num_seconds().max(0),"components":{"server":components.server,"sqlite":components.sqlite}});
    Ok(Json(value))
}

/// Explicit memo locator: read one record even when it is outside the first page.
pub async fn home_memo(
    State(ctx): State<Arc<AppContext>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<crate::models::timeline::Memo>, BrainError> {
    if id.len() > 200 {
        return Err(BrainError::MemoValidation("小记 ID 过长".into()));
    }
    let manager = ctx.memo_manager.clone();
    tokio::task::spawn_blocking(move || manager.get_memo(&id))
        .await
        .map_err(|e| BrainError::Internal(e.to_string()))?
        .map(Json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_home_task_focus_is_applied_before_pagination() {
        use crate::models::task::{TaskFocus, TaskQuery};
        let (ctx, _dir, _old) = AppContext::for_test();
        let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
        ctx.db.with_connection(|conn| {
            conn.execute("INSERT INTO task_documents(path,document_kind,revision,content_hash,indexed_at) VALUES('doc','long_task',1,'hash','2026-10-04')",[])?;
            for n in 0..8 {
                let id=uuid::Uuid::new_v4().to_string();
                conn.execute("INSERT INTO task_nodes(id,root_id,storage_path,kind,role,title,status,importance,start_date,end_date,position,created_at,updated_at,revision) VALUES(?1,?1,'doc','long','root',?2,?3,'normal','2026-10-01',?4,0,'2026-10-01T00:00:00Z','2026-10-04T00:00:00Z',1)",rusqlite::params![id,format!("Task {n}"),if n==0 {"completed"} else {"planned"},if n < 4 {"2026-10-03"} else {"2026-10-04"}])?;
            }
            Ok(())
        }).unwrap();
        let result = ctx
            .task_service
            .list_tasks(TaskQuery {
                focus: Some(TaskFocus::Today),
                focus_date: Some(day),
                limit: 2,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(result.tasks.len(), 2);
        assert_eq!(result.next_cursor.as_deref(), Some("2"));
        assert!(result.tasks.iter().all(|t| t.node.end_date == day));
        let result = ctx
            .task_service
            .list_tasks(TaskQuery {
                focus: Some(TaskFocus::Overdue),
                focus_date: Some(day),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(result.tasks.len(), 3);
    }

    #[tokio::test]
    async fn test_home_http_is_read_only_bounded_and_rejects_invalid_dates() {
        let (ctx, _dir, _old) = AppContext::for_test();
        ctx.db
            .set_state("system_config", r#"{"api_key":"must-not-leak"}"#)
            .unwrap();
        let before = ctx.db.get_state("system_config").unwrap();
        let app = crate::api::router::create_router(ctx.clone());
        let response = app
            .clone()
            .oneshot(
                Request::get("/v1/home/overview?today=2026-10-04")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        let data: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(data["today"], "2026-10-04");
        assert_eq!(data["tasks"]["data"]["active_count"], 0);
        assert!(data["system"]["version"].is_string());
        assert!(!String::from_utf8_lossy(&bytes).contains("must-not-leak"));
        assert_eq!(ctx.db.get_state("system_config").unwrap(), before);
        let response = app
            .clone()
            .oneshot(
                Request::get("/v1/home/overview?today=2026-02-30")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::UNPROCESSABLE_ENTITY
        );
        let response = app
            .oneshot(
                Request::get("/v1/timeline/memos/not-found")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }
}
