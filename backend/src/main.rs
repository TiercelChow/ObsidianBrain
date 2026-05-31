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
use crate::infra::llm_client::{LlmClientFactory, LlmProvider};
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

    // 4. Create core services
    let memory_service = Arc::new(MemoryService::new(
        tantivy.clone(),
        qdrant.clone(),
        embedding.clone(),
        config.vault.path.clone(),
        config.vault.name.clone(),
    ));
    tracing::info!("MemoryService 初始化完成");

    let search_engine = Arc::new(HybridSearchEngine::new(
        tantivy.clone(),
        qdrant.clone(),
        embedding.clone(),
        config.vault.name.clone(),
    ));
    tracing::info!("HybridSearchEngine 初始化完成");

    // 5. Build AppContext and register tools
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
    });

    register_all_tools(&tool_registry, ctx.clone()).await;
    tracing::info!("已注册 {} 个工具", ctx.tool_registry.count().await);

    // 6. Build router and serve
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
    struct StubEmbedder;

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
            });

            (ctx, dir, vault_path)
        }
    }

    use crate::config::QdrantConfig;
}
