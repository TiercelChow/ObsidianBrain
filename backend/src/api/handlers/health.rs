use axum::Json;
use serde_json::{json, Value};

use crate::AppContext;

/// 健康检查
pub async fn health_check(_ctx: axum::extract::State<std::sync::Arc<AppContext>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "components": {
            "server": "ok",
            "qdrant": "not_configured",
            "sqlite": "not_configured",
            "tantivy": "not_configured",
            "embedding": "not_configured",
            "llm": "not_configured"
        }
    }))
}
