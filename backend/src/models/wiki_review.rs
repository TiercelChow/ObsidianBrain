use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TopicReviewLocation {
    pub page_id: String,
    pub title: String,
    pub heading: String,
    pub line: usize,
    pub quote: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TopicReviewKind {
    Number,
    Quotation,
    Unsupported,
    MissingCondition,
    Contradiction,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TopicReviewIssue {
    pub kind: TopicReviewKind,
    pub location: TopicReviewLocation,
    pub related: Option<TopicReviewLocation>,
    pub detail: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TopicReviewReport {
    pub run_id: String,
    pub reviewed_at: String,
    pub reviewed_pages: usize,
    pub published_pages: usize,
    pub reviewed_passages: usize,
    pub eligible_passages: usize,
    pub issues: Vec<TopicReviewIssue>,
}

#[derive(Debug, Serialize)]
pub struct TopicReviewStatus {
    pub suspected: Vec<TopicReviewIssue>,
    pub latest: Option<TopicReviewReport>,
}

pub(crate) struct TopicReviewPage {
    pub id: String,
    pub title: String,
    pub revision: String,
    pub run_id: String,
    pub markdown: String,
    pub stale: bool,
}
