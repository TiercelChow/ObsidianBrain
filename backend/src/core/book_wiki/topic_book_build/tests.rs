use crate::core::book_wiki::topic_page_tests::Fixture;

#[tokio::test]
async fn test_whole_book_build_syncs_every_file_and_reuses_successful_work() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.book.join("p99.md"),
        "# 灰度边界\n\nCLOCK 已测得 P99 为 1.9 ms。过期读取待查。",
    )
    .unwrap();
    fixture.runtime.reply(r#"{"topics":[{"title":"CLOCK 的上线条件","focus":"综合平均延迟与尾延迟的实验条件","sources":[1,2]}]}"#);
    fixture
        .runtime
        .reply("## 实验条件\n\n平均延迟为 1.8 ms。[S1] 热点 P99 为 1.9 ms。[S2] 过期读取仍待查。");
    let first = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    assert_eq!(first.status, "ready");
    assert_eq!(first.sources.len(), 2);
    assert!(first
        .sources
        .iter()
        .all(|source| source.page_ids.len() == 1));
    let drafts = first
        .pages
        .iter()
        .map(|page| page.revision_id.clone())
        .collect::<Vec<_>>();
    let indexed = fixture
        .service
        .store()
        .list_current_source_documents(&fixture.base)
        .unwrap()
        .into_iter()
        .map(|source| source.updated_at)
        .collect::<Vec<_>>();
    let repeated = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    assert_eq!(
        repeated
            .pages
            .iter()
            .map(|page| page.revision_id.clone())
            .collect::<Vec<_>>(),
        drafts
    );
    assert_eq!(
        fixture
            .service
            .store()
            .list_current_source_documents(&fixture.base)
            .unwrap()
            .into_iter()
            .map(|source| source.updated_at)
            .collect::<Vec<_>>(),
        indexed
    );
    let prompts = fixture.runtime.prompts.lock().unwrap();
    assert_eq!(prompts.len(), 2);
    assert!(prompts[0].contains("1.8 ms") && prompts[0].contains("1.9 ms"));
}

#[tokio::test]
async fn test_whole_book_places_and_cites_a_chapter_omitted_by_the_outline() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.book.join("clock-tail.md"),
        "# CLOCK 热点尾延迟\n\n热点尾延迟与 CLOCK 扫描长度一同测量。",
    )
    .unwrap();
    std::fs::write(
        fixture.book.join("ttl.md"),
        "# TTL 校验\n\n显式 TTL 为 60 秒。",
    )
    .unwrap();
    fixture.runtime.reply(r#"{"independent_topics_reason":"分别说明热点实验和读路径时效。","topics":[{"title":"CLOCK 热点实验","focus":"热点尾延迟及扫描长度","sources":[1]},{"title":"TTL 校验","focus":"读取时效","sources":[3]}]}"#);
    fixture
        .runtime
        .reply("## 观测口径\n\n热点实验记录平均延迟。[S1] 热点尾延迟与扫描长度一同测量。[S2]");
    fixture
        .runtime
        .reply("## 读路径\n\n显式 TTL 为 60 秒。[S1]");
    let build = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    assert_eq!(build.status, "ready");
    assert_eq!(build.sources.len(), 3);
    assert!(build.sources.iter().all(|source| source.status == "linked"));
    assert_eq!(build.pages[0].assigned_documents.len(), 2);
    assert_eq!(build.pages[1].assigned_documents.len(), 1);
    let prompts = fixture.runtime.prompts.lock().unwrap();
    assert_eq!(prompts.len(), 3);
    assert!(prompts[1].contains("热点尾延迟与 CLOCK 扫描长度一同测量"));
}

#[tokio::test]
async fn test_whole_book_build_retries_a_failed_page_independently() {
    let fixture = Fixture::new();
    std::fs::write(fixture.book.join("ttl.md"), "# 过期\n\n显式 TTL 为 60 秒。").unwrap();
    fixture.runtime.reply(r#"{"topics":[{"title":"时钟策略","focus":"替换开销","sources":[1],"related":[{"topic":2,"reason":"结合时效边界理解缓存的失效条件。"}]},{"title":"过期策略","focus":"读取时效","sources":[2]}]}"#);
    fixture.runtime.reply("## CLOCK\n\n平均延迟为 1.8 ms。[S1]");
    fixture.runtime.reply("## TTL\n\n显式 TTL 为 60 秒。[S9]");
    fixture.runtime.reply("## TTL\n\n显式 TTL 为 60 秒。[S9]");
    let partial = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    assert_eq!(partial.status, "partial");
    assert!(partial.pages[0].revision_id.is_some());
    assert!(partial.pages[1].error.as_deref().unwrap().contains("S9"));
    fixture.runtime.reply("## TTL\n\n显式 TTL 为 60 秒。[S1]");
    let recovered = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    assert_eq!(recovered.status, "ready");
    assert_eq!(recovered.pages[0].revision_id, partial.pages[0].revision_id);
    assert_eq!(fixture.runtime.prompts.lock().unwrap().len(), 5);
}

#[tokio::test]
async fn test_whole_book_starts_unwritten_topics_before_model_retry() {
    let fixture = Fixture::new();
    for (file, text) in [
        ("b.md", "# 运行状态\n\n就绪状态触发执行。"),
        ("c.md", "# 内容清理\n\n每次取消都会回收对象。"),
        ("d.md", "# 启用过程\n\n分配内存后才可读取。"),
    ] {
        std::fs::write(fixture.book.join(file), text).unwrap();
    }
    fixture.runtime.reply(r#"{"independent_topics_reason":"分别记录实验、运行、清理和启用的边界。","topics":[{"title":"实验","focus":"时钟策略","sources":[1]},{"title":"运行","focus":"状态检查","sources":[2]},{"title":"清理","focus":"取消影响","sources":[3]},{"title":"启用","focus":"启动条件","sources":[4]}]}"#);
    fixture.runtime.fail(crate::error::BrainError::LlmApiError {
        provider: "deepseek_harness".into(),
        detail: "DeepSeek Harness 在 110 秒内没有完成回答".into(),
    });
    fixture.runtime.reply("## 状态\n\n就绪后触发。[S1]");
    fixture.runtime.reply("## 回收\n\n取消时回收。[S1]");
    fixture.runtime.reply("## 条件\n\n分配内存后读取。[S1]");
    fixture
        .runtime
        .reply("## 结论\n\nCLOCK 平均延迟为 1.8 ms。[S1]");

    let build = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    assert_eq!(build.status, "ready");
    let prompts = fixture.runtime.prompts.lock().unwrap();
    assert_eq!(prompts.len(), 6);
    assert!(prompts[4].contains(r#""topic":"启用""#));
    assert!(prompts[5].contains("优先完成一篇紧凑的主题页"));
}

#[tokio::test]
async fn test_whole_book_timeout_retries_concisely_and_keeps_coalesced_text() {
    use crate::error::BrainError;
    use std::sync::atomic::Ordering;

    let f = Fixture::new();
    f.runtime.emit_fragments.store(true, Ordering::Relaxed);
    f.runtime
        .reply(r#"{"topics":[{"title":"时钟策略","focus":"替换开销","sources":[1]}]}"#);
    f.runtime.fail(BrainError::LlmApiError {
        provider: "deepseek_harness".into(),
        detail: "DeepSeek Harness 在 110 秒内没有完成回答".into(),
    });
    let draft = format!("## 证据\n\n{}[S1]", "保留格式和引用。".repeat(100));
    f.runtime.reply(&draft);
    let build = f.service.build_topic_book_now(&f.base).await.unwrap();
    assert_eq!(build.status, "ready");
    let page = f
        .service
        .store()
        .topic_page(&f.base, &build.pages[0].page_id)
        .unwrap();
    let revision = page.draft.unwrap();
    let run = f.service.store().get_agent_run(&revision.run_id).unwrap();
    assert_eq!(run.input["request_max_output_tokens"], 3_200);
    assert!(f
        .runtime
        .prompts
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .contains("优先完成一篇紧凑的主题页"));
    let events = f
        .service
        .store()
        .list_agent_run_events(&revision.run_id)
        .unwrap();
    let text = events
        .iter()
        .filter(|event| event.event_type == "run.text_delta")
        .filter_map(|event| event.payload["delta"].as_str())
        .collect::<Vec<_>>();
    assert!(text.len() <= 3);
    assert_eq!(text.concat(), draft);
}

#[tokio::test]
async fn test_whole_book_shares_one_credential_read_and_noop_build_skips_it() {
    use crate::error::BrainError;
    use crate::infra::credential_store::ProviderCredentialStore;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Default)]
    struct Credentials(AtomicUsize);

    impl ProviderCredentialStore for Credentials {
        fn get(&self, _: &str) -> Result<Option<String>, BrainError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(Some("fixture-credential".into()))
        }

        fn set(&self, _: &str, _: &str) -> Result<(), BrainError> {
            Ok(())
        }

        fn delete(&self, _: &str) -> Result<(), BrainError> {
            Ok(())
        }
    }

    let f = Fixture::new();
    std::fs::write(f.book.join("ttl.md"), "# 过期\n\n显式 TTL 为 60 秒。").unwrap();
    f.runtime.reply(
        r#"{"topics":[{"title":"时钟策略","focus":"替换开销","sources":[1],"related":[{"topic":2,"reason":"结合时效边界理解缓存的失效条件。"}]},{"title":"过期策略","focus":"读取时效","sources":[2]}]}"#,
    );
    f.runtime.reply("## CLOCK\n\n平均延迟为 1.8 ms。[S1]");
    f.runtime.reply("## TTL\n\n显式 TTL 为 60 秒。[S1]");
    let provider = f
        .service
        .store()
        .save_model_provider_profile(
            "wiki-build-test",
            "测试供应商",
            "openai-completions",
            "https://example.com/v1",
            "fixture-model",
            "local_file",
            "",
            true,
            true,
            None,
            None,
            "auto",
            0,
        )
        .unwrap();
    let profile = f.service.active_runtime_profile().unwrap();
    f.service
        .store()
        .save_runtime_profile(
            &profile.id,
            &profile.executable,
            "",
            Some(&provider.provider_id),
            true,
            profile.revision,
        )
        .unwrap();
    let credentials = Arc::new(Credentials::default());
    let service = f.service.with_credential_store(credentials.clone());
    assert_eq!(
        service.build_topic_book_now(&f.base).await.unwrap().status,
        "ready"
    );
    assert_eq!(credentials.0.load(Ordering::Relaxed), 1);
    assert_eq!(
        service.build_topic_book_now(&f.base).await.unwrap().status,
        "ready"
    );
    assert_eq!(credentials.0.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn test_whole_book_build_plans_once_and_retrieves_late_evidence_from_the_whole_book() {
    let fixture = Fixture::new();
    let long = format!(
        "# 跨区间试验\n\n入口规则。\n\n{}\n\n## 恢复条件\n\nTAIL-EVIDENCE: 仅在 checkpoint 有效时可复用 7360 个 token。",
        "仅供阅读的背景。\n\n".repeat(3900)
    );
    std::fs::write(fixture.book.join("long.md"), long).unwrap();
    std::fs::write(
        fixture.book.join("SOURCE_NAVIGATION.md"),
        format!("# 源码导航\n\n{}\n", "索引条目。".repeat(26000)),
    )
    .unwrap();
    fixture.runtime.reply(r#"{"topics":[{"title":"实验与恢复","focus":"连接性能与缓存恢复条件","sources":[1,2],"queries":["TAIL-EVIDENCE 7360","CLOCK 平均延迟"]}]}"#);
    fixture
        .runtime
        .reply("## 实验与恢复\n\nCLOCK 平均延迟为 1.8 ms。[S1] checkpoint 有效时可复用 7360 个 token。[S2]");
    let build = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    let page = fixture
        .service
        .store()
        .topic_page(&fixture.base, &build.pages[0].page_id)
        .unwrap();
    let run = page.draft.unwrap().run_id;
    let sources = fixture
        .service
        .store()
        .list_agent_run_citations(&run)
        .unwrap();
    assert_eq!(build.status, "ready");
    assert_eq!(build.total_groups, 1);
    assert_eq!(build.pages.len(), 1);
    assert!(sources
        .iter()
        .any(|source| source.content_md.contains("TAIL-EVIDENCE")));
    let prompts = fixture.runtime.prompts.lock().unwrap();
    assert_eq!(prompts.len(), 2);
    assert!(prompts[0].contains("恢复条件"));
    assert!(!prompts[0].contains(&"索引条目。".repeat(20)));
    assert_eq!(build.sources.len(), 3);
    assert!(build
        .sources
        .iter()
        .any(|source| source.path == "SOURCE_NAVIGATION.md" && source.status == "indexed"));
}

#[tokio::test]
async fn test_whole_book_keeps_section_bodies_and_the_authors_worked_example() {
    let f = Fixture::new();
    std::fs::write(
        f.book.join("avg.md"),
        r#"# 请求资源与索引

篇目：

- [资源预留与重算规则](#资源预留与重算规则)
- [抢占与回滚约束](#抢占与回滚约束)
- [在途请求进度](#在途请求进度)
- [Decode 的地址转换](#decode-的地址转换)

## 资源预留与重算规则

资源预留必须按源块的边界声明，重算会更新尚未确认的输入。

## 抢占与回滚约束

抢占时回滚当前请求的临时占用，旧状态仍用于恢复。

## 在途请求进度

在途请求进度以提交事件为准，设备准备的进度单独登记。

## Decode 的地址转换

把逻辑索引交给块表，按物理块与块内偏移计算：physical_slot = block_table[logical_block] * block_width + in_block。

## 数值算例

原书演算：总长度为 768，复用前缀为 512，剩余 256 行。记录只适用于对齐恢复。
"#,
    )
    .unwrap();
    f.runtime.reply(
        r#"{"topics":[{"title":"请求资源的准备与读取","focus":"说明资源预留、抢占与回滚、在途进度和地址转换","sources":[1],"queries":["资源预留与重算规则","抢占与回滚约束","在途请求进度","Decode 的地址转换"]}]}"#,
    );
    f.runtime
        .reply("## 证据\n\n逐节解释资源约束和地址转换。[S1]");
    let build = f.service.build_topic_book_now(&f.base).await.unwrap();
    assert_eq!(build.status, "ready");
    let prompts = f.runtime.prompts.lock().unwrap();
    assert!(prompts[1].contains("physical_slot = block_table[logical_block]"));
    assert!(prompts[1].contains("原书演算：总长度为 768"));
}

#[tokio::test]
async fn test_whole_book_repairs_plan_and_numeric_placeholders_before_saving() {
    let fixture = Fixture::new();
    fixture
        .runtime
        .reply(r#"{"topics":[{"title":"失去覆盖","focus":"说明实验","sources":[9]}]}"#);
    fixture
        .runtime
        .reply(r#"{"topics":[{"title":"时钟策略","focus":"解释对照试验","sources":[1]}]}"#);
    fixture
        .runtime
        .reply("## 对照\n\n| 指标 | 值 |\n|---|---|\n| 延迟 | 1,? |\n\n来源 [S1]。");
    fixture
        .runtime
        .reply("## 对照\n\nCLOCK 平均延迟为 1.8 ms。[S1]");
    let build = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    assert_eq!(build.status, "ready");
    assert!(build.failed_groups.is_empty());
    let page = fixture
        .service
        .store()
        .topic_page(&fixture.base, &build.pages[0].page_id)
        .unwrap();
    assert!(page.draft.unwrap().markdown.contains("1.8 ms"));
    let prompts = fixture.runtime.prompts.lock().unwrap();
    assert_eq!(prompts.len(), 4);
    assert!(prompts[1].contains("原文编号"));
    assert!(prompts[3].contains("占位"));
}

#[tokio::test]
async fn test_whole_book_publishes_current_batch_and_preserves_a_newer_edit() {
    let f = Fixture::new();
    std::fs::write(f.book.join("ttl.md"), "# 过期\n\n显式 TTL 为 60 秒。").unwrap();
    f.runtime.reply(
        r#"{"topics":[{"title":"时钟策略","focus":"替换开销","sources":[1],"related":[{"topic":2,"reason":"结合时效边界理解缓存的失效条件。"}]},{"title":"过期策略","focus":"读取时效","sources":[2]}]}"#,
    );
    f.runtime.reply("## CLOCK\n\n平均延迟为 1.8 ms。[S1]");
    f.runtime.reply("## TTL\n\n显式 TTL 为 60 秒。[S1]");
    let build = f.service.build_topic_book_now(&f.base).await.unwrap();
    assert_eq!(build.status, "ready");
    let first = &build.pages[0];
    let second = &build.pages[1];
    let edited = f
        .service
        .edit_topic_draft(
            &f.base,
            &second.page_id,
            second.revision_id.as_ref().unwrap(),
            "## TTL\n\n在测试场景下，显式 TTL 为 60 秒。[S1]",
        )
        .unwrap()
        .draft
        .unwrap();

    assert_eq!(
        f.service.publish_topic_build(&f.base, &build.id).unwrap(),
        1
    );
    assert_eq!(
        f.service.publish_topic_build(&f.base, &build.id).unwrap(),
        0
    );
    assert!(f
        .service
        .store()
        .topic_page(&f.base, &first.page_id)
        .unwrap()
        .published
        .is_some());
    let current = f
        .service
        .store()
        .topic_page(&f.base, &second.page_id)
        .unwrap();
    assert_eq!(current.draft.unwrap().id, edited.id);
    assert!(current.published.is_none());
    let hits = f
        .service
        .store()
        .search_topic_sections(&f.base, "CLOCK", false)
        .unwrap();
    assert_eq!(hits[0].page_id, first.page_id);

    std::fs::write(f.book.join("ttl.md"), "# 过期\n\n显式 TTL 为 90 秒。").unwrap();
    f.service.sync(&f.base).unwrap();
    assert!(f.service.publish_topic_build(&f.base, &build.id).is_err());
}

#[tokio::test]
async fn test_whole_book_build_marks_unreadable_files_and_continues_with_valid_material() {
    let fixture = Fixture::new();
    std::fs::write(fixture.book.join("bad.md"), b"# Invalid UTF-8 \xff").unwrap();
    fixture
        .runtime
        .reply(r#"{"topics":[{"title":"时钟策略","focus":"替换开销","sources":[1]}]}"#);
    fixture.runtime.reply("## CLOCK\n\n平均延迟为 1.8 ms。[S1]");
    let build = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    assert_eq!(build.status, "partial");
    assert!(build.pages[0].revision_id.is_some());
    assert!(build.sources.iter().any(|source| source.path == "bad.md"
        && source.status == "unreadable"
        && source.error.is_some()));
}

#[tokio::test]
async fn test_whole_book_build_deduplicates_launch_and_can_resume_after_restart() {
    let fixture = Fixture::new();
    let store = fixture.service.store();
    let claim = store
        .claim_topic_book_build(&fixture.base)
        .unwrap()
        .unwrap();
    assert!(store
        .claim_topic_book_build(&fixture.base)
        .unwrap()
        .is_none());
    assert!(fixture.service.sync(&fixture.base).is_err());
    store.recover_interrupted_tasks().unwrap();
    let recovered = store.topic_book_build(&fixture.base).unwrap().unwrap();
    assert_eq!(recovered.id, claim.attempt_id);
    assert_eq!(recovered.status, "partial");
    assert_eq!(recovered.phase, "interrupted");
    fixture
        .runtime
        .reply(r#"{"topics":[{"title":"时钟策略","focus":"热点实验","sources":[1]}]}"#);
    fixture
        .runtime
        .reply("## 热点实验\n\nCLOCK 平均延迟为 1.8 ms。[S1]");
    let next = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    assert_eq!(next.status, "ready");
    assert_ne!(next.id, claim.attempt_id);
    std::fs::write(
        fixture.book.join("new.md"),
        "# 新材料\n\n默认 TTL 为 30 秒。",
    )
    .unwrap();
    fixture.service.sync(&fixture.base).unwrap();
    assert!(
        store
            .topic_book_build(&fixture.base)
            .unwrap()
            .unwrap()
            .outdated
    );
}

#[tokio::test]
async fn test_whole_book_build_stops_at_a_checkpoint_and_can_start_again() {
    let fixture = Fixture::new();
    let store = fixture.service.store();
    let claim = store
        .claim_topic_book_build(&fixture.base)
        .unwrap()
        .unwrap();
    assert!(store
        .request_topic_book_stop(&fixture.base, "different-attempt")
        .is_err());
    let pending = store
        .request_topic_book_stop(&fixture.base, &claim.attempt_id)
        .unwrap();
    assert_eq!(pending.id, claim.attempt_id);
    assert!(pending.stop_requested);
    assert!(store
        .claim_topic_book_build(&fixture.base)
        .unwrap()
        .is_none());
    let stopped = fixture
        .service
        .run_topic_book_build(&fixture.base, claim)
        .await
        .unwrap();
    assert_eq!(stopped.phase, "stopped");
    assert_eq!(stopped.status, "partial");
    assert!(fixture.runtime.prompts.lock().unwrap().is_empty());

    fixture
        .runtime
        .reply(r#"{"topics":[{"title":"时钟策略","focus":"热点实验","sources":[1]}]}"#);
    fixture
        .runtime
        .reply("## 热点实验\n\nCLOCK 平均延迟为 1.8 ms。[S1]");
    let resumed = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    assert_eq!(resumed.status, "ready");
    assert!(!resumed.stop_requested);
}

#[tokio::test]
async fn test_published_wiki_keeps_a_cross_topic_reading_path() {
    let fixture = Fixture::new();
    std::fs::write(fixture.book.join("ttl.md"), "# 过期\n\n显式 TTL 为 60 秒。").unwrap();
    fixture.runtime.reply(r#"{"overview":"读完容量淘汰后，对照过期条件。","topics":[{"title":"时钟策略","focus":"替换开销","sources":[1],"related":[{"topic":2,"reason":"在容量之外再看读取时效的限制。"}]},{"title":"过期策略","focus":"读取时效","sources":[2]}]}"#);
    fixture.runtime.reply("## CLOCK\n\n平均延迟为 1.8 ms。[S1]");
    fixture.runtime.reply("## TTL\n\n显式 TTL 为 60 秒。[S1]");
    let result = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    assert_eq!(result.status, "ready");
    let first_page = &result.pages[0];
    let before = fixture
        .service
        .store()
        .topic_wiki_index(&fixture.base)
        .unwrap();
    assert!(before.pages.is_empty() && before.overview.is_none());
    assert_eq!(before.builds[0].linked_sources, 2);
    fixture
        .service
        .publish_topic(
            &fixture.base,
            &first_page.page_id,
            first_page.revision_id.as_ref().unwrap(),
        )
        .unwrap();
    let partial = fixture
        .service
        .store()
        .topic_wiki_index(&fixture.base)
        .unwrap();
    assert!(partial.overview.is_none());
    assert_eq!(partial.issues[0].code, "unpublished_link");
    assert_eq!(
        fixture
            .service
            .publish_topic_build(&fixture.base, &result.id)
            .unwrap(),
        1
    );
    let index = fixture
        .service
        .store()
        .topic_wiki_index(&fixture.base)
        .unwrap();
    assert_eq!(
        index.overview.as_deref(),
        Some("读完容量淘汰后，对照过期条件。")
    );
    assert!(index.issues.is_empty());
    assert_eq!(index.publications.len(), 2);
    assert_eq!(index.pages[0].summary, "平均延迟为 1.8 ms。");
    let first = fixture
        .service
        .store()
        .topic_page(&fixture.base, &result.pages[0].page_id)
        .unwrap();
    let second = fixture
        .service
        .store()
        .topic_page(&fixture.base, &result.pages[1].page_id)
        .unwrap();
    let first = serde_json::to_value(first).unwrap();
    let second = serde_json::to_value(second).unwrap();
    assert_eq!(
        first["published"]["links"][0]["page_id"],
        result.pages[1].page_id
    );
    assert_eq!(second["backlinks"][0]["page_id"], result.pages[0].page_id);

    let database = fixture.book.parent().unwrap().join("topic-pages.db");
    let store = crate::infra::sqlite_store::SqliteStore::new(&database).unwrap();
    let exported = crate::core::book_wiki_export::export_book_wiki(
        &store,
        &fixture.base,
        crate::core::book_wiki_export::BookWikiExportFormat::Markdown,
    )
    .unwrap();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(exported.path).unwrap()).unwrap();
    let read = |zip: &mut zip::ZipArchive<std::fs::File>, path: &str| {
        use std::io::Read;
        let mut markdown = String::new();
        zip.by_name(path)
            .unwrap()
            .read_to_string(&mut markdown)
            .unwrap();
        markdown
    };
    let page = read(&mut zip, &format!("topics/{}.md", first_page.page_id));
    assert!(page.contains(&format!("[过期策略]({}.md)", result.pages[1].page_id)));
    assert!(read(&mut zip, "topics/INDEX.md").contains("平均延迟为 1.8 ms。"));
    assert!(read(&mut zip, "topics/LOG.md").contains("publish | [时钟策略]"));
}

#[tokio::test]
async fn test_rebuild_reads_current_wiki_and_keeps_the_existing_topic_identity() {
    let fixture = Fixture::new();
    fixture
        .runtime
        .reply(r#"{"topics":[{"title":"替换策略的成本","focus":"平均延迟","sources":[1]}]}"#);
    fixture
        .runtime
        .reply("## 同口径比较\n\n本实验报告 CLOCK 平均延迟 1.8 ms。[S1]");
    let original = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .service
            .publish_topic_build(&fixture.base, &original.id)
            .unwrap(),
        1
    );
    std::fs::write(
        fixture.book.join("avg.md"),
        "# 受控实验\n\n同配置新一次测量，CLOCK 平均延迟为 1.7 ms。 ",
    )
    .unwrap();
    let existing_id = &original.pages[0].page_id;
    fixture
        .runtime
        .reply(r#"{"sources":[{"source":1,"reason":"同口径下新增一次 1.7 ms 的观测"}]}"#);
    fixture
        .runtime
        .reply("## 更新的实验\n\n本次 CLOCK 平均延迟为 1.7 ms。[S1]");
    let updated = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap();
    assert_eq!(updated.pages[0].page_id, *existing_id);
    assert_eq!(updated.pages[0].title, "替换策略的成本");
    let prompts = fixture.runtime.prompts.lock().unwrap();
    assert!(prompts[2].contains(existing_id));
    assert!(prompts[2].contains("本实验报告 CLOCK 平均延迟 1.8 ms。"));
}

#[tokio::test]
async fn test_unlinked_multi_topic_plan_stops_before_generating_disconnected_drafts() {
    let fixture = Fixture::new();
    std::fs::write(fixture.book.join("ttl.md"), "# 过期\n\n显式 TTL 为 60 秒。").unwrap();
    let invalid = r#"{"topics":[{"title":"容量","focus":"驱逐条件","sources":[1]},{"title":"时效","focus":"过期条件","sources":[2]}]}"#;
    fixture.runtime.reply(invalid);
    fixture.runtime.reply(invalid);
    let error = fixture
        .service
        .build_topic_book_now(&fixture.base)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("缺少阅读关系"));
    assert!(fixture
        .service
        .store()
        .list_topic_pages(&fixture.base)
        .unwrap()
        .is_empty());
    assert_eq!(fixture.runtime.prompts.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn test_source_change_preserves_unaffected_page_and_refreshes_both_dependents() {
    let f = Fixture::new();
    std::fs::write(f.book.join("p99.md"), "# 尾延迟\n\n排队条件决定 P99。").unwrap();
    std::fs::write(f.book.join("ttl.md"), "# 时效\n\nTTL 为 90 秒。").unwrap();
    f.runtime.reply(r#"{"topics":[{"title":"均值","focus":"CLOCK 平均延迟","sources":[1],"related":[{"topic":2,"reason":"核对尾部约束"}]},{"title":"尾部","focus":"比较均值和尾延迟","sources":[2],"queries":["平均延迟"],"related":[{"topic":3,"reason":"核对过期语义"}]},{"title":"时效","focus":"TTL","sources":[3]}]}"#);
    f.runtime
        .reply("## 平均延迟\n\nCLOCK 平均延迟为 1.8 ms。[S1]");
    f.runtime
        .reply("## 尾延迟\n\n排队条件决定 P99。[S1] 平均延迟为 1.8 ms。[S2]");
    f.runtime.reply("## 时效\n\nTTL 为 90 秒。[S1]");
    let first = f.service.build_topic_book_now(&f.base).await.unwrap();
    assert_eq!(
        f.service.publish_topic_build(&f.base, &first.id).unwrap(),
        3
    );

    std::fs::write(
        f.book.join("avg.md"),
        "# 受控实验\n\nCLOCK 平均延迟为 1.7 ms。",
    )
    .unwrap();
    f.runtime.reply(
        r#"{"sources":[{"source":1,"pages":[],"new_pages":[],"reason":"更新原有实验口径"}]}"#,
    );
    f.runtime
        .reply("## 平均延迟\n\nCLOCK 平均延迟为 1.7 ms。[S1]");
    f.runtime
        .reply("## 尾延迟\n\n排队条件决定 P99。[S1] 平均延迟为 1.7 ms。[S2]");
    let update = f.service.build_topic_book_now(&f.base).await.unwrap();
    assert_eq!(update.status, "ready");
    assert_ne!(update.pages[0].revision_id, first.pages[0].revision_id);
    assert_ne!(update.pages[1].revision_id, first.pages[1].revision_id);
    assert_eq!(update.pages[2].revision_id, first.pages[2].revision_id);
    let changes = update.maintenance.as_ref().unwrap();
    assert_eq!(changes.reused_pages, 1);
    assert_eq!(
        changes.changes[0].page_ids,
        vec![
            first.pages[0].page_id.clone(),
            first.pages[1].page_id.clone(),
        ]
    );
    assert_eq!(
        f.service.publish_topic_build(&f.base, &update.id).unwrap(),
        2
    );
    let repeated = f.service.build_topic_book_now(&f.base).await.unwrap();
    assert_eq!(repeated.pages[2].revision_id, first.pages[2].revision_id);
    assert_eq!(repeated.sources, update.sources);
    assert_eq!(f.runtime.prompts.lock().unwrap().len(), 7);
    let history = f.service.store().topic_wiki_index(&f.base).unwrap().builds;
    assert_eq!(history[0].written_pages, 0);
    assert_eq!(history[1].written_pages, 2);
    assert_eq!(
        history[1].maintenance.as_ref().unwrap().changes[0].path,
        "avg.md"
    );
}

#[tokio::test]
async fn test_new_source_extends_the_wiki_without_recompiling_the_original_page() {
    let f = Fixture::new();
    f.runtime
        .reply(r#"{"topics":[{"title":"平均延迟","focus":"本次试验的均值","sources":[1]}]}"#);
    f.runtime.reply("## 试验\n\nCLOCK 平均延迟为 1.8 ms。[S1]");
    let first = f.service.build_topic_book_now(&f.base).await.unwrap();
    let parent = &first.pages[0];
    f.service.publish_topic_build(&f.base, &first.id).unwrap();
    std::fs::write(
        f.book.join("failover.md"),
        "# 故障恢复\n\n快照损坏时挂起写入。",
    )
    .unwrap();
    std::fs::write(f.book.join("note.md"), "# 样式\n\n只更换排版字体。").unwrap();
    let routed = serde_json::json!({"sources":[
        {"source":1,"reason":"给出损坏时的恢复条件","new_pages":[{
            "title":"故障恢复的边界","focus":"快照损坏后的写入条件",
            "related":[{"page_id":parent.page_id,"reason":"对照正常路径的实验条件"}]
        }]},
        {"source":2,"reason":"只涉及排版，没有技术增量"}
    ]});
    f.runtime.reply(&routed.to_string());
    f.runtime
        .reply("## 自动恢复的限制\n\n快照损坏时挂起写入。[S1]");
    let next = f.service.build_topic_book_now(&f.base).await.unwrap();
    assert_eq!(next.status, "ready");
    assert_eq!(next.pages.len(), 2);
    assert_eq!(next.pages[0].revision_id, parent.revision_id);
    let maintenance = next.maintenance.as_ref().unwrap();
    assert_eq!(maintenance.changes[0].disposition, "add");
    assert_eq!(maintenance.changes[1].disposition, "no_change");
    assert_eq!(f.service.publish_topic_build(&f.base, &next.id).unwrap(), 1);
    let page = f
        .service
        .store()
        .topic_page(&f.base, &parent.page_id)
        .unwrap();
    assert_eq!(page.backlinks[0].page_id, next.pages[1].page_id);
    assert_eq!(f.runtime.prompts.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn test_deleted_source_leaves_its_published_page_for_review() {
    let f = Fixture::new();
    std::fs::write(f.book.join("ttl.md"), "# 时效\n\nTTL 为 90 秒。").unwrap();
    f.runtime.reply(r#"{"topics":[{"title":"均值","focus":"延迟","sources":[1],"related":[{"topic":2,"reason":"核对时效实验"}]},{"title":"时效","focus":"TTL","sources":[2]}]}"#);
    f.runtime
        .reply("## 平均延迟\n\nCLOCK 平均延迟为 1.8 ms。[S1]");
    f.runtime.reply("## 时效\n\nTTL 为 90 秒。[S1]");
    let first = f.service.build_topic_book_now(&f.base).await.unwrap();
    f.service.publish_topic_build(&f.base, &first.id).unwrap();
    std::fs::remove_file(f.book.join("ttl.md")).unwrap();
    let next = f.service.build_topic_book_now(&f.base).await.unwrap();
    assert_eq!(next.status, "partial");
    assert_eq!(next.pages[0].revision_id, first.pages[0].revision_id);
    assert!(next.pages[1]
        .error
        .as_deref()
        .unwrap()
        .contains("依据已移除"));
    assert_eq!(
        next.maintenance.as_ref().unwrap().changes[0].change,
        "removed"
    );
    let page = f
        .service
        .store()
        .topic_page(&f.base, &first.pages[1].page_id)
        .unwrap();
    assert!(page.page.needs_review);
    assert_eq!(
        page.published.unwrap().id,
        *first.pages[1].revision_id.as_ref().unwrap()
    );
    std::fs::remove_file(f.book.join("avg.md")).unwrap();
    let no_sources = f.service.build_topic_book_now(&f.base).await.unwrap();
    assert_eq!(no_sources.status, "partial");
    assert!(no_sources.sources.is_empty());
    assert!(no_sources.pages.iter().all(|page| page.error.is_some()));
    assert_eq!(f.runtime.prompts.lock().unwrap().len(), 3);
}

#[tokio::test]
async fn test_source_update_does_not_overwrite_an_independently_edited_draft() {
    let f = Fixture::new();
    f.runtime
        .reply(r#"{"topics":[{"title":"平均延迟","focus":"本次试验的均值","sources":[1]}]}"#);
    f.runtime.reply("## 试验\n\nCLOCK 平均延迟为 1.8 ms。[S1]");
    let first = f.service.build_topic_book_now(&f.base).await.unwrap();
    let record = &first.pages[0];
    let draft = f
        .service
        .edit_topic_draft(
            &f.base,
            &record.page_id,
            record.revision_id.as_deref().unwrap(),
            "## 实验数据待复核\n\n初稿观测值为 1.8 ms。[S1]",
        )
        .unwrap()
        .draft
        .unwrap();
    std::fs::write(
        f.book.join("avg.md"),
        "# 编辑后实验\n\n同配置重新计得 1.7 ms。",
    )
    .unwrap();
    f.runtime
        .reply(r#"{"sources":[{"source":1,"reason":"更新测量结果"}]}"#);
    let next = f.service.build_topic_book_now(&f.base).await.unwrap();
    assert_eq!(next.status, "partial");
    assert!(next.pages[0].error.as_deref().unwrap().contains("独立编辑"));
    assert_eq!(f.service.publish_topic_build(&f.base, &next.id).unwrap(), 0);
    let saved = f
        .service
        .store()
        .topic_page(&f.base, &record.page_id)
        .unwrap();
    assert_eq!(saved.draft.unwrap().id, draft.id);
    assert_eq!(f.runtime.prompts.lock().unwrap().len(), 3);
}
