use super::tests::FakeRuntime;
use super::*;
use crate::infra::sqlite_store::SqliteStore;
use crate::models::book_wiki::{BookKind, ReaderBook};
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};

struct ReviewRuntime(Arc<AtomicBool>, Arc<AtomicBool>);

#[async_trait]
impl AgentRuntime for ReviewRuntime {
    async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
        if self.1.load(Ordering::SeqCst) {
            return Err(BrainError::Internal("测试运行中断".into()));
        }
        if self.0.load(Ordering::SeqCst) {
            return Ok(serde_json::json!({"entries":[],"no_material_reason":"本批没有新增知识，但原主题仍需人工复核。"}).to_string());
        }
        FakeRuntime.prompt(request).await
    }
}

#[tokio::test]
async fn test_explicit_source_review_retries_after_no_material_without_reusing_old_result() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("book");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("chapter.md"), "# 章节\n原始机制。").unwrap();
    let store = BookWikiStore::new(Arc::new(
        SqliteStore::new(&dir.path().join("wiki.db")).unwrap(),
    ));
    store
        .save_reader_books(&[ReaderBook {
            id: "source-review".into(),
            path: root.to_string_lossy().into(),
            kind: BookKind::Folder,
            name: "来源复核".into(),
            description: String::new(),
            category: String::new(),
            added_at: 1,
            progress: None,
        }])
        .unwrap();
    let no_material = Arc::new(AtomicBool::new(false));
    let failure = Arc::new(AtomicBool::new(false));
    let service = BookWikiService::new(
        store.clone(),
        Arc::new(ReviewRuntime(no_material.clone(), failure.clone())),
    );
    let base = service
        .initialize_and_sync("source-review")
        .unwrap()
        .knowledge_base;
    let first = service.compile_semantic_wiki(&base.id).await.unwrap();
    store
        .resolve_change_set(&first.change_set.id, true, "")
        .unwrap();
    let entry_id = &first.change_set.changes[0].object_id;
    std::fs::write(root.join("chapter.md"), "# 章节\n更新后的机制与条件。").unwrap();
    service.sync(&base.id).unwrap();
    no_material.store(true, Ordering::SeqCst);
    let checked = service.compile_semantic_wiki(&base.id).await.unwrap();
    assert_eq!(checked.change_set.status, "applied");
    assert!(service.prepare_semantic_compile(&base.id).is_err());
    let stale = store.get_entry(entry_id).unwrap();
    assert_eq!(stale.entry.status, "stale");
    let fp = service
        .semantic_compile_context(&base.id)
        .unwrap()
        .fingerprint;

    service.prepare_source_review_compile(&base.id).unwrap();
    assert!(service.prepare_source_review_compile(&base.id).is_err());
    assert!(store
        .list_source_spans_pending_compile(&base.id, &fp)
        .unwrap()
        .is_empty());
    let retry_empty = service
        .execute_prepared_semantic_compile(&base.id, true)
        .await
        .unwrap();
    assert_ne!(retry_empty.change_set.id, checked.change_set.id);
    assert_eq!(store.get_entry(entry_id).unwrap().revision, stale.revision);
    assert!(service.prepare_semantic_compile(&base.id).is_err());

    no_material.store(false, Ordering::SeqCst);
    failure.store(true, Ordering::SeqCst);
    service.prepare_source_review_compile(&base.id).unwrap();
    assert!(service
        .execute_prepared_semantic_compile(&base.id, true)
        .await
        .is_err());
    assert_eq!(store.get_entry(entry_id).unwrap().revision, stale.revision);
    assert!(store
        .list_source_spans_pending_compile(&base.id, &fp)
        .unwrap()
        .is_empty());
    failure.store(false, Ordering::SeqCst);
    service.prepare_source_review_compile(&base.id).unwrap();
    let retry = service
        .execute_prepared_semantic_compile(&base.id, true)
        .await
        .unwrap();
    assert_eq!(retry.change_set.status, "proposed");
    assert_ne!(retry.change_set.id, retry_empty.change_set.id);
    assert_eq!(store.get_entry(entry_id).unwrap().entry.status, "stale");
    assert!(service.prepare_source_review_compile(&base.id).is_err());
    assert!(
        store.begin_source_review_compile(&base.id, 1).is_err(),
        "transactional claim must reject pending reviews too"
    );
    store
        .resolve_change_set(&retry.change_set.id, true, "重新检查当前依据")
        .unwrap();
    assert_eq!(store.get_entry(entry_id).unwrap().source_impact_count, 0);
    assert!(service.prepare_source_review_compile(&base.id).is_err());
    store.sync_markdown_sources(&base.id, &[]).unwrap();
    assert!(
        service.prepare_source_review_compile(&base.id).is_err(),
        "missing evidence must not be replaced with old source versions"
    );
    assert_eq!(store.get_entry(entry_id).unwrap().entry.status, "stale");
}
