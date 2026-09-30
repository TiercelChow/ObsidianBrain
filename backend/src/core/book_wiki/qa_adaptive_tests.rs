use super::*;
use crate::infra::sqlite_store::SqliteStore;
use crate::models::book_wiki::{BookKind, ReaderBook};
use async_trait::async_trait;
use std::sync::Mutex;

struct AdaptiveFixtureRuntime {
    requests: Arc<Mutex<Vec<(String, Option<u32>)>>>,
}

#[async_trait]
impl AgentRuntime for AdaptiveFixtureRuntime {
    async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
        self.requests
            .lock()
            .unwrap()
            .push((request.prompt.clone(), None));
        if request.prompt.contains("只读检索规划器") {
            let second = request.prompt.contains("<current_question>\n换个主题");
            let candidates = (0..14)
                .map(|i| format!("compiled-{i}"))
                .filter(|id| request.prompt.contains(&format!("\"id\":\"{id}\"")))
                .collect::<Vec<_>>();
            return Ok(serde_json::json!({"standalone_question":if second {"新的主题"} else {"全书机制与边界"},"candidate_ids":candidates,"answer_mode":"book_lookup",
                "plan":{"goal":if second {"新的主题"} else {"全书机制与边界"},"constraints":if second {vec![]} else {vec!["以中文解释，先给结论"]},"subquestions":["机制是什么","边界是什么","公式与步骤是什么"],"depth":"comprehensive","scope":"whole_book"},
                "memory_update":{"objective":if second {"新的主题"} else {"全书机制与边界"},"constraints":if second {vec![]} else {vec!["以中文解释，先给结论"]},"unresolved_questions":[],"entity_ids":[]}
            }).to_string());
        }
        Ok(if request.prompt.contains("<direct_reply>") {
            "不客气"
        } else {
            "本轮综合结论。[S1]"
        }
        .into())
    }
}

fn fixture() -> (BookWikiStore, tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(SqliteStore::new(&dir.path().join("qa-adaptive.db")).unwrap());
    let store = BookWikiStore::new(db.clone());
    store
        .save_reader_books(&[ReaderBook {
            id: "adaptive-qa-book".into(),
            path: dir.path().display().to_string(),
            kind: BookKind::Folder,
            name: "问答预算书".into(),
            description: String::new(),
            category: String::new(),
            added_at: 1,
            progress: None,
        }])
        .unwrap();
    let base = store.initialize_base("adaptive-qa-book").unwrap();
    db.with_connection(|conn|{
        for i in 0..14 {
            conn.execute("INSERT INTO knowledge_entries (id,knowledge_base_id,entry_type,slug,title,summary,content_md,status,created_at,updated_at) VALUES (?1,?2,'concept',?3,?4,'概述','完整的机制、条件和公式。','draft',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)",rusqlite::params![format!("compiled-{i}"),base.id,format!("mechanism-{i}"),format!("编译机制{i}")])?;
        }
        Ok(())
    }).unwrap();
    (store, dir, base.id)
}

#[tokio::test]
async fn test_qa_declared_small_output_defers_oversized_plan_before_answer_run() {
    use crate::infra::credential_store::{
        tests::MemoryProviderCredentialStore, ProviderCredentialStore,
    };

    struct OversizedPlanRuntime(Arc<Mutex<Vec<String>>>);
    #[async_trait]
    impl AgentRuntime for OversizedPlanRuntime {
        async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
            self.0.lock().unwrap().push(request.prompt.clone());
            if request.prompt.contains("只读检索规划器") {
                return Ok(serde_json::json!({
                    "standalone_question":"全面分析全书机制和适用条件",
                    "candidate_ids":["compiled-0"],
                    "answer_mode":"book_lookup",
                    "plan":{
                        "goal":"全面分析全书机制和适用条件",
                        "constraints":[],
                        "subquestions":["机制","证据","边界"],
                        "evidence_requirements":["编译知识和原文"],
                        "depth":"comprehensive",
                        "scope":"whole_book",
                        "expected_output_tokens":50_000
                    },
                    "memory_update":{"objective":"全面分析全书机制和适用条件","constraints":[],"unresolved_questions":[],"entity_ids":[]}
                })
                .to_string());
            }
            panic!("infeasible answer must not start a paid answer run");
        }
    }

    let (store, _dir, base) = fixture();
    let provider = store
        .save_model_provider_profile(
            "small-qa-output",
            "问答输出容量测试",
            "openai-completions",
            "https://example.com/v1",
            "qa-cap-test",
            "keychain",
            "",
            true,
            true,
            Some(1_048_576),
            Some(4_096),
            "auto",
            0,
        )
        .unwrap();
    let profile = store.list_runtime_profiles().unwrap().remove(0);
    store
        .save_runtime_profile(
            &profile.id,
            &profile.executable,
            &provider.model,
            Some(&provider.provider_id),
            true,
            profile.revision,
        )
        .unwrap();
    let credentials = Arc::new(MemoryProviderCredentialStore::default());
    credentials
        .set(&provider.provider_id, "fixture-only")
        .unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let service = BookWikiService::new(
        store.clone(),
        Arc::new(OversizedPlanRuntime(requests.clone())),
    )
    .with_credential_store(credentials);
    let (events, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let error = service
        .ask_streaming(&base, "全面分析全书机制和适用条件", None, events)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("qa_output_scope_limit"));
    assert_eq!(requests.lock().unwrap().len(), 1);
    let emitted = std::iter::from_fn(|| receiver.try_recv().ok()).collect::<Vec<_>>();
    assert!(emitted.iter().any(|event| matches!(
        event,
        KnowledgeChatStreamEvent::PlanningReady { knowledge_base_id, question, standalone_question }
            if knowledge_base_id == &base
                && question == "全面分析全书机制和适用条件"
                && standalone_question == "全面分析全书机制和适用条件"
    )));
}

#[tokio::test]
async fn test_adaptive_qa_reads_more_than_old_top_seven_and_remembers_only_current_intent() {
    let (store, _dir, base) = fixture();
    let requests = Arc::new(Mutex::new(vec![]));
    let service = BookWikiService::new(
        store.clone(),
        Arc::new(AdaptiveFixtureRuntime {
            requests: requests.clone(),
        }),
    );
    let result = service
        .ask(&base, "全面分析全书的机制、公式和边界", None)
        .await
        .unwrap();
    assert_eq!(result.evidence.len(), 14);
    assert_eq!(
        store
            .list_agent_run_citations(&result.run_id)
            .unwrap()
            .len(),
        14
    );
    let run = store.get_agent_run(&result.run_id).unwrap();
    assert!(run.input["request_max_output_tokens"].as_u64().unwrap() > 4096);
    assert!(
        run.input["adaptive_budget"]["hard_tool_calls"]
            .as_u64()
            .unwrap()
            > 20
    );
    let snapshot = store
        .get_agent_run_inspection(&result.run_id)
        .unwrap()
        .snapshot
        .unwrap();
    assert!(!snapshot.prompt_text.contains("compiled_knowledge_catalog"));
    assert_eq!(snapshot.prompt_text.matches("<evidence>").count(), 1);
    assert!(snapshot.prompt_text.contains("[S14]"));
    assert!(snapshot.evidence_refs["adaptive_state"].is_object());
    let memory = store
        .get_conversation_memory(&base, &result.conversation_id)
        .unwrap();
    assert_eq!(memory.constraints, vec!["以中文解释，先给结论"]);
    service
        .ask(
            &base,
            "换个主题，取消刚才的格式要求",
            Some(&result.conversation_id),
        )
        .await
        .unwrap();
    let updated = store
        .get_conversation_memory(&base, &result.conversation_id)
        .unwrap();
    assert_eq!(updated.objective, "新的主题");
    assert!(updated.constraints.is_empty());
    assert_eq!(updated.revision, 2);
    let calls = requests.lock().unwrap();
    assert_eq!(calls.len(), 4);
    assert!(!calls[3].0.contains("以中文解释，先给结论"));
    assert!(!calls[3].0.contains("本轮综合结论"));
}

#[test]
fn test_adaptive_planner_has_one_complete_output_contract() {
    let prompt = build_adaptive_selection_prompt(
        "测试书",
        "比较机制",
        &[],
        &ConversationMemory::default(),
        &[],
        None,
    );
    assert!(prompt.contains("\"plan\":{\"goal\":"));
    assert!(prompt.contains("\"memory_update\":{\"objective\":"));
    assert!(prompt.contains("\"expected_output_tokens\":4096"));
}

#[test]
fn test_token_catalog_paging_and_selection_preserve_all_candidates() {
    let catalog = (0..1000)
        .map(|i| QaCatalogEntry {
            id: format!("candidate-{i}"),
            title: format!("主题{i}"),
            aliases: vec![],
            summary: "长概述".repeat(100),
            status: "draft".into(),
            entry_type: "concept".into(),
        })
        .collect::<Vec<_>>();
    let chunks = qa_catalog_chunks(&catalog, 8192, 2000);
    assert!(chunks.len() > 1);
    assert_eq!(chunks.iter().map(Vec::len).sum::<usize>(), 1000);
    for chunk in &chunks {
        assert!(
            chunk
                .iter()
                .map(|e| estimated_tokens(&qa_catalog_row(e)) + 1)
                .sum::<u64>()
                + 2000
                <= 8192
        );
    }
    let raw=serde_json::json!({"standalone_question":"全书","candidate_ids":catalog.iter().take(60).map(|e|e.id.clone()).collect::<Vec<_>>()}).to_string();
    assert_eq!(
        parse_qa_selection(&raw, &catalog, "全书")
            .candidate_ids
            .len(),
        60
    );
}

#[tokio::test]
async fn test_conversational_thanks_does_not_replace_the_active_goal() {
    let (store, _dir, base) = fixture();
    let requests = Arc::new(Mutex::new(vec![]));
    let service =
        BookWikiService::new(store.clone(), Arc::new(AdaptiveFixtureRuntime { requests }));
    let answer = service.ask(&base, "全面分析全书机制", None).await.unwrap();
    let before = store
        .get_conversation_memory(&base, &answer.conversation_id)
        .unwrap();
    service
        .ask(&base, "谢谢", Some(&answer.conversation_id))
        .await
        .unwrap();
    let after = store
        .get_conversation_memory(&base, &answer.conversation_id)
        .unwrap();
    assert_eq!(before.objective, after.objective);
    assert_eq!(before.constraints, after.constraints);
    assert_eq!(after.revision, before.revision + 1);
}

#[tokio::test]
async fn test_simple_greeting_uses_one_runtime_and_no_catalog_or_evidence() {
    let (store, _dir, base) = fixture();
    let requests = Arc::new(Mutex::new(vec![]));
    struct GreetingRuntime(Arc<Mutex<Vec<String>>>);
    #[async_trait]
    impl AgentRuntime for GreetingRuntime {
        async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
            self.0.lock().unwrap().push(request.prompt);
            Ok("你好，有什么关于这本书的问题？".into())
        }
    }
    let service = BookWikiService::new(store.clone(), Arc::new(GreetingRuntime(requests.clone())));
    let answer = service.ask(&base, "你好", None).await.unwrap();
    assert!(answer.evidence.is_empty());
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert!(store
        .get_agent_run_inspection(&answer.run_id)
        .unwrap()
        .snapshot
        .unwrap()
        .tool_names
        .is_empty());
}

#[test]
fn test_resume_requires_same_book_question_conversation_and_explicit_truncation() {
    let (store, _dir, base) = fixture();
    let run=store.start_agent_run(&base,"deepseek_harness","knowledge_qa",&serde_json::json!({"question":"原问题","conversation_id":null,"request_max_output_tokens":4096})).unwrap();
    store
        .append_agent_run_event(
            &run.id,
            "run.text_delta",
            Some("answer"),
            "",
            &serde_json::json!({"delta":"旧草稿 [S42] 和公式 $S_1$"}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &run.id,
            "run.runtime_completed",
            Some("completion"),
            "",
            &serde_json::json!({"stop_reason":"max_tokens","complete":false}),
        )
        .unwrap();
    store.fail_agent_run(&run.id, "max_tokens").unwrap();
    let (draft, output) = validated_qa_resume(&store, &base, "原问题", None, &run.id).unwrap();
    assert!(!draft.contains("[S42]"));
    assert!(draft.contains("$S_1$"));
    assert_eq!(output, 4096);
    assert!(validated_qa_resume(&store, &base, "其他问题", None, &run.id).is_err());
    assert!(validated_qa_resume(&store, "other-book", "原问题", None, &run.id).is_err());
    assert!(
        validated_qa_resume(&store, &base, "原问题", Some("other-conversation"), &run.id).is_err()
    );
    let refusal = store
        .start_agent_run(
            &base,
            "deepseek_harness",
            "knowledge_qa",
            &serde_json::json!({"question":"原问题"}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &refusal.id,
            "run.text_delta",
            Some("answer"),
            "",
            &serde_json::json!({"delta":"partial"}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &refusal.id,
            "run.runtime_completed",
            Some("completion"),
            "",
            &serde_json::json!({"stop_reason":"refusal","complete":false}),
        )
        .unwrap();
    store.fail_agent_run(&refusal.id, "refusal").unwrap();
    assert!(validated_qa_resume(&store, &base, "原问题", None, &refusal.id).is_err());
}

#[tokio::test]
async fn test_qa_resume_at_output_hard_limit_rejects_before_paid_planning() {
    let (store, _dir, base) = fixture();
    let question = "全面分析全书机制";
    let run = store
        .start_agent_run(
            &base,
            "deepseek_harness",
            "knowledge_qa",
            &serde_json::json!({"question": question, "conversation_id": null, "request_max_output_tokens": crate::models::agent_budget::MAX_AGENT_OUTPUT_TOKENS}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &run.id,
            "run.text_delta",
            Some("answer"),
            "",
            &serde_json::json!({"delta": "未完成的正文"}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &run.id,
            "run.runtime_completed",
            Some("completion"),
            "",
            &serde_json::json!({"stop_reason": "max_tokens", "complete": false}),
        )
        .unwrap();
    store.fail_agent_run(&run.id, "max_tokens").unwrap();
    let requests = Arc::new(Mutex::new(vec![]));
    let service = BookWikiService::new(
        store,
        Arc::new(AdaptiveFixtureRuntime {
            requests: requests.clone(),
        }),
    );
    let (events, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let error = service
        .resume_qa_streaming(&base, question, None, &run.id, events)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("qa_output_hard_limit"));
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn test_qa_turn_limit_cannot_use_output_budget_resume() {
    let (store, _dir, base) = fixture();
    let question = "全面分析全书机制";
    let run = store
        .start_agent_run(
            &base,
            "deepseek_harness",
            "knowledge_qa",
            &serde_json::json!({"question": question, "conversation_id": null, "request_max_output_tokens": 8192}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &run.id,
            "run.text_delta",
            Some("answer"),
            "",
            &serde_json::json!({"delta": "未完成的正文"}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &run.id,
            "run.runtime_completed",
            Some("completion"),
            "",
            &serde_json::json!({"stop_reason": "max_turn_requests", "complete": false}),
        )
        .unwrap();
    store.fail_agent_run(&run.id, "max_turn_requests").unwrap();
    let requests = Arc::new(Mutex::new(vec![]));
    let service = BookWikiService::new(
        store,
        Arc::new(AdaptiveFixtureRuntime {
            requests: requests.clone(),
        }),
    );
    let (events, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let error = service
        .resume_qa_streaming(&base, question, None, &run.id, events)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("qa_turn_limit"));
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn test_qa_turn_limit_stops_after_one_answer_run_without_output_expansion() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct TurnLimitRuntime(AtomicUsize);
    #[async_trait]
    impl AgentRuntime for TurnLimitRuntime {
        async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            if request.prompt.contains("只读检索规划器") {
                return Ok(serde_json::json!({
                    "standalone_question":"全面分析全书机制",
                    "candidate_ids":[],
                    "answer_mode":"book_lookup",
                    "plan":{"goal":"全面分析全书机制","constraints":[],"subquestions":["机制"],"depth":"comprehensive","scope":"whole_book"}
                }).to_string());
            }
            Err(BrainError::LlmApiError {
                provider: "deepseek_harness".into(),
                detail: "DeepSeek Harness 达到请求轮次上限 (stop_reason=max_turn_requests)".into(),
            })
        }
    }
    let (store, _dir, base) = fixture();
    let runtime = Arc::new(TurnLimitRuntime(AtomicUsize::new(0)));
    let service = BookWikiService::new(store.clone(), runtime.clone());
    let error = service
        .ask(&base, "全面分析全书机制", None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("qa_turn_limit"));
    assert_eq!(runtime.0.load(Ordering::SeqCst), 2);
    assert!(store.list_conversations(&base, 10).unwrap().is_empty());
    let unfinished = store.list_unfinished_qa_runs(&base, 10).unwrap();
    assert_eq!(unfinished.len(), 1);
    assert_eq!(
        unfinished[0].stop_reason.as_deref(),
        Some("max_turn_requests")
    );
    assert!(!unfinished[0].has_partial_answer);
}

#[tokio::test]
async fn test_qa_resume_uses_previous_run_observed_context_as_hard_limit() {
    let (store, _dir, base) = fixture();
    let question = "全面分析全书机制";
    let profile = store.list_runtime_profiles().unwrap().remove(0);
    let run = store
        .start_agent_run(
            &base,
            "deepseek_harness",
            "knowledge_qa",
            &serde_json::json!({"question": question, "conversation_id": null, "request_max_output_tokens": 8192, "model": profile.model, "provider_id": profile.provider_id}),
        )
        .unwrap();
    store
        .init_adaptive_run_budget(
            &run.id,
            &AdaptiveBudgetPolicy {
                initial_prompt_tokens: 1000,
                soft_tool_calls: 1,
                hard_tool_calls: 2,
                soft_retrieval_tokens: 1000,
                hard_retrieval_tokens: 2000,
                context_window: None,
                max_output_tokens: Some(8192),
                timeout_seconds: 60,
                subquestions: vec![question.into()],
            },
        )
        .unwrap();
    store
        .observe_adaptive_context(&run.id, 4000, 16_384)
        .unwrap();
    store
        .append_agent_run_event(
            &run.id,
            "run.text_delta",
            Some("answer"),
            "",
            &serde_json::json!({"delta": "未完成的正文"}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &run.id,
            "run.runtime_completed",
            Some("completion"),
            "",
            &serde_json::json!({"stop_reason": "max_tokens", "complete": false}),
        )
        .unwrap();
    store.fail_agent_run(&run.id, "max_tokens").unwrap();
    let requests = Arc::new(Mutex::new(vec![]));
    let service = BookWikiService::new(
        store,
        Arc::new(AdaptiveFixtureRuntime {
            requests: requests.clone(),
        }),
    );
    let (events, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let error = service
        .resume_qa_streaming(&base, question, None, &run.id, events)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("qa_output_hard_limit"));
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn test_qa_resume_after_model_change_does_not_reuse_old_observed_context() {
    let (store, _dir, base) = fixture();
    let question = "全面分析全书机制";
    let profile = store.list_runtime_profiles().unwrap().remove(0);
    let run = store
        .start_agent_run(
            &base,
            "deepseek_harness",
            "knowledge_qa",
            &serde_json::json!({"question": question, "conversation_id": null, "request_max_output_tokens": 8192, "model": profile.model, "provider_id": profile.provider_id}),
        )
        .unwrap();
    store
        .init_adaptive_run_budget(
            &run.id,
            &AdaptiveBudgetPolicy {
                initial_prompt_tokens: 1000,
                soft_tool_calls: 1,
                hard_tool_calls: 2,
                soft_retrieval_tokens: 1000,
                hard_retrieval_tokens: 2000,
                context_window: None,
                max_output_tokens: Some(8192),
                timeout_seconds: 60,
                subquestions: vec![question.into()],
            },
        )
        .unwrap();
    store
        .observe_adaptive_context(&run.id, 4000, 16_384)
        .unwrap();
    store
        .append_agent_run_event(
            &run.id,
            "run.text_delta",
            Some("answer"),
            "",
            &serde_json::json!({"delta": "未完成的正文"}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &run.id,
            "run.runtime_completed",
            Some("completion"),
            "",
            &serde_json::json!({"stop_reason": "max_tokens", "complete": false}),
        )
        .unwrap();
    store.fail_agent_run(&run.id, "max_tokens").unwrap();
    store
        .save_runtime_profile(
            &profile.id,
            &profile.executable,
            "larger-context-model",
            profile.provider_id.as_deref(),
            true,
            profile.revision,
        )
        .unwrap();

    let requests = Arc::new(Mutex::new(vec![]));
    let service = BookWikiService::new(
        store.clone(),
        Arc::new(AdaptiveFixtureRuntime {
            requests: requests.clone(),
        }),
    );
    let (events, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let answer = service
        .resume_qa_streaming(&base, question, None, &run.id, events)
        .await
        .unwrap();
    assert_eq!(answer.answer, "本轮综合结论。[S1]");
    let resumed = store.get_agent_run(&answer.run_id).unwrap();
    assert_eq!(resumed.input["resume_run_id"], run.id);
    assert_eq!(resumed.input["model"], "larger-context-model");
    assert!(resumed.input["request_max_output_tokens"].as_u64().unwrap() > 8192);
    assert!(!requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn test_qa_empty_max_tokens_can_resume_with_more_output_without_a_fake_draft() {
    let (store, _dir, base) = fixture();
    let question = "全面分析全书机制";
    let run = store
        .start_agent_run(
            &base,
            "deepseek_harness",
            "knowledge_qa",
            &serde_json::json!({"question":question,"conversation_id":null,"request_max_output_tokens":8192}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &run.id,
            "run.runtime_completed",
            Some("completion"),
            "",
            &serde_json::json!({"stop_reason":"max_tokens","complete":false}),
        )
        .unwrap();
    store
        .fail_agent_run(&run.id, "stop_reason=max_tokens")
        .unwrap();
    let (draft, previous_output) =
        validated_qa_resume(&store, &base, question, None, &run.id).unwrap();
    assert!(draft.is_empty());
    assert_eq!(previous_output, 8192);

    let requests = Arc::new(Mutex::new(vec![]));
    let service = BookWikiService::new(
        store.clone(),
        Arc::new(AdaptiveFixtureRuntime {
            requests: requests.clone(),
        }),
    );
    let (events, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let answer = service
        .resume_qa_streaming(&base, question, None, &run.id, events)
        .await
        .unwrap();
    let resumed = store.get_agent_run(&answer.run_id).unwrap();
    assert_eq!(resumed.input["resume_run_id"], run.id);
    assert!(resumed.input["request_max_output_tokens"].as_u64().unwrap() > 8192);
    assert!(requests
        .lock()
        .unwrap()
        .iter()
        .all(|call| !call.0.contains("<incomplete_draft>")));
}

#[test]
fn test_unfinished_qa_runs_remain_discoverable_until_a_resume_completes() {
    let (store, _dir, base) = fixture();
    let failed = store
        .start_agent_run(
            &base,
            "deepseek_harness",
            "knowledge_qa",
            &serde_json::json!({"question":"未完成的问题","conversation_id":null}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &failed.id,
            "run.text_delta",
            Some("answer"),
            "",
            &serde_json::json!({"delta":"未完成草稿 [S1]"}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &failed.id,
            "run.runtime_completed",
            Some("completion"),
            "",
            &serde_json::json!({"stop_reason":"max_tokens","complete":false}),
        )
        .unwrap();
    store
        .fail_agent_run(&failed.id, "stop_reason=max_tokens")
        .unwrap();
    let pending = store.list_unfinished_qa_runs(&base, 10).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].run_id, failed.id);
    assert_eq!(pending[0].question, "未完成的问题");
    assert_eq!(pending[0].stop_reason.as_deref(), Some("max_tokens"));
    assert!(pending[0].has_partial_answer);
    assert!(store.list_conversations(&base, 10).unwrap().is_empty());

    let retried = store
        .start_agent_run(
            &base,
            "deepseek_harness",
            "knowledge_qa",
            &serde_json::json!({"question":"未完成的问题","conversation_id":null,"qa_retry_parent_run_id":failed.id}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &retried.id,
            "run.text_delta",
            Some("answer"),
            "",
            &serde_json::json!({"delta":"第二版仍未完成"}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &retried.id,
            "run.runtime_completed",
            Some("completion"),
            "",
            &serde_json::json!({"stop_reason":"max_tokens","complete":false}),
        )
        .unwrap();
    store
        .fail_agent_run(&retried.id, "stop_reason=max_tokens")
        .unwrap();
    let pending = store.list_unfinished_qa_runs(&base, 10).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].run_id, retried.id);

    let completed = store
        .start_agent_run(
            &base,
            "deepseek_harness",
            "knowledge_qa",
            &serde_json::json!({"question":"未完成的问题","conversation_id":null,"resume_run_id":retried.id}),
        )
        .unwrap();
    store
        .complete_agent_run(&completed.id, &serde_json::json!({"answer":"完整答案"}))
        .unwrap();
    assert_eq!(store.list_unfinished_qa_runs(&base, 10).unwrap().len(), 1);
    store
        .save_conversation_exchange(&base, None, "未完成的问题", "完整答案", &completed.id, &[])
        .unwrap();
    assert!(store.list_unfinished_qa_runs(&base, 10).unwrap().is_empty());
}

#[test]
fn test_unfinished_qa_run_retains_earlier_draft_when_newer_retry_has_no_body() {
    let (store, _dir, base) = fixture();
    let first = store
        .start_agent_run(
            &base,
            "deepseek_harness",
            "knowledge_qa",
            &serde_json::json!({"question":"复杂问题","conversation_id":null}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &first.id,
            "run.text_delta",
            Some("answer"),
            "",
            &serde_json::json!({"delta":"可恢复的早期草稿"}),
        )
        .unwrap();
    store
        .fail_agent_run(&first.id, "stop_reason=max_tokens")
        .unwrap();

    let retry = store
        .start_agent_run(
            &base,
            "deepseek_harness",
            "knowledge_qa",
            &serde_json::json!({"question":"复杂问题","conversation_id":null,"qa_retry_parent_run_id":first.id}),
        )
        .unwrap();
    store
        .append_agent_run_event(
            &retry.id,
            "run.runtime_completed",
            Some("completion"),
            "",
            &serde_json::json!({"stop_reason":"max_tokens","complete":false}),
        )
        .unwrap();
    store
        .fail_agent_run(&retry.id, "stop_reason=max_tokens")
        .unwrap();

    let pending = store.list_unfinished_qa_runs(&base, 10).unwrap();
    assert_eq!(pending.len(), 2);
    assert!(pending
        .iter()
        .any(|run| run.run_id == first.id && run.has_partial_answer));
    assert!(pending
        .iter()
        .any(|run| run.run_id == retry.id && !run.has_partial_answer));
}

#[tokio::test]
async fn test_truncated_answer_automatically_retries_with_fresh_citations() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Runtime {
        inner: AdaptiveFixtureRuntime,
        answers: AtomicUsize,
    }
    #[async_trait]
    impl AgentRuntime for Runtime {
        async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
            self.inner.prompt(request).await
        }
        async fn prompt_with_events(
            &self,
            request: AgentPromptRequest,
            events: Option<tokio::sync::mpsc::UnboundedSender<AgentRuntimeEvent>>,
            _cancel: tokio::sync::watch::Receiver<bool>,
        ) -> Result<String, BrainError> {
            if !request.prompt.contains("只读检索规划器")
                && self.answers.fetch_add(1, Ordering::SeqCst) == 0
            {
                self.inner
                    .requests
                    .lock()
                    .unwrap()
                    .push((request.prompt, None));
                if let Some(events) = events {
                    events
                        .send(AgentRuntimeEvent::TextDelta {
                            delta: "第一段尚未完成 [S99]".into(),
                        })
                        .unwrap();
                    events
                        .send(AgentRuntimeEvent::Completed {
                            stop_reason: "max_tokens".into(),
                            complete: false,
                        })
                        .unwrap();
                }
                return Err(BrainError::LlmApiError {
                    provider: "deepseek_harness".into(),
                    detail: "DeepSeek Harness 达到输出 token 上限，已生成正文保留为部分结果 (stop_reason=max_tokens)".into(),
                });
            }
            self.inner.prompt(request).await
        }
    }
    let (store, _dir, base) = fixture();
    let requests = Arc::new(Mutex::new(vec![]));
    let runtime = Arc::new(Runtime {
        inner: AdaptiveFixtureRuntime {
            requests: requests.clone(),
        },
        answers: AtomicUsize::new(0),
    });
    let service = BookWikiService::new(store.clone(), runtime);
    let (events, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let question = "全面分析全书机制";
    let result = service
        .ask_streaming(&base, question, None, events)
        .await
        .unwrap();
    let run_id = std::iter::from_fn(|| receiver.try_recv().ok())
        .filter_map(|event| match event {
            KnowledgeChatStreamEvent::RunStarted { run_id } => Some(run_id),
            _ => None,
        })
        .find(|run_id| {
            store
                .get_agent_run(run_id)
                .is_ok_and(|run| run.task_type == "knowledge_qa" && run.status == "failed")
        })
        .unwrap();
    let failed = store.get_agent_run(&run_id).unwrap();
    assert_eq!(failed.status, "failed");
    assert!(failed.output.unwrap()["partial_answer"]
        .as_str()
        .unwrap()
        .contains("第一段"));
    let run = store.get_agent_run(&result.run_id).unwrap();
    assert_eq!(run.input["qa_retry_parent_run_id"], run_id);
    assert!(
        run.input["request_max_output_tokens"].as_u64().unwrap()
            > failed.input["request_max_output_tokens"].as_u64().unwrap()
    );
    assert_eq!(run.status, "completed");
    let calls = requests.lock().unwrap();
    assert!(calls.last().unwrap().0.contains("incomplete_draft"));
    assert!(!calls.last().unwrap().0.contains("[S99]"));
    assert_eq!(result.answer, "本轮综合结论。[S1]");
    assert!(
        store
            .list_agent_run_citations(&result.run_id)
            .unwrap()
            .len()
            > 10
    );
}

#[tokio::test]
async fn test_qa_empty_max_tokens_stops_after_two_expansions_or_hard_cap() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct AlwaysEmpty {
        answers: AtomicUsize,
        expected_output_tokens: u32,
    }
    #[async_trait]
    impl AgentRuntime for AlwaysEmpty {
        async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
            if request.prompt.contains("只读检索规划器") {
                return Ok(serde_json::json!({
                    "standalone_question":"全书",
                    "candidate_ids":[],
                    "answer_mode":"book_lookup",
                    "plan":{"goal":"全书","constraints":[],"subquestions":["机制"],"depth":"comprehensive","scope":"whole_book","expected_output_tokens":self.expected_output_tokens},
                    "memory_update":{"objective":"全书","constraints":[],"unresolved_questions":[],"entity_ids":[]}
                }).to_string());
            }
            self.answers.fetch_add(1, Ordering::SeqCst);
            Err(BrainError::LlmApiError {
                provider: "deepseek_harness".into(),
                detail: "DeepSeek Harness 达到输出 token 上限且未返回正文 (stop_reason=max_tokens)"
                    .into(),
            })
        }
    }
    for (expected_output_tokens, calls, failure_code) in [
        (262_144, 1, "qa_output_hard_limit"),
        (4096, 3, "qa_output_retry_exhausted"),
    ] {
        let (store, _dir, base) = fixture();
        let runtime = Arc::new(AlwaysEmpty {
            answers: AtomicUsize::new(0),
            expected_output_tokens,
        });
        let service = BookWikiService::new(store.clone(), runtime.clone());
        let error = service
            .ask(&base, "全面分析全书机制", None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("stop_reason=max_tokens"));
        assert!(error.to_string().contains(failure_code));
        assert_eq!(runtime.answers.load(Ordering::SeqCst), calls);
        assert!(store.list_conversations(&base, 10).unwrap().is_empty());
        let unfinished = store.list_unfinished_qa_runs(&base, 10).unwrap();
        assert_eq!(unfinished.len(), 1);
        assert_eq!(unfinished[0].stop_reason.as_deref(), Some("max_tokens"));
        assert!(!unfinished[0].has_partial_answer);
    }
}

#[tokio::test]
async fn test_qa_partial_max_tokens_stops_after_bounded_retries_without_saving_conversation() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct AlwaysPartial {
        answers: AtomicUsize,
    }
    #[async_trait]
    impl AgentRuntime for AlwaysPartial {
        async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
            if request.prompt.contains("只读检索规划器") {
                return Ok(serde_json::json!({
                    "standalone_question":"全书",
                    "candidate_ids":[],
                    "answer_mode":"book_lookup",
                    "plan":{"goal":"全书","constraints":[],"subquestions":["机制"],"depth":"standard","scope":"whole_book","expected_output_tokens":4096},
                    "memory_update":{"objective":"全书","constraints":[],"unresolved_questions":[],"entity_ids":[]}
                }).to_string());
            }
            Err(BrainError::Internal("测试应走事件型运行接口".into()))
        }
        async fn prompt_with_events(
            &self,
            request: AgentPromptRequest,
            events: Option<tokio::sync::mpsc::UnboundedSender<AgentRuntimeEvent>>,
            _cancel: tokio::sync::watch::Receiver<bool>,
        ) -> Result<String, BrainError> {
            if request.prompt.contains("只读检索规划器") {
                return self.prompt(request).await;
            }
            self.answers.fetch_add(1, Ordering::SeqCst);
            if let Some(events) = events {
                events
                    .send(AgentRuntimeEvent::TextDelta {
                        delta: "未完成正文 [S99]".into(),
                    })
                    .unwrap();
                events
                    .send(AgentRuntimeEvent::Completed {
                        stop_reason: "max_tokens".into(),
                        complete: false,
                    })
                    .unwrap();
            }
            Err(BrainError::LlmApiError {
                provider: "deepseek_harness".into(),
                detail: "DeepSeek Harness 达到输出 token 上限，已生成正文保留为部分结果 (stop_reason=max_tokens)".into(),
            })
        }
    }
    let (store, _dir, base) = fixture();
    let runtime = Arc::new(AlwaysPartial {
        answers: AtomicUsize::new(0),
    });
    let service = BookWikiService::new(store.clone(), runtime.clone());
    let error = service
        .ask(&base, "全面分析全书机制", None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("stop_reason=max_tokens"));
    assert!(error.to_string().contains("qa_output_retry_exhausted"));
    assert_eq!(runtime.answers.load(Ordering::SeqCst), 3);
    assert!(store.list_conversations(&base, 10).unwrap().is_empty());
}

#[tokio::test]
async fn test_qa_transient_transport_failure_retries_only_answer_run_once() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct TransportThenAnswer {
        planner: AdaptiveFixtureRuntime,
        answers: AtomicUsize,
    }
    #[async_trait]
    impl AgentRuntime for TransportThenAnswer {
        async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
            self.planner.prompt(request).await
        }
        async fn prompt_with_events(
            &self,
            request: AgentPromptRequest,
            _events: Option<tokio::sync::mpsc::UnboundedSender<AgentRuntimeEvent>>,
            _cancel: tokio::sync::watch::Receiver<bool>,
        ) -> Result<String, BrainError> {
            if request.prompt.contains("只读检索规划器") {
                return self.planner.prompt(request).await;
            }
            if self.answers.fetch_add(1, Ordering::SeqCst) == 0 {
                return Err(BrainError::LlmApiError {
                    provider: "deepseek_harness".into(),
                    detail: "ACP 调用失败: HTTP 503 Service Unavailable".into(),
                });
            }
            Ok("本轮综合结论。[S1]".into())
        }
    }
    let (store, _dir, base) = fixture();
    let runtime = Arc::new(TransportThenAnswer {
        planner: AdaptiveFixtureRuntime {
            requests: Arc::new(Mutex::new(vec![])),
        },
        answers: AtomicUsize::new(0),
    });
    let service = BookWikiService::new(store.clone(), runtime.clone());
    let answer = service
        .ask(&base, "全面分析全书的机制和边界", None)
        .await
        .unwrap();
    assert_eq!(answer.answer, "本轮综合结论。[S1]");
    assert_eq!(runtime.answers.load(Ordering::SeqCst), 2);
    let completed = store.get_agent_run(&answer.run_id).unwrap();
    assert_eq!(completed.input["transient_retry_attempt"], 1);
    assert!(completed.input["qa_retry_parent_run_id"].is_string());
    assert_eq!(store.list_conversations(&base, 10).unwrap().len(), 1);
}

#[tokio::test]
async fn test_qa_planner_invalid_request_does_not_start_answer_run() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct InvalidPlanner(AtomicUsize);
    #[async_trait]
    impl AgentRuntime for InvalidPlanner {
        async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
            if request.prompt.contains("只读检索规划器") {
                return Err(BrainError::LlmApiError {
                    provider: "deepseek_harness".into(),
                    detail: "ACP 调用失败: HTTP 400 invalid_request".into(),
                });
            }
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok("不应发送的回答".into())
        }
    }
    let (store, _dir, base) = fixture();
    let runtime = Arc::new(InvalidPlanner(AtomicUsize::new(0)));
    let service = BookWikiService::new(store.clone(), runtime.clone());
    let error = service
        .ask(&base, "全面分析全书机制", None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("HTTP 400"));
    assert_eq!(runtime.0.load(Ordering::SeqCst), 0);
    assert!(store.list_conversations(&base, 10).unwrap().is_empty());
}

#[tokio::test]
async fn test_qa_planner_empty_answer_reports_visible_fallback_and_failed_run() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct EmptyPlanner(AtomicUsize);
    #[async_trait]
    impl AgentRuntime for EmptyPlanner {
        async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
            if request.prompt.contains("只读检索规划器") {
                return Err(BrainError::LlmApiError {
                    provider: "deepseek_harness".into(),
                    detail: "DeepSeek Harness 返回了空回答 (stop_reason=end_turn)".into(),
                });
            }
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok("目前资料不足以支持确定结论。".into())
        }
    }
    let (store, _dir, base) = fixture();
    let runtime = Arc::new(EmptyPlanner(AtomicUsize::new(0)));
    let service = BookWikiService::new(store.clone(), runtime.clone());
    let (events, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let answer = service
        .ask_streaming(&base, "全面分析全书机制", None, events)
        .await
        .unwrap();
    assert_eq!(runtime.0.load(Ordering::SeqCst), 1);
    let run = store.get_agent_run(&answer.run_id).unwrap();
    let failed_plan = run.input["planning_run_ids"][0].as_str().unwrap();
    assert_eq!(store.get_agent_run(failed_plan).unwrap().status, "failed");
    assert!(std::iter::from_fn(|| receiver.try_recv().ok()).any(|event| {
        matches!(event, KnowledgeChatStreamEvent::Phase { message, .. } if message.contains("目录规划未完成"))
    }));
}

#[tokio::test]
async fn test_qa_empty_max_tokens_expands_only_answer_run_and_keeps_failed_run() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct EmptyThenAnswer {
        planner: AdaptiveFixtureRuntime,
        answers: AtomicUsize,
    }
    #[async_trait]
    impl AgentRuntime for EmptyThenAnswer {
        async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
            self.planner.prompt(request).await
        }
        async fn prompt_with_events(
            &self,
            request: AgentPromptRequest,
            events: Option<tokio::sync::mpsc::UnboundedSender<AgentRuntimeEvent>>,
            _cancel: tokio::sync::watch::Receiver<bool>,
        ) -> Result<String, BrainError> {
            if request.prompt.contains("只读检索规划器") {
                return self.planner.prompt(request).await;
            }
            if self.answers.fetch_add(1, Ordering::SeqCst) == 0 {
                if let Some(events) = events {
                    events
                        .send(AgentRuntimeEvent::UsageContext {
                            used: 8192,
                            size: 131_072,
                        })
                        .unwrap();
                    events
                        .send(AgentRuntimeEvent::Completed {
                            stop_reason: "max_tokens".into(),
                            complete: false,
                        })
                        .unwrap();
                }
                return Err(BrainError::LlmApiError {
                    provider: "deepseek_harness".into(),
                    detail:
                        "DeepSeek Harness 达到输出 token 上限且未返回正文 (stop_reason=max_tokens)"
                            .into(),
                });
            }
            Ok("本轮综合结论。[S1]".into())
        }
    }
    let (store, _dir, base) = fixture();
    let runtime = Arc::new(EmptyThenAnswer {
        planner: AdaptiveFixtureRuntime {
            requests: Arc::new(Mutex::new(vec![])),
        },
        answers: AtomicUsize::new(0),
    });
    let service = BookWikiService::new(store.clone(), runtime.clone());
    let (events, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let answer = service
        .ask_streaming(&base, "全面分析全书的机制和边界", None, events)
        .await
        .unwrap();
    assert_eq!(runtime.answers.load(Ordering::SeqCst), 2);
    let stream_events = std::iter::from_fn(|| receiver.try_recv().ok()).collect::<Vec<_>>();
    assert!(stream_events.iter().any(|event| {
        matches!(event, KnowledgeChatStreamEvent::Phase { message, .. } if message.contains("扩容输出预算"))
    }));
    let answer_runs = stream_events
        .into_iter()
        .filter_map(|event| match event {
            KnowledgeChatStreamEvent::RunStarted { run_id } => Some(run_id),
            _ => None,
        })
        .filter(|id| store.get_agent_run(id).unwrap().task_type == "knowledge_qa")
        .collect::<Vec<_>>();
    assert_eq!(answer_runs.len(), 2);
    let failed = store.get_agent_run(&answer_runs[0]).unwrap();
    let completed = store.get_agent_run(&answer_runs[1]).unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(completed.status, "completed");
    assert_eq!(answer.run_id, completed.id);
    assert!(
        completed.input["request_max_output_tokens"]
            .as_u64()
            .unwrap()
            > failed.input["request_max_output_tokens"].as_u64().unwrap()
    );
    assert_eq!(completed.input["output_expansion_attempt"], 1);
    assert_eq!(completed.input["qa_retry_parent_run_id"], failed.id);
    assert_eq!(
        completed.input["adaptive_budget"]["context_window"],
        131_072
    );
    assert_eq!(
        completed.input["qa_resources"]["context_capacity_known"],
        true
    );
    assert_eq!(
        store
            .get_agent_run_inspection(&completed.id)
            .unwrap()
            .snapshot
            .unwrap()
            .evidence_refs["runtime_budget"]["context_window"],
        131_072
    );
    assert!(store
        .list_agent_run_events(&completed.id)
        .unwrap()
        .iter()
        .any(|event| event.event_type == "run.output_budget_expanded"));
    assert_eq!(answer.answer, "本轮综合结论。[S1]");
    assert_eq!(
        store.list_conversations(&base, 10).unwrap()[0].message_count,
        2
    );
}
