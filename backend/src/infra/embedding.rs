use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config::EmbeddingConfig;
use crate::error::BrainError;

/// Unified interface for embedding text into vectors.
#[allow(dead_code)] // Public API for future tasks (Phase 1+)
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError>;
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError>;
    fn dimensions(&self) -> usize;
}

// ── OpenAI ──

#[allow(dead_code)] // Public API for future tasks (Phase 1+)
pub struct OpenAiEmbedder {
    client: Client,
    api_key: String,
    model: String,
    dimensions: usize,
    base_url: String,
    batch_size: usize,
}

#[derive(Serialize)]
#[allow(dead_code)] // Used by embed_batch_inner at runtime
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)] // Used by embed_batch_inner at runtime
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
#[allow(dead_code)] // Referenced by EmbeddingResponse at runtime
struct EmbeddingData {
    embedding: Vec<f32>,
    index: usize,
}

impl OpenAiEmbedder {
    #[allow(dead_code)] // Public API for future tasks (Phase 1+)
    pub fn new(config: &EmbeddingConfig) -> Result<Self, BrainError> {
        let api_key = config
            .api_key_env
            .as_ref()
            .and_then(|env| std::env::var(env).ok())
            .unwrap_or_default();

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(5)
            .build()
            .map_err(|e| BrainError::EmbeddingError(format!("HTTP 客户端创建失败: {e}")))?;

        Ok(OpenAiEmbedder {
            client,
            api_key,
            model: config.model.clone(),
            dimensions: 1536,
            base_url: config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            batch_size: config.batch_size,
        })
    }

    async fn embed_batch_inner(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError> {
        let request = EmbeddingRequest {
            model: self.model.clone(),
            input: texts.to_vec(),
        };

        let mut last_err = None;
        for attempt in 0..3 {
            if attempt > 0 {
                let delay = Duration::from_millis(100 * 2u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
            }

            let resp = self
                .client
                .post(format!("{}/embeddings", self.base_url))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&request)
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    let body: EmbeddingResponse = r
                        .json()
                        .await
                        .map_err(|e| BrainError::EmbeddingError(format!("响应解析失败: {e}")))?;
                    let mut data = body.data;
                    data.sort_by_key(|d| d.index);
                    return Ok(data.into_iter().map(|d| d.embedding).collect());
                }
                Ok(r) => {
                    let status = r.status();
                    let text = r.text().await.unwrap_or_default();
                    last_err = Some(BrainError::EmbeddingError(format!(
                        "API 错误 {status}: {text}"
                    )));
                }
                Err(e) => {
                    last_err = Some(BrainError::EmbeddingError(format!("请求失败: {e}")));
                }
            }
        }

        Err(last_err.unwrap_or_else(|| BrainError::EmbeddingError("重试耗尽".to_string())))
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAiEmbedder {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        let results = self.embed_batch_inner(&[text.to_string()]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| BrainError::EmbeddingError("空响应".to_string()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError> {
        let mut results = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(self.batch_size) {
            let batch = self.embed_batch_inner(chunk).await?;
            results.extend(batch);
        }
        Ok(results)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

// ── Ollama ──

#[allow(dead_code)] // Public API for future tasks (Phase 1+)
pub struct OllamaEmbedder {
    client: Client,
    model: String,
    base_url: String,
}

#[derive(Serialize)]
#[allow(dead_code)] // Used by OllamaEmbedder::embed_text at runtime
struct OllamaEmbedRequest {
    model: String,
    input: String,
}

#[derive(Deserialize)]
#[allow(dead_code)] // Used by OllamaEmbedder::embed_text at runtime
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

impl OllamaEmbedder {
    #[allow(dead_code)] // Public API for future tasks (Phase 1+)
    pub fn new(config: &EmbeddingConfig) -> Result<Self, BrainError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| BrainError::EmbeddingError(format!("HTTP 客户端创建失败: {e}")))?;

        Ok(OllamaEmbedder {
            client,
            model: config.model.clone(),
            base_url: config
                .base_url
                .clone()
                .unwrap_or_else(|| "http://127.0.0.1:11434".to_string()),
        })
    }
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbedder {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        let request = OllamaEmbedRequest {
            model: self.model.clone(),
            input: text.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/api/embed", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| BrainError::EmbeddingError(format!("Ollama 请求失败: {e}")))?;

        let resp: OllamaEmbedResponse = response
            .json()
            .await
            .map_err(|e| BrainError::EmbeddingError(format!("Ollama 响应解析失败: {e}")))?;

        resp.embeddings
            .into_iter()
            .next()
            .ok_or_else(|| BrainError::EmbeddingError("Ollama 空响应".to_string()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed_text(text).await?);
        }
        Ok(results)
    }

    fn dimensions(&self) -> usize {
        768
    }
}

// ── Factory ──

#[allow(dead_code)] // Public API for future tasks (Phase 1+)
pub struct EmbeddingFactory;

impl EmbeddingFactory {
    #[allow(dead_code)] // Public API for future tasks (Phase 1+)
    pub fn create(config: &EmbeddingConfig) -> Result<Box<dyn EmbeddingProvider>, BrainError> {
        match config.provider.as_str() {
            "openai" => Ok(Box::new(OpenAiEmbedder::new(config)?)),
            "ollama" => Ok(Box::new(OllamaEmbedder::new(config)?)),
            other => Err(BrainError::ConfigError(format!(
                "未知的 Embedding provider: {other}"
            ))),
        }
    }
}
