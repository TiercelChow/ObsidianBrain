use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum BookKind {
    Folder,
    Pdf,
}

impl BookKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Pdf => "pdf",
        }
    }
}

impl TryFrom<&str> for BookKind {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "folder" => Ok(Self::Folder),
            "pdf" => Ok(Self::Pdf),
            _ => Err(format!("未知书籍类型: {value}")),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookProgress {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_file: Option<String>,
    pub position: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<i64>,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReaderBook {
    pub id: String,
    pub path: String,
    pub kind: BookKind,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    pub added_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<BookProgress>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeBaseSummary {
    pub id: String,
    pub book_id: String,
    pub book_name: String,
    pub book_path: String,
    pub book_kind: String,
    pub book_description: String,
    pub book_category: String,
    pub lifecycle: String,
    pub sync_state: String,
    pub health_state: String,
    pub last_error: Option<String>,
    pub last_synced_at: Option<String>,
    pub last_scanned_at: Option<String>,
    pub source_count: i64,
    pub entry_count: i64,
    pub claim_count: i64,
    pub task_count: i64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct BookKnowledgeCard {
    pub book: ReaderBook,
    pub knowledge_base: Option<KnowledgeBaseSummary>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeEntrySummary {
    pub id: String,
    pub knowledge_base_id: String,
    pub entry_type: String,
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub confidence: Option<f64>,
    pub source_path: Option<String>,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeCitation {
    pub id: String,
    pub source_path: String,
    pub heading: Option<String>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    pub quote_text: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeEntryDetail {
    #[serde(flatten)]
    pub entry: KnowledgeEntrySummary,
    pub content_md: String,
    pub citations: Vec<KnowledgeCitation>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeTask {
    pub id: String,
    pub knowledge_base_id: String,
    pub book_name: String,
    pub title: String,
    pub description: String,
    pub task_type: String,
    pub status: String,
    pub result_summary: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ConfigDocument {
    pub id: String,
    pub knowledge_base_id: Option<String>,
    pub scope: String,
    pub name: String,
    pub content_md: String,
    pub revision: i64,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct RuntimeProfile {
    pub id: String,
    pub name: String,
    pub runtime: String,
    pub executable: String,
    pub model: String,
    pub enabled: bool,
    pub revision: i64,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct RuntimeHealth {
    pub profile: RuntimeProfile,
    pub available: bool,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct RuntimeVerification {
    pub profile_id: String,
    pub available: bool,
    pub message: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeAnswer {
    pub run_id: String,
    pub answer: String,
    pub runtime: String,
    pub evidence: Vec<KnowledgeEntrySummary>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeTaskExecution {
    pub task: KnowledgeTask,
    pub run_id: String,
    pub evidence: Vec<KnowledgeEntrySummary>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AgentRun {
    pub id: String,
    pub knowledge_base_id: Option<String>,
    pub runtime: String,
    pub task_type: String,
    pub status: String,
    pub input: Value,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub created_at: String,
}
