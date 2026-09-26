//! Business resource policy. Harness still owns reasoning and the tool loop.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct QaPlan {
    pub goal: String,
    pub constraints: Vec<String>,
    pub subquestions: Vec<String>,
    pub evidence_requirements: Vec<String>,
    pub depth: String,
    pub scope: String,
    pub expected_output_tokens: Option<u32>,
}

impl QaPlan {
    pub fn fallback(question: &str) -> Self {
        // Used only before/when the lightweight planner is unavailable; the
        // model's explicit plan supersedes this conservative heuristic.
        let lower = question.to_lowercase();
        let broad = ["全书", "全部", "所有", "全面", "whole book", "all chapters"]
            .iter()
            .any(|word| lower.contains(word));
        let detailed = ["详细", "深入", "公式", "推导", "比较", "compare", "derive"]
            .iter()
            .any(|word| lower.contains(word));
        Self {
            goal: question.chars().take(800).collect(),
            constraints: vec![],
            subquestions: vec![question.chars().take(400).collect()],
            evidence_requirements: vec!["优先编译正文；公式、条件或冲突需要回读原文".into()],
            depth: if broad {
                "comprehensive"
            } else if detailed {
                "detailed"
            } else {
                "standard"
            }
            .into(),
            scope: if broad {
                "whole_book"
            } else if detailed {
                "cross_topic"
            } else {
                "focused"
            }
            .into(),
            expected_output_tokens: None,
        }
    }

    pub fn normalize(&mut self, question: &str) {
        self.goal = self.goal.trim().chars().take(800).collect();
        if self.goal.is_empty() {
            self.goal = question.chars().take(800).collect();
        }
        self.constraints = bounded_list(&self.constraints, 16, 300);
        self.subquestions = bounded_list(&self.subquestions, 12, 400);
        if self.subquestions.is_empty() {
            self.subquestions.push(question.chars().take(400).collect());
        }
        self.evidence_requirements = bounded_list(&self.evidence_requirements, 12, 400);
        if !["brief", "standard", "detailed", "comprehensive"].contains(&self.depth.as_str()) {
            self.depth = "standard".into();
        }
        if !["focused", "cross_topic", "whole_book"].contains(&self.scope.as_str()) {
            self.scope = "focused".into();
        }
        self.expected_output_tokens = self
            .expected_output_tokens
            .filter(|v| *v > 0)
            .map(|v| v.min(262_144));
    }
}

pub(super) fn bounded_list(values: &[String], items: usize, chars: usize) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        let value: String = value.trim().chars().take(chars).collect();
        if !value.is_empty() && !result.contains(&value) {
            result.push(value);
        }
        if result.len() >= items {
            break;
        }
    }
    result
}

/// A conservative UTF-8 estimate, NOT the provider's tokenizer or bill. Runtime
/// occupancy supersedes it once ACP reports a context update.
pub(super) fn estimated_tokens(value: &str) -> u64 {
    let (cjk, other_bytes) = value.chars().fold((0_u64, 0_u64), |(cjk, other), ch| {
        if matches!(ch as u32,0x3400..=0x4dbf|0x4e00..=0x9fff|0xf900..=0xfaff) {
            (cjk + 1, other)
        } else {
            (cjk, other + ch.len_utf8() as u64)
        }
    });
    cjk.saturating_mul(2) + other_bytes.div_ceil(3)
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct QaResources {
    pub prompt_tokens: u64,
    pub output_tokens: u32,
    pub tool_reserve_tokens: u64,
    pub initial_entry_target: usize,
    pub soft_tool_calls: u32,
    pub hard_tool_calls: u32,
    pub soft_retrieval_tokens: u64,
    pub hard_retrieval_tokens: u64,
    pub timeout_seconds: u32,
    pub planning_token_limit: u64,
    pub planning_seconds: u32,
    pub context_capacity_known: bool,
}

impl QaResources {
    pub fn for_resume(
        mut self,
        previous_output: u32,
        context: Option<u32>,
        cap: Option<u32>,
    ) -> Self {
        let capacity = u64::from(context.unwrap_or(32_768)).min(1_048_576);
        self.output_tokens = self
            .output_tokens
            .max(previous_output.saturating_mul(2))
            .min(cap.unwrap_or(u32::MAX))
            .min((capacity / 3) as u32)
            .max(1);
        let available = capacity.saturating_sub(u64::from(self.output_tokens));
        self.tool_reserve_tokens = available / 3;
        self.prompt_tokens = available
            .saturating_sub(self.tool_reserve_tokens)
            .saturating_sub(1024);
        self.soft_retrieval_tokens = self
            .tool_reserve_tokens
            .min(u64::from(self.soft_tool_calls) * 2500);
        self.hard_retrieval_tokens = self.soft_retrieval_tokens.saturating_mul(4);
        self
    }
    pub fn new(
        plan: &QaPlan,
        context: Option<u32>,
        output_cap: Option<u32>,
        catalog_size: usize,
    ) -> Self {
        // Unknown capacity is intentionally an application request guard, not
        // a fabricated claim about the model. Known large contexts are usable.
        let capacity = u64::from(context.unwrap_or(32_768)).min(1_048_576);
        let weight: u32 = match plan.depth.as_str() {
            "brief" => 1,
            "detailed" => 3,
            "comprehensive" => 5,
            _ => 2,
        };
        let breadth: u32 = match plan.scope.as_str() {
            "whole_book" => 5,
            "cross_topic" => 3,
            _ => 1,
        };
        let questions = plan.subquestions.len().max(1) as u32;
        let desired = plan
            .expected_output_tokens
            .unwrap_or(match plan.depth.as_str() {
                "brief" => 1024,
                "detailed" => 8192,
                "comprehensive" => 16384,
                _ => 4096,
            })
            .saturating_add(768 * questions.saturating_sub(1));
        let output_tokens = desired
            .min(output_cap.unwrap_or(u32::MAX))
            .min((capacity / 4) as u32)
            .max(1);
        let available = capacity.saturating_sub(u64::from(output_tokens));
        let tool_reserve_tokens = (available / 3).max(1);
        let prompt_tokens = available
            .saturating_sub(tool_reserve_tokens)
            .saturating_sub(1024);
        let target = (if plan.scope == "whole_book" {
            catalog_size.max(6)
        } else {
            (breadth * 4 + questions * 2) as usize
        })
        .min(if catalog_size == 0 {
            usize::MAX
        } else {
            catalog_size
        })
        .min((prompt_tokens / 768).max(1) as usize);
        let soft_tool_calls = (6 + questions * 3 + breadth * 4 + (target / 4) as u32).min(80);
        let hard_tool_calls = soft_tool_calls.saturating_mul(4).min(240);
        let soft_retrieval_tokens = tool_reserve_tokens.min(u64::from(soft_tool_calls) * 2500);
        Self {
            prompt_tokens,
            output_tokens,
            tool_reserve_tokens,
            initial_entry_target: target,
            soft_tool_calls,
            hard_tool_calls,
            soft_retrieval_tokens,
            hard_retrieval_tokens: soft_retrieval_tokens.saturating_mul(4),
            timeout_seconds: (90 + weight * 45 + questions * 10 + desired / 128).min(600),
            planning_token_limit: capacity.saturating_mul(2).min(2_097_152),
            planning_seconds: 60 + breadth * 30,
            context_capacity_known: context.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broad_question_has_larger_but_bounded_budget() {
        let small = QaPlan::fallback("KV cache是什么？");
        let broad = QaPlan::fallback("请详细比较全书的全部优化机制，给出公式、步骤和边界");
        let small_budget = QaResources::new(&small, None, None, 100);
        let broad_budget = QaResources::new(&broad, Some(131_072), Some(20_000), 100);
        assert!(broad_budget.output_tokens > small_budget.output_tokens);
        assert!(broad_budget.initial_entry_target > 10);
        assert!(broad_budget.soft_tool_calls > small_budget.soft_tool_calls);
        assert!(broad_budget.output_tokens <= 20_000);
        assert!(broad_budget.prompt_tokens + u64::from(broad_budget.output_tokens) < 131_072);
    }

    #[test]
    fn test_small_declared_context_reserves_output_and_tool_room() {
        let plan = QaPlan::fallback("请详细解释全部章节");
        let budget = QaResources::new(&plan, Some(8192), Some(2048), 1000);
        assert!(
            budget.prompt_tokens + u64::from(budget.output_tokens) + budget.tool_reserve_tokens
                <= 8192
        );
        assert!(budget.output_tokens <= 2048);
        assert!(budget.initial_entry_target < 1000);
    }

    #[test]
    fn test_budget_estimate_is_conservative_for_cjk_and_handles_empty() {
        assert_eq!(estimated_tokens(""), 0);
        assert!(estimated_tokens("测试abcd") >= 5);
    }

    #[test]
    fn test_plan_normalization_does_not_accept_unbounded_model_fields() {
        let mut plan = QaPlan {
            goal: "g".repeat(3000),
            constraints: vec!["c".repeat(1000); 100],
            subquestions: vec!["q".repeat(1000); 100],
            evidence_requirements: vec!["e".repeat(1000); 100],
            depth: "invalid".into(),
            scope: "invalid".into(),
            expected_output_tokens: None,
        };
        plan.normalize("current question");
        assert_eq!(plan.goal.chars().count(), 800);
        assert_eq!(plan.subquestions.len(), 1);
        assert_eq!(plan.depth, "standard");
        assert_eq!(plan.scope, "focused");
        assert!(!plan.subquestions[0].is_empty());
    }
}
