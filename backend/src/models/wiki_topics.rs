use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TopicSourceSelection {
    pub document_id: String,
    pub span_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<TopicSourceRange>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TopicSourceRange {
    pub offset: usize,
    pub length: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TopicCompileRequest {
    pub page_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub guidance: String,
    pub sources: Vec<TopicSourceSelection>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TopicSourceSection {
    pub id: String,
    pub heading: Option<String>,
    pub line_start: Option<i64>,
    pub total_chars: usize,
}

#[derive(Debug, Serialize)]
pub struct TopicPageSummary {
    pub id: String,
    pub title: String,
    pub has_publication: bool,
    pub has_draft: bool,
    pub draft_revision_id: Option<String>,
    pub needs_review: bool,
    pub compiling: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TopicRevision {
    pub id: String,
    #[serde(skip)]
    pub parent_id: Option<String>,
    pub markdown: String,
    pub run_id: String,
    pub evidence_count: usize,
    pub created_at: String,
    pub links: Vec<TopicLinkedPage>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TopicLink {
    pub page_id: String,
    pub title: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct TopicLinkedPage {
    #[serde(flatten)]
    pub link: TopicLink,
    pub published: bool,
}

#[derive(Debug, Serialize)]
pub struct TopicRevisionSummary {
    pub id: String,
    pub created_at: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopicWritebackRequest {
    pub run_id: String,
    pub expected_revision_id: String,
    pub passages: Vec<usize>,
}

#[derive(Debug, Serialize)]
pub struct TopicWritebackMaterial {
    pub title: String,
    pub kind: TopicWritebackKind,
    pub evidence_count: usize,
    pub passages: Vec<TopicWritebackPassage>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TopicWritebackKind {
    Answer,
    Report,
}

#[derive(Debug, Serialize)]
pub struct TopicWritebackPassage {
    pub index: usize,
    pub title: String,
    pub markdown: String,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) struct TopicDraftCitation {
    pub index: usize,
    pub run_id: String,
    pub origin_index: usize,
}

#[derive(Debug, Serialize)]
pub struct TopicQuestion {
    pub id: String,
    pub question: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TopicPage {
    pub page: TopicPageSummary,
    pub published: Option<TopicRevision>,
    pub draft: Option<TopicRevision>,
    pub last_request: Option<TopicCompileRequest>,
    pub revisions: Vec<TopicRevisionSummary>,
    pub questions: Vec<TopicQuestion>,
    pub backlinks: Vec<TopicLink>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TopicBookBuild {
    pub id: String,
    pub status: String,
    pub phase: String,
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub stop_requested: bool,
    pub outdated: bool,
    pub reviewed_groups: usize,
    pub total_groups: usize,
    pub sources: Vec<TopicBookSource>,
    pub pages: Vec<TopicBookBuildPage>,
    pub failed_groups: Vec<TopicBookIssue>,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_baseline: Option<Vec<TopicSourceStamp>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editorial_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maintenance: Option<TopicMaintenance>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TopicSourceStamp {
    pub document_id: String,
    pub version: String,
    pub path: String,
    pub has_text: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TopicMaintenance {
    pub changes: Vec<TopicSourceChange>,
    pub reused_pages: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TopicSourceChange {
    pub path: String,
    pub change: String,
    pub disposition: String,
    pub reason: String,
    pub page_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TopicBookSource {
    pub path: String,
    pub page_ids: Vec<String>,
    pub total_sections: usize,
    pub linked_sections: usize,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TopicBookBuildPage {
    pub page_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
    pub revision_id: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub assigned_documents: Vec<String>,
    #[serde(default)]
    pub queries: Vec<String>,
    #[serde(default)]
    pub links: Vec<TopicLink>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub update_notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicBookBuildSummary {
    pub status: String,
    pub phase: String,
    pub outdated: bool,
    pub source_count: usize,
    pub ready_pages: usize,
    pub total_pages: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TopicBookIssue {
    pub group: usize,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct TopicWikiIndex {
    pub overview: Option<String>,
    pub pages: Vec<TopicWikiIndexPage>,
    pub issues: Vec<TopicWikiIssue>,
    pub builds: Vec<TopicWikiBuildEvent>,
    pub publications: Vec<TopicWikiPublication>,
}

#[derive(Debug, Serialize)]
pub struct TopicWikiIndexPage {
    pub page_id: String,
    pub title: String,
    pub scope: Option<String>,
    pub summary: String,
    pub needs_review: bool,
}

#[derive(Debug, Serialize)]
pub struct TopicWikiIssue {
    pub page_id: String,
    pub code: &'static str,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct TopicWikiBuildEvent {
    pub attempt_id: String,
    pub status: String,
    pub source_count: usize,
    pub linked_sources: usize,
    pub written_pages: usize,
    pub planned_pages: usize,
    pub recorded_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintenance: Option<TopicMaintenance>,
}

#[derive(Debug, Serialize)]
pub struct TopicWikiPublication {
    pub page_id: String,
    pub title: String,
    pub revision_id: String,
    pub published_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct TopicBookPlan {
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub independent_topics_reason: Option<String>,
    pub topics: Vec<TopicBookProposal>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct TopicBookProposal {
    pub title: String,
    pub focus: String,
    pub sources: Vec<usize>,
    #[serde(default)]
    pub queries: Vec<String>,
    #[serde(default)]
    pub reuse_page_id: Option<String>,
    #[serde(default)]
    pub related: Vec<TopicRelationProposal>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct TopicRelationProposal {
    pub topic: usize,
    pub reason: String,
}

pub(crate) struct TopicBookDocument {
    pub id: String,
    pub path: String,
    pub version: String,
    pub sections: Vec<TopicBookSection>,
}

pub(crate) struct TopicBookSection {
    pub id: String,
    pub heading: Option<String>,
    pub total_chars: usize,
}

pub(crate) struct TopicExcerpt {
    pub span_id: String,
    pub document_id: String,
    pub version_id: String,
    pub source_path: String,
    pub heading: Option<String>,
    pub offset: usize,
    pub content: String,
}

pub(crate) struct TopicSection {
    pub heading: String,
    pub markdown: String,
}

pub(crate) struct TopicPublication {
    pub page_id: String,
    pub revision_id: String,
    pub sections: Vec<TopicSection>,
}

pub(crate) struct TopicSectionMatch {
    pub page_id: String,
    pub revision_id: String,
    pub run_id: String,
    pub title: String,
    pub section: TopicSection,
}
