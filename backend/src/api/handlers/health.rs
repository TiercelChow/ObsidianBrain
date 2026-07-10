use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppContext;

pub async fn health_check(State(ctx): State<Arc<AppContext>>) -> Json<Value> {
    // Collect component statuses then release the lock before any .await calls.
    let component_snapshot = {
        let components = ctx.components.lock().unwrap();
        (components.server.clone(), components.obsidian.clone())
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
            "obsidian": component_snapshot.1,
        },
        "vault": {
            "path": vault_path_str,
            "exists": vault_exists,
        }
    }))
}
