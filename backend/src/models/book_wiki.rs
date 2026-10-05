use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Markdown 抽取算法版本。切分规则变更时递增并纳入 source version 派生：
/// 旧版本与其 span 原样保留（历史引用可回放），新同步建立新版本。
pub const MARKDOWN_EXTRACTION_VERSION: &str = "markdown-v3-structured";

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read_at: Option<i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub by_file: BTreeMap<String, ReaderFileProgress>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReaderFileKind {
    Md,
    Pdf,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReaderFileProgress {
    pub kind: ReaderFileKind,
    pub position: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<i64>,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReaderProgressState {
    pub last_file: String,
    pub last_read_at: i64,
    pub by_file: BTreeMap<String, ReaderFileProgress>,
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
    pub compile_phase: String,
    pub compile_message: String,
    pub compile_current_batch: i64,
    pub compile_total_batches: i64,
    pub compile_active_run_id: Option<String>,
    pub compile_change_set_id: Option<String>,
    pub compile_started_at: Option<String>,
    pub compile_heartbeat_at: Option<String>,
    pub compile_cancel_requested: bool,
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
pub struct KnowledgeGraphRelation {
    pub id: String,
    pub from_entry_id: String,
    pub to_entry_id: String,
    pub relation_type: String,
    pub strength: Option<f64>,
    pub evidence: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeGraphSnapshot {
    pub entries: Vec<KnowledgeEntrySummary>,
    pub relations: Vec<KnowledgeGraphRelation>,
    pub total_entries: i64,
    pub truncated: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct BookKnowledgeCard {
    pub book: ReaderBook,
    pub knowledge_base: Option<KnowledgeBaseSummary>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
    pub source_impact_count: i64,
    /// Bounded preview; the count reports additional affected sources.
    pub source_impacts: Vec<KnowledgeSourceImpact>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeSourceImpact {
    pub source_document_id: String,
    pub source_path: String,
    pub previous_version_id: String,
    pub current_version_id: Option<String>,
    pub reason: String,
    pub affected_via_entry_id: Option<String>,
    pub affected_via_entry_title: Option<String>,
    pub detected_at: String,
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
    pub external_research_enabled: bool,
    pub external_domains: Vec<String>,
    pub external_request_limit: i64,
    pub external_requests_used: i64,
    pub brief: ResearchBrief,
    pub created_at: String,
    pub updated_at: String,
}

/// User-confirmed editorial contract; it is task input, not model evidence.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ResearchBrief {
    pub confirmed: bool,
    pub audience: String,
    pub purpose: String,
    pub tone: String,
    pub depth: String,
    pub presentation_theme: String,
    /// Narrative material by default; Q&A slides require explicit confirmation.
    pub presentation_format: String,
    pub emphasis: String,
}

impl Default for ResearchBrief {
    fn default() -> Self {
        Self {
            confirmed: false,
            audience: "general".into(),
            purpose: "understand".into(),
            tone: "analytical".into(),
            depth: "standard".into(),
            presentation_theme: "editorial".into(),
            presentation_format: "narrative".into(),
            emphasis: String::new(),
        }
    }
}

impl ResearchBrief {
    pub fn validate(&self) -> Result<(), String> {
        for (name, value, allowed) in [
            (
                "受众",
                self.audience.as_str(),
                &["general", "specialist", "beginner", "self"] as &[&str],
            ),
            (
                "用途",
                self.purpose.as_str(),
                &["understand", "decision", "teach", "reference"],
            ),
            (
                "表述",
                self.tone.as_str(),
                &["analytical", "technical", "narrative", "concise"],
            ),
            ("深度", self.depth.as_str(), &["brief", "standard", "deep"]),
            (
                "演示主题",
                self.presentation_theme.as_str(),
                &["editorial", "midnight", "sage"],
            ),
            (
                "演示编排",
                self.presentation_format.as_str(),
                &["narrative", "qa"],
            ),
        ] {
            if !allowed.contains(&value) {
                return Err(format!("未知的研究{name}偏好"));
            }
        }
        if self.emphasis.chars().count() > 500 {
            return Err("研究强调事项不能超过 500 个字符".into());
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResearchDecisionPoint {
    pub field: String,
    pub question: String,
    pub impact: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResearchPreflight {
    pub summary: String,
    pub recommended: ResearchBrief,
    /// A ranked subset of audience, purpose, tone, depth and presentation choices.
    pub focus_decisions: Vec<String>,
    /// Contextual prompts for the corresponding decisions. Older responses may omit these.
    #[serde(default)]
    pub decision_points: Vec<ResearchDecisionPoint>,
    pub cautions: Vec<String>,
}

impl ResearchPreflight {
    pub fn validate(&self, deliverable_type: &str) -> Result<(), String> {
        self.recommended.validate()?;
        if self.summary.trim().is_empty() || self.summary.chars().count() > 600 {
            return Err("研究预分析摘要长度不合适".into());
        }
        if self.focus_decisions.is_empty() || self.focus_decisions.len() > 4 {
            return Err("研究预分析必须指出一至四个决策点".into());
        }
        let mut seen = std::collections::HashSet::new();
        for field in &self.focus_decisions {
            if !matches!(
                field.as_str(),
                "audience"
                    | "purpose"
                    | "tone"
                    | "depth"
                    | "presentation_theme"
                    | "presentation_format"
            ) || (field == "presentation_theme" && deliverable_type != "presentation")
                || (field == "presentation_format" && deliverable_type != "presentation")
                || !seen.insert(field)
            {
                return Err("研究预分析包含无效或重复决策点".into());
            }
        }
        if deliverable_type != "presentation" && self.recommended.presentation_format != "narrative"
        {
            return Err("研究报告不需要演示编排方式".into());
        }
        if self.decision_points.len() > self.focus_decisions.len() {
            return Err("研究预分析的决策说明数量超出范围".into());
        }
        let mut explained = std::collections::HashSet::new();
        for decision in &self.decision_points {
            if !self.focus_decisions.contains(&decision.field)
                || !explained.insert(&decision.field)
                || decision.question.trim().is_empty()
                || decision.question.chars().count() > 160
                || decision.impact.trim().is_empty()
                || decision.impact.chars().count() > 160
            {
                return Err("研究预分析包含无效或重复的决策说明".into());
            }
        }
        if self.cautions.len() > 3 || self.cautions.iter().any(|v| v.chars().count() > 160) {
            return Err("研究预分析提示超出长度限制".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod research_brief_tests {
    use super::ResearchBrief;

    #[test]
    fn test_research_brief_rejects_unknown_presentation_theme() {
        let mut brief = ResearchBrief::default();
        brief.presentation_theme = "neon".into();
        assert!(brief.validate().is_err());
    }

    #[test]
    fn test_research_brief_presentation_format_is_explicit_and_legacy_safe() {
        let old: ResearchBrief = serde_json::from_value(serde_json::json!({
            "confirmed":true,"audience":"general","purpose":"understand",
            "tone":"analytical","depth":"standard","presentation_theme":"editorial",
            "emphasis":""
        }))
        .unwrap();
        assert_eq!(old.presentation_format, "narrative");
        let mut brief = old;
        brief.presentation_format = "qa".into();
        assert!(brief.validate().is_ok());
        brief.presentation_format = "unknown".into();
        assert!(brief.validate().is_err());
    }

    #[test]
    fn test_research_brief_accepts_bounded_user_preferences() {
        let brief = ResearchBrief {
            confirmed: true,
            audience: "specialist".into(),
            purpose: "decision".into(),
            tone: "technical".into(),
            depth: "deep".into(),
            presentation_theme: "midnight".into(),
            presentation_format: "narrative".into(),
            emphasis: "优先比较方案的适用边界".into(),
        };
        assert!(brief.validate().is_ok());
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResearchQuestion {
    pub id: String,
    pub title: String,
    pub question: String,
    pub required_evidence: Vec<String>,
    /// Final-content estimate, not the request cap including JSON and reasoning.
    #[serde(default)]
    pub expected_output_tokens: Option<u32>,
    /// Stable compiled objects selected by planning for review/refresh.
    #[serde(default)]
    pub target_entry_ids: Vec<String>,
}

/// Public business scope and output criteria, never hidden reasoning.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResearchPlan {
    pub goal: String,
    /// Reader-facing deliverable title; absent from plans saved before this field existed.
    #[serde(default)]
    pub report_title: Option<String>,
    pub constraints: Vec<String>,
    pub acceptance: Vec<String>,
    pub depth: String,
    pub terminology: Vec<String>,
    pub questions: Vec<ResearchQuestion>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResearchFinding {
    pub finding: String,
    pub status: String,
    /// This stage's one-based, actually read S numbers; not global numbering.
    pub citation_indices: Vec<usize>,
    pub limitations: Vec<String>,
    #[serde(default)]
    pub baseline_entry_id: Option<String>,
    #[serde(default)]
    pub baseline_claim_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResearchSectionOutput {
    pub summary: String,
    pub content_md: String,
    pub findings: Vec<ResearchFinding>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResearchSectionCheck {
    pub question_id: String,
    pub revision: i64,
    pub assessment: String,
    pub note: String,
}

/// Cross-topic business conclusions and explicit limits, not a proof of truth.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResearchSynthesisOutput {
    pub summary: String,
    pub content_md: String,
    pub findings: Vec<ResearchFinding>,
    pub section_checks: Vec<ResearchSectionCheck>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResearchEvidenceReference {
    pub run_id: String,
    pub citation_index: usize,
    pub kind: String,
    pub object_id: String,
    pub version_id: String,
    pub snapshot_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResearchStageSummary {
    pub stage_key: String,
    pub title: String,
    pub kind: String,
    pub ordinal: i64,
    pub status: String,
    pub revision: i64,
    pub run_id: Option<String>,
    pub summary: String,
    pub content_characters: i64,
    pub finding_count: i64,
    pub error: Option<String>,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ResearchWorkspace {
    pub task_id: String,
    pub knowledge_base_id: String,
    pub original_request: Value,
    pub plan: Option<ResearchPlan>,
    pub stages: Vec<ResearchStageSummary>,
    pub baselines: Vec<ResearchBaselineSummary>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResearchBaselineSummary {
    pub question_id: String,
    pub entry_id: String,
    pub revision: i64,
    pub title: String,
    pub status: String,
    pub content_characters: usize,
    pub claim_count: usize,
    pub captured_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResearchStageContent {
    pub stage: ResearchStageSummary,
    /// Active retry and retained content have different provenance.
    #[serde(default)]
    pub content_run_id: Option<String>,
    pub content_md: String,
    pub findings: Vec<ResearchFinding>,
    pub evidence: Vec<ResearchEvidenceReference>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ResearchUnpublishedOutput {
    pub run_id: String,
    /// interrupted: incomplete Run; rejected: complete model answer not saved as a stage.
    pub kind: String,
    pub text: String,
    pub stop_reason: Option<String>,
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
pub struct ModelProviderProfile {
    pub provider_id: String,
    pub display_name: String,
    pub api_protocol: String,
    pub base_url: String,
    pub model: String,
    pub credential_source: String,
    pub api_key_env: String,
    pub api_key_configured: bool,
    pub enabled: bool,
    /// Declared model capacity; absent means unknown, not a guessed default.
    #[serde(default)]
    pub context_window: Option<u32>,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
    /// Application policy. `auto` inherits Harness/catalog reasoning defaults.
    #[serde(default = "default_reasoning_policy")]
    pub reasoning_policy: String,
    pub revision: i64,
    pub updated_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveModelProviderRequest {
    pub provider_id: Option<String>,
    pub display_name: String,
    pub api_protocol: String,
    pub base_url: String,
    pub model: String,
    pub credential_source: String,
    #[serde(default)]
    pub api_key_env: String,
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub context_window: Option<u32>,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
    #[serde(default = "default_reasoning_policy")]
    pub reasoning_policy: String,
    #[serde(default)]
    pub expected_revision: i64,
}

fn default_reasoning_policy() -> String {
    "auto".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct RuntimeProfile {
    pub id: String,
    pub name: String,
    pub runtime: String,
    pub executable: String,
    pub model: String,
    pub provider_id: Option<String>,
    pub provider_config: Option<ModelProviderProfile>,
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
pub struct UnfinishedQaRun {
    pub run_id: String,
    pub question: String,
    pub conversation_id: Option<String>,
    pub completed_without_history: bool,
    pub stop_reason: Option<String>,
    pub has_partial_answer: bool,
    pub error: String,
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
pub struct AgentRunInspectionSnapshot {
    pub run_id: String,
    pub prompt_text: String,
    pub prompt_hash: String,
    pub prompt_characters: i64,
    pub skill_snapshots: Value,
    pub config_snapshots: Value,
    pub tool_names: Vec<String>,
    pub evidence_refs: Value,
    pub created_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AgentRunInspection {
    pub run: AgentRun,
    pub events: Vec<AgentRunEvent>,
    pub snapshot: Option<AgentRunInspectionSnapshot>,
    pub evidence: Vec<Value>,
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
pub struct WikiSkillFile {
    pub relative_path: String,
    pub media_type: String,
    pub content_text: String,
    pub content_hash: String,
    pub size_bytes: i64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct WikiSkillOrigin {
    pub repository_url: String,
    pub source_path: String,
    pub source_ref: String,
    pub license_spdx: String,
    pub attribution: String,
    pub adaptation_notes: String,
    pub reviewed_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WikiSkillEvaluationFinding {
    pub case_id: String,
    pub name: String,
    pub passed: bool,
    pub missing_concepts: Vec<String>,
    pub forbidden_concepts: Vec<String>,
    pub weight: f64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct WikiSkillEvaluationRun {
    pub id: String,
    pub skill_version_id: String,
    pub suite_id: String,
    pub suite_name: String,
    pub score: f64,
    pub baseline_score: Option<f64>,
    pub passed: bool,
    pub findings: Vec<WikiSkillEvaluationFinding>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WikiSkillBenchmarkCase {
    pub id: String,
    pub suite_id: String,
    pub ordinal: i64,
    pub name: String,
    pub scenario_type: String,
    pub fixture: Value,
    pub expectations: Value,
    pub weight: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WikiSkillBenchmarkCaseResult {
    pub case_id: String,
    pub name: String,
    pub variant: String,
    pub agent_run_id: Option<String>,
    pub response_text: String,
    pub citations: Vec<String>,
    pub metrics: Value,
    pub score: f64,
    pub passed: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct WikiSkillBenchmarkRun {
    pub id: String,
    pub skill_id: String,
    pub skill_version_id: String,
    pub baseline_version_id: String,
    pub suite_id: String,
    pub suite_name: String,
    pub knowledge_base_id: String,
    pub runtime_profile_id: String,
    pub model: String,
    pub status: String,
    pub total_cases: i64,
    pub completed_cases: i64,
    pub candidate_score: Option<f64>,
    pub baseline_score: Option<f64>,
    pub score_delta: Option<f64>,
    pub passed: bool,
    pub metrics: Value,
    pub error: Option<String>,
    pub results: Vec<WikiSkillBenchmarkCaseResult>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct WikiSkillVersion {
    pub id: String,
    pub revision: i64,
    pub content_hash: String,
    pub release_state: String,
    pub parent_version_id: Option<String>,
    pub changelog: String,
    pub created_at: String,
    pub files: Vec<WikiSkillFile>,
    pub origin: Option<WikiSkillOrigin>,
    pub latest_evaluation: Option<WikiSkillEvaluationRun>,
    pub latest_benchmark: Option<WikiSkillBenchmarkRun>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct WikiSkillDetail {
    pub skill: WikiSkill,
    pub current_version_id: String,
    pub versions: Vec<WikiSkillVersion>,
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
    /// Stable outline/neighbor locations; compilation fragments add exact
    /// character ranges. Empty for legacy extractions, never inferred facts.
    pub locator: Value,
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
    pub classification: String,
    pub citation_audit: Value,
    pub impact: Value,
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
    pub classification_summary: Value,
    pub citation_audit: Value,
    pub impact_summary: Value,
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
pub struct KnowledgeCompileFragment {
    pub ordinal: i64,
    pub batch: i64,
    pub source_document_id: String,
    pub source_version_id: String,
    pub source_span_id: String,
    pub source_path: String,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    pub locator: Value,
    pub status: String,
    pub run_id: Option<String>,
    /// Candidate references are batch-level, not a proof of fragment coverage.
    pub candidate_slugs: Vec<String>,
    pub reason: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct KnowledgeCompileReport {
    pub id: String,
    pub knowledge_base_id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub selected_sources: i64,
    pub current_sources: i64,
    pub selected_spans: i64,
    pub planned: bool,
    pub fragment_total: i64,
    pub analyzed_fragments: i64,
    pub no_material_fragments: i64,
    pub failed_fragments: i64,
    pub unprocessed_fragments: i64,
    pub fragment_offset: usize,
    pub fragment_has_more: bool,
    pub fragments: Vec<KnowledgeCompileFragment>,
    pub topic_total: i64,
    pub topic_offset: usize,
    pub topic_has_more: bool,
    pub topics: Vec<Value>,
    pub change_set_id: Option<String>,
    pub error: Option<String>,
    pub previous_report_id: Option<String>,
    pub next_report_id: Option<String>,
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
    pub validation_details: Value,
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
