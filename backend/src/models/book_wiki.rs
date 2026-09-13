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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RuntimeProviderConfig {
    pub provider_id: String,
    pub display_name: String,
    pub api_protocol: String,
    pub base_url: String,
    pub api_key_env: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct RuntimeProfile {
    pub id: String,
    pub name: String,
    pub runtime: String,
    pub executable: String,
    pub model: String,
    pub provider_config: Option<RuntimeProviderConfig>,
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
    pub conversation_id: String,
    pub answer: String,
    pub runtime: String,
    pub evidence: Vec<KnowledgeEntrySummary>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeConversationSummary {
    pub id: String,
    pub knowledge_base_id: String,
    pub title: String,
    pub message_count: i64,
    pub preview: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub run_id: Option<String>,
    pub evidence: Vec<KnowledgeEntrySummary>,
    pub created_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeConversationDetail {
    #[serde(flatten)]
    pub conversation: KnowledgeConversationSummary,
    pub messages: Vec<KnowledgeMessage>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeTaskExecution {
    pub task: KnowledgeTask,
    pub run_id: String,
    pub evidence: Vec<KnowledgeEntrySummary>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AgentTokenUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub usage_source: String,
}

impl Default for AgentTokenUsage {
    fn default() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            reasoning_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            usage_source: "unavailable".to_string(),
        }
    }
}

impl AgentTokenUsage {
    pub fn estimated(input_tokens: i64, output_tokens: i64) -> Self {
        Self {
            input_tokens,
            output_tokens,
            usage_source: "estimated".to_string(),
            ..Self::default()
        }
    }
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct AgentUsageTotals {
    pub runs: i64,
    pub unreported_runs: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AgentUsagePoint {
    pub date: String,
    #[serde(flatten)]
    pub totals: AgentUsageTotals,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AgentUsageCaller {
    pub caller: String,
    #[serde(flatten)]
    pub totals: AgentUsageTotals,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AgentUsageStats {
    pub start_date: String,
    pub end_date: String,
    pub caller: Option<String>,
    pub usage_source: String,
    pub totals: AgentUsageTotals,
    pub daily: Vec<AgentUsagePoint>,
    pub by_caller: Vec<AgentUsageCaller>,
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
