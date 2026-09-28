//! Run-scoped adaptive resource limits, not billing usage or agent orchestration.

use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;

use super::{insert_agent_run_event, BookWikiStore};
use crate::error::BrainError;
use crate::models::agent_budget::context_capacity;

const MANAGEMENT_CALL_LIMIT: u32 = 128;
const EXTENSION_LIMIT: u32 = 3;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AdaptiveBudgetPolicy {
    pub initial_prompt_tokens: u64,
    pub soft_tool_calls: u32,
    pub hard_tool_calls: u32,
    pub soft_retrieval_tokens: u64,
    pub hard_retrieval_tokens: u64,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub timeout_seconds: u32,
    pub subquestions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AdaptiveEvidenceCoverage {
    /// Zero-based; citation_indices are the Run's one-based S numbers.
    pub question_index: usize,
    pub question: String,
    pub status: String,
    pub citation_indices: Vec<usize>,
    /// Agent-reported finding, not an application proof of entailment.
    pub finding: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AdaptiveRunBudget {
    pub run_id: String,
    pub policy: AdaptiveBudgetPolicy,
    pub soft_tool_calls: u32,
    pub soft_retrieval_tokens: u64,
    pub used_tool_calls: u32,
    pub used_management_calls: u32,
    pub management_call_limit: u32,
    pub estimated_tool_payload_tokens: u64,
    pub estimated_data_payload_tokens: u64,
    pub observed_context_tokens: Option<u64>,
    pub observed_context_window: Option<u64>,
    pub context_payload_tokens_at_observation: u64,
    pub extension_count: u32,
    pub last_extension_evidence_segments: u64,
    pub coverage: Vec<AdaptiveEvidenceCoverage>,
    pub deadline: String,
    pub revision: u64,
}

impl BookWikiStore {
    pub fn record_adaptive_limit_event(
        &self,
        run_id: &str,
        tool: &str,
        reason: &str,
    ) -> Result<(), BrainError> {
        self.db.transaction(|conn|{
            ensure_running(conn,run_id)?;
            let count:i64=conn.query_row("SELECT COUNT(*) FROM agent_run_events WHERE run_id=?1 AND event_type='run.budget_limited'",[run_id],|row|row.get(0))?;
            // Bounded diagnostics; repeated rejected requests cannot grow the
            // inspection indefinitely. This is a denial, not an ACP completion.
            if count<64 {
                let reason:String=reason.chars().take(1000).collect();
                insert_agent_run_event(conn,run_id,"run.budget_limited",Some("budget"),&reason,&json!({"tool":tool.chars().take(128).collect::<String>(),"reason":reason}).to_string(),&Utc::now().to_rfc3339())?;
            }
            Ok(())
        })
    }
    pub fn init_adaptive_run_budget(
        &self,
        run_id: &str,
        policy: &AdaptiveBudgetPolicy,
    ) -> Result<AdaptiveRunBudget, BrainError> {
        validate_policy(policy)?;
        let now = Utc::now();
        let state = AdaptiveRunBudget {
            run_id: run_id.into(),
            policy: policy.clone(),
            soft_tool_calls: policy.soft_tool_calls,
            soft_retrieval_tokens: policy.soft_retrieval_tokens,
            used_tool_calls: 0,
            used_management_calls: 0,
            management_call_limit: MANAGEMENT_CALL_LIMIT,
            estimated_tool_payload_tokens: 0,
            estimated_data_payload_tokens: 0,
            observed_context_tokens: None,
            observed_context_window: None,
            context_payload_tokens_at_observation: 0,
            extension_count: 0,
            last_extension_evidence_segments: 0,
            coverage: policy
                .subquestions
                .iter()
                .enumerate()
                .map(|(question_index, question)| AdaptiveEvidenceCoverage {
                    question_index,
                    question: question.clone(),
                    status: "missing".into(),
                    citation_indices: Vec::new(),
                    finding: String::new(),
                })
                .collect(),
            deadline: (now + Duration::seconds(i64::from(policy.timeout_seconds))).to_rfc3339(),
            revision: 1,
        };
        self.db.transaction(|conn| {
            ensure_running(conn, run_id)?;
            if load_budget(conn, run_id)?.is_some() {
                return Err(validation("本次运行已初始化预算，不能重置硬上限或计数"));
            }
            let existing_calls: u32 = conn.query_row("SELECT COALESCE(SUM(used_calls),0) FROM agent_run_capabilities WHERE run_id=?1", [run_id], |row| row.get(0))?;
            if existing_calls != 0 {
                return Err(validation("工具调用开始后不能重置为新预算"));
            }
            conn.execute("INSERT INTO agent_run_adaptive_budgets
                (run_id,policy_json,soft_tool_calls,soft_retrieval_tokens,coverage_json,deadline,created_at,updated_at)
                VALUES(?1,?2,?3,?4,?5,?6,?7,?7)",
                params![run_id,to_json(policy)?,state.soft_tool_calls,state.soft_retrieval_tokens,to_json(&state.coverage)?,state.deadline,now.to_rfc3339()])?;
            // No change to scopes, tools, TTL or credential material.
            conn.execute("UPDATE agent_run_capabilities SET max_calls=?2 WHERE run_id=?1",params![run_id,policy.hard_tool_calls])?;
            insert_agent_run_event(conn,run_id,"run.budget_changed",Some("budget"),"已建立本轮自适应预算",&to_json(&state)?,&now.to_rfc3339())?;
            Ok(state.clone())
        })
    }

    pub fn get_adaptive_run_budget(
        &self,
        run_id: &str,
    ) -> Result<Option<AdaptiveRunBudget>, BrainError> {
        self.get_agent_run(run_id)?;
        self.db.with_connection(|conn| load_budget(conn, run_id))
    }

    pub fn request_adaptive_budget_extension(
        &self,
        run_id: &str,
        reason: &str,
        missing_question_indices: &[usize],
    ) -> Result<AdaptiveRunBudget, BrainError> {
        let reason = reason.trim();
        if reason.is_empty() || reason.chars().count() > 2000 {
            return Err(validation("预算扩展需填写不超过2000字的具体证据缺口"));
        }
        self.db.transaction(|conn| {
            let mut state = require_live_budget(conn,run_id)?;
            if state.extension_count >= EXTENSION_LIMIT { return Err(validation("本次运行已达到预算扩展硬上限")); }
            let mut seen = HashSet::new();
            if missing_question_indices.is_empty() || missing_question_indices.iter().any(|index| {
                !seen.insert(*index) || state.coverage.get(*index).is_none_or(|item| item.status == "supported")
            }) { return Err(validation("扩展必须指向本轮真实且未充分覆盖的子问题")); }
            let evidence_segments = evidence_segment_count(conn,run_id)?;
            if state.extension_count > 0 && evidence_segments <= state.last_extension_evidence_segments {
                return Err(validation("上次扩展后没有新读取证据，停止继续扩大预算"));
            }
            if state.soft_tool_calls >= state.policy.hard_tool_calls && state.soft_retrieval_tokens >= state.policy.hard_retrieval_tokens {
                return Err(validation("软预算已达到本轮硬上限，无法继续扩展"));
            }
            state.soft_tool_calls = state.soft_tool_calls.saturating_mul(2).min(state.policy.hard_tool_calls);
            state.soft_retrieval_tokens = state.soft_retrieval_tokens.saturating_mul(2).min(state.policy.hard_retrieval_tokens);
            state.extension_count += 1;
            state.last_extension_evidence_segments = evidence_segments;
            save_budget(conn,&mut state)?;
            insert_agent_run_event(conn,run_id,"run.budget_changed",Some("budget"),reason,&json!({"reason":reason,"missing_question_indices":missing_question_indices,"budget":state}).to_string(),&Utc::now().to_rfc3339())?;
            Ok(state)
        })
    }

    pub fn record_adaptive_tool_payload(
        &self,
        run_id: &str,
        estimated_tokens: u64,
    ) -> Result<AdaptiveRunBudget, BrainError> {
        self.record_adaptive_payload(run_id, estimated_tokens, false)
    }

    /// Management responses are bounded by total hard payload, not the data
    /// soft limit/output reserve, so a stopped agent can still inspect its gap.
    pub fn record_adaptive_management_payload(
        &self,
        run_id: &str,
        estimated_tokens: u64,
    ) -> Result<AdaptiveRunBudget, BrainError> {
        self.record_adaptive_payload(run_id, estimated_tokens, true)
    }

    fn record_adaptive_payload(
        &self,
        run_id: &str,
        estimated_tokens: u64,
        management: bool,
    ) -> Result<AdaptiveRunBudget, BrainError> {
        self.db.transaction(|conn| {
            let mut state = require_live_budget(conn, run_id)?;
            let total = state
                .estimated_tool_payload_tokens
                .checked_add(estimated_tokens)
                .filter(|value| *value <= state.policy.hard_retrieval_tokens)
                .ok_or_else(|| validation("累计工具负载已达到硬上限，不能返回更多内容"))?;
            if management {
                // Status/extension must remain callable inside output-reserve
                // pressure, but may not overflow the actual model capacity.
                let mut management_state = state.clone();
                management_state.policy.max_output_tokens = None;
                check_context(&management_state, estimated_tokens)?;
            } else {
                let data = state
                    .estimated_data_payload_tokens
                    .checked_add(estimated_tokens)
                    .filter(|value| *value <= state.soft_retrieval_tokens)
                    .ok_or_else(|| validation("检索内容已达到软预算，请报告具体缺口并申请扩展"))?;
                check_context(&state, estimated_tokens)?;
                state.estimated_data_payload_tokens = data;
            }
            state.estimated_tool_payload_tokens = total;
            save_budget(conn, &mut state)?;
            Ok(state)
        })
    }

    pub fn observe_adaptive_context(
        &self,
        run_id: &str,
        used: u64,
        size: u64,
    ) -> Result<AdaptiveRunBudget, BrainError> {
        if size == 0 || size > i64::MAX as u64 || used > size {
            return Err(validation("Runtime上下文占用必须在真实容量范围内"));
        }
        self.db.transaction(|conn| {
            let mut state = require_live_budget(conn, run_id)?;
            state.observed_context_tokens = Some(used);
            state.observed_context_window = Some(size);
            state.context_payload_tokens_at_observation = state.estimated_tool_payload_tokens;
            save_budget(conn, &mut state)?;
            Ok(state)
        })
    }

    pub fn report_adaptive_evidence_coverage(
        &self,
        run_id: &str,
        question_index: usize,
        status: &str,
        citation_indices: &[usize],
        finding: &str,
    ) -> Result<AdaptiveRunBudget, BrainError> {
        if !matches!(status, "supported" | "partial" | "missing" | "conflict")
            || finding.trim().is_empty()
            || finding.chars().count() > 4000
            || citation_indices.len() > 64
        {
            return Err(validation("覆盖报告状态/发现格式不合法"));
        }
        if status == "supported" && citation_indices.is_empty() {
            return Err(validation(
                "supported必须引用本轮已读证据；这是Agent报告而不是事实证明",
            ));
        }
        self.db.transaction(|conn| {
            let mut state = require_live_budget(conn,run_id)?;
            let base_id: String = conn.query_row("SELECT knowledge_base_id FROM agent_runs WHERE id=?1",[run_id],|row|row.get(0))?;
            let mut seen = HashSet::new();
            for index in citation_indices {
                if *index==0 || !seen.insert(*index) || *index>i64::MAX as usize {return Err(validation("引用编号必须是本轮真实的一基编号且不能重复"));}
                let valid: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM agent_run_citations WHERE run_id=?1 AND citation_index=?2 AND json_extract(snapshot_json,'$.entry.knowledge_base_id')=?3)",params![run_id,*index as i64,base_id],|row|row.get(0))?;
                if !valid {return Err(validation("覆盖报告引用未读证据或其他书籍/运行的编号"));}
            }
            let item=state.coverage.get_mut(question_index).ok_or_else(||validation("子问题编号不属于本轮规划"))?;
            item.status=status.into(); item.citation_indices=citation_indices.into(); item.finding=finding.trim().into();
            save_budget(conn,&mut state)?;
            insert_agent_run_event(conn,run_id,"run.evidence_coverage",Some("evidence"),"Agent更新了子问题证据覆盖（非事实验证）",&json!({"agent_reported":true,"coverage":state.coverage[question_index]}).to_string(),&Utc::now().to_rfc3339())?;
            Ok(state)
        })
    }
}

pub(super) fn consume_call_in_transaction(
    conn: &Connection,
    run_id: &str,
    tool: &str,
) -> Result<bool, BrainError> {
    let Some(mut state) = load_budget(conn, run_id)? else {
        return Ok(false);
    };
    check_live(conn, &state)?;
    let management = matches!(
        tool,
        "knowledge_get_run_budget"
            | "knowledge_request_budget_extension"
            | "knowledge_report_evidence_coverage"
    );
    if management {
        if state.used_management_calls >= MANAGEMENT_CALL_LIMIT {
            return Err(validation("预算管理工具已达到独立调用硬上限"));
        }
        if state.estimated_tool_payload_tokens >= state.policy.hard_retrieval_tokens {
            return Err(validation("累计工具负载已达到硬上限"));
        }
        state.used_management_calls += 1;
    } else {
        if state.used_tool_calls >= state.policy.hard_tool_calls {
            return Err(validation("数据工具调用已达到本轮硬上限"));
        }
        if state.used_tool_calls >= state.soft_tool_calls {
            return Err(validation(
                "数据工具调用已达到软预算，请先报告缺口并申请扩展",
            ));
        }
        if state.estimated_data_payload_tokens >= state.soft_retrieval_tokens {
            return Err(validation("检索负载已达到软预算，请先报告缺口并申请扩展"));
        }
        check_context(&state, 0)?;
        state.used_tool_calls += 1;
    }
    save_budget(conn, &mut state)?;
    Ok(true)
}

fn validate_policy(policy: &AdaptiveBudgetPolicy) -> Result<(), BrainError> {
    if policy.soft_tool_calls == 0
        || policy.soft_tool_calls > policy.hard_tool_calls
        || policy.hard_tool_calls > 4096
        || policy.soft_retrieval_tokens == 0
        || policy.soft_retrieval_tokens > policy.hard_retrieval_tokens
        || policy.hard_retrieval_tokens > i64::MAX as u64
        || policy.initial_prompt_tokens > i64::MAX as u64
        || policy.timeout_seconds == 0
        || policy.timeout_seconds > 86400
        || policy.subquestions.is_empty()
        || policy.subquestions.len() > 64
        || policy
            .subquestions
            .iter()
            .any(|q| q.trim().is_empty() || q.chars().count() > 1000)
        || policy.context_window == Some(0)
        || policy.max_output_tokens == Some(0)
    {
        return Err(validation(
            "自适应预算必须有合理正数、软上限不超过硬上限及有效子问题",
        ));
    }
    let initial_capacity = context_capacity(policy.context_window);
    if policy
        .initial_prompt_tokens
        .saturating_add(u64::from(policy.max_output_tokens.unwrap_or(0)))
        >= initial_capacity
    {
        return Err(validation("初始Prompt及输出预留已超过模型上下文容量"));
    }
    Ok(())
}

fn check_context(state: &AdaptiveRunBudget, extra: u64) -> Result<(), BrainError> {
    let capacity = match (
        state.policy.context_window.map(u64::from),
        state.observed_context_window,
    ) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) | (None, Some(a)) => a,
        // An application guard, never a fabricated model capability. A real
        // ACP capacity/occupancy observation replaces the unknown fallback.
        (None, None) => context_capacity(None),
    };
    let usage = state
        .observed_context_tokens
        .map(|used| {
            used.saturating_add(
                state
                    .estimated_tool_payload_tokens
                    .saturating_sub(state.context_payload_tokens_at_observation),
            )
        })
        .unwrap_or_else(|| {
            state
                .policy
                .initial_prompt_tokens
                .saturating_add(state.estimated_tool_payload_tokens)
        });
    if usage
        .saturating_add(extra)
        .saturating_add(u64::from(state.policy.max_output_tokens.unwrap_or(0)))
        >= capacity
    {
        return Err(validation(
            "上下文接近容量，需由Harness压缩或完成当前回答，不能继续扩大上下文",
        ));
    }
    Ok(())
}

fn ensure_running(conn: &Connection, run_id: &str) -> Result<(), BrainError> {
    let status = conn
        .query_row(
            "SELECT status FROM agent_runs WHERE id=?1",
            [run_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| BrainError::KnowledgeNotFound(run_id.into()))?;
    if status != "running" {
        return Err(validation("Agent运行已结束，拒绝修改预算或覆盖"));
    }
    Ok(())
}

fn check_live(conn: &Connection, state: &AdaptiveRunBudget) -> Result<(), BrainError> {
    ensure_running(conn, &state.run_id)?;
    let deadline = DateTime::parse_from_rfc3339(&state.deadline)
        .map_err(|e| BrainError::Internal(format!("预算截止时间损坏:{e}")))?;
    if deadline <= Utc::now() {
        return Err(validation("本次运行已达到总执行时间上限"));
    }
    Ok(())
}

fn require_live_budget(conn: &Connection, run_id: &str) -> Result<AdaptiveRunBudget, BrainError> {
    let state = load_budget(conn, run_id)?.ok_or_else(|| validation("本次运行没有自适应预算"))?;
    check_live(conn, &state)?;
    Ok(state)
}

fn evidence_segment_count(conn: &Connection, run_id: &str) -> Result<u64, BrainError> {
    Ok(conn.query_row("SELECT COALESCE(SUM(json_array_length(snapshot_json,'$.segments')),0) FROM agent_run_citations WHERE run_id=?1",[run_id],|row|row.get(0))?)
}

fn load_budget(conn: &Connection, run_id: &str) -> Result<Option<AdaptiveRunBudget>, BrainError> {
    let values=conn.query_row("SELECT policy_json,soft_tool_calls,soft_retrieval_tokens,used_tool_calls,used_management_calls,
        estimated_tool_payload_tokens,estimated_data_payload_tokens,observed_context_tokens,observed_context_window,
        context_payload_tokens_at_observation,extension_count,last_extension_evidence_segments,coverage_json,deadline,revision
        FROM agent_run_adaptive_budgets WHERE run_id=?1",[run_id],|row| {
            Ok((row.get::<_,String>(0)?,row.get::<_,u32>(1)?,row.get::<_,u64>(2)?,row.get::<_,u32>(3)?,row.get::<_,u32>(4)?,row.get::<_,u64>(5)?,row.get::<_,u64>(6)?,row.get::<_,Option<u64>>(7)?,row.get::<_,Option<u64>>(8)?,row.get::<_,u64>(9)?,row.get::<_,u32>(10)?,row.get::<_,u64>(11)?,row.get::<_,String>(12)?,row.get::<_,String>(13)?,row.get::<_,u64>(14)?))
        }).optional()?;
    values
        .map(
            |(
                policy,
                soft_tool_calls,
                soft_retrieval_tokens,
                used_tool_calls,
                used_management_calls,
                estimated_tool_payload_tokens,
                estimated_data_payload_tokens,
                observed_context_tokens,
                observed_context_window,
                context_payload_tokens_at_observation,
                extension_count,
                last_extension_evidence_segments,
                coverage,
                deadline,
                revision,
            )| {
                Ok(AdaptiveRunBudget {
                    run_id: run_id.into(),
                    policy: serde_json::from_str(&policy)
                        .map_err(|e| BrainError::Internal(format!("预算策略损坏:{e}")))?,
                    soft_tool_calls,
                    soft_retrieval_tokens,
                    used_tool_calls,
                    used_management_calls,
                    management_call_limit: MANAGEMENT_CALL_LIMIT,
                    estimated_tool_payload_tokens,
                    estimated_data_payload_tokens,
                    observed_context_tokens,
                    observed_context_window,
                    context_payload_tokens_at_observation,
                    extension_count,
                    last_extension_evidence_segments,
                    coverage: serde_json::from_str(&coverage)
                        .map_err(|e| BrainError::Internal(format!("证据覆盖损坏:{e}")))?,
                    deadline,
                    revision,
                })
            },
        )
        .transpose()
}

fn save_budget(conn: &Connection, state: &mut AdaptiveRunBudget) -> Result<(), BrainError> {
    state.revision += 1;
    conn.execute("UPDATE agent_run_adaptive_budgets SET soft_tool_calls=?2,soft_retrieval_tokens=?3,used_tool_calls=?4,used_management_calls=?5,
        estimated_tool_payload_tokens=?6,estimated_data_payload_tokens=?7,observed_context_tokens=?8,observed_context_window=?9,
        context_payload_tokens_at_observation=?10,extension_count=?11,last_extension_evidence_segments=?12,coverage_json=?13,revision=?14,updated_at=?15 WHERE run_id=?1",
        params![state.run_id,state.soft_tool_calls,state.soft_retrieval_tokens,state.used_tool_calls,state.used_management_calls,state.estimated_tool_payload_tokens,state.estimated_data_payload_tokens,state.observed_context_tokens,state.observed_context_window,state.context_payload_tokens_at_observation,state.extension_count,state.last_extension_evidence_segments,to_json(&state.coverage)?,state.revision,Utc::now().to_rfc3339()])?;
    Ok(())
}

fn to_json(value: &impl Serialize) -> Result<String, BrainError> {
    serde_json::to_string(value).map_err(|e| BrainError::Internal(format!("预算序列化失败:{e}")))
}
fn validation(message: &str) -> BrainError {
    BrainError::KnowledgeValidation(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::book_wiki_store::BookWikiStore;
    use crate::infra::sqlite_store::SqliteStore;
    use crate::models::book_wiki::ReaderBook;
    use serde_json::json;
    use std::sync::Arc;

    #[test]
    fn test_unknown_context_budget_supports_large_input_but_observed_capacity_is_authoritative() {
        let (store, _dir, _, run) = fixture();
        let mut limits = policy();
        limits.context_window = None;
        limits.initial_prompt_tokens = 100_000;
        limits.max_output_tokens = Some(32_768);
        limits.soft_retrieval_tokens = 100_000;
        limits.hard_retrieval_tokens = 400_000;
        store.init_adaptive_run_budget(&run, &limits).unwrap();
        assert!(store.record_adaptive_tool_payload(&run, 50_000).is_ok());
        store
            .observe_adaptive_context(&run, 60_000, 65_536)
            .unwrap();
        assert!(store.record_adaptive_tool_payload(&run, 100).is_err());
        store.observe_adaptive_context(&run, 1000, 65_536).unwrap();
        assert!(store.record_adaptive_tool_payload(&run, 100).is_ok());
    }

    fn fixture() -> (BookWikiStore, tempfile::TempDir, String, String) {
        let dir = tempfile::tempdir().unwrap();
        let store = BookWikiStore::new(Arc::new(
            SqliteStore::new(&dir.path().join("adaptive.db")).unwrap(),
        ));
        let book: ReaderBook = serde_json::from_value(json!({
            "id":"adaptive-book", "name":"预算测试", "path":"/tmp/adaptive-book",
            "kind":"folder", "description":"", "category":"", "addedAt":123, "progress":null
        }))
        .unwrap();
        store.save_reader_books(&[book]).unwrap();
        let base = store.initialize_base("adaptive-book").unwrap();
        let run = store
            .start_agent_run(&base.id, "deepseek_harness", "knowledge_qa", &json!({}))
            .unwrap();
        (store, dir, base.id, run.id)
    }

    fn policy() -> AdaptiveBudgetPolicy {
        AdaptiveBudgetPolicy {
            initial_prompt_tokens: 100,
            soft_tool_calls: 2,
            hard_tool_calls: 6,
            soft_retrieval_tokens: 200,
            hard_retrieval_tokens: 800,
            context_window: Some(4096),
            max_output_tokens: Some(1024),
            timeout_seconds: 60,
            subquestions: vec!["机制是什么？".into(), "边界是什么？".into()],
        }
    }

    fn token(store: &BookWikiStore, base: &str, run: &str) -> String {
        store
            .issue_agent_run_capability(
                run,
                &[base.into()],
                &[
                    "knowledge_get_entry".into(),
                    "knowledge_get_run_budget".into(),
                    "knowledge_request_budget_extension".into(),
                    "knowledge_report_evidence_coverage".into(),
                ],
                120,
            )
            .unwrap()
            .token
    }

    fn source(store: &BookWikiStore, base: &str) {
        use crate::infra::book_wiki_store::{MarkdownSourceDraft, SourceSectionDraft};
        store
            .sync_markdown_sources(
                base,
                &[MarkdownSourceDraft {
                    id: "budget-source".into(),
                    version_id: "budget-version".into(),
                    original_path: "/tmp/adaptive-book/source.md".into(),
                    relative_path: "source.md".into(),
                    title: "证据".into(),
                    ordinal: 0,
                    content_hash: "budget-content".into(),
                    size_bytes: 10,
                    modified_at: None,
                    sections: vec![SourceSectionDraft {
                        id: "budget-span".into(),
                        entry_id: "budget-entry".into(),
                        slug: "budget-entry".into(),
                        title: "证据".into(),
                        summary: "证据".into(),
                        content_md: "0123456789".into(),
                        line_start: 1,
                        line_end: 1,
                        content_hash: "budget-span-content".into(),
                    }],
                }],
            )
            .unwrap();
    }

    fn read_segment(store: &BookWikiStore, run: &str, offset: usize) -> usize {
        store
            .record_visible_agent_evidence(
                run,
                "source_span",
                "budget-span",
                "budget-version",
                &json!({"offset_chars":offset,"content_md":offset.to_string()}),
            )
            .unwrap()
    }

    #[test]
    fn test_adaptive_budget_soft_stop_and_management_do_not_expand_permissions() {
        let (store, _dir, base, run) = fixture();
        store.init_adaptive_run_budget(&run, &policy()).unwrap();
        let token = token(&store, &base, &run);
        for _ in 0..2 {
            store
                .consume_agent_tool_call(&token, "knowledge_get_entry")
                .unwrap();
        }
        assert!(store
            .consume_agent_tool_call(&token, "knowledge_get_entry")
            .is_err());
        store
            .consume_agent_tool_call(&token, "knowledge_get_run_budget")
            .unwrap();
        assert!(store
            .consume_agent_tool_call(&token, "book_fetch_external")
            .is_err());
        store
            .request_adaptive_budget_extension(&run, "需要核对第二个子问题的条件", &[1])
            .unwrap();
        store
            .consume_agent_tool_call(&token, "knowledge_get_entry")
            .unwrap();
        let state = store.get_adaptive_run_budget(&run).unwrap().unwrap();
        assert_eq!(state.used_tool_calls, 3);
        assert_eq!(state.used_management_calls, 1);
        assert!(state.soft_tool_calls <= state.policy.hard_tool_calls);
        let grant = store.validate_agent_run_token(&token).unwrap();
        assert_eq!(grant.knowledge_base_ids, vec![base]);
        assert!(!grant.allowed_tools.contains(&"book_fetch_external".into()));
    }

    #[test]
    fn test_adaptive_budget_atomic_concurrent_calls_cannot_exceed_soft_limit() {
        let (store, _dir, base, run) = fixture();
        store.init_adaptive_run_budget(&run, &policy()).unwrap();
        let token = token(&store, &base, &run);
        let handles = (0..12)
            .map(|_| {
                let store = store.clone();
                let token = token.clone();
                std::thread::spawn(move || {
                    store
                        .consume_agent_tool_call(&token, "knowledge_get_entry")
                        .is_ok()
                })
            })
            .collect::<Vec<_>>();
        let successes = handles
            .into_iter()
            .map(|handle| usize::from(handle.join().unwrap()))
            .sum::<usize>();
        assert_eq!(successes, 2);
        assert_eq!(
            store
                .get_adaptive_run_budget(&run)
                .unwrap()
                .unwrap()
                .used_tool_calls,
            2
        );
    }

    #[test]
    fn test_adaptive_budget_payload_and_actual_context_enforce_limits() {
        let (store, _dir, base, run) = fixture();
        store.init_adaptive_run_budget(&run, &policy()).unwrap();
        store.record_adaptive_tool_payload(&run, 180).unwrap();
        assert!(store.record_adaptive_tool_payload(&run, 21).is_err());
        assert_eq!(
            store
                .get_adaptive_run_budget(&run)
                .unwrap()
                .unwrap()
                .estimated_tool_payload_tokens,
            180
        );
        store.observe_adaptive_context(&run, 3050, 4096).unwrap();
        assert!(store.record_adaptive_tool_payload(&run, 23).is_err());
        let token = token(&store, &base, &run);
        store
            .consume_agent_tool_call(&token, "knowledge_get_run_budget")
            .unwrap();
        store.record_adaptive_management_payload(&run, 40).unwrap();
        store.observe_adaptive_context(&run, 500, 4096).unwrap();
        store.record_adaptive_tool_payload(&run, 20).unwrap();
        store.observe_adaptive_context(&run, 4090, 4096).unwrap();
        assert!(store.record_adaptive_management_payload(&run, 10).is_err());
        store.observe_adaptive_context(&run, 500, 4096).unwrap();
        assert!(store
            .record_adaptive_management_payload(&run, 1000)
            .is_err());
    }

    #[test]
    fn test_adaptive_budget_extension_requires_gap_and_new_read_progress() {
        let (store, _dir, _base, run) = fixture();
        store.init_adaptive_run_budget(&run, &policy()).unwrap();
        store
            .request_adaptive_budget_extension(&run, "尚未读取边界证据", &[1])
            .unwrap();
        assert!(store
            .request_adaptive_budget_extension(&run, "仍然需要边界证据", &[1])
            .is_err());
        assert!(store
            .request_adaptive_budget_extension(&run, "", &[1])
            .is_err());
        assert!(store
            .request_adaptive_budget_extension(&run, "无效子问题", &[99])
            .is_err());
        assert!(store
            .request_adaptive_budget_extension(&run, "没有说明缺口", &[])
            .is_err());
        assert_eq!(
            store
                .get_adaptive_run_budget(&run)
                .unwrap()
                .unwrap()
                .extension_count,
            1
        );
    }

    #[test]
    fn test_adaptive_budget_rejects_deadline_and_finished_run() {
        let (store, _dir, base, run) = fixture();
        store.init_adaptive_run_budget(&run, &policy()).unwrap();
        let token = token(&store, &base, &run);
        store.db.with_connection(|conn|{conn.execute("UPDATE agent_run_adaptive_budgets SET deadline='2000-01-01T00:00:00Z' WHERE run_id=?1",[&run])?;Ok(())}).unwrap();
        assert!(store
            .consume_agent_tool_call(&token, "knowledge_get_run_budget")
            .is_err());
        assert!(store.record_adaptive_tool_payload(&run, 1).is_err());
        assert!(store
            .request_adaptive_budget_extension(&run, "时间已结束", &[0])
            .is_err());
        store
            .complete_agent_run(&run, &json!({"answer":"结束"}))
            .unwrap();
        assert!(store.observe_adaptive_context(&run, 10, 4096).is_err());
        assert!(store
            .report_adaptive_evidence_coverage(&run, 0, "missing", &[], "缺少来源")
            .is_err());
        assert!(store.get_adaptive_run_budget(&run).unwrap().is_some());
    }

    #[test]
    fn test_adaptive_budget_legacy_run_keeps_capability_call_limit() {
        let (store, _dir, base, run) = fixture();
        assert!(store.get_adaptive_run_budget(&run).unwrap().is_none());
        let token = token(&store, &base, &run);
        for _ in 0..20 {
            store
                .consume_agent_tool_call(&token, "knowledge_get_entry")
                .unwrap();
        }
        assert!(store
            .consume_agent_tool_call(&token, "knowledge_get_entry")
            .is_err());
    }

    #[test]
    fn test_unknown_capacity_uses_application_guard_until_acp_reports_capacity() {
        let (store, _dir, _base, run) = fixture();
        let mut initial = policy();
        initial.context_window = None;
        initial.initial_prompt_tokens = context_capacity(None) - 1200;
        initial.hard_retrieval_tokens = 2000;
        store.init_adaptive_run_budget(&run, &initial).unwrap();
        assert!(store
            .record_adaptive_management_payload(&run, 1300)
            .is_err());
        store.observe_adaptive_context(&run, 30000, 131072).unwrap();
        let state = store
            .record_adaptive_management_payload(&run, 1300)
            .unwrap();
        assert_eq!(state.policy.context_window, None);
        assert_eq!(state.observed_context_window, Some(131072));
    }

    #[test]
    fn test_budget_limit_diagnostics_are_bounded_and_not_completion_events() {
        let (store, _dir, _base, run) = fixture();
        store.init_adaptive_run_budget(&run, &policy()).unwrap();
        for _ in 0..70 {
            store
                .record_adaptive_limit_event(&run, "knowledge_get_entry", "达到软预算")
                .unwrap();
        }
        let events = store.list_agent_run_events(&run).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == "run.budget_limited")
                .count(),
            64
        );
        assert_eq!(store.get_agent_run(&run).unwrap().status, "running");
        store.fail_agent_run(&run, "max_tokens").unwrap();
        assert!(store
            .record_adaptive_limit_event(&run, "knowledge_get_entry", "不能覆盖已结束记录")
            .is_err());
    }

    #[test]
    fn test_adaptive_budget_extension_ceiling_and_duplicate_reads_not_progress() {
        let (store, _dir, base, run) = fixture();
        source(&store, &base);
        let mut policy = policy();
        policy.hard_tool_calls = 20;
        policy.hard_retrieval_tokens = 2000;
        store.init_adaptive_run_budget(&run, &policy).unwrap();
        store
            .request_adaptive_budget_extension(&run, "缺少机制原文", &[0])
            .unwrap();
        read_segment(&store, &run, 0);
        store
            .request_adaptive_budget_extension(&run, "缺少条件原文", &[1])
            .unwrap();
        read_segment(&store, &run, 0);
        assert!(store
            .request_adaptive_budget_extension(&run, "重复读取不算进展", &[1])
            .is_err());
        read_segment(&store, &run, 1);
        let third = store
            .request_adaptive_budget_extension(&run, "补查边界", &[1])
            .unwrap();
        assert_eq!(third.extension_count, 3);
        assert!(third.soft_tool_calls <= policy.hard_tool_calls);
        assert!(third.soft_retrieval_tokens <= policy.hard_retrieval_tokens);
        read_segment(&store, &run, 2);
        assert!(store
            .request_adaptive_budget_extension(&run, "不能无限扩展", &[1])
            .is_err());
    }

    #[test]
    fn test_adaptive_budget_coverage_rejects_unread_cross_scope_and_supported_extensions() {
        let (store, _dir, base, run) = fixture();
        source(&store, &base);
        store.init_adaptive_run_budget(&run, &policy()).unwrap();
        assert!(store
            .report_adaptive_evidence_coverage(&run, 0, "supported", &[], "无引用")
            .is_err());
        assert!(store
            .report_adaptive_evidence_coverage(&run, 0, "supported", &[1], "未读引用")
            .is_err());
        let index = read_segment(&store, &run, 0);
        let state = store
            .report_adaptive_evidence_coverage(&run, 0, "supported", &[index], "机制已有直接来源")
            .unwrap();
        assert_eq!(state.coverage[0].citation_indices, vec![1]);
        assert!(store
            .request_adaptive_budget_extension(&run, "不能对充分支持问题扩展", &[0])
            .is_err());
        assert!(store
            .report_adaptive_evidence_coverage(&run, 2, "missing", &[], "无效编号")
            .is_err());
        assert!(store
            .report_adaptive_evidence_coverage(&run, 1, "proven", &[1], "不支持的验证状态")
            .is_err());
        store.db.with_connection(|conn| {
            conn.execute("INSERT INTO agent_run_citations(run_id,citation_index,kind,object_id,version_id,snapshot_json,created_at)
                VALUES(?1,2,'entry','cross-book','1',?2,CURRENT_TIMESTAMP)",params![run,json!({"entry":{"knowledge_base_id":"other-book"},"segments":[]}).to_string()])?;
            Ok(())
        }).unwrap();
        assert!(store
            .report_adaptive_evidence_coverage(&run, 1, "supported", &[2], "跨书引用拒绝")
            .is_err());
        let events = store.list_agent_run_events(&run).unwrap();
        assert!(events
            .iter()
            .any(|event| event.event_type == "run.evidence_coverage"));
        let report = events
            .iter()
            .find(|event| event.event_type == "run.evidence_coverage")
            .unwrap();
        assert_eq!(report.payload["agent_reported"], true);
    }

    #[test]
    fn test_adaptive_budget_management_hard_limit_and_expiry_cannot_be_bypassed() {
        let (store, _dir, base, run) = fixture();
        store.init_adaptive_run_budget(&run, &policy()).unwrap();
        let token = token(&store, &base, &run);
        for _ in 0..128 {
            store
                .consume_agent_tool_call(&token, "knowledge_get_run_budget")
                .unwrap();
        }
        assert!(store
            .consume_agent_tool_call(&token, "knowledge_get_run_budget")
            .is_err());
        assert_eq!(
            store
                .get_adaptive_run_budget(&run)
                .unwrap()
                .unwrap()
                .used_tool_calls,
            0
        );
        store.db.with_connection(|conn|{conn.execute("UPDATE agent_run_capabilities SET expires_at='2000-01-01T00:00:00Z' WHERE run_id=?1",[&run])?;Ok(())}).unwrap();
        assert!(store
            .consume_agent_tool_call(&token, "knowledge_get_entry")
            .is_err());
    }

    #[test]
    fn test_adaptive_budget_invalid_policy_and_reinitialize_cannot_reset_limits() {
        let (store, _dir, _base, run) = fixture();
        let mut cases = vec![policy(); 8];
        cases[0].soft_tool_calls = 7;
        cases[1].soft_retrieval_tokens = 801;
        cases[2].timeout_seconds = 0;
        cases[3].subquestions.clear();
        cases[4].initial_prompt_tokens = 4000;
        cases[5].max_output_tokens = Some(0);
        cases[6].context_window = Some(0);
        cases[7].hard_retrieval_tokens = u64::MAX;
        for invalid in cases {
            assert!(store.init_adaptive_run_budget(&run, &invalid).is_err());
        }
        store.init_adaptive_run_budget(&run, &policy()).unwrap();
        store.record_adaptive_tool_payload(&run, 50).unwrap();
        assert!(store.init_adaptive_run_budget(&run, &policy()).is_err());
        assert_eq!(
            store
                .get_adaptive_run_budget(&run)
                .unwrap()
                .unwrap()
                .estimated_tool_payload_tokens,
            50
        );
    }
}
