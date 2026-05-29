use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level application configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub vault: VaultConfig,
    #[serde(default)]
    pub qdrant: QdrantConfig,
    #[serde(default)]
    pub embedding: EmbeddingConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl AppConfig {
    /// Load configuration.
    ///
    /// TODO: When the `config` or `toml` crate is added, read `config/default.toml`
    /// and merge with environment variable overrides. For now, returns defaults.
    pub fn load() -> Self {
        let config_path = PathBuf::from("config/default.toml");
        if config_path.exists() {
            tracing::info!(
                path = %config_path.display(),
                "Config file found but TOML parsing not yet available; using defaults"
            );
        } else {
            tracing::info!("No config file found, using default configuration");
        }
        Self::default()
    }
}

/// HTTP server bind configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 9876,
        }
    }
}

/// Obsidian vault configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub path: String,
    pub name: String,
    pub watch_enabled: bool,
    pub exclude_patterns: Vec<String>,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            path: "~/Documents/ObsidianVault".to_string(),
            name: "MyVault".to_string(),
            watch_enabled: true,
            exclude_patterns: vec![
                ".obsidian".to_string(),
                ".trash".to_string(),
                "node_modules".to_string(),
            ],
        }
    }
}

/// Qdrant vector database configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantConfig {
    pub url: String,
    pub collection_name: String,
    pub vector_size: usize,
}

impl Default for QdrantConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:6333".to_string(),
            collection_name: "obsidian_brain".to_string(),
            vector_size: 1536,
        }
    }
}

/// Embedding provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    pub provider: String,
    pub model: String,
    pub api_key_env: String,
    pub base_url: String,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model: "text-embedding-3-small".to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
        }
    }
}

/// LLM provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub api_key_env: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
        }
    }
}

/// Memory engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub chunk_min_tokens: usize,
    pub chunk_max_tokens: usize,
    pub search_top_k: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            chunk_min_tokens: 100,
            chunk_max_tokens: 512,
            search_top_k: 10,
        }
    }
}

/// Storage paths configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub db_path: String,
    pub index_path: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: "data/obsidian_brain.db".to_string(),
            index_path: "data/tantivy_index".to_string(),
        }
    }
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
        }
    }
}
