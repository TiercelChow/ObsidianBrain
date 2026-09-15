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
    pub source_available: bool,
    pub lifecycle: String,
    pub sync_state: String,
    pub compile_mode: String,
    pub compile_state: String,
    pub compile_error: Option<String>,
    pub health_state: String,
    pub last_error: Option<String>,
    pub last_synced_at: Option<String>,
    pub last_scanned_at: Option<String>,
    pub last_compiled_at: Option<String>,
    pub compile_processed_sources: i64,
    pub compile_total_sources: i64,
    pub pending_review_count: i64,
    pub source_count: i64,
    pub entry_count: i64,
    pub claim_count: i64,
    pub task_count: i64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeEntryPage {
    pub entries: Vec<KnowledgeEntrySummary>,
    pub offset: i64,
    pub limit: i64,
    pub total: i64,
    pub has_more: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeBridgeEntry {
    #[serde(flatten)]
    pub entry: KnowledgeEntrySummary,
    pub degree: i64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeGraphOverview {
    pub relation_count: i64,
    pub orphan_entries: Vec<KnowledgeEntrySummary>,
    pub bridge_entries: Vec<KnowledgeBridgeEntry>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeGraphPath {
    pub entries: Vec<KnowledgeEntrySummary>,
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
    pub aliases: Vec<String>,
    pub edit_policy: String,
    pub revision: i64,
    pub citations: Vec<KnowledgeCitation>,
    pub claims: Vec<KnowledgeClaimSummary>,
    pub relations: Vec<KnowledgeRelationSummary>,
    pub versions: Vec<KnowledgeEntryVersionSummary>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeClaimSummary {
    pub id: String,
    pub predicate: String,
    pub object_text: Option<String>,
    pub claim_text: String,
    pub confidence: Option<f64>,
    pub verification_status: String,
    pub citation_count: i64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeRelationSummary {
    pub id: String,
    pub direction: String,
    pub relation_type: String,
    pub related_entry_id: String,
    pub related_entry_title: String,
    pub strength: Option<f64>,
    pub evidence: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeEntryVersionSummary {
    pub revision: i64,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub created_at: String,
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
    pub deliverable_type: String,
    pub artifact_state: String,
    pub knowledge_change_state: String,
    pub cancel_requested: bool,
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
    pub artifacts: Vec<KnowledgeArtifact>,
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

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AgentRunEvent {
    pub run_id: String,
    pub sequence: i64,
    pub event_type: String,
    pub phase: Option<String>,
    pub message: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct WikiSkill {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub source_type: String,
    pub status: String,
    pub permissions: Vec<String>,
    pub requirements: Vec<String>,
    pub revision: i64,
    pub instructions: String,
    pub enabled: bool,
    pub usage_scope: String,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SourceSpanSnapshot {
    pub id: String,
    pub source_document_id: String,
    pub source_version_id: String,
    pub source_path: String,
    pub heading: Option<String>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    pub content: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SourceDocumentSummary {
    pub id: String,
    pub knowledge_base_id: String,
    pub relative_path: String,
    pub title: String,
    pub current_version_id: String,
    pub sync_status: String,
    pub span_count: i64,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeChange {
    pub id: String,
    pub ordinal: i64,
    pub operation: String,
    pub object_type: String,
    pub object_id: String,
    pub expected_revision: Option<i64>,
    pub before: Option<Value>,
    pub after: Value,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeChangeSet {
    pub id: String,
    pub knowledge_base_id: String,
    pub agent_run_id: Option<String>,
    pub title: String,
    pub reason: String,
    pub risk_level: String,
    pub status: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<String>,
    pub changes: Vec<KnowledgeChange>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SemanticCompileResult {
    pub knowledge_base: KnowledgeBaseSummary,
    pub change_set: KnowledgeChangeSet,
    pub processed_sources: i64,
    pub total_sources: i64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeArtifact {
    pub id: String,
    pub knowledge_base_id: String,
    pub knowledge_task_id: Option<String>,
    pub agent_run_id: Option<String>,
    pub skill_id: Option<String>,
    pub artifact_type: String,
    pub title: String,
    pub relative_path: String,
    pub mime_type: String,
    pub content_hash: String,
    pub size_bytes: i64,
    pub validation_state: String,
    pub validation_message: String,
    pub created_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeHealthIssue {
    pub code: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub object_ids: Vec<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeHealthReport {
    pub knowledge_base_id: String,
    pub state: String,
    pub semantic_entry_count: i64,
    pub source_span_count: i64,
    pub pending_review_count: i64,
    pub issues: Vec<KnowledgeHealthIssue>,
    pub generated_at: String,
}
