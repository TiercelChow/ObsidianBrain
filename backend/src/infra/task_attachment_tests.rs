use super::*;

fn fixture() -> (tempfile::TempDir, TaskAttachmentStore) {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(SqliteStore::new(&dir.path().join("brain.db")).unwrap());
    db.with_connection(|c| {
        c.execute_batch("INSERT INTO task_documents(path,document_kind,revision,content_hash,indexed_at) VALUES ('test','long_task',1,'test','now');
        INSERT INTO task_nodes(id,root_id,storage_path,kind,role,title,description,status,importance,start_date,end_date,position,created_at,updated_at,revision) VALUES ('task','task','test','long','root','test','','planned','normal','2026-01-01','2026-12-31',0,'now','now',1);
        INSERT INTO task_progress(id,root_id,task_id,storage_path,recorded_at,note,created_at) VALUES ('progress','task','task','test','now','note','now');")?;
        Ok(())
    }).unwrap();
    (dir, TaskAttachmentStore::new(db))
}

#[test]
fn test_attachment_scope_and_upload_lifecycle() {
    let (_dir, store) = fixture();
    assert!(store.reserve("missing", None, "a.txt").is_err());
    assert!(store.reserve("task", Some("wrong"), "a.txt").is_err());
    let draft = store
        .reserve("task", Some("progress"), "中文资料.txt")
        .unwrap();
    assert!(store.list("task", Some("progress")).unwrap().is_empty());
    assert!(store.get(&draft.id).is_err());
    std::fs::create_dir_all(store.directory()).unwrap();
    std::fs::write(store.file_path(&draft.id).unwrap(), b"hello").unwrap();
    store.finish(&draft.id, 5).unwrap();
    assert_eq!(
        store.list("task", Some("progress")).unwrap()[0].size_bytes,
        5
    );
    assert!(store.list("task", None).unwrap().is_empty());
    // Ordinary document edits rewrite the task rows, not attachment identity.
    store
        .db
        .with_connection(|c| {
            c.execute_batch("BEGIN; CREATE TEMP TABLE keep_nodes AS SELECT * FROM task_nodes; CREATE TEMP TABLE keep_progress AS SELECT * FROM task_progress;
              DELETE FROM task_documents WHERE path='test';
              INSERT INTO task_documents(path,document_kind,revision,content_hash,indexed_at) VALUES ('test','long_task',2,'edited','now');
              INSERT INTO task_nodes SELECT * FROM keep_nodes; INSERT INTO task_progress SELECT * FROM keep_progress;
              DROP TABLE keep_nodes; DROP TABLE keep_progress; COMMIT;")?;
            Ok(())
        })
        .unwrap();
    assert_eq!(store.get(&draft.id).unwrap().filename, "中文资料.txt");
    store.delete(&draft.id).unwrap();
    assert!(!store.file_path(&draft.id).unwrap().exists());
    assert!(store.get(&draft.id).is_err());
}

#[test]
fn test_attachment_limits_safe_paths_and_interrupted_cleanup() {
    let (_dir, store) = fixture();
    assert!(store.reserve("task", None, "../escape").is_err());
    assert!(store.file_path("../../outside").is_err());
    let pending = store.reserve("task", None, "partial.txt").unwrap();
    std::fs::create_dir_all(store.directory()).unwrap();
    std::fs::write(store.file_path(&pending.id).unwrap(), b"partial").unwrap();
    store.recover_interrupted().unwrap();
    assert!(!store.file_path(&pending.id).unwrap().exists());
    for n in 0..20 {
        store.reserve("task", None, &format!("{n}.txt")).unwrap();
    }
    assert!(store.reserve("task", None, "21.txt").is_err());
}

#[test]
fn test_attachment_retry_key_reuses_completed_upload_and_is_scope_bound() {
    let (_dir, store) = fixture();
    let (item, new) = store
        .reserve_with_key("task", Some("progress"), "note.txt", "retry-key")
        .unwrap();
    assert!(new);
    assert!(store
        .reserve_with_key("task", Some("progress"), "note.txt", "retry-key")
        .is_err());
    store.finish(&item.id, 10).unwrap();
    let (retry, new) = store
        .reserve_with_key("task", Some("progress"), "note.txt", "retry-key")
        .unwrap();
    assert!(!new);
    assert_eq!(retry.id, item.id);
    assert_eq!(store.list("task", Some("progress")).unwrap().len(), 1);
    assert!(store
        .reserve_with_key("task", Some("progress"), "other.txt", "retry-key")
        .is_err());
    let (other, _) = store
        .reserve_with_key("task", None, "note.txt", "retry-key")
        .unwrap();
    assert_ne!(item.id, other.id);
}

#[test]
fn test_stale_cleanup_does_not_remove_a_retried_active_file() {
    let (_dir, store) = fixture();
    let (item, _) = store
        .reserve_with_key("task", None, "a.txt", "stable")
        .unwrap();
    std::fs::create_dir_all(store.directory()).unwrap();
    let path = store.file_path(&item.id).unwrap();
    std::fs::write(&path, b"saved").unwrap();
    store.finish(&item.id, 5).unwrap();
    // Another cleanup worker may have collected this ID before it was retried.
    store.cleanup_one(&item.id).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"saved");
    store
        .db
        .with_connection(|c| {
            c.execute(
                "UPDATE task_attachments SET state='deleted' WHERE id=?1",
                [&item.id],
            )?;
            Ok(())
        })
        .unwrap();
    store.cleanup_one(&item.id).unwrap();
    assert!(!path.exists());
}
