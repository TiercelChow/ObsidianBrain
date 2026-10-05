use super::*;
use rusqlite::params;

fn database() -> (tempfile::TempDir, SqliteStore) {
    let dir = tempfile::tempdir().unwrap();
    let db = SqliteStore::new(&dir.path().join("home.db")).unwrap();
    (dir, db)
}
fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 4).unwrap()
}
fn task(db: &SqliteStore, id: &str, role: &str, status: &str, end: &str, archived: bool) {
    db.with_connection(|conn| {
        conn.execute("INSERT OR IGNORE INTO task_documents(path,document_kind,revision,content_hash,indexed_at) VALUES('doc','long_task',1,'h','2026-10-04')", [])?;
        conn.execute("INSERT INTO task_nodes(id,root_id,parent_id,storage_path,kind,role,title,status,importance,start_date,end_date,position,created_at,updated_at,revision,archived_at) VALUES(?1,?2,?3,'doc','long',?4,?1,?5,'normal','2026-10-01',?6,0,'2026-10-01','2026-10-04',1,?7)", params![id, if role == "root" { id } else { "root" }, if role == "root" { None } else { Some("root") }, role, status, end, archived.then_some("2026-10-04")])?;
        Ok(())
    }).unwrap();
}

#[test]
fn test_overview_empty_has_known_counts_and_monday_week_start() {
    let (_dir, db) = database();
    let result = overview(&db, date()).unwrap();
    assert_eq!(result.week_start, "2026-09-28");
    assert_eq!(result.tasks.data.unwrap().active_count, 0);
    assert_eq!(
        result.storage.data.unwrap().cache_limit_bytes,
        256 * 1024 * 1024
    );
}

#[test]
fn test_overview_task_counts_are_unbounded_root_only_and_child_risk_is_preserved() {
    let (_dir, db) = database();
    task(&db, "root", "root", "in_progress", "2026-10-04", false);
    task(&db, "child", "subtask", "blocked", "2026-10-02", false);
    task(&db, "closed", "root", "completed", "2026-10-01", false);
    task(&db, "archived", "root", "planned", "2026-10-01", true);
    for n in 0..8 {
        task(
            &db,
            &format!("overdue-{n}"),
            "root",
            "planned",
            "2026-10-03",
            false,
        );
    }
    let result = overview(&db, date()).unwrap().tasks.data.unwrap();
    assert_eq!(result.active_count, 9);
    assert_eq!(result.overdue_count, 8);
    assert_eq!(result.today_count, 1);
    assert_eq!(result.items.len(), 4);
    // Remove overdue roots so the selected parent can be inspected.
    db.with_connection(|conn| {
        conn.execute("DELETE FROM task_nodes WHERE id LIKE 'overdue-%'", [])?;
        Ok(())
    })
    .unwrap();
    let root = overview(&db, date())
        .unwrap()
        .tasks
        .data
        .unwrap()
        .items
        .remove(0);
    assert_eq!(root.child_risk_id.as_deref(), Some("child"));
}

#[test]
fn test_overview_reading_uses_progress_not_added_date_and_skips_unread_removed() {
    let (_dir, db) = database();
    db.with_connection(|conn| {
        for (id, progress, shelf, added) in [("old", r#"{"position":0.9,"updatedAt":100,"lastFile":"chapter.md"}"#, "active", 999), ("recent", r#"{"position":12,"pageCount":80,"updatedAt":200}"#, "active", 1), ("unread", "null", "active", 1000), ("removed", r#"{"position":1,"updatedAt":300}"#, "removed", 10)] {
            conn.execute("INSERT INTO reader_books(id,path,kind,name,added_at,progress_json,shelf_state) VALUES(?1,?1,'folder',?1,?2,?3,?4)", params![id, added, progress, shelf])?;
        }
        Ok(())
    }).unwrap();
    let books = overview(&db, date()).unwrap().reading.data.unwrap();
    assert_eq!(
        books.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
        vec!["recent", "old"]
    );
}

#[test]
fn test_overview_section_failure_does_not_zero_other_sections() {
    let (_dir, db) = database();
    db.with_connection(|conn| {
        conn.execute("DROP TABLE memos", [])?;
        Ok(())
    })
    .unwrap();
    let result = overview(&db, date()).unwrap();
    assert!(result.memos.data.is_none());
    assert!(result.memos.error.is_some());
    assert!(result.tasks.data.is_some());
}

#[test]
fn test_overview_wiki_counts_waiting_review_once_and_bounds_activity() {
    let (_dir, db) = database();
    db.with_connection(|conn| {
        conn.execute("INSERT INTO reader_books(id,path,kind,name,added_at) VALUES('book','/test','folder','测试书',1)", [])?;
        conn.execute("INSERT INTO knowledge_bases(id,book_id,created_at,updated_at,compile_state,compile_phase) VALUES('base','book','2026-10-01','2026-10-04','compiling','waiting_review')", [])?;
        for n in 0..7 {
            conn.execute("INSERT INTO knowledge_change_sets(id,knowledge_base_id,title,idempotency_key,created_at) VALUES(?1,'base','待审核',?1,'2026-10-04')", [format!("review-{n}")])?;
        }
        for (id,status,artifact) in [("running","running","pending"),("queued","queued","not_requested"),("completed","completed","failed"),("failed","failed","not_requested")] {
            conn.execute("INSERT INTO knowledge_tasks(id,knowledge_base_id,title,status,artifact_state,created_at,updated_at) VALUES(?1,'base',?1,?2,?3,'2026-10-04','2026-10-04')",params![id,status,artifact])?;
        }
        Ok(())
    }).unwrap();
    let result = overview(&db, date()).unwrap().wiki.data.unwrap();
    assert_eq!(result.running_count, 2);
    assert_eq!(result.review_count, 7);
    assert_eq!(result.failed_count, 2);
    assert_eq!(result.items.len(), 5);
    assert!(!result.items.iter().any(|i| i.kind == "compile"));
    assert_eq!(
        result
            .items
            .iter()
            .find(|i| i.id == "completed")
            .unwrap()
            .artifact_state
            .as_deref(),
        Some("failed")
    );
    db.with_connection(|conn| {
        conn.execute("UPDATE knowledge_bases SET lifecycle='archived'", [])?;
        Ok(())
    })
    .unwrap();
    let archived = overview(&db, date()).unwrap().wiki.data.unwrap();
    assert_eq!(
        (
            archived.running_count,
            archived.review_count,
            archived.failed_count
        ),
        (0, 0, 0)
    );
    assert!(archived.items.is_empty());
}

#[test]
fn test_overview_memo_week_count_is_not_latest_three_and_excerpt_is_bounded() {
    let (_dir, db) = database();
    db.with_connection(|conn| {
        for n in 0..9 {
            conn.execute("INSERT INTO memos(id,timestamp,date,content,images,tags,file_path,created_at) VALUES(?1,?2,?3,?4,'[\"Timeline/images/test.png\"]','[\"思考\",\"阅读\"]','',?2)",params![format!("memo-{n}"),format!("2026-10-04T08:00:0{n}Z"),if n==0 {"2026-09-27"} else {"2026-10-04"},"长".repeat(10000)])?;
        }
        Ok(())
    }).unwrap();
    let result = overview(&db, date()).unwrap().memos.data.unwrap();
    assert_eq!(
        (result.total_count, result.week_count, result.items.len()),
        (9, 8, 3)
    );
    assert_eq!(result.items[0].excerpt.chars().count(), 240);
    assert_eq!(
        result.items[0].thumbnail.as_deref(),
        Some("Timeline/images/test.png")
    );
}
