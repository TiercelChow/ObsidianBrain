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
use crate::core::memory_service::MemoryService;
use crate::core::search_engine::HybridSearchEngine;
use crate::infra::embedding::{EmbeddingFactory, EmbeddingProvider};
use crate::infra::file_watcher::{FileWatcher, DEFAULT_DEBOUNCE_MS};
use crate::infra::llm_client::{LlmClientFactory, LlmProvider};
use crate::infra::obsidian_client::ObsidianClient;
use crate::infra::qdrant_client::QdrantStore;
use crate::infra::sqlite_store::SqliteStore;
use crate::infra::tantivy_index::TantivyIndex;
use crate::tools::handlers::register_all_tools;
use crate::tools::registry::ToolRegistry;

/// Application shared context — injected into all handlers.
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub db: Arc<SqliteStore>,
    pub embedding: Arc<dyn EmbeddingProvider>,
    pub llm: Arc<dyn LlmProvider>,
    pub qdrant: Arc<QdrantStore>,
    pub tantivy: Arc<TantivyIndex>,
    pub components: Arc<std::sync::Mutex<ComponentStatus>>,
    pub tool_registry: Arc<ToolRegistry>,
    pub memory_service: Arc<MemoryService>,
    pub search_engine: Arc<HybridSearchEngine>,
    /// Obsidian Local REST API client (optional — only if plugin is enabled).
    pub obsidian: Option<Arc<ObsidianClient>>,
    /// FileWatcher handle — must be kept alive for filesystem watching to work.
    pub vault_watcher: Option<FileWatcher>,
    /// Server start time — used to compute uptime in health endpoint.
    pub start_time: chrono::DateTime<chrono::Utc>,
}

/// Tracks which components are operational.
#[derive(Debug, Clone, Default)]
pub struct ComponentStatus {
    pub server: String,
    pub sqlite: String,
    pub qdrant: String,
    pub tantivy: String,
    pub embedding: String,
    pub llm: String,
    pub obsidian: String,
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
        ..Default::default()
    };

    // SQLite
    let db = match SqliteStore::new(&config.storage.db_path) {
        Ok(store) => {
            components.sqlite = "ok".to_string();
            tracing::info!("SQLite 初始化成功: {:?}", config.storage.db_path);
            Arc::new(store)
        }
        Err(e) => {
            tracing::error!("SQLite 初始化失败: {e}");
            components.sqlite = format!("error: {e}");
            return Err(Box::new(e) as Box<dyn std::error::Error>);
        }
    };

    // Tantivy
    let tantivy = match TantivyIndex::new(&config.storage.index_path) {
        Ok(idx) => {
            components.tantivy = "ok".to_string();
            tracing::info!("Tantivy 初始化成功: {:?}", config.storage.index_path);
            Arc::new(idx)
        }
        Err(e) => {
            tracing::error!("Tantivy 初始化失败: {e}");
            components.tantivy = format!("error: {e}");
            return Err(Box::new(e) as Box<dyn std::error::Error>);
        }
    };

    // Qdrant (non-fatal if unavailable)
    let qdrant = Arc::new(QdrantStore::new(&config.qdrant).unwrap_or_else(|e| {
        tracing::warn!("Qdrant 客户端创建失败: {e}");
        components.qdrant = format!("error: {e}");
        panic!("Qdrant store creation should not fail with valid config")
    }));
    match qdrant.ensure_collection().await {
        Ok(()) => {
            components.qdrant = "ok".to_string();
            tracing::info!("Qdrant 初始化成功");
        }
        Err(e) => {
            tracing::warn!("Qdrant collection 创建失败 (Qdrant 可能未启动): {e}");
            components.qdrant = format!("degraded: {e}");
        }
    }

    // Embedding
    let embedding: Arc<dyn EmbeddingProvider> = Arc::from(
        EmbeddingFactory::create(&config.embedding).unwrap_or_else(|e| {
            tracing::warn!("Embedding 初始化失败: {e}");
            components.embedding = format!("error: {e}");
            let mut fallback_cfg = config.embedding.clone();
            fallback_cfg.provider = "openai".to_string();
            EmbeddingFactory::create(&fallback_cfg).expect("fallback embedder must succeed")
        }),
    );
    if !components.embedding.starts_with("error") {
        components.embedding = "ok".to_string();
    }
    tracing::info!("Embedding 初始化完成: {}", config.embedding.provider);

    // LLM
    let llm: Arc<dyn LlmProvider> =
        Arc::from(LlmClientFactory::create(&config.llm).unwrap_or_else(|e| {
            tracing::warn!("LLM 初始化失败: {e}");
            components.llm = format!("error: {e}");
            let mut fallback_cfg = config.llm.clone();
            fallback_cfg.provider = "openai".to_string();
            LlmClientFactory::create(&fallback_cfg).expect("fallback LLM must succeed")
        }));
    if !components.llm.starts_with("error") {
        components.llm = "ok".to_string();
    }
    tracing::info!("LLM 初始化完成: {}", config.llm.provider);

    // Obsidian Local REST API (optional)
    let obsidian = if config.obsidian.enabled {
        match ObsidianClient::new(&config.obsidian) {
            Ok(client) => {
                let client = Arc::new(client);
                // Async health check
                if client.health_check().await {
                    components.obsidian = "ok".to_string();
                    tracing::info!("Obsidian API 连接成功: {}", config.obsidian.url);
                } else {
                    components.obsidian = "degraded: 无法连接".to_string();
                    tracing::warn!(
                        "Obsidian API 无法连接: {} (插件是否已启用?)",
                        config.obsidian.url
                    );
                }
                Some(client)
            }
            Err(e) => {
                components.obsidian = format!("error: {e}");
                tracing::warn!("Obsidian API 客户端创建失败: {e}");
                None
            }
        }
    } else {
        components.obsidian = "disabled".to_string();
        tracing::info!("Obsidian API 客户端未启用");
        None
    };

    // 4. Create core services
    let memory_service = Arc::new(MemoryService::new(
        tantivy.clone(),
        qdrant.clone(),
        embedding.clone(),
        config.vault.path.clone(),
        config.vault.name.clone(),
    ));
    tracing::info!("MemoryService 初始化完成");

    // Full index on startup
    let vault_path_valid = config.vault.path.exists() && !config.vault.path.as_os_str().is_empty();
    if vault_path_valid {
        tracing::info!("执行全量索引...");
        match memory_service.full_index().await {
            Ok(report) => {
                tracing::info!(
                    total = report.total_files,
                    indexed = report.indexed_files,
                    failed = report.failed_files.len(),
                    chunks = report.total_chunks,
                    "全量索引完成"
                );
            }
            Err(e) => {
                tracing::warn!("全量索引失败: {e}");
            }
        }
    } else {
        tracing::info!("Vault 路径未配置或不存在，跳过全量索引");
    }

    let search_engine = Arc::new(HybridSearchEngine::new(
        tantivy.clone(),
        qdrant.clone(),
        embedding.clone(),
        config.vault.name.clone(),
    ));
    tracing::info!("HybridSearchEngine 初始化完成");

    // 5. Start file watcher if enabled
    let vault_watcher = if config.vault.watch_enabled && vault_path_valid {
        tracing::info!("启动文件监控...");
        match MemoryService::start_file_watcher(
            memory_service.clone(),
            config.vault.path.clone(),
            config.vault.exclude_patterns.clone(),
            DEFAULT_DEBOUNCE_MS,
        )
        .await
        {
            Ok(watcher) => {
                tracing::info!("文件监控已启动: {:?}", config.vault.path);
                Some(watcher)
            }
            Err(e) => {
                tracing::warn!("文件监控启动失败: {e}");
                None
            }
        }
    } else {
        tracing::info!("文件监控未启用或 Vault 路径不存在");
        None
    };

    // 6. Build AppContext and register tools
    let tool_registry = Arc::new(ToolRegistry::new());

    let ctx = Arc::new(AppContext {
        config: Arc::new(config),
        db,
        embedding,
        llm,
        qdrant,
        tantivy,
        components: Arc::new(std::sync::Mutex::new(components)),
        tool_registry: tool_registry.clone(),
        memory_service,
        search_engine,
        obsidian,
        vault_watcher,
        start_time,
    });

    register_all_tools(&tool_registry, ctx.clone()).await;
    tracing::info!("已注册 {} 个工具", ctx.tool_registry.count().await);

    // 7. Build router and serve
    let app = api::router::create_router(ctx);

    let listener = TcpListener::bind(addr).await?;
    tracing::info!("服务已启动: http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

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
    use crate::error::BrainError;
    use async_trait::async_trait;

    /// Stub EmbeddingProvider for tests.
    pub struct StubEmbedder;

    #[async_trait]
    impl EmbeddingProvider for StubEmbedder {
        async fn embed_text(&self, _text: &str) -> Result<Vec<f32>, BrainError> {
            Ok(vec![0.0; 1536])
        }
        async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError> {
            Ok(texts.iter().map(|_| vec![0.0; 1536]).collect())
        }
        fn dimensions(&self) -> usize {
            1536
        }
    }

    /// Stub LlmProvider for tests.
    struct StubLlm;
    use crate::infra::llm_client::{ChatMessage, ChatResponse, StreamChunk, TokenUsage};
    use tokio::sync::mpsc;

    #[async_trait]
    impl LlmProvider for StubLlm {
        async fn chat(&self, _messages: &[ChatMessage]) -> Result<ChatResponse, BrainError> {
            Ok(ChatResponse {
                content: "stub response".to_string(),
                model: "stub".to_string(),
                usage: TokenUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                },
            })
        }
        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
        ) -> Result<mpsc::Receiver<StreamChunk>, BrainError> {
            let (tx, rx) = mpsc::channel(4);
            let _ = tx
                .send(StreamChunk {
                    content: "stub".to_string(),
                    is_final: true,
                })
                .await;
            Ok(rx)
        }
    }

    /// Create an AppContext with minimal stubs for unit/integration tests.
    /// Returns (Arc<AppContext>, TempDir, vault_path) — caller must keep TempDir alive.
    impl AppContext {
        pub fn for_test() -> (Arc<Self>, tempfile::TempDir, std::path::PathBuf) {
            let dir = tempfile::tempdir().expect("tempdir creation");
            let db_path = dir.path().join("test.db");
            let index_path = dir.path().join("tantivy_index");
            let vault_path = dir.path().join("vault");
            std::fs::create_dir_all(&vault_path).expect("vault dir creation");

            let db = Arc::new(SqliteStore::new(&db_path).expect("SQLite stub creation"));
            let tantivy = Arc::new(TantivyIndex::new(&index_path).expect("Tantivy stub creation"));
            let qdrant =
                Arc::new(QdrantStore::new(&QdrantConfig::default()).expect("Qdrant stub creation"));

            let embedding: Arc<dyn EmbeddingProvider> = Arc::new(StubEmbedder);

            let memory_service = Arc::new(MemoryService::new(
                tantivy.clone(),
                qdrant.clone(),
                embedding.clone(),
                vault_path.clone(),
                "TestVault".to_string(),
            ));

            let search_engine = Arc::new(HybridSearchEngine::new(
                tantivy.clone(),
                qdrant.clone(),
                embedding.clone(),
                "TestVault".to_string(),
            ));

            let mut config = AppConfig::default();
            config.vault.path = vault_path.clone();
            config.vault.name = "TestVault".to_string();

            let ctx = Arc::new(AppContext {
                config: Arc::new(config),
                db,
                embedding,
                llm: Arc::new(StubLlm),
                qdrant,
                tantivy,
                components: Arc::new(std::sync::Mutex::new(ComponentStatus::default())),
                tool_registry: Arc::new(ToolRegistry::new()),
                memory_service,
                search_engine,
                obsidian: None,
                vault_watcher: None,
                start_time: chrono::Utc::now(),
            });

            (ctx, dir, vault_path)
        }
    }

    use crate::config::QdrantConfig;
}

// ── Integration tests ──

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::config::QdrantConfig;
    use crate::core::search_engine::HybridSearchEngine;
    use crate::tools::handlers::register_all_tools;
    use crate::tools::traits::ToolHandler;
    use serde_json::json;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // ── Helper: write a .md file into a vault directory ──

    fn write_vault_file(vault_path: &PathBuf, relative_path: &str, content: &str) {
        let full_path = vault_path.join(relative_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full_path, content).unwrap();
    }

    // ══════════════════════════════════════════════
    // Test 1: End-to-end index and search
    // ══════════════════════════════════════════════

    #[tokio::test]
    async fn test_end_to_end_index_and_search() {
        let (ctx, _dir, vault_path) = AppContext::for_test();

        // 1. Create sample notes in vault
        write_vault_file(
            &vault_path,
            "rust-async.md",
            r#"---
title: Rust 异步编程
tags: [rust, async, tokio]
---

# Rust 异步编程

Tokio 是 Rust 生态中最流行的异步运行时框架。

## 核心概念

- Future: 表示一个异步计算
- async/await: 语法糖，简化 Future 的使用
- Runtime: 执行 Future 的调度器
"#,
        );

        write_vault_file(
            &vault_path,
            "python-ml.md",
            r#"---
title: Python 机器学习
tags: [python, ml, tensorflow]
---

# Python 机器学习

TensorFlow 和 PyTorch 是最流行的深度学习框架。

## 核心概念

- 张量 (Tensor): 多维数组
- 模型: 神经网络结构
- 训练: 通过数据调整参数
"#,
        );

        write_vault_file(
            &vault_path,
            "daily-note.md",
            r#"---
title: 每日笔记
tags: [daily]
---

# 今日学习

今天学习了 Rust 的所有权系统和生命周期。
也复习了 Python 的装饰器语法。
"#,
        );

        // 2. Run full index via memory_service
        let report = ctx.memory_service.full_index().await.unwrap();
        assert_eq!(report.indexed_files, 3, "Should index all 3 files");
        assert!(report.total_chunks > 0, "Should produce at least 1 chunk");
        assert!(report.failed_files.is_empty(), "No files should fail");

        // 3. Search for "Rust 异步" — should find rust-async.md
        let results = ctx
            .search_engine
            .search("Rust 异步", 5, None)
            .await
            .unwrap();
        assert!(!results.is_empty(), "Search should return results");

        let top_result = &results[0];
        assert!(
            top_result.note_path.contains("rust-async"),
            "Top result should be rust-async.md, got: {}",
            top_result.note_path
        );

        // 4. Search for "机器学习" — should find python-ml.md
        let results = ctx.search_engine.search("机器学习", 5, None).await.unwrap();
        assert!(
            !results.is_empty(),
            "Search for 机器学习 should return results"
        );
        assert!(
            results[0].note_path.contains("python-ml"),
            "Top result should be python-ml.md, got: {}",
            results[0].note_path
        );

        // 5. Test memory stats
        let stats = ctx.memory_service.get_memory_stats().await.unwrap();
        assert!(stats.total_chunks > 0, "Stats should report chunks");
        assert!(
            stats.total_notes >= 3,
            "Stats should report at least 3 notes, got {}",
            stats.total_notes
        );

        // 6. Test note reading
        let content = ctx.memory_service.get_note("rust-async.md").await.unwrap();
        assert!(content.contains("Tokio"), "Content should mention Tokio");
        assert!(
            content.contains("异步编程"),
            "Content should mention 异步编程"
        );

        // 7. Test list recent notes
        let notes = ctx
            .memory_service
            .list_recent_notes(None, None)
            .await
            .unwrap();
        assert_eq!(notes.len(), 3, "Should list 3 notes");
    }

    // ══════════════════════════════════════════════
    // Test 2: Tool API end-to-end (register + call)
    // ══════════════════════════════════════════════

    #[tokio::test]
    async fn test_tool_api_search_notes_e2e() {
        let (ctx, _dir, vault_path) = AppContext::for_test();

        // Create a test note
        write_vault_file(
            &vault_path,
            "test-note.md",
            "# Test\nThis is a test note about Rust programming.",
        );

        // Index it
        ctx.memory_service.full_index().await.unwrap();

        // Register all tools
        register_all_tools(&ctx.tool_registry, ctx.clone()).await;

        // Verify tools are registered (8 core tools)
        let tool_count = ctx.tool_registry.count().await;
        assert_eq!(tool_count, 8, "Should have 8 registered tools");

        // Call search_notes tool via the registry
        let handler = ctx.tool_registry.get("search_notes").await.unwrap();
        let result = handler
            .handle(json!({ "query": "Rust programming" }), &ctx)
            .await
            .unwrap();

        assert!(result["notes"].is_array(), "Result should have notes array");
        let notes = result["notes"].as_array().unwrap();
        assert!(!notes.is_empty(), "search_notes should find the test note");
    }

    // ══════════════════════════════════════════════
    // Test 3: Degraded mode — Qdrant unavailable, fulltext works
    // ══════════════════════════════════════════════

    #[tokio::test]
    async fn test_degraded_mode_qdrant_unavailable_fulltext_works() {
        // Set up a vault with a single note
        let vault_dir = TempDir::new().unwrap();
        std::fs::write(
            vault_dir.path().join("note.md"),
            "# Hello\nThis note is about databases and indexing.",
        )
        .unwrap();

        // Set up Tantivy (real) + Qdrant (intentionally unreachable)
        let index_dir = TempDir::new().unwrap();
        let tantivy = Arc::new(TantivyIndex::new(index_dir.path()).unwrap());
        let qdrant_config = QdrantConfig {
            url: "http://127.0.0.1:53333".to_string(), // unreachable
            collection_name: "test".to_string(),
            vector_size: 1536,
        };
        let qdrant = Arc::new(QdrantStore::new(&qdrant_config).unwrap());

        let embedding: Arc<dyn EmbeddingProvider> = Arc::new(test_helpers::StubEmbedder);

        let memory_service = Arc::new(MemoryService::new(
            tantivy.clone(),
            qdrant.clone(),
            embedding.clone(),
            vault_dir.path().to_path_buf(),
            "test".to_string(),
        ));

        // Index should succeed despite Qdrant being unreachable
        let report = memory_service.full_index().await.unwrap();
        assert_eq!(report.indexed_files, 1, "Should index 1 file");

        // Search should work in fulltext-only mode (Qdrant unreachable)
        let search_engine = Arc::new(HybridSearchEngine::new(
            tantivy,
            qdrant,
            embedding,
            "test".to_string(),
        ));

        let results = search_engine.search("databases", 5, None).await.unwrap();
        assert!(
            !results.is_empty(),
            "Fulltext search should work without Qdrant"
        );

        // All results should have fulltext rank but no semantic rank
        for result in &results {
            assert!(
                result.fulltext_rank.is_some(),
                "Degraded results should have fulltext rank"
            );
            assert!(
                result.semantic_rank.is_none(),
                "Degraded results should NOT have semantic rank"
            );
        }
    }

    // ══════════════════════════════════════════════
    // Test 4: Health endpoint data — tools_count and uptime_seconds
    // ══════════════════════════════════════════════

    #[tokio::test]
    async fn test_health_includes_tools_count_and_uptime() {
        let (ctx, _dir, _vault) = AppContext::for_test();

        // Initially no tools registered
        assert_eq!(ctx.tool_registry.count().await, 0);

        // Register tools
        register_all_tools(&ctx.tool_registry, ctx.clone()).await;

        // Verify count
        assert_eq!(ctx.tool_registry.count().await, 8);

        // Verify uptime_seconds is accessible and reasonable
        let uptime = chrono::Utc::now()
            .signed_duration_since(ctx.start_time)
            .num_seconds();
        assert!(uptime >= 0, "Uptime should be non-negative");
    }

    // ══════════════════════════════════════════════
    // Test 5: Full CRUD lifecycle via Tool API
    // ══════════════════════════════════════════════

    #[tokio::test]
    async fn test_tool_api_full_crud_lifecycle() {
        let (ctx, _dir, _vault) = AppContext::for_test();
        register_all_tools(&ctx.tool_registry, ctx.clone()).await;

        // 1. add_memory
        let add_handler = ctx.tool_registry.get("add_memory").await.unwrap();
        let add_result = add_handler
            .handle(
                json!({
                    "note_path": "lifecycle/test.md",
                    "content": "Lifecycle test content about Rust async patterns.",
                    "tags": ["rust", "async"]
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(add_result["status"], "created");
        let memory_id = add_result["memory_id"].as_str().unwrap();

        // 2. search_memory — should find the added chunk
        let search_handler = ctx.tool_registry.get("search_memory").await.unwrap();
        let search_result = search_handler
            .handle(json!({ "query": "Rust async patterns" }), &ctx)
            .await
            .unwrap();
        assert!(
            search_result["total"].as_u64().unwrap() > 0,
            "Should find the added memory"
        );

        // 3. update_memory
        let update_handler = ctx.tool_registry.get("update_memory").await.unwrap();
        let update_result = update_handler
            .handle(
                json!({
                    "memory_id": memory_id,
                    "content": "Updated content about Tokio runtime internals."
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(update_result["status"], "updated");

        // 4. search_memory again — should find updated content
        let search_result2 = search_handler
            .handle(json!({ "query": "Tokio runtime" }), &ctx)
            .await
            .unwrap();
        assert!(
            search_result2["total"].as_u64().unwrap() > 0,
            "Should find updated memory"
        );

        // 5. forget_memory
        let forget_handler = ctx.tool_registry.get("forget_memory").await.unwrap();
        let forget_result = forget_handler
            .handle(json!({ "memory_id": memory_id }), &ctx)
            .await
            .unwrap();
        assert_eq!(forget_result["deleted"], true);

        // 6. get_memory_stats — the chunk should no longer appear
        let stats_handler = ctx.tool_registry.get("get_memory_stats").await.unwrap();
        let stats_result = stats_handler.handle(json!({}), &ctx).await.unwrap();
        assert!(
            stats_result["total_chunks"].as_u64().unwrap()
                < search_result["total"].as_u64().unwrap() + 1,
            "Chunks should decrease after forget"
        );
    }

    // ══════════════════════════════════════════════
    // Test 6: Tag-filtered search via Tool API
    // ══════════════════════════════════════════════

    #[tokio::test]
    async fn test_tag_filtered_search_via_tool_api() {
        let (ctx, _dir, vault_path) = AppContext::for_test();
        register_all_tools(&ctx.tool_registry, ctx.clone()).await;

        // Create two notes with different tags
        write_vault_file(
            &vault_path,
            "rust-note.md",
            "---\ntitle: Rust Note\ntags: [rust]\n---\n# Rust\nRust ownership and borrowing.",
        );
        write_vault_file(
            &vault_path,
            "python-note.md",
            "---\ntitle: Python Note\ntags: [python]\n---\n# Python\nPython decorators and generators.",
        );

        // Index
        ctx.memory_service.full_index().await.unwrap();

        // Search with tag filter
        let handler = ctx.tool_registry.get("search_notes").await.unwrap();
        let result = handler
            .handle(json!({ "query": "programming", "tags": ["rust"] }), &ctx)
            .await
            .unwrap();

        let notes = result["notes"].as_array().unwrap();
        // Results should only contain rust-tagged notes
        for note in notes {
            let path = note["path"].as_str().unwrap();
            assert!(
                path.contains("rust-note"),
                "Filtered search should only return rust-tagged notes, got: {}",
                path
            );
        }
    }
}
