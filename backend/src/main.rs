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
use crate::core::code_repo::manager::{RepoManager, RepoManagerConfig};
use crate::core::code_repo::note_linker::NoteLinker;
use crate::core::memory_service::MemoryService;
use crate::core::timeline::store::TimelineStore;
use crate::core::timeline::{TimelineConfig, TimelineService};
use crate::infra::obsidian_client::ObsidianClient;
use crate::infra::sqlite_store::SqliteStore;
use crate::tools::handlers::register_all_tools;
use crate::tools::registry::ToolRegistry;

/// Application shared context — injected into all handlers.
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub components: Arc<std::sync::Mutex<ComponentStatus>>,
    pub tool_registry: Arc<ToolRegistry>,
    pub memory_service: Arc<MemoryService>,
    pub repo_manager: Arc<RepoManager>,
    pub note_linker: Arc<NoteLinker>,
    pub timeline_service: Arc<TimelineService>,
    /// Server start time — used to compute uptime in health endpoint.
    pub start_time: chrono::DateTime<chrono::Utc>,
}

/// Tracks which components are operational.
#[derive(Debug, Clone, Default)]
pub struct ComponentStatus {
    pub server: String,
    pub obsidian: String,
    pub sqlite: String,
    pub timeline: String,
    pub code_repo: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Init logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "obsidian_brain=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("ObsidianBrain 启动中...");

    // Record start time for uptime calculation
    let start_time = chrono::Utc::now();

    // 2. Load config
    let config = AppConfig::load().unwrap_or_else(|e| {
        tracing::warn!("配置加载失败: {e}，使用默认配置");
        AppConfig::default()
    });
    let addr = SocketAddr::from(([127, 0, 0, 1], config.server.port));
    tracing::info!("配置加载完成: {}:{}", addr.ip(), addr.port());

    // 3. Initialize infrastructure
    let mut components = ComponentStatus {
        server: "ok".to_string(),
        obsidian: "disabled".to_string(),
        sqlite: "pending".to_string(),
        timeline: "pending".to_string(),
        code_repo: "pending".to_string(),
    };

    // Obsidian Local REST API (required)
    let obsidian = if config.obsidian.enabled {
        match ObsidianClient::new(&config.obsidian) {
            Ok(client) => {
                let client = Arc::new(client);
                // Async health check
                if client.health_check().await {
                    components.obsidian = "ok".to_string();
                    tracing::info!("Obsidian API 连接成功: {}", config.obsidian.url);
                    Some(client)
                } else {
                    components.obsidian = "degraded: 无法连接".to_string();
                    tracing::warn!(
                        "Obsidian API 无法连接: {} (插件是否已启用?)",
                        config.obsidian.url
                    );
                    Some(client) // Still provide client, just degraded
                }
            }
            Err(e) => {
                components.obsidian = format!("error: {e}");
                tracing::error!("Obsidian API 客户端创建失败: {e}");
                None
            }
        }
    } else {
        tracing::warn!("Obsidian API 客户端未启用，搜索功能将不可用");
        None
    };

    // 4. Initialize SQLite
    let db = match SqliteStore::new(&config.storage.db_path) {
        Ok(store) => {
            components.sqlite = "ok".to_string();
            tracing::info!("SQLite 初始化成功: {:?}", config.storage.db_path);
            Arc::new(store)
        }
        Err(e) => {
            tracing::error!("SQLite 初始化失败: {e}");
            components.sqlite = format!("error: {e}");
            // SQLite is required, so fail fast
            std::process::exit(1);
        }
    };

    // 5. Create core services
    let memory_service = Arc::new(MemoryService::new(
        obsidian,
        config.vault.path.clone(),
        config.vault.name.clone(),
    ));
    tracing::info!("MemoryService 初始化完成");

    let repo_manager = Arc::new(RepoManager::new(db.clone(), RepoManagerConfig::default()));
    let note_linker = Arc::new(NoteLinker::new(db.clone()));
    let timeline_store = Arc::new(TimelineStore::new(db.clone()));
    let timeline_service = Arc::new(TimelineService::new(
        timeline_store,
        TimelineConfig::default(),
    ));
    tracing::info!("CodeRepo & Timeline 服务初始化完成");

    // 6. Build AppContext and register tools
    let tool_registry = Arc::new(ToolRegistry::new());

    let ctx = Arc::new(AppContext {
        config: Arc::new(config),
        components: Arc::new(std::sync::Mutex::new(components)),
        tool_registry: tool_registry.clone(),
        memory_service,
        repo_manager,
        note_linker,
        timeline_service,
        start_time,
    });

    register_all_tools(&tool_registry, ctx.clone()).await;
    tracing::info!("已注册 {} 个工具", ctx.tool_registry.count().await);

    // 6. Build router and serve
    let app = api::router::create_router(ctx);

    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("绑定地址失败: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("服务已启动: http://{}", addr);

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        tracing::error!("服务运行失败: {e}");
        std::process::exit(1);
    }

    tracing::info!("ObsidianBrain 已关闭");
    Ok(())
}

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

// ── Test helpers ──

#[cfg(test)]
mod test_helpers {
    use super::*;

    /// Create an AppContext with minimal stubs for unit/integration tests.
    /// Returns (Arc<AppContext>, TempDir, vault_path) — caller must keep TempDir alive.
    impl AppContext {
        pub fn for_test() -> (Arc<Self>, tempfile::TempDir, std::path::PathBuf) {
            let dir = tempfile::tempdir().expect("tempdir creation");
            let vault_path = dir.path().join("vault");
            std::fs::create_dir_all(&vault_path).expect("vault dir creation");

            let mut config = AppConfig::default();
            config.vault.path = vault_path.clone();
            config.vault.name = "TestVault".to_string();
            config.obsidian.enabled = false;

            // Create a disabled Obsidian client for testing
            let obsidian = Arc::new(
                ObsidianClient::new(&config.obsidian).unwrap_or_else(|_| {
                    // Create a dummy client that will fail on all calls
                    ObsidianClient::new(&crate::config::ObsidianApiConfig {
                        enabled: true,
                        url: "http://127.0.0.1:1".to_string(), // unreachable
                        api_key: None,
                    })
                    .expect("dummy client creation")
                }),
            );

            let memory_service = Arc::new(MemoryService::new(
                Some(obsidian),
                vault_path.clone(),
                "TestVault".to_string(),
            ));

            let db = Arc::new(
                SqliteStore::new(&dir.path().join("test.db")).expect("SQLite creation"),
            );
            let repo_manager = Arc::new(RepoManager::new(db.clone(), RepoManagerConfig::default()));
            let note_linker = Arc::new(NoteLinker::new(db.clone()));
            let timeline_store = Arc::new(TimelineStore::new(db.clone()));
            let timeline_service = Arc::new(TimelineService::new(
                timeline_store,
                TimelineConfig::default(),
            ));

            let ctx = Arc::new(AppContext {
                config: Arc::new(config),
                components: Arc::new(std::sync::Mutex::new(ComponentStatus::default())),
                tool_registry: Arc::new(ToolRegistry::new()),
                memory_service,
                repo_manager,
                note_linker,
                timeline_service,
                start_time: chrono::Utc::now(),
            });

            (ctx, dir, vault_path)
        }
    }
}
