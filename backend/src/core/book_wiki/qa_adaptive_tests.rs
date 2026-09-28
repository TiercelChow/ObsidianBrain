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
async fn test_truncated_answer_explicit_resume_replans_and_returns_complete_fresh_citations() {
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
    assert!(service
        .ask_streaming(&base, question, None, events)
        .await
        .is_err());
    let run_id = std::iter::from_fn(|| receiver.try_recv().ok())
        .filter_map(|event| match event {
            KnowledgeChatStreamEvent::RunStarted { run_id } => Some(run_id),
            _ => None,
        })
        .last()
        .unwrap();
    let failed = store.get_agent_run(&run_id).unwrap();
    assert_eq!(failed.status, "failed");
    assert!(failed.output.unwrap()["partial_answer"]
        .as_str()
        .unwrap()
        .contains("第一段"));
    let (events, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let result = service
        .resume_qa_streaming(&base, question, None, &run_id, events)
        .await
        .unwrap();
    let run = store.get_agent_run(&result.run_id).unwrap();
    assert_eq!(run.input["resume_run_id"], run_id);
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
    for (expected_output_tokens, calls) in [(262_144, 1), (4096, 3)] {
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
        assert_eq!(runtime.answers.load(Ordering::SeqCst), calls);
        assert!(store.list_conversations(&base, 10).unwrap().is_empty());
    }
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
