mod api;
mod config;
mod core;
mod daemon;
mod error;
mod frontend_assets;
mod infra;
mod models;
mod paths;
mod timeline_migration;
mod tools;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::AppConfig;
use crate::core::book_wiki::BookWikiService;
use crate::core::code_repo::manager::{RepoManager, RepoManagerConfig};
use crate::core::code_repo::note_linker::NoteLinker;
use crate::core::tasks::TaskService;
use crate::core::timeline::store::TimelineStore;
use crate::core::timeline::{MemoManager, TimelineConfig, TimelineService};
use crate::infra::book_wiki_store::BookWikiStore;
use crate::infra::deepseek_harness::DeepSeekHarnessRuntime;
use crate::infra::sqlite_store::SqliteStore;
use crate::infra::task_index_store::SqliteTaskIndexStore;
use crate::infra::timeline_images::{TimelineImages, DEFAULT_CACHE_BYTES};
use crate::timeline_migration::migrate_timeline_images;
use crate::tools::handlers::register_all_tools;
use crate::tools::registry::ToolRegistry;

/// Application shared context — injected into all handlers.
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub components: Arc<std::sync::Mutex<ComponentStatus>>,
    pub tool_registry: Arc<ToolRegistry>,
    pub db: Arc<SqliteStore>,
    pub repo_manager: Arc<RepoManager>,
    pub note_linker: Arc<NoteLinker>,
    pub timeline_service: Arc<TimelineService>,
    pub memo_manager: Arc<MemoManager>,
    pub task_service: Arc<TaskService>,
    pub book_wiki_service: Arc<BookWikiService>,
    /// Server start time — used to compute uptime in health endpoint.
    pub start_time: chrono::DateTime<chrono::Utc>,
}

/// Tracks which components are operational.
#[derive(Debug, Clone, Default)]
pub struct ComponentStatus {
    pub server: String,
    pub sqlite: String,
    pub timeline: String,
    pub code_repo: String,
}

// ── CLI ───────────────────────────────────────────────────────────────

#[derive(clap::Parser)]
#[command(
    name = "obsidian-brain",
    version,
    about = "Local knowledge engine, reader, timeline and LLM Wiki"
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
    /// Copy referenced legacy Timeline photos once; never starts the server.
    MigrateTimelineImages {
        #[arg(long)]
        database: std::path::PathBuf,
        #[arg(long)]
        source: std::path::PathBuf,
    },
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
                // 端口预检：避免 daemonize 后才发现端口被占，留下孤儿 PID 文件
                let bind_port = port
                    .or_else(|| AppConfig::load().ok().map(|config| config.server.port))
                    .unwrap_or_else(|| AppConfig::default().server.port);
                if let Some(message) = daemon::port_conflict_message(bind_port) {
                    eprintln!("{message}");
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
        Some(Command::MigrateTimelineImages { database, source }) => {
            init_logging();
            let result = tokio::runtime::Runtime::new()
                .map_err(error::BrainError::from)
                .and_then(|runtime| runtime.block_on(migrate_timeline_images(&database, &source)));
            match result.and_then(|report| {
                serde_json::to_string_pretty(&report).map_err(|error| {
                    error::BrainError::Internal(format!("迁移报告序列化失败: {error}"))
                })
            }) {
                Ok(report) => println!("{report}"),
                Err(error) => {
                    eprintln!("图片迁移失败: {error}");
                    std::process::exit(1);
                }
            }
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
            daemon::remove_own_pid();
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

    // CLI overrides applied later (after DB config) for highest priority.

    // Initialize SQLite
    let mut components = ComponentStatus {
        server: "ok".to_string(),
        sqlite: "pending".to_string(),
        timeline: "pending".to_string(),
        code_repo: "pending".to_string(),
    };
    let db = match SqliteStore::new_with_backup_retention(
        &config.storage.db_path,
        config.storage.backup_retention,
    ) {
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
            // Server config (host/port) — can be set via `obsidian-brain config set server.host`
            if let Some(srv) = saved.get("server") {
                if let Some(h) = srv
                    .get("host")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    config.server.host = h.to_string();
                }
                if let Some(p) = srv.get("port").and_then(|v| v.as_u64()) {
                    config.server.port = p as u16;
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

    // Apply CLI overrides last (highest priority — beats DB config).
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

    // Local assets are colocated with the configured database, including isolated tests.
    let image_root = paths::timeline_dir(&config.storage.db_path);
    let cache_budget = db
        .get_state("timeline_cache_limit_bytes")?
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_CACHE_BYTES);
    let images = Arc::new(TimelineImages::new(db.clone(), image_root, cache_budget)?);
    let legacy_directory = db
        .get_state("timeline_legacy_directory")?
        .or_else(|| {
            db.get_state("system_config")
                .ok()
                .flatten()
                .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
                .and_then(|v| {
                    v.get("vault")?
                        .get("path")?
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                })
        })
        .or_else(|| {
            config
                .legacy_vault
                .path
                .to_str()
                .filter(|p| !p.is_empty())
                .map(str::to_string)
        });
    if let Some(path) = &legacy_directory {
        db.set_state("timeline_legacy_directory", path)?;
    }
    // Preserve all unrelated settings; retire credentials and live vault configuration.
    if let Some(saved) = db.get_state("system_config")? {
        let mut value: serde_json::Value = serde_json::from_str(&saved)?;
        if let Some(object) = value.as_object_mut() {
            object.remove("obsidian");
            object.remove("vault");
        }
        db.set_state("system_config", &value.to_string())?;
    }
    let maintenance_images = images.clone();
    tokio::spawn(async move {
        if let Err(error) = maintenance_images.maintain().await {
            tracing::warn!(%error,"启动图片存储维护失败，将自动重试");
        }
        // Maintenance only: never scan or import from the legacy vault on startup.
        // Import requires an explicit CLI command or the storage panel's manual action.
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            if let Err(error) = maintenance_images.maintain().await {
                tracing::warn!(%error,"图片存储维护失败，将自动重试");
            }
        }
    });
    // Core services
    let repo_manager = Arc::new(RepoManager::new(db.clone(), RepoManagerConfig::default()));
    let note_linker = Arc::new(NoteLinker::new(db.clone()));
    let timeline_store = Arc::new(TimelineStore::new(db.clone()));
    let timeline_service = Arc::new(TimelineService::new(
        timeline_store,
        TimelineConfig::default(),
    ));
    let memo_manager = Arc::new(MemoManager::new(db.clone(), images.clone()));
    let task_service = Arc::new(TaskService::new(Arc::new(SqliteTaskIndexStore::new(
        db.clone(),
    ))));
    let book_wiki_service = Arc::new(
        BookWikiService::new(
            BookWikiStore::new(db.clone()),
            Arc::new(DeepSeekHarnessRuntime::default()),
        )
        .with_agent_tool_gateway(format!(
            "http://127.0.0.1:{}/v1/knowledge/agent-mcp",
            addr.port()
        )),
    );

    // Build context + register tools
    let tool_registry = Arc::new(ToolRegistry::new());
    let ctx = Arc::new(AppContext {
        config: Arc::new(config),
        components: Arc::new(std::sync::Mutex::new(components)),
        tool_registry: tool_registry.clone(),
        db: db.clone(),
        repo_manager,
        note_linker,
        timeline_service,
        memo_manager,
        task_service,
        book_wiki_service,
        start_time,
    });
    ctx.book_wiki_service.clone().start_task_worker()?;
    register_all_tools(&tool_registry, ctx.clone()).await;
    tracing::info!("已注册 {} 个工具", ctx.tool_registry.count().await);

    // Serve
    let app = api::router::create_router(ctx);
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("绑定地址失败: {e}");
            // 清理自己留下的孤儿 PID 文件，避免 stop 之后误判「已在运行」
            daemon::remove_own_pid();
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
        daemon::remove_own_pid();
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
                if let Some(vault) = h.get("storage").and_then(|v| v.as_object()) {
                    if let Some(path) = vault.get("path").and_then(|v| v.as_str()) {
                        println!("  Data: {path}");
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
            config.storage.db_path = dir.path().join("test.db");
            let db =
                Arc::new(SqliteStore::new(&dir.path().join("test.db")).expect("SQLite creation"));
            let repo_manager = Arc::new(RepoManager::new(db.clone(), RepoManagerConfig::default()));
            let note_linker = Arc::new(NoteLinker::new(db.clone()));
            let timeline_store = Arc::new(TimelineStore::new(db.clone()));
            let timeline_service = Arc::new(TimelineService::new(
                timeline_store,
                TimelineConfig::default(),
            ));
            let memo_manager = Arc::new(MemoManager::new(
                db.clone(),
                Arc::new(
                    TimelineImages::new(
                        db.clone(),
                        dir.path().join("timeline"),
                        DEFAULT_CACHE_BYTES,
                    )
                    .expect("local image store"),
                ),
            ));
            let task_service = Arc::new(TaskService::new(Arc::new(SqliteTaskIndexStore::new(
                db.clone(),
            ))));
            let book_wiki_service = Arc::new(BookWikiService::new(
                BookWikiStore::new(db.clone()),
                Arc::new(DeepSeekHarnessRuntime::default()),
            ));

            let ctx = Arc::new(AppContext {
                config: Arc::new(config),
                components: Arc::new(std::sync::Mutex::new(ComponentStatus::default())),
                tool_registry: Arc::new(ToolRegistry::new()),
                db: db.clone(),
                repo_manager,
                note_linker,
                timeline_service,
                memo_manager,
                task_service,
                book_wiki_service,
                start_time: chrono::Utc::now(),
            });

            (ctx, dir, vault_path)
        }
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_manual_image_migration_requires_explicit_database_and_source() {
        assert!(Cli::try_parse_from(["obsidian-brain", "migrate-timeline-images"]).is_err());
        assert!(Cli::try_parse_from([
            "obsidian-brain",
            "migrate-timeline-images",
            "--database",
            "/tmp/brain.db"
        ])
        .is_err());
        let cli = Cli::try_parse_from([
            "obsidian-brain",
            "migrate-timeline-images",
            "--database",
            "/tmp/brain.db",
            "--source",
            "/tmp/old vault",
        ])
        .unwrap();
        assert!(
            matches!(cli.cmd, Some(Command::MigrateTimelineImages { database, source })
            if database == std::path::Path::new("/tmp/brain.db")
                && source == std::path::Path::new("/tmp/old vault"))
        );
    }

    #[tokio::test]
    async fn test_manual_image_migration_copies_without_changing_memos_or_old_files() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("brain.db");
        let source = temp.path().join("old vault");
        std::fs::create_dir_all(source.join("Timeline/images")).unwrap();
        let mut image = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut image, image::ImageFormat::Png)
            .unwrap();
        let bytes = image.into_inner();
        let path = "Timeline/images/example.png";
        std::fs::write(source.join(path), &bytes).unwrap();
        let db = SqliteStore::new(&database).unwrap();
        db.with_connection(|conn| {
            conn.execute("INSERT INTO memos(id,timestamp,date,content,images,tags,file_path) VALUES('old','2026-10-01T00:00:00Z','2026-10-01','original',?1,'[]','old.md')", [format!("[\"{path}\"]")])?;
            Ok(())
        }).unwrap();
        let report = migrate_timeline_images(&database, &source).await.unwrap();
        assert_eq!(report.copied, 1);
        assert!(report.missing.is_empty());
        assert!(report.backup.is_file());
        assert_eq!(std::fs::read(source.join(path)).unwrap(), bytes);
        let images = TimelineImages::new(Arc::new(db), paths::timeline_dir(&database), 0).unwrap();
        assert_eq!(images.original(path).await.unwrap().0, bytes);
        let snapshot = rusqlite::Connection::open(report.backup).unwrap();
        let before: String = snapshot
            .query_row(
                "SELECT content || images || timestamp FROM memos WHERE id='old'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let after: String = rusqlite::Connection::open(&database)
            .unwrap()
            .query_row(
                "SELECT content || images || timestamp FROM memos WHERE id='old'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(before, after);
        assert_eq!(
            migrate_timeline_images(&database, &source)
                .await
                .unwrap()
                .copied,
            0
        );
    }

    #[tokio::test]
    async fn test_manual_image_migration_rejects_missing_inputs_without_creating_database() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("missing.db");
        assert!(migrate_timeline_images(&database, temp.path())
            .await
            .is_err());
        assert!(!database.exists());
        let database = temp.path().join("brain.db");
        let _db = SqliteStore::new(&database).unwrap();
        assert!(
            migrate_timeline_images(&database, &temp.path().join("missing"))
                .await
                .is_err()
        );
        assert!(!temp.path().join("timeline").exists());
    }
}
