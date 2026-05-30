use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

use crate::config::QdrantConfig;
use crate::error::BrainError;

/// Qdrant vector store client (REST API).
pub struct QdrantStore {
    client: Client,
    base_url: String,
    collection_name: String,
    vector_size: usize,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct VectorPoint {
    pub id: String,
    pub vector: Vec<f32>,
    pub payload: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub payload: Value,
}

/// Payload stored alongside each vector in Qdrant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ChunkPayload {
    pub note_path: String,
    pub chunk_index: usize,
    pub content: String,
    pub title: String,
    pub tags: Vec<String>,
    pub heading_path: Vec<String>,
    pub word_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

impl QdrantStore {
    pub fn new(config: &QdrantConfig) -> Result<Self, BrainError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| BrainError::QdrantError(format!("HTTP 客户端创建失败: {e}")))?;

        Ok(QdrantStore {
            client,
            base_url: config.url.clone(),
            collection_name: config.collection_name.clone(),
            vector_size: config.vector_size,
        })
    }

    /// Check if Qdrant is reachable.
    pub async fn health_check(&self) -> bool {
        self.client
            .get(format!("{}/healthz", self.base_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Create the collection if it does not exist.
    pub async fn ensure_collection(&self) -> Result<(), BrainError> {
        let resp = self
            .client
            .get(format!(
                "{}/collections/{}",
                self.base_url, self.collection_name
            ))
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("查询 collection 失败: {e}")))?;

        if resp.status().is_success() {
            return Ok(());
        }

        #[derive(Serialize)]
        struct CreateReq {
            vectors: VectorParams,
            hnsw_config: HnswConfig,
        }
        #[derive(Serialize)]
        struct VectorParams {
            size: usize,
            distance: String,
        }
        #[derive(Serialize)]
        struct HnswConfig {
            m: usize,
            ef_construct: usize,
        }

        let body = CreateReq {
            vectors: VectorParams {
                size: self.vector_size,
                distance: "Cosine".to_string(),
            },
            hnsw_config: HnswConfig {
                m: 16,
                ef_construct: 200,
            },
        };

        self.client
            .put(format!(
                "{}/collections/{}",
                self.base_url, self.collection_name
            ))
            .json(&body)
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("创建 collection 失败: {e}")))?;

        tracing::info!("Qdrant collection 创建: {}", self.collection_name);
        Ok(())
    }

    /// Insert or update vector points.
    pub async fn upsert_points(&self, points: Vec<VectorPoint>) -> Result<(), BrainError> {
        if points.is_empty() {
            return Ok(());
        }

        #[derive(Serialize)]
        struct UpsertReq {
            points: Vec<PointData>,
        }
        #[derive(Serialize)]
        struct PointData {
            id: String,
            vector: Vec<f32>,
            payload: Value,
        }

        let body = UpsertReq {
            points: points
                .into_iter()
                .map(|p| PointData {
                    id: p.id,
                    vector: p.vector,
                    payload: p.payload,
                })
                .collect(),
        };

        self.client
            .put(format!(
                "{}/collections/{}/points",
                self.base_url, self.collection_name
            ))
            .json(&body)
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("Upsert 失败: {e}")))?;

        Ok(())
    }

    /// Search by vector.
    pub async fn search(
        &self,
        query_vector: &[f32],
        top_k: usize,
        filter: Option<Value>,
    ) -> Result<Vec<SearchResult>, BrainError> {
        #[derive(Serialize)]
        struct SearchReq {
            vector: Vec<f32>,
            limit: usize,
            with_payload: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            filter: Option<Value>,
        }

        let body = SearchReq {
            vector: query_vector.to_vec(),
            limit: top_k,
            with_payload: true,
            filter,
        };

        let resp = self
            .client
            .post(format!(
                "{}/collections/{}/points/search",
                self.base_url, self.collection_name
            ))
            .json(&body)
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("搜索失败: {e}")))?;

        #[derive(Deserialize)]
        struct SearchResp {
            result: Vec<SearchResultItem>,
        }
        #[derive(Deserialize)]
        struct SearchResultItem {
            id: String,
            score: f32,
            payload: Value,
        }

        let search_resp: SearchResp = resp
            .json()
            .await
            .map_err(|e| BrainError::QdrantError(format!("响应解析失败: {e}")))?;

        Ok(search_resp
            .result
            .into_iter()
            .map(|r| SearchResult {
                id: r.id,
                score: r.score,
                payload: r.payload,
            })
            .collect())
    }

    /// Delete points by ID.
    pub async fn delete_points(&self, ids: &[String]) -> Result<(), BrainError> {
        if ids.is_empty() {
            return Ok(());
        }

        #[derive(Serialize)]
        struct DeleteReq {
            points: Vec<String>,
        }

        self.client
            .post(format!(
                "{}/collections/{}/points/delete",
                self.base_url, self.collection_name
            ))
            .json(&DeleteReq {
                points: ids.to_vec(),
            })
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("删除失败: {e}")))?;

        Ok(())
    }
}
