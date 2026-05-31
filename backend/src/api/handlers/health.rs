use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppContext;

pub async fn health_check(State(ctx): State<Arc<AppContext>>) -> Json<Value> {
    // Collect component statuses then release the lock before any .await calls.
    let component_snapshot = {
        let components = ctx.components.lock().unwrap();
        (
            components.server.clone(),
            components.sqlite.clone(),
            components.qdrant.clone(),
            components.tantivy.clone(),
            components.embedding.clone(),
            components.llm.clone(),
        )
    };

    // Live check on SQLite and Tantivy
    let sqlite_status = if ctx.db.health_check() {
        component_snapshot.1.clone()
    } else {
        "unhealthy".to_string()
    };

    let tantivy_status = if ctx.tantivy.health_check() {
        component_snapshot.3.clone()
    } else {
        "unhealthy".to_string()
    };

    // Vault status
    let vault_path_str = ctx.config.vault.path.to_string_lossy().to_string();
    let vault_exists = ctx.config.vault.path.exists();

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
            "sqlite": sqlite_status,
            "qdrant": component_snapshot.2,
            "tantivy": tantivy_status,
            "embedding": component_snapshot.4,
            "llm": component_snapshot.5,
        },
        "vault": {
            "path": vault_path_str,
            "exists": vault_exists,
            "watching": ctx.vault_watcher.is_some(),
        }
    }))
}
