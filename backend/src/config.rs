use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::BrainError;

// ── Top-level ──

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    /// Read old TOML paths solely for one-way image migration; never connect to the old application.
    #[serde(default, rename = "vault", skip_serializing)]
    pub legacy_vault: LegacyVaultConfig,
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
    pub fn load() -> Result<Self, BrainError> {
        let builder = Config::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name("config/local").required(false))
            .add_source(
                Environment::with_prefix("OBRAIN")
                    .separator("__")
                    .try_parsing(true),
            );

        let config = builder
            .build()
            .map_err(|e| BrainError::ConfigError(format!("配置加载失败: {e}")))?;

        let app_config: AppConfig = config
            .try_deserialize()
            .map_err(|e| BrainError::ConfigError(format!("配置解析失败: {e}")))?;

        app_config.validate()?;
        Ok(app_config)
    }

    pub fn validate(&self) -> Result<(), BrainError> {
        if self.server.port < 1024 {
            return Err(BrainError::ConfigError(format!(
                "端口号不能低于 1024: {}",
                self.server.port
            )));
        }
        if self.memory.chunk_min_tokens >= self.memory.chunk_max_tokens {
            return Err(BrainError::ConfigError(
                "chunk_min_tokens 必须小于 chunk_max_tokens".to_string(),
            ));
        }
        if self.qdrant.vector_size == 0 {
            return Err(BrainError::ConfigError("向量维度不能为 0".to_string()));
        }
        Ok(())
    }
}

// ── Sub-configs ──

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_protocol")]
    pub protocol: String, // "mcp" | "http" | "both"
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            protocol: default_protocol(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LegacyVaultConfig {
    #[serde(default)]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QdrantConfig {
    #[serde(default = "default_qdrant_url")]
    pub url: String,
    #[serde(default = "default_collection_name")]
    pub collection_name: String,
    #[serde(default = "default_vector_size")]
    pub vector_size: usize,
}
impl Default for QdrantConfig {
    fn default() -> Self {
        Self {
            url: default_qdrant_url(),
            collection_name: default_collection_name(),
            vector_size: default_vector_size(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingConfig {
    #[serde(default = "default_openai")]
    pub provider: String,
    #[serde(default = "default_embedding_model")]
    pub model: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}
impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: default_openai(),
            model: default_embedding_model(),
            api_key_env: None,
            base_url: None,
            batch_size: default_batch_size(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmConfig {
    #[serde(default = "default_openai")]
    pub provider: String,
    #[serde(default = "default_llm_model")]
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
}
impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: default_openai(),
            model: default_llm_model(),
            api_key: None,
            api_key_env: None,
            base_url: None,
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MemoryConfig {
    #[serde(default = "default_chunk_min")]
    pub chunk_min_tokens: usize,
    #[serde(default = "default_chunk_max")]
    pub chunk_max_tokens: usize,
    #[serde(default = "default_top_k")]
    pub search_top_k: usize,
}
impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            chunk_min_tokens: default_chunk_min(),
            chunk_max_tokens: default_chunk_max(),
            search_top_k: default_top_k(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
    #[serde(default = "default_index_path")]
    pub index_path: PathBuf,
    #[serde(default = "default_backup_retention")]
    pub backup_retention: usize,
}
impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: default_db_path(),
            index_path: default_index_path(),
            backup_retention: default_backup_retention(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub file: Option<PathBuf>,
}
impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: None,
        }
    }
}

// ── Default value functions ──

fn default_host() -> String {
    "0.0.0.0".to_string()
}
fn default_port() -> u16 {
    9876
}
fn default_protocol() -> String {
    "http".to_string()
}
fn default_qdrant_url() -> String {
    "http://127.0.0.1:6333".to_string()
}
fn default_collection_name() -> String {
    "obsidian_brain".to_string()
}
fn default_vector_size() -> usize {
    1536
}
fn default_openai() -> String {
    "openai".to_string()
}
fn default_embedding_model() -> String {
    "text-embedding-3-small".to_string()
}
fn default_llm_model() -> String {
    "gpt-4o-mini".to_string()
}
fn default_max_tokens() -> u32 {
    2048
}
fn default_temperature() -> f64 {
    0.7
}
fn default_batch_size() -> usize {
    100
}
fn default_chunk_min() -> usize {
    300
}
fn default_chunk_max() -> usize {
    800
}
fn default_top_k() -> usize {
    5
}
fn default_db_path() -> PathBuf {
    crate::paths::db_path()
}
fn default_index_path() -> PathBuf {
    crate::paths::index_path()
}
fn default_backup_retention() -> usize {
    7
}
fn default_log_level() -> String {
    "info".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_port_rejected() {
        let mut config = AppConfig::default();
        config.server.port = 80;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_chunk_params_rejected_when_min_ge_max() {
        let mut config = AppConfig::default();
        config.memory.chunk_min_tokens = 1000;
        config.memory.chunk_max_tokens = 500;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_load_falls_back_to_defaults() {
        let config = AppConfig::load();
        assert!(config.is_ok());
    }

    #[test]
    fn test_server_config_default_protocol_is_http() {
        let config = ServerConfig::default();
        assert_eq!(config.protocol, "http");
    }
}
