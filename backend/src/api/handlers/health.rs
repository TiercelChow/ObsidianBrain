use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppContext;

pub async fn health_check(State(ctx): State<Arc<AppContext>>) -> Json<Value> {
    // Collect component statuses then release the lock before any .await calls.
    let component_snapshot = {
        let components = ctx.components.lock().unwrap();
        (components.server.clone(), components.sqlite.clone())
    };

    let data_path = ctx
        .config
        .storage
        .db_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));

    let uptime_seconds = chrono::Utc::now()
        .signed_duration_since(ctx.start_time)
        .num_seconds();

    let tools_count = ctx.tool_registry.count().await;

    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "tools_count": tools_count,
        "uptime_seconds": uptime_seconds,
        "components": {
            "server": component_snapshot.0,
            "sqlite": component_snapshot.1,
        },
        "storage": {
            "path": data_path,
            "exists": data_path.exists(),
        }
    }))
}
