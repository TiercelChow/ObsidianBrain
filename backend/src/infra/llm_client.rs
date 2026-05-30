use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::config::LlmConfig;
use crate::error::BrainError;

/// A single chat message.
#[allow(dead_code)] // Public API for future tasks (Phase 1+)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

#[allow(dead_code)] // Public API for future tasks (Phase 1+)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// Complete chat response.
#[allow(dead_code)] // Public API for future tasks (Phase 1+)
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub usage: TokenUsage,
}

#[allow(dead_code)] // Public API for future tasks (Phase 1+)
#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// A single chunk from a streaming response.
#[allow(dead_code)] // Public API for future tasks (Phase 1+)
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: String,
    pub is_final: bool,
}

/// Unified LLM interface.
#[allow(dead_code)] // Public API for future tasks (Phase 1+)
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<ChatResponse, BrainError>;

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<StreamChunk>, BrainError>;

    /// Convenience: single user message → text.
    async fn generate(&self, prompt: &str) -> Result<String, BrainError> {
        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: prompt.to_string(),
        }];
        let resp = self.chat(&messages).await?;
        Ok(resp.content)
    }

    /// Rough token count estimate.
    fn estimate_tokens(&self, text: &str) -> u32 {
        let char_count = text.chars().count();
        let cjk_count = text
            .chars()
            .filter(|c| (*c as u32) > 0x4E00 && (*c as u32) < 0x9FFF)
            .count();
        let non_cjk = char_count - cjk_count;
        ((non_cjk as f64 / 4.0) + (cjk_count as f64 / 2.0)).ceil() as u32
    }
}

// ── OpenAI ──

#[allow(dead_code)] // Public API for future tasks (Phase 1+)
pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
    temperature: f64,
    base_url: String,
}

#[allow(dead_code)] // Used by OpenAiProvider chat methods at runtime
#[derive(Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    max_tokens: u32,
    temperature: f64,
    stream: bool,
}

#[allow(dead_code)] // Used by OpenAiProvider chat methods at runtime
#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[allow(dead_code)] // Referenced by OpenAiChatResponse at runtime
#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    usage: OpenAiUsage,
    model: String,
}

#[allow(dead_code)] // Referenced by OpenAiChatResponse at runtime
#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessageResp,
}

#[allow(dead_code)] // Referenced by OpenAiChoice at runtime
#[derive(Deserialize)]
struct OpenAiMessageResp {
    content: String,
}

#[allow(dead_code)] // Referenced by OpenAiChatResponse at runtime
#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[allow(dead_code)] // Used by OpenAiProvider and OllamaProvider at runtime
fn role_to_string(role: &MessageRole) -> String {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
    }
    .to_string()
}

impl OpenAiProvider {
    #[allow(dead_code)] // Public API for future tasks (Phase 1+)
    pub fn new(config: &LlmConfig) -> Result<Self, BrainError> {
        let api_key = config
            .api_key_env
            .as_ref()
            .and_then(|env| std::env::var(env).ok())
            .unwrap_or_default();

        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| BrainError::LlmApiError {
                provider: "openai".to_string(),
                detail: format!("HTTP 客户端创建失败: {e}"),
            })?;

        Ok(OpenAiProvider {
            client,
            api_key,
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            base_url: config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        })
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<ChatResponse, BrainError> {
        let request = OpenAiChatRequest {
            model: self.model.clone(),
            messages: messages
                .iter()
                .map(|m| OpenAiMessage {
                    role: role_to_string(&m.role),
                    content: m.content.clone(),
                })
                .collect(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| BrainError::LlmApiError {
                provider: "openai".to_string(),
                detail: format!("请求失败: {e}"),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BrainError::LlmApiError {
                provider: "openai".to_string(),
                detail: format!("API 错误 {status}: {body}"),
            });
        }

        let resp: OpenAiChatResponse =
            response.json().await.map_err(|e| BrainError::LlmApiError {
                provider: "openai".to_string(),
                detail: format!("响应解析失败: {e}"),
            })?;

        let choice = resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| BrainError::LlmApiError {
                provider: "openai".to_string(),
                detail: "空响应".to_string(),
            })?;

        Ok(ChatResponse {
            content: choice.message.content,
            model: resp.model,
            usage: TokenUsage {
                prompt_tokens: resp.usage.prompt_tokens,
                completion_tokens: resp.usage.completion_tokens,
                total_tokens: resp.usage.total_tokens,
            },
        })
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<StreamChunk>, BrainError> {
        let (tx, rx) = mpsc::channel(64);

        let request = OpenAiChatRequest {
            model: self.model.clone(),
            messages: messages
                .iter()
                .map(|m| OpenAiMessage {
                    role: role_to_string(&m.role),
                    content: m.content.clone(),
                })
                .collect(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            stream: true,
        };

        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();

        tokio::spawn(async move {
            let resp = client
                .post(format!("{}/chat/completions", base_url))
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&request)
                .send()
                .await;

            let resp = match resp {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx
                        .send(StreamChunk {
                            content: format!("错误: {e}"),
                            is_final: true,
                        })
                        .await;
                    return;
                }
            };

            let mut stream = resp.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk) = stream.next().await {
                if let Ok(bytes) = chunk {
                    buffer.push_str(&String::from_utf8_lossy(&bytes));

                    while let Some(line_end) = buffer.find('\n') {
                        let line = buffer[..line_end].trim().to_string();
                        buffer = buffer[line_end + 1..].to_string();

                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                let _ = tx
                                    .send(StreamChunk {
                                        content: String::new(),
                                        is_final: true,
                                    })
                                    .await;
                                return;
                            }

                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                if let Some(content) =
                                    json["choices"][0]["delta"]["content"].as_str()
                                {
                                    let _ = tx
                                        .send(StreamChunk {
                                            content: content.to_string(),
                                            is_final: false,
                                        })
                                        .await;
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}

// ── Ollama ──

#[allow(dead_code)] // Public API for future tasks (Phase 1+)
pub struct OllamaProvider {
    client: Client,
    model: String,
    base_url: String,
}

impl OllamaProvider {
    #[allow(dead_code)] // Public API for future tasks (Phase 1+)
    pub fn new(config: &LlmConfig) -> Result<Self, BrainError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| BrainError::LlmApiError {
                provider: "ollama".to_string(),
                detail: format!("HTTP 客户端创建失败: {e}"),
            })?;

        Ok(OllamaProvider {
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
impl LlmProvider for OllamaProvider {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<ChatResponse, BrainError> {
        #[derive(Serialize)]
        struct Req {
            model: String,
            messages: Vec<OpenAiMessage>,
            stream: bool,
        }

        let request = Req {
            model: self.model.clone(),
            messages: messages
                .iter()
                .map(|m| OpenAiMessage {
                    role: role_to_string(&m.role),
                    content: m.content.clone(),
                })
                .collect(),
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| BrainError::LlmApiError {
                provider: "ollama".to_string(),
                detail: format!("请求失败: {e}"),
            })?;

        #[derive(Deserialize)]
        struct Resp {
            message: OpenAiMessageResp,
            model: String,
        }

        let resp: Resp = response.json().await.map_err(|e| BrainError::LlmApiError {
            provider: "ollama".to_string(),
            detail: format!("响应解析失败: {e}"),
        })?;

        Ok(ChatResponse {
            content: resp.message.content,
            model: resp.model,
            usage: TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        })
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<StreamChunk>, BrainError> {
        let (tx, rx) = mpsc::channel(4);
        let response = self.chat(messages).await?;
        let _ = tx
            .send(StreamChunk {
                content: response.content,
                is_final: true,
            })
            .await;
        Ok(rx)
    }
}

// ── Factory ──

#[allow(dead_code)] // Public API for future tasks (Phase 1+)
pub struct LlmClientFactory;

impl LlmClientFactory {
    #[allow(dead_code)] // Public API for future tasks (Phase 1+)
    pub fn create(config: &LlmConfig) -> Result<Box<dyn LlmProvider>, BrainError> {
        match config.provider.as_str() {
            "openai" => Ok(Box::new(OpenAiProvider::new(config)?)),
            "ollama" => Ok(Box::new(OllamaProvider::new(config)?)),
            other => Err(BrainError::ConfigError(format!(
                "未知的 LLM provider: {other}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_english() {
        let provider = OpenAiProvider {
            client: Client::new(),
            api_key: String::new(),
            model: String::new(),
            max_tokens: 0,
            temperature: 0.0,
            base_url: String::new(),
        };
        let estimate = provider.estimate_tokens("hello world this is a test");
        assert!(estimate > 3 && estimate < 10);
    }

    #[test]
    fn test_estimate_tokens_chinese() {
        let provider = OpenAiProvider {
            client: Client::new(),
            api_key: String::new(),
            model: String::new(),
            max_tokens: 0,
            temperature: 0.0,
            base_url: String::new(),
        };
        let estimate = provider.estimate_tokens("你好世界");
        assert!(estimate >= 2 && estimate <= 4);
    }
}
