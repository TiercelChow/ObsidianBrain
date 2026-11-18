use super::*;
use crate::infra::sqlite_store::SqliteStore;
use crate::models::book_wiki::{BookKind, ReaderBook};
use crate::models::wiki_topics::{
    TopicCompileRequest, TopicSourceRange, TopicSourceSelection, TopicWritebackRequest,
};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Default)]
pub(super) struct TopicRuntime {
    results: Mutex<VecDeque<Result<String, BrainError>>>,
    pub(super) prompts: Mutex<Vec<String>>,
    pub(super) emit_fragments: AtomicBool,
}

impl TopicRuntime {
    pub(super) fn reply(&self, markdown: &str) {
        self.results.lock().unwrap().push_back(Ok(markdown.into()));
    }

    pub(super) fn fail(&self, error: BrainError) {
        self.results.lock().unwrap().push_back(Err(error));
    }
}

#[async_trait]
impl AgentRuntime for TopicRuntime {
    async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
        self.prompts.lock().unwrap().push(request.prompt);
        self.results.lock().unwrap().pop_front().unwrap()
    }

    async fn prompt_with_events(
        &self,
        request: AgentPromptRequest,
        events: Option<tokio::sync::mpsc::UnboundedSender<AgentRuntimeEvent>>,
        _: tokio::sync::watch::Receiver<bool>,
    ) -> Result<String, BrainError> {
        let answer = self.prompt(request).await?;
        if let Some(events) = events.filter(|_| self.emit_fragments.load(Ordering::Relaxed)) {
            for character in answer.chars() {
                let _ = events.send(AgentRuntimeEvent::TextDelta {
                    delta: character.to_string(),
                });
            }
        }
        Ok(answer)
    }
}

pub(super) struct Fixture {
    _dir: tempfile::TempDir,
    pub(super) book: PathBuf,
    pub(super) service: BookWikiService,
    pub(super) runtime: Arc<TopicRuntime>,
    pub(super) base: String,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let book = dir.path().join("book");
        std::fs::create_dir(&book).unwrap();
        std::fs::write(
            book.join("avg.md"),
            "# 北辰 Cache 热点实验\n\n相同配置下，CLOCK 平均延迟为 1.8 ms，LRU 为 2.4 ms。",
        )
        .unwrap();
        let store = BookWikiStore::new(Arc::new(
            SqliteStore::new(&dir.path().join("topic-pages.db")).unwrap(),
        ));
        store
            .save_reader_books(&[ReaderBook {
                id: "topic-test-book".into(),
                path: book.to_string_lossy().into(),
                kind: BookKind::Folder,
                name: "北辰 Cache".into(),
                description: String::new(),
                category: String::new(),
                added_at: 1,
                progress: None,
            }])
            .unwrap();
        let runtime = Arc::new(TopicRuntime::default());
        let service = BookWikiService::new(store, runtime.clone());
        let base = service
            .initialize_and_sync("topic-test-book")
            .unwrap()
            .knowledge_base
            .id;
        Self {
            _dir: dir,
            book,
            service,
            runtime,
            base,
        }
    }

    fn sources(&self) -> Vec<TopicSourceSelection> {
        self.service
            .store()
            .list_current_source_documents(&self.base)
            .unwrap()
            .into_iter()
            .map(|document| TopicSourceSelection {
                document_id: document.id,
                span_id: None,
                range: None,
            })
            .collect()
    }

    fn request(&self) -> TopicCompileRequest {
        TopicCompileRequest {
            page_id: None,
            title: "CLOCK 与 LRU 的取舍".into(),
            guidance: String::new(),
            sources: self.sources(),
        }
    }
}

#[tokio::test]
async fn test_topic_question_reads_cited_original_beyond_the_topic_preview() {
    use crate::models::wiki_topics::TopicSourceRange;

    let f = Fixture::new();
    let lead = format!(
        "# EOS 提交规则\n\nEOS 提交规则用于说明终止。{}\n\n",
        "一条已知请求的背景说明。".repeat(180)
    );
    let tail = "## 判定\n\nEOS 提交规则先逐 token 更新账本，再判定停止。TAIL_COMMIT_MARKER";
    std::fs::write(f.book.join("commit.md"), format!("{lead}{tail}")).unwrap();
    f.service.sync(&f.base).unwrap();
    let (_, inventory) = f.service.store().topic_book_inventory(&f.base).unwrap();
    let original = inventory.iter().find(|d| d.path == "commit.md").unwrap();
    let published = format!(
        "## EOS 提交规则\n\n{}\n\n必须先写账本，再判定停止。[S1]",
        "这段说明用于理解请求提交。".repeat(130)
    );
    f.runtime.reply(&published);
    let compiled = f
        .service
        .compile_topic(
            &f.base,
            TopicCompileRequest {
                page_id: None,
                title: "EOS 提交规则".into(),
                guidance: String::new(),
                sources: vec![TopicSourceSelection {
                    document_id: original.id.clone(),
                    span_id: Some(original.sections[0].id.clone()),
                    range: Some(TopicSourceRange {
                        offset: lead.chars().count(),
                        length: tail.chars().count(),
                    }),
                }],
            },
        )
        .await
        .unwrap();
    f.service
        .publish_topic(&f.base, &compiled.page.id, &compiled.draft.unwrap().id)
        .unwrap();

    f.runtime.reply("逐 token 更新账本后判定 EOS 停止。[S1]");
    f.service
        .ask_topic(&f.base, "EOS 提交规则里的停止顺序是什么？", None, None)
        .await
        .unwrap();
    let prompts = f.runtime.prompts.lock().unwrap();
    let answer_material = prompts
        .last()
        .unwrap()
        .split_once("<answer_material>\n")
        .unwrap()
        .1
        .split_once("\n</answer_material>")
        .unwrap()
        .0;
    let context: serde_json::Value = serde_json::from_str(answer_material).unwrap();
    assert!(context["sources"].as_array().unwrap().iter().any(|source| {
        source["markdown"]
            .as_str()
            .unwrap()
            .contains("TAIL_COMMIT_MARKER")
    }));
}

#[tokio::test]
async fn test_topic_question_finds_a_later_direct_source_passage() {
    let f = Fixture::new();
    let source = format!(
        "# 输出账本\n\n索引列出 EOS、如何、改变、候选 token 和提交顺序等条目。\n\n{}\n\n## 更新顺序\n\n候选逐 token 写入账本，遇到 EOS 后停止提交。OUTPUT_COMMIT_MARKER",
        "这一段说明设备运行的其他限制。\n\n".repeat(220)
    );
    std::fs::write(f.book.join("ledger.md"), source).unwrap();
    f.service.sync(&f.base).unwrap();
    f.runtime.reply("遇到 EOS 后停止追加账本。[S1]");
    f.service
        .ask_topic(&f.base, "EOS 如何改变候选 token 的提交顺序？", None, None)
        .await
        .unwrap();

    assert!(f
        .runtime
        .prompts
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .contains("OUTPUT_COMMIT_MARKER"));
}

#[tokio::test]
async fn test_topic_question_keeps_the_original_behind_the_best_matching_section() {
    let f = Fixture::new();
    for index in 0..16 {
        std::fs::write(
            f.book.join(format!("comparison-{index}.md")),
            format!("# 冷启动第一次使用哪条路由\n\n这是热备用的对照 {index}。冷启动第一次使用哪条路由还待确认。"),
        ).unwrap();
    }
    std::fs::write(
        f.book.join("actual.md"),
        "# 初始化路径\n\n首个批次走主队列。BOOT_ROUTE_PROOF",
    )
    .unwrap();
    f.service.sync(&f.base).unwrap();
    let (_, inventory) = f.service.store().topic_book_inventory(&f.base).unwrap();
    let original = inventory
        .iter()
        .find(|doc| doc.path == "actual.md")
        .unwrap();
    f.runtime
        .reply("## 冷启动第一次使用哪条路由\n\n首个批次走主队列。[S1]");
    let compiled = f
        .service
        .compile_topic(
            &f.base,
            TopicCompileRequest {
                page_id: None,
                title: "初始化流程".into(),
                guidance: String::new(),
                sources: vec![TopicSourceSelection {
                    document_id: original.id.clone(),
                    span_id: Some(original.sections[0].id.clone()),
                    range: None,
                }],
            },
        )
        .await
        .unwrap();
    f.service
        .publish_topic(&f.base, &compiled.page.id, &compiled.draft.unwrap().id)
        .unwrap();

    f.runtime.reply("首个批次走主队列。[S1]");
    f.service
        .ask_topic(&f.base, "冷启动第一次使用哪条路由？", None, None)
        .await
        .unwrap();
    let prompts = f.runtime.prompts.lock().unwrap();
    let context = prompts.last().unwrap();
    assert!(context.contains("BOOT_ROUTE_PROOF"));
}

#[tokio::test]
async fn test_topic_update_and_new_conversation_use_published_knowledge_and_originals() {
    let f = Fixture::new();
    f.runtime.reply("## 热点平均延迟\n\n在同条件下 CLOCK 低于 LRU。[S1]\n\n## 上线证据\n\n尚无 P99 和过期读取记录。");
    let compiled = f.service.compile_topic(&f.base, f.request()).await.unwrap();
    let page = compiled.page.id;
    let draft = compiled.draft.unwrap();
    assert!(compiled.published.is_none());
    let first = f.service.publish_topic(&f.base, &page, &draft.id).unwrap();
    let first = first.published.unwrap();
    let duplicate = f.service.compile_topic(&f.base, f.request()).await.unwrap();
    assert_eq!(duplicate.published.unwrap().id, first.id);
    f.runtime
        .reply("当前实验只记录了平均延迟。[S1] P99 和过期读取仍待测。");
    let gap_answer = f
        .service
        .ask_topic(&f.base, "CLOCK 达到 P99 门槛了吗？", None, None)
        .await
        .unwrap();
    let with_question = f
        .service
        .store()
        .save_topic_question(&f.base, &page, &gap_answer.run_id)
        .unwrap();
    let gap = &with_question.questions[0];
    assert!(gap.question.contains("P99 门槛"));
    assert!(gap.resolved_at.is_none());
    f.service
        .store()
        .save_topic_question(&f.base, &page, &gap_answer.run_id)
        .unwrap();
    assert_eq!(f.runtime.prompts.lock().unwrap().len(), 2);

    std::fs::write(
        f.book.join("p99.md"),
        "# CLOCK P99\n\n热点灰度中 CLOCK 的 P99 为 1.9 ms。尚未统计过期读取。",
    )
    .unwrap();
    f.service.sync(&f.base).unwrap();
    let mut update = f.request();
    update.page_id = Some(page.clone());
    f.runtime.reply("## 热点实验\n\n平均延迟仍为 1.8 ms。[S1]\n\n## 灰度门槛\n\nCLOCK 的 P99 为 1.9 ms，[S2] 还需核实过期读取。");
    let revised = f.service.compile_topic(&f.base, update).await.unwrap();
    assert_eq!(revised.published.unwrap().id, first.id);
    let draft = revised.draft.unwrap();
    f.service.publish_topic(&f.base, &page, &draft.id).unwrap();
    let old = f
        .service
        .store()
        .topic_revision(&f.base, &page, &first.id)
        .unwrap();
    assert!(old.markdown.contains("尚无 P99"));

    f.runtime
        .reply("CLOCK P99 为 1.9 ms。[S1] 过期读取仍待核实。");
    let answer = f
        .service
        .ask_topic(&f.base, "CLOCK P99 怎么样？", None, None)
        .await
        .unwrap();
    let prompts = f.runtime.prompts.lock().unwrap();
    assert_eq!(prompts.len(), 4);
    assert!(
        prompts[2].contains("尚无 P99")
            && prompts[2].contains("1.9 ms")
            && prompts[2].contains("达到 P99 门槛")
    );
    assert!(prompts[3].contains("灰度门槛") && prompts[3].contains("P99 为 1.9 ms"));
    assert!(f
        .service
        .store()
        .list_agent_run_citations(&answer.run_id)
        .unwrap()
        .iter()
        .any(|citation| citation.content_md.contains("1.9 ms")));
}

#[tokio::test]
async fn test_topic_manual_edit_keeps_old_draft_and_rejects_out_of_range_citations() {
    let f = Fixture::new();
    f.runtime.reply("## 热点实验\n\nCLOCK 为 1.8 ms。[S1]");
    let compiled = f.service.compile_topic(&f.base, f.request()).await.unwrap();
    let draft = compiled.draft.unwrap();
    let page_id = compiled.page.id;
    assert!(f
        .service
        .edit_topic_draft(&f.base, &page_id, &draft.id, "## 热点\n\n错误出处。[S9]")
        .is_err());
    let edited = f
        .service
        .edit_topic_draft(
            &f.base,
            &page_id,
            &draft.id,
            "## 热点评估\n\n同条件下 CLOCK 的平均延迟为 1.8 ms。[S1]",
        )
        .unwrap();
    let next = edited.draft.unwrap();
    assert_ne!(draft.id, next.id);
    assert_eq!(draft.run_id, next.run_id);
    assert!(f
        .service
        .publish_topic(&f.base, &page_id, &draft.id)
        .is_err());
    let published = f
        .service
        .publish_topic(&f.base, &page_id, &next.id)
        .unwrap()
        .published
        .unwrap();
    assert!(published.markdown.contains("热点评估"));
    assert_eq!(f.runtime.prompts.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_topic_http_routes_compile_publish_answer_and_keep_books_isolated() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;

    let (mut ctx, dir, _) = crate::AppContext::for_test();
    let runtime = Arc::new(TopicRuntime::default());
    let context = Arc::get_mut(&mut ctx).unwrap();
    context.book_wiki_service = Arc::new(BookWikiService::new(
        BookWikiStore::new(context.db.clone()),
        runtime.clone(),
    ));
    let service = ctx.book_wiki_service.clone();
    let mut bases = Vec::new();
    for book in ["one", "two"] {
        let path = dir.path().join(book);
        std::fs::create_dir(&path).unwrap();
        std::fs::write(
            path.join("chapter.md"),
            format!("# 取舍\n\n同口径下 CLOCK 的平均延迟为 1.8 ms。来源标记：{book}。"),
        )
        .unwrap();
        service
            .store()
            .save_reader_books(&[ReaderBook {
                id: book.into(),
                path: path.to_string_lossy().into(),
                kind: BookKind::Folder,
                name: book.into(),
                description: String::new(),
                category: String::new(),
                added_at: 1,
                progress: None,
            }])
            .unwrap();
        bases.push(service.initialize_and_sync(book).unwrap().knowledge_base.id);
    }
    let base = &bases[0];
    let other = &bases[1];
    let app = crate::api::router::create_router(ctx.clone());
    let root = format!("/v1/knowledge/bases/{base}/topics");
    let send = |method: &'static str, path: String, payload: Value| {
        let app = app.clone();
        async move {
            let response = app
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .header("content-type", "application/json")
                        .body(Body::from(payload.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
            (status, String::from_utf8(body.to_vec()).unwrap())
        }
    };

    let (status, empty) = send("GET", root.clone(), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty, "[]");
    let (status, raw) = send("GET", format!("{root}/sources"), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    let sources: Value = serde_json::from_str(&raw).unwrap();
    let doc = sources[0]["id"].as_str().unwrap();
    let (status, raw) = send("GET", format!("{root}/sources/{doc}"), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    let sections: Value = serde_json::from_str(&raw).unwrap();
    let selection =
        json!({"title":"CLOCK 取舍","sources":[{"document_id":doc,"span_id":sections[0]["id"]}]});
    let (status, _) = send(
        "POST",
        format!("/v1/knowledge/bases/{other}/topics"),
        selection.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    runtime.reply("## 受控实验\n\nCLOCK 平均延迟为 1.8 ms。[S1]");
    let (status, raw) = send("POST", root.clone(), selection).await;
    assert_eq!(status, StatusCode::OK, "{raw}");
    let detail: Value = serde_json::from_str(&raw).unwrap();
    let id = detail["page"]["id"].as_str().unwrap();
    let revision = detail["draft"]["id"].as_str().unwrap();
    assert_eq!(
        send(
            "GET",
            format!("/v1/knowledge/bases/{other}/topics/{id}"),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            "POST",
            format!("{root}/{id}/revisions/{revision}/publish"),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );

    runtime.reply("CLOCK 平均延迟为 1.8 ms。[S1]");
    let (status, response) = send(
        "POST",
        "/v1/knowledge/chat/stream".into(),
        json!({"knowledge_base_id":base,"question":"CLOCK 延迟？"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let events = response
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|raw| serde_json::from_str::<Value>(raw).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(events[0]["type"], "evidence");
    let completed = events.last().unwrap();
    assert_eq!(completed["type"], "completed", "{response}");
    let run_id = completed["result"]["run_id"].as_str().unwrap();
    let answer = service.store().get_agent_run(run_id).unwrap();
    assert_eq!(answer.input["wiki_mode"], "topics");
    assert_eq!(answer.input["topic_revisions"][0], revision);
    assert!(service
        .store()
        .get_agent_run_citation(run_id, 0)
        .unwrap()
        .content_md
        .contains("来源标记：one"));
    assert!(!runtime.prompts.lock().unwrap()[1].contains("来源标记：two"));

    let (status, raw) = send(
        "GET",
        format!("{root}/writeback-material/{run_id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{raw}");
    let material: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(material["kind"], "answer");
    assert_eq!(
        material["passages"][0]["markdown"],
        "CLOCK 平均延迟为 1.8 ms。[S1]"
    );
    assert_eq!(
        send(
            "GET",
            format!("/v1/knowledge/bases/{other}/topics/writeback-material/{run_id}"),
            Value::Null,
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let request = json!({"run_id":run_id,"expected_revision_id":revision,"passages":[material["passages"][0]["index"]]});
    let (status, raw) = send(
        "POST",
        format!("{root}/{id}/writeback-draft"),
        request.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{raw}");
    let imported: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(imported["published"]["id"], revision);
    let new_id = imported["draft"]["id"].as_str().unwrap();
    let (status, raw) = send("POST", format!("{root}/{id}/writeback-draft"), request).await;
    assert_eq!(status, StatusCode::OK, "{raw}");
    let repeated: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(repeated["draft"]["id"], new_id);
}

#[tokio::test]
async fn test_answer_draft_adds_only_selected_text_and_exact_cited_ranges() {
    use serde_json::json;

    let f = Fixture::new();
    let store = f.service.store();
    std::fs::write(f.book.join("p99.md"), "# 尾延迟\n\nCLOCK P99 为 1.9 ms。").unwrap();
    std::fs::write(f.book.join("unused.md"), "# 无关证据\n\n独立实验。").unwrap();
    f.service.sync(&f.base).unwrap();
    let documents = store.list_current_source_documents(&f.base).unwrap();
    let source = |name: &str| {
        let doc = documents
            .iter()
            .find(|item| item.relative_path == name)
            .unwrap();
        let section = &store
            .list_topic_source_sections(&f.base, &doc.id, 0, 1)
            .unwrap()[0];
        store.get_current_source_span(&f.base, &section.id).unwrap()
    };
    let (avg, p99, unused) = (source("avg.md"), source("p99.md"), source("unused.md"));
    let avg_doc = documents
        .iter()
        .find(|doc| doc.relative_path == "avg.md")
        .unwrap();
    let review_line = "## 原有章节\n\n同口径实验的标题见原文。[S1] 代码标记 `[S7]` 保留。";
    f.runtime.reply(review_line);
    let mut compile = f.request();
    compile.sources = vec![TopicSourceSelection {
        document_id: avg_doc.id.clone(),
        span_id: Some(avg.id.clone()),
        range: Some(TopicSourceRange {
            offset: 0,
            length: 8,
        }),
    }];
    let page = f.service.compile_topic(&f.base, compile).await.unwrap();
    let id = &page.page.id;
    let prior = f
        .service
        .publish_topic(&f.base, id, &page.draft.unwrap().id)
        .unwrap()
        .published
        .unwrap();

    let answer = store
        .start_agent_run(
            &f.base,
            "test",
            "knowledge_qa",
            &json!({"question":"平均延迟与尾延迟的实验依据？"}),
        )
        .unwrap();
    let add = |span: &crate::models::book_wiki::SourceSpanSnapshot, offset: usize, chars: usize| {
        let content = span
            .content
            .chars()
            .skip(offset)
            .take(chars)
            .collect::<String>();
        store
            .record_visible_agent_evidence(
                &answer.id,
                "source_span",
                &span.id,
                &span.source_version_id,
                &json!({"content_md":content,"offset_chars":offset}),
            )
            .unwrap()
    };
    assert_eq!(add(&unused, 0, 5), 1);
    assert_eq!(add(&p99, 6, 12), 2);
    assert_eq!(add(&avg, 19, 11), 3);
    store.complete_agent_run(&answer.id, &json!({"answer":
        "## 可纳入的对照\n\n先核对旧实验。[S3] 尾延迟另有记录。[S2]\n\n`[S99]` 和 [说明链接](https://example.invalid/[S4])。\n\n## 留在对话中的旁支\n\n独立实验。[S1]"
    })).unwrap();
    let request = || TopicWritebackRequest {
        run_id: answer.id.clone(),
        expected_revision_id: prior.id.clone(),
        passages: vec![0],
    };
    let prepared = f
        .service
        .draft_topic_from_material(&f.base, id, request())
        .unwrap();
    assert_eq!(prepared.published.unwrap().markdown, review_line);
    let draft = prepared.draft.unwrap();
    assert!(draft.markdown.starts_with(review_line));
    assert!(draft
        .markdown
        .contains("先核对旧实验。[S1] 尾延迟另有记录。[S2]"));
    assert!(draft.markdown.contains("`[S99]`"));
    assert!(draft.markdown.contains("https://example.invalid/[S4]"));
    assert!(!draft.markdown.contains("留在对话中的旁支"));
    assert_ne!(draft.run_id, answer.id);
    assert_eq!(
        store.get_agent_run(&draft.run_id).unwrap().input["source_run_id"],
        answer.id
    );

    let ledger = store.list_agent_run_citations(&draft.run_id).unwrap();
    assert_eq!(ledger.len(), 2);
    assert_eq!(ledger[0].object_id, avg.id);
    assert_eq!(
        ledger[0]
            .read_ranges
            .iter()
            .map(|range| (range.offset_chars, range.returned_chars))
            .collect::<Vec<_>>(),
        vec![(0, 8), (19, 11)]
    );
    assert_eq!(ledger[1].object_id, p99.id);
    assert_eq!(ledger[1].read_ranges.len(), 1);
    assert_eq!(ledger[1].read_ranges[0].offset_chars, 6);
    assert_eq!(ledger[1].read_ranges[0].returned_chars, 12);
    assert_eq!(store.list_agent_run_citations(&answer.id).unwrap().len(), 3);

    let edited = f
        .service
        .edit_topic_draft(
            &f.base,
            id,
            &draft.id,
            &format!("{}\n\n读者手改说明。", draft.markdown),
        )
        .unwrap()
        .draft
        .unwrap();
    let retried = f
        .service
        .draft_topic_from_material(&f.base, id, request())
        .unwrap()
        .draft
        .unwrap();
    assert_eq!(edited.id, retried.id);
    assert!(retried.markdown.contains("读者手改说明。"));
    let other = f.service.draft_topic_from_material(
        &f.base,
        id,
        TopicWritebackRequest {
            passages: vec![1],
            ..request()
        },
    );
    assert!(
        matches!(other, Err(BrainError::KnowledgeConflict(message)) if message.contains("草稿"))
    );
    f.service.publish_topic(&f.base, id, &edited.id).unwrap();
    assert!(f
        .service
        .draft_topic_from_material(&f.base, id, request())
        .unwrap()
        .draft
        .is_none());
    assert_eq!(f.runtime.prompts.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_answer_draft_rejects_stale_and_incomplete_answers() {
    let f = Fixture::new();
    f.runtime.reply("## 热点口径\n\n同配置下记录平均延迟。[S1]");
    let compiled = f.service.compile_topic(&f.base, f.request()).await.unwrap();
    let id = &compiled.page.id;
    let published = f
        .service
        .publish_topic(&f.base, id, &compiled.draft.unwrap().id)
        .unwrap()
        .published
        .unwrap();

    std::fs::write(
        f.book.join("new.md"),
        "# 容量测试\n\n容量压缩后有另一项数据。",
    )
    .unwrap();
    f.service.sync(&f.base).unwrap();
    let store = f.service.store();
    let doc = store
        .list_current_source_documents(&f.base)
        .unwrap()
        .into_iter()
        .find(|doc| doc.relative_path == "new.md")
        .unwrap();
    let span_id = store
        .list_topic_source_sections(&f.base, &doc.id, 0, 1)
        .unwrap()[0]
        .id
        .clone();
    let span = store.get_current_source_span(&f.base, &span_id).unwrap();
    let answer = store
        .start_agent_run(
            &f.base,
            "test",
            "knowledge_qa",
            &serde_json::json!({"question":"容量压缩有什么数据？"}),
        )
        .unwrap();
    store
        .record_visible_agent_evidence(
            &answer.id,
            "source_span",
            &span.id,
            &span.source_version_id,
            &serde_json::json!({"content_md":span.content,"offset_chars":0}),
        )
        .unwrap();
    let request = || TopicWritebackRequest {
        run_id: answer.id.clone(),
        expected_revision_id: published.id.clone(),
        passages: vec![0],
    };
    assert!(matches!(
        f.service.draft_topic_from_material(&f.base, id, request()),
        Err(BrainError::KnowledgeValidation(_))
    ));
    store
        .complete_agent_run(
            &answer.id,
            &serde_json::json!({"answer":"容量压缩后测得另一项数据。[S1]"}),
        )
        .unwrap();
    std::fs::write(
        f.book.join("new.md"),
        "# 容量测试\n\n新一轮实验使用不同条件。",
    )
    .unwrap();
    f.service.sync(&f.base).unwrap();
    assert!(matches!(
        f.service.draft_topic_from_material(&f.base, id, request()),
        Err(BrainError::KnowledgeConflict(message)) if message.contains("原文已变化")
    ));
    let page = store.topic_page(&f.base, id).unwrap();
    assert!(page.draft.is_none());
    assert_eq!(page.published.unwrap().id, published.id);
    assert_eq!(page.revisions.len(), 1);
}

#[tokio::test]
async fn test_topic_failure_and_source_change_preserve_last_publication() {
    let f = Fixture::new();
    f.runtime.reply("## 热点实验\n\n没有对应的原文。[S88]");
    let request = f.request();
    assert!(f
        .service
        .compile_topic(&f.base, request.clone())
        .await
        .is_err());
    let pages = f.service.store().list_topic_pages(&f.base).unwrap();
    assert_eq!(pages.len(), 1);
    assert!(pages[0].last_error.as_deref().unwrap().contains("S88"));

    f.runtime.reply("## 热点实验\n\nCLOCK 为 1.8 ms。[S1]");
    let page = f
        .service
        .compile_topic(&f.base, request.clone())
        .await
        .unwrap();
    let id = page.page.id;
    let draft = page.draft.unwrap();
    f.service.publish_topic(&f.base, &id, &draft.id).unwrap();
    let mut revision = request;
    revision.guidance = "澄清平均延迟的口径".into();
    f.runtime.reply("## 口径\n\nCLOCK 平均延迟为 1.8 ms。[S1]");
    let pending = f
        .service
        .compile_topic(&f.base, revision)
        .await
        .unwrap()
        .draft
        .unwrap();
    std::fs::write(
        f.book.join("avg.md"),
        "# 北辰 Cache 热点实验\n\nCLOCK 平均延迟为 2.1 ms。",
    )
    .unwrap();
    f.service.sync(&f.base).unwrap();
    assert!(f.service.publish_topic(&f.base, &id, &pending.id).is_err());
    let page = f.service.store().topic_page(&f.base, &id).unwrap();
    assert!(page.page.needs_review);
    assert_eq!(page.published.unwrap().id, draft.id);
    assert!(f
        .service
        .store()
        .search_topic_sections(&f.base, "CLOCK", false)
        .unwrap()
        .is_empty());
    assert!(f
        .service
        .store()
        .get_agent_run_citation(&draft.run_id, 0)
        .unwrap()
        .content_md
        .contains("1.8 ms"));
}

#[tokio::test]
async fn test_topic_writeback_accepts_only_the_current_validated_report() {
    use rusqlite::params;
    use serde_json::json;

    let f = Fixture::new();
    let store = f.service.store();
    f.runtime
        .reply("## 原有主题\n\nCLOCK 的平均延迟见实验。[S1]");
    let page = f.service.compile_topic(&f.base, f.request()).await.unwrap();
    let id = &page.page.id;
    let published = f
        .service
        .publish_topic(&f.base, id, &page.draft.unwrap().id)
        .unwrap()
        .published
        .unwrap();
    let task = store
        .create_task(&f.base, "对比淘汰算法的边界", "", "research")
        .unwrap();
    store.start_task_execution(&task.id).unwrap();
    store.ensure_research_workspace(&task.id).unwrap();
    let report = store
        .start_agent_run(
            &f.base,
            "application",
            "knowledge_task_report",
            &json!({"knowledge_task_id":task.id,"research_stage_key":"report"}),
        )
        .unwrap();
    let source = &f.sources()[0].document_id;
    let span_id = &store
        .list_topic_source_sections(&f.base, source, 0, 1)
        .unwrap()[0]
        .id;
    let span = store.get_current_source_span(&f.base, span_id).unwrap();
    store
        .record_visible_agent_evidence(
            &report.id,
            "source_span",
            &span.id,
            &span.source_version_id,
            &json!({"content_md":span.content,"offset_chars":0}),
        )
        .unwrap();
    let body = "## 适用边界\n\n平均延迟的比较来自相同配置。[S1]\n\n## 下轮实验\n\n收集尾延迟。";
    store
        .complete_agent_run(&report.id, &json!({"answer":body}))
        .unwrap();
    let db = rusqlite::Connection::open(f._dir.path().join("topic-pages.db")).unwrap();
    db.execute(
        "INSERT INTO knowledge_research_stages(task_id,stage_key,title,kind,ordinal,status,content_run_id,content_md,updated_at)
         VALUES(?1,'report','报告','report',1,'completed',?2,?3,datetime('now')),
               (?1,'validation','交付校验','validation',2,'pending',NULL,'',datetime('now'))",
        params![task.id, report.id, body],
    ).unwrap();
    assert!(f
        .service
        .topic_writeback_material(&f.base, &report.id)
        .is_err());

    db.execute(
        "UPDATE knowledge_research_stages SET status='completed' WHERE task_id=?1 AND stage_key='validation'",
        [&task.id],
    ).unwrap();
    let material = f
        .service
        .topic_writeback_material(&f.base, &report.id)
        .unwrap();
    assert_eq!(material.title, task.title);
    assert!(matches!(
        material.kind,
        crate::models::wiki_topics::TopicWritebackKind::Report
    ));
    assert_eq!(material.passages.len(), 1);
    assert_eq!(material.passages[0].title, "适用边界");
    let request = || TopicWritebackRequest {
        run_id: report.id.clone(),
        expected_revision_id: published.id.clone(),
        passages: vec![material.passages[0].index],
    };
    let saved = f
        .service
        .draft_topic_from_material(&f.base, id, request())
        .unwrap();
    let draft = saved.draft.unwrap();
    assert!(draft.markdown.contains("## 适用边界"));
    assert!(!draft.markdown.contains("收集尾延迟"));
    let run = store.get_agent_run(&draft.run_id).unwrap();
    assert_eq!(run.input["source_run_id"], report.id);
    assert_eq!(run.input["source_kind"], "report");
    assert_eq!(f.runtime.prompts.lock().unwrap().len(), 1);

    db.execute(
        "UPDATE knowledge_research_stages SET content_run_id=NULL WHERE task_id=?1 AND stage_key='report'",
        [&task.id],
    ).unwrap();
    assert!(f
        .service
        .draft_topic_from_material(&f.base, id, request())
        .is_err());
    assert_eq!(
        store.topic_page(&f.base, id).unwrap().draft.unwrap().id,
        draft.id
    );
}

#[test]
fn test_reader_selection_saves_a_cited_topic_draft_without_model_or_legacy_entry() {
    let fixture = Fixture::new();
    let page = fixture
        .service
        .draft_reader_selection(&fixture.base, "avg.md", "CLOCK 平均延迟为 1.8 ms", None)
        .unwrap();
    let draft = page.draft.unwrap();
    assert!(page.published.is_none());
    assert!(draft.markdown.contains("[S1]"));
    assert!(draft.markdown.contains("CLOCK 平均延迟为 1.8 ms"));
    let excerpt = fixture
        .service
        .store()
        .get_agent_run_citation(&draft.run_id, 0)
        .unwrap();
    assert!(excerpt.content_md.ends_with("CLOCK 平均延迟为 1.8 ms"));
    assert_eq!(
        excerpt.read_ranges[0].returned_chars,
        "CLOCK 平均延迟为 1.8 ms".chars().count()
    );
    assert!(fixture.runtime.prompts.lock().unwrap().is_empty());
    let duplicate = fixture
        .service
        .draft_reader_selection(&fixture.base, "avg.md", "CLOCK 平均延迟为 1.8 ms", None)
        .unwrap();
    assert_eq!(duplicate.draft.unwrap().id, draft.id);
    assert!(fixture
        .service
        .draft_reader_selection(&fixture.base, "avg.md", "CLOCK 平均延迟为 9 ms", None)
        .is_err());
}
