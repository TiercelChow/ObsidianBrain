mod api;
mod config;
mod core;
mod daemon;
mod error;
mod frontend_assets;
mod infra;
mod models;
mod paths;
mod tools;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::AppConfig;
use crate::core::code_repo::manager::{RepoManager, RepoManagerConfig};
use crate::core::code_repo::note_linker::NoteLinker;
use crate::core::inspiration::InspirationService;
use crate::core::memory_service::MemoryService;
use crate::core::radar::RadarService;
use crate::core::timeline::store::TimelineStore;
use crate::core::timeline::{MemoManager, TimelineConfig, TimelineService};
use crate::infra::obsidian_client::{new_provider, ObsidianClient};
use crate::infra::sqlite_store::SqliteStore;
use crate::tools::handlers::register_all_tools;
use crate::tools::registry::ToolRegistry;

/// Application shared context — injected into all handlers.
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub components: Arc<std::sync::Mutex<ComponentStatus>>,
    pub tool_registry: Arc<ToolRegistry>,
    pub db: Arc<SqliteStore>,
    pub obsidian: crate::infra::obsidian_client::ObsidianProvider,
    pub memory_service: Arc<MemoryService>,
    pub repo_manager: Arc<RepoManager>,
    pub note_linker: Arc<NoteLinker>,
    pub timeline_service: Arc<TimelineService>,
    pub memo_manager: Arc<MemoManager>,
    pub inspiration_service: Arc<InspirationService>,
    pub radar_service: Arc<RadarService>,
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

// ── CLI ───────────────────────────────────────────────────────────────

#[derive(clap::Parser)]
#[command(
    name = "obsidian-brain",
    version,
    about = "Local Rust knowledge engine with LLM Tool API for Obsidian"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Command>,

    /// Override the bind host (used when no subcommand is given).
    #[arg(long, global = true)]
    host: Option<String>,

    /// Override the bind port (used when no subcommand is given).
    #[arg(long, global = true)]
    port: Option<u16>,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Start the server (background by default).
    Start {
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        /// Run in the foreground (don't daemonize).
        #[arg(long)]
        foreground: bool,
    },
    /// Stop the running server.
    Stop,
    /// Show server status.
    Status,
    /// View or modify configuration.
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
    /// Print version information.
    Version,
}

#[derive(clap::Subcommand)]
enum ConfigCmd {
    /// Show all configuration.
    Show,
    /// Get a specific config value.
    Get { key: String },
    /// Set a config value (persisted to the database).
    Set { key: String, value: String },
}

fn main() {
    let cli = <Cli as clap::Parser>::parse();

    match cli.cmd {
        None => {
            // No subcommand — default to foreground start (dev mode: `cargo run`).
            init_logging();
            run_server(cli.host, cli.port);
        }
        Some(Command::Start {
            host,
            port,
            foreground,
        }) => {
            if foreground {
                // Run in foreground — init logging to stderr, run server directly.
                init_logging();
                run_server(host, port);
            } else {
                // Daemonize.
                if daemon::is_running() {
                    eprintln!(
                        "ObsidianBrain is already running (PID: {:?})",
                        daemon::read_pid()
                    );
                    std::process::exit(1);
                }
                match daemon::daemonize() {
                    Ok(0) => {
                        // We're the child — init logging and run.
                        init_logging();
                        run_server(host, port);
                    }
                    Ok(child_pid) => {
                        // We're the parent — child is running.
                        println!("ObsidianBrain started (PID: {child_pid})");
                        println!("Log: {}", paths::log_file().display());
                    }
                    Err(e) => {
                        eprintln!("Failed to start daemon: {e}");
                        std::process::exit(1);
                    }
                }
            }
        }
        Some(Command::Stop) => match daemon::stop() {
            Ok(()) => println!("ObsidianBrain stopped."),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!("ObsidianBrain is not running.");
            }
            Err(e) => {
                eprintln!("Failed to stop: {e}");
                std::process::exit(1);
            }
        },
        Some(Command::Status) => {
            show_status();
        }
        Some(Command::Config { action }) => {
            config_command(action);
        }
        Some(Command::Version) => {
            println!("obsidian-brain {}", env!("CARGO_PKG_VERSION"));
            println!("Data directory: {}", paths::data_dir().display());
        }
    }
}

fn init_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "obsidian_brain=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn run_server(host_override: Option<String>, port_override: Option<u16>) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create Tokio runtime");
    rt.block_on(async {
        if let Err(e) = run_server_async(host_override, port_override).await {
            tracing::error!("Fatal error: {e}");
            std::process::exit(1);
        }
    });
}

async fn run_server_async(
    host_override: Option<String>,
    port_override: Option<u16>,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("ObsidianBrain 启动中...");

    let start_time = chrono::Utc::now();

    // Load config
    let mut config = AppConfig::load().unwrap_or_else(|e| {
        tracing::warn!("配置加载失败: {e}，使用默认配置");
        AppConfig::default()
    });

    // Apply CLI overrides
    if let Some(h) = host_override {
        config.server.host = h;
    }
    if let Some(p) = port_override {
        config.server.port = p;
    }

    let host: std::net::IpAddr = config
        .server
        .host
        .parse()
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    let addr = SocketAddr::new(host, config.server.port);
    tracing::info!("配置加载完成: {}:{}", addr.ip(), addr.port());

    // Initialize SQLite
    let mut components = ComponentStatus {
        server: "ok".to_string(),
        obsidian: "disabled".to_string(),
        sqlite: "pending".to_string(),
        timeline: "pending".to_string(),
        code_repo: "pending".to_string(),
    };
    let db = match SqliteStore::new(&config.storage.db_path) {
        Ok(store) => {
            components.sqlite = "ok".to_string();
            tracing::info!("SQLite 初始化成功: {:?}", config.storage.db_path);
            Arc::new(store)
        }
        Err(e) => {
            tracing::error!("SQLite 初始化失败: {e}");
            std::process::exit(1);
        }
    };

    // Load saved config from DB
    if let Ok(Some(saved_json)) = db.get_state("system_config") {
        if let Ok(saved) = serde_json::from_str::<serde_json::Value>(&saved_json) {
            if let Some(vault) = saved.get("vault") {
                if let Some(path) = vault.get("path").and_then(|v| v.as_str()) {
                    config.vault.path = std::path::PathBuf::from(path);
                }
                if let Some(name) = vault.get("name").and_then(|v| v.as_str()) {
                    config.vault.name = name.to_string();
                }
            }
            if let Some(obs) = saved.get("obsidian") {
                if let Some(enabled) = obs.get("enabled").and_then(|v| v.as_bool()) {
                    config.obsidian.enabled = enabled;
                }
                if let Some(url) = obs.get("url").and_then(|v| v.as_str()) {
                    config.obsidian.url = url.to_string();
                }
                if let Some(key) = obs.get("api_key").and_then(|v| v.as_str()) {
                    config.obsidian.api_key = if key.is_empty() {
                        None
                    } else {
                        Some(key.to_string())
                    };
                }
            }
            if let Some(llm) = saved.get("llm") {
                if let Some(p) = llm.get("provider").and_then(|v| v.as_str()) {
                    config.llm.provider = p.to_string();
                }
                if let Some(m) = llm.get("model").and_then(|v| v.as_str()) {
                    config.llm.model = m.to_string();
                }
                if let Some(k) = llm
                    .get("api_key")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    config.llm.api_key = Some(k.to_string());
                }
                if let Some(k) = llm
                    .get("api_key_env")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    config.llm.api_key_env = Some(k.to_string());
                }
                if let Some(u) = llm
                    .get("base_url")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    config.llm.base_url = Some(u.to_string());
                }
                if let Some(t) = llm.get("max_tokens").and_then(|v| v.as_u64()) {
                    config.llm.max_tokens = t as u32;
                }
                if let Some(t) = llm.get("temperature").and_then(|v| v.as_f64()) {
                    config.llm.temperature = t;
                }
            }
            tracing::info!("已从数据库加载保存的配置");
        }
    }

    // Obsidian
    let obsidian_client = if config.obsidian.enabled {
        match ObsidianClient::new(&config.obsidian) {
            Ok(client) => {
                let client = Arc::new(client);
                if client.health_check().await {
                    components.obsidian = "ok".to_string();
                    tracing::info!("Obsidian API 连接成功: {}", config.obsidian.url);
                    Some(client)
                } else {
                    components.obsidian = "degraded: 无法连接".to_string();
                    tracing::warn!("Obsidian API 无法连接: {}", config.obsidian.url);
                    Some(client)
                }
            }
            Err(e) => {
                components.obsidian = format!("error: {e}");
                tracing::error!("Obsidian API 客户端创建失败: {e}");
                None
            }
        }
    } else {
        tracing::warn!("Obsidian API 未启用");
        None
    };
    let obsidian = new_provider(obsidian_client);

    // Core services
    let memory_service = Arc::new(MemoryService::new(
        obsidian.clone(),
        config.vault.path.clone(),
        config.vault.name.clone(),
    ));
    let repo_manager = Arc::new(RepoManager::new(db.clone(), RepoManagerConfig::default()));
    let note_linker = Arc::new(NoteLinker::new(db.clone()));
    let timeline_store = Arc::new(TimelineStore::new(db.clone()));
    let timeline_service = Arc::new(TimelineService::new(
        timeline_store,
        TimelineConfig::default(),
    ));
    let memo_manager = Arc::new(MemoManager::new(db.clone(), obsidian.clone()));

    let llm: Arc<dyn crate::infra::llm_client::LlmProvider> =
        crate::infra::llm_client::LlmClientFactory::create(&config.llm)
            .map(Arc::from)
            .unwrap_or_else(|e| {
                tracing::warn!("LLM 客户端创建失败: {e}，灵感功能将受限");
                Arc::from(
                    crate::infra::llm_client::LlmClientFactory::create(&crate::config::LlmConfig {
                        provider: "ollama".to_string(),
                        ..Default::default()
                    })
                    .expect("Fallback LLM creation failed"),
                )
            });

    let inspiration_service = Arc::new(InspirationService::new(
        db.clone(),
        obsidian.clone(),
        llm,
        crate::models::inspiration::InspirationConfig::default(),
    ));

    let radar_config = crate::models::radar::RadarConfig {
        sources_path: std::path::PathBuf::from("config/radar_sources.toml"),
        ..Default::default()
    };
    let radar_service = match RadarService::new(db.clone(), obsidian.clone(), radar_config) {
        Ok(service) => Arc::new(service),
        Err(e) => {
            tracing::warn!("RadarService 初始化失败: {e}");
            Arc::new(
                RadarService::new(
                    db.clone(),
                    obsidian.clone(),
                    crate::models::radar::RadarConfig::default(),
                )
                .expect("Fallback RadarService failed"),
            )
        }
    };

    // Build context + register tools
    let tool_registry = Arc::new(ToolRegistry::new());
    let ctx = Arc::new(AppContext {
        config: Arc::new(config),
        components: Arc::new(std::sync::Mutex::new(components)),
        tool_registry: tool_registry.clone(),
        db: db.clone(),
        obsidian: obsidian.clone(),
        memory_service,
        repo_manager,
        note_linker,
        timeline_service,
        memo_manager,
        inspiration_service,
        radar_service,
        start_time,
    });
    register_all_tools(&tool_registry, ctx.clone()).await;
    tracing::info!("已注册 {} 个工具", ctx.tool_registry.count().await);

    // Serve
    let app = api::router::create_router(ctx);
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("绑定地址失败: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("服务已启动: http://{}", addr);

    // Write PID (in case we were daemonized, the daemon module already wrote it,
    // but rewrite to be safe for foreground mode too).
    let _ = daemon::write_pid();

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        tracing::error!("服务运行失败: {e}");
        std::process::exit(1);
    }

    daemon::remove_pid();
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

// ── CLI subcommand implementations ────────────────────────────────────

fn show_status() {
    match daemon::read_pid() {
        Some(pid) if daemon::is_process_running(pid) => {
            println!("ObsidianBrain is running (PID: {pid})");

            // Try to reach the health endpoint.
            let rt = tokio::runtime::Runtime::new().expect("runtime");
            let health = rt.block_on(async {
                match reqwest::get("http://127.0.0.1:9876/v1/health").await {
                    Ok(r) => r.json::<serde_json::Value>().await.ok(),
                    Err(_) => None,
                }
            });

            if let Some(h) = &health {
                if let Some(status) = h.get("status").and_then(|v| v.as_str()) {
                    println!("  Status: {status}");
                }
                if let Some(tools) = h.get("tools_count").and_then(|v| v.as_u64()) {
                    println!("  Tools: {tools}");
                }
                if let Some(uptime) = h.get("uptime_seconds").and_then(|v| v.as_u64()) {
                    println!("  Uptime: {}s", uptime);
                }
                if let Some(vault) = h.get("vault").and_then(|v| v.as_object()) {
                    if let Some(path) = vault.get("path").and_then(|v| v.as_str()) {
                        println!("  Vault: {path}");
                    }
                }
            } else {
                println!("  (health endpoint unreachable — server may still be starting)");
            }
        }
        Some(_) => {
            println!("ObsidianBrain is not running (stale PID file found).");
            daemon::remove_pid();
        }
        None => {
            println!("ObsidianBrain is not running.");
        }
    }
    println!("  Data dir: {}", paths::data_dir().display());
}

fn config_command(action: ConfigCmd) {
    // Open the DB directly to read/write system_config.
    let db_path = paths::db_path();
    let db = match SqliteStore::new(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open database at {}: {e}", db_path.display());
            std::process::exit(1);
        }
    };

    match action {
        ConfigCmd::Show => match db.get_state("system_config") {
            Ok(Some(json)) => {
                let parsed: serde_json::Value =
                    serde_json::from_str(&json).unwrap_or(serde_json::Value::String(json));
                println!(
                    "{}",
                    serde_json::to_string_pretty(&parsed).unwrap_or_default()
                );
            }
            _ => println!("No saved configuration. Using defaults + config/default.toml."),
        },
        ConfigCmd::Get { key } => match db.get_state("system_config") {
            Ok(Some(json)) => {
                let parsed: serde_json::Value =
                    serde_json::from_str(&json).unwrap_or(serde_json::Value::Null);
                let parts: Vec<&str> = key.split('.').collect();
                let mut current = &parsed;
                for part in &parts {
                    current = current.get(part).unwrap_or(&serde_json::Value::Null);
                }
                println!("{key} = {}", current);
            }
            _ => println!("No saved configuration."),
        },
        ConfigCmd::Set { key, value } => {
            // Read existing config, update the dotted key, write back.
            let mut config: serde_json::Value = match db.get_state("system_config") {
                Ok(Some(json)) => serde_json::from_str(&json).unwrap_or(serde_json::json!({})),
                _ => serde_json::json!({}),
            };

            // Parse value as JSON if possible, otherwise treat as string.
            let parsed_value: serde_json::Value =
                serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value.clone()));

            // Navigate to the nested key and set it.
            let parts: Vec<&str> = key.split('.').collect();
            let mut current = &mut config;
            for (i, part) in parts.iter().enumerate() {
                if i == parts.len() - 1 {
                    current[part] = parsed_value.clone();
                } else {
                    if !current[part].is_object() {
                        current[part] = serde_json::json!({});
                    }
                    current = &mut current[part];
                }
            }

            let json_str = serde_json::to_string(&config).unwrap_or_default();
            match db.set_state("system_config", &json_str) {
                Ok(()) => println!("Config updated: {key} = {value}"),
                Err(e) => eprintln!("Failed to save config: {e}"),
            }
        }
    }
}

// ── Test helpers ──

#[cfg(test)]
mod test_helpers {
    use super::*;

    impl AppContext {
        pub fn for_test() -> (Arc<Self>, tempfile::TempDir, std::path::PathBuf) {
            let dir = tempfile::tempdir().expect("tempdir creation");
            let vault_path = dir.path().join("vault");
            std::fs::create_dir_all(&vault_path).expect("vault dir creation");

            let mut config = AppConfig::default();
            config.vault.path = vault_path.clone();
            config.vault.name = "TestVault".to_string();
            config.obsidian.enabled = false;

            let obsidian = Arc::new(ObsidianClient::new(&config.obsidian).unwrap_or_else(|_| {
                ObsidianClient::new(&crate::config::ObsidianApiConfig {
                    enabled: true,
                    url: "http://127.0.0.1:1".to_string(),
                    api_key: None,
                })
                .expect("dummy client creation")
            }));
            let obsidian_provider = new_provider(Some(obsidian));

            let memory_service = Arc::new(MemoryService::new(
                obsidian_provider.clone(),
                vault_path.clone(),
                "TestVault".to_string(),
            ));
            let db =
                Arc::new(SqliteStore::new(&dir.path().join("test.db")).expect("SQLite creation"));
            let repo_manager = Arc::new(RepoManager::new(db.clone(), RepoManagerConfig::default()));
            let note_linker = Arc::new(NoteLinker::new(db.clone()));
            let timeline_store = Arc::new(TimelineStore::new(db.clone()));
            let timeline_service = Arc::new(TimelineService::new(
                timeline_store,
                TimelineConfig::default(),
            ));
            let memo_manager = Arc::new(MemoManager::new(db.clone(), obsidian_provider.clone()));

            let llm_config = crate::config::LlmConfig::default();
            let llm: Arc<dyn crate::infra::llm_client::LlmProvider> = Arc::from(
                crate::infra::llm_client::LlmClientFactory::create(&llm_config)
                    .expect("Test LLM client creation failed"),
            );
            let inspiration_service = Arc::new(InspirationService::new(
                db.clone(),
                obsidian_provider.clone(),
                llm,
                crate::models::inspiration::InspirationConfig::default(),
            ));

            let radar_config = crate::models::radar::RadarConfig {
                sources_path: dir.path().join("radar_sources.toml"),
                ..Default::default()
            };
            let radar_service = Arc::new(
                RadarService::new(db.clone(), obsidian_provider.clone(), radar_config)
                    .expect("Test RadarService creation failed"),
            );

            let ctx = Arc::new(AppContext {
                config: Arc::new(config),
                components: Arc::new(std::sync::Mutex::new(ComponentStatus::default())),
                tool_registry: Arc::new(ToolRegistry::new()),
                db: db.clone(),
                obsidian: obsidian_provider.clone(),
                memory_service,
                repo_manager,
                note_linker,
                timeline_service,
                memo_manager,
                inspiration_service,
                radar_service,
                start_time: chrono::Utc::now(),
            });

            (ctx, dir, vault_path)
        }
    }
}
