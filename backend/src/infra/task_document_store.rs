//! Task document storage abstraction and Obsidian adapter.

use async_trait::async_trait;

use crate::error::BrainError;
use crate::infra::obsidian_client::{get_client, ObsidianProvider};

#[async_trait]
pub trait TaskDocumentStore: Send + Sync {
    async fn read(&self, path: &str) -> Result<Option<String>, BrainError>;
    async fn write(&self, path: &str, content: &str) -> Result<(), BrainError>;
    async fn list(&self, prefix: &str) -> Result<Vec<String>, BrainError>;
}

pub struct ObsidianTaskDocumentStore {
    provider: ObsidianProvider,
}

impl ObsidianTaskDocumentStore {
    pub fn new(provider: ObsidianProvider) -> Self {
        Self { provider }
    }

    fn client(
        &self,
    ) -> Result<std::sync::Arc<crate::infra::obsidian_client::ObsidianClient>, BrainError> {
        get_client(&self.provider).map_err(|_| BrainError::ObsidianUnavailable)
    }
}

#[async_trait]
impl TaskDocumentStore for ObsidianTaskDocumentStore {
    async fn read(&self, path: &str) -> Result<Option<String>, BrainError> {
        match self.client()?.read_file(path).await {
            Ok(content) => Ok(Some(content)),
            Err(BrainError::NoteNotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn write(&self, path: &str, content: &str) -> Result<(), BrainError> {
        self.client()?.write_file(path, content).await
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, BrainError> {
        let normalized = format!("{}/", prefix.trim_end_matches('/'));
        let entries = match self.client()?.list_files(Some(&normalized)).await {
            Ok(entries) => entries,
            Err(BrainError::NoteNotFound(_)) => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        Ok(entries
            .into_iter()
            .filter(|entry| !entry.ends_with('/'))
            .filter(|entry| entry.ends_with(".md"))
            .map(|entry| {
                if entry.starts_with(&normalized) {
                    entry
                } else {
                    format!("{normalized}{entry}")
                }
            })
            .collect())
    }
}
