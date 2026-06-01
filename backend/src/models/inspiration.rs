//! 灵感熔炉相关数据模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// 灵感类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspirationType {
    ConceptCombo,
    ReverseQuestion,
    Counterpoint,
}

impl InspirationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            InspirationType::ConceptCombo => "concept_combo",
            InspirationType::ReverseQuestion => "reverse_question",
            InspirationType::Counterpoint => "counterpoint",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "concept_combo" => Some(InspirationType::ConceptCombo),
            "reverse_question" => Some(InspirationType::ReverseQuestion),
            "counterpoint" => Some(InspirationType::Counterpoint),
            _ => None,
        }
    }
}

impl std::fmt::Display for InspirationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 灵感记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspirationRecord {
    pub id: Uuid,
    pub inspiration_type: InspirationType,
    pub input_refs: serde_json::Value,
    pub output: String,
    pub created_at: DateTime<Utc>,
}

/// 概念组合输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboOutput {
    pub inspiration: String,
    pub suggestions: Vec<String>,
    pub experiment_idea: Option<String>,
}

/// 反向提问输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOutput {
    pub questions: Vec<QuestionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionItem {
    pub question: String,
    pub why_it_matters: String,
    pub question_type: String,
}

/// 对立观点输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterpointOutput {
    pub counterpoints: Vec<CounterpointItem>,
    pub overall_assessment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterpointItem {
    pub claim: String,
    pub counter: String,
    pub weakness: String,
    pub suggestion: String,
}

/// 概念
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub term: String,
    pub weight: f64,
    pub source: ConceptSource,
    pub note_paths: Vec<String>,
}

/// 概念来源
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConceptSource {
    NoteTag,
    NoteKeyword,
    CodeRepo,
}

/// 概念池
#[derive(Debug, Clone)]
pub struct ConceptPool {
    pub concepts: Vec<Concept>,
    pub built_at: DateTime<Utc>,
}

/// 灵感配置
#[derive(Debug, Clone)]
pub struct InspirationConfig {
    pub max_concepts: usize,
    pub min_tfidf: f64,
    pub min_distance: f64,
    pub max_distance: f64,
    pub dedup_days: u32,
    pub cache_ttl_secs: u64,
    pub combo_temperature: f32,
    pub question_temperature: f32,
    pub counterpoint_temperature: f32,
    pub max_tokens: u32,
}

impl Default for InspirationConfig {
    fn default() -> Self {
        Self {
            max_concepts: 5000,
            min_tfidf: 0.01,
            min_distance: 0.6,
            max_distance: 0.95,
            dedup_days: 7,
            cache_ttl_secs: 3600,
            combo_temperature: 0.9,
            question_temperature: 0.8,
            counterpoint_temperature: 0.7,
            max_tokens: 2048,
        }
    }
}

/// SQLite 行模型
#[derive(Debug, Clone)]
pub struct InspirationRow {
    pub id: String,
    pub insp_type: String,
    pub input_refs: String,
    pub output: String,
    pub created_at: String,
}

/// 灵感结果（返回给前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InspirationResult {
    #[serde(rename = "concept_combo")]
    ConceptCombo {
        concept_a: ConceptRef,
        concept_b: ConceptRef,
        inspiration: String,
        suggestions: Vec<String>,
        experiment_idea: Option<String>,
        generated_at: DateTime<Utc>,
    },
    #[serde(rename = "reverse_question")]
    ReverseQuestion {
        note: NoteRef,
        questions: Vec<QuestionItem>,
        generated_at: DateTime<Utc>,
    },
    #[serde(rename = "counterpoint")]
    Counterpoint {
        note: NoteRef,
        counterpoints: Vec<CounterpointItem>,
        overall_assessment: String,
        related_notes: Vec<String>,
        generated_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptRef {
    pub term: String,
    pub source: String,
    pub source_path: Option<String>,
    pub obsidian_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteRef {
    pub path: String,
    pub title: String,
    pub obsidian_uri: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspiration_type_roundtrip() {
        assert_eq!(InspirationType::from_str("concept_combo"), Some(InspirationType::ConceptCombo));
        assert_eq!(InspirationType::from_str("reverse_question"), Some(InspirationType::ReverseQuestion));
        assert_eq!(InspirationType::from_str("counterpoint"), Some(InspirationType::Counterpoint));
        assert_eq!(InspirationType::from_str("unknown"), None);
    }

    #[test]
    fn test_inspiration_record_roundtrip() {
        let record = InspirationRecord {
            id: Uuid::new_v4(),
            inspiration_type: InspirationType::ConceptCombo,
            input_refs: serde_json::json!({"a": "rust", "b": "ai"}),
            output: "test output".to_string(),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&record).unwrap();
        let parsed: InspirationRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, record.id);
        assert_eq!(parsed.inspiration_type, InspirationType::ConceptCombo);
    }
}
