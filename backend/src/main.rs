mod api;
mod config;
mod core;
mod error;
mod infra;
mod models;
mod tools;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::AppConfig;

/// 应用共享上下文
pub struct AppContext {
    pub config: Arc<AppConfig>,
    // TODO: Phase 0 实现
    // pub db: Arc<SqliteStore>,
    // pub embedding: Arc<dyn EmbeddingProvider>,
    // pub llm: Arc<dyn LlmProvider>,
    // pub qdrant: Arc<QdrantStore>,
    // pub tantivy: Arc<TantivyIndex>,
    // pub file_watcher: Arc<FileWatcher>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 初始化日志
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "obsidian_brain=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("ObsidianBrain 启动中...");

    // 2. 加载配置
    let config = AppConfig::load().unwrap_or_else(|e| {
        tracing::warn!("配置加载失败: {e}，使用默认配置");
        AppConfig::default()
    });
    let addr = SocketAddr::from(([127, 0, 0, 1], config.server.port));
    tracing::info!("配置加载完成: {}:{}", addr.ip(), addr.port());

    // 3. 构建共享上下文
    let ctx = Arc::new(AppContext {
        config: Arc::new(config),
    });

    // 4. 构建路由
    let app = api::router::create_router(ctx);

    // 5. 启动服务
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("服务已启动: http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("ObsidianBrain 已关闭");
    Ok(())
}

/// 优雅关闭信号
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("收到 Ctrl+C 信号"),
        _ = terminate => tracing::info!("收到 SIGTERM 信号"),
    }
}
