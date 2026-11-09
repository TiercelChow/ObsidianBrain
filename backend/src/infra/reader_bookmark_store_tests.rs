use super::*;
use crate::infra::book_wiki_store::BookWikiStore;
use crate::models::book_wiki::{BookKind, ReaderBook};

fn setup() -> (ReaderBookmarkStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(SqliteStore::new(&dir.path().join("bookmarks.db")).unwrap());
    BookWikiStore::new(db.clone())
        .save_reader_books(&["a", "b"].map(|id| ReaderBook {
            id: id.into(),
            path: format!("/books/{id}"),
            kind: BookKind::Folder,
            name: id.into(),
            description: String::new(),
            category: String::new(),
            added_at: 1,
            progress: None,
        }))
        .unwrap();
    (ReaderBookmarkStore::new(db), dir)
}

fn draft() -> BookmarkWrite {
    serde_json::from_value(serde_json::json!({"book_id":"a","file_path":"/books/a/one.md","title":"重点","note_md":"## 笔记\n\n$x_i$ 与 **重点**","location":{"kind":"md","ratio":0.4,"block_index":3,"text":"目标段落","offset":0.2}})).unwrap()
}

#[test]
fn test_bookmarks_crud_persistence_isolation_and_optimistic_lock() {
    let (store, dir) = setup();
    let saved = store.save(draft()).unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(store.list("a").unwrap().len(), 1);
    assert!(store.list("b").unwrap().is_empty());
    let mut edit = draft();
    edit.id = Some(saved.id.clone());
    edit.revision = Some(1);
    edit.note_md = "更新".into();
    let updated = store.save(edit.clone()).unwrap();
    assert_eq!(updated.revision, 2);
    assert!(matches!(
        store.save(edit),
        Err(BrainError::KnowledgeConflict(_))
    ));
    assert!(store.delete("b", &saved.id, 2).is_err());
    assert!(store.delete("a", &saved.id, 1).is_err());
    let reopened = ReaderBookmarkStore::new(Arc::new(
        SqliteStore::new(&dir.path().join("bookmarks.db")).unwrap(),
    ));
    assert_eq!(reopened.list("a").unwrap()[0].note_md, "更新");
    store.delete("a", &saved.id, 2).unwrap();
    assert!(store.list("a").unwrap().is_empty());
}

#[test]
fn test_bookmarks_validate_paths_types_positions_and_preserve_data_on_failure() {
    let (store, _dir) = setup();
    let original = store.save(draft()).unwrap();
    for path in [
        "/books/ab/out.md",
        "/books/a/../out.md",
        "relative.md",
        "/books/a/image.png",
    ] {
        let mut bad = draft();
        bad.file_path = path.into();
        assert!(store.save(bad).is_err(), "{path}");
    }
    let mut bad = draft();
    bad.location = BookmarkLocation::Pdf {
        page: 1,
        offset: 0.0,
        x: 0.0,
    };
    assert!(store.save(bad).is_err());
    for ratio in [-0.1, 1.1, f64::NAN] {
        let mut bad = draft();
        bad.location = BookmarkLocation::Md {
            ratio,
            block_index: 0,
            text: "".into(),
            offset: 0.0,
        };
        assert!(store.save(bad).is_err());
    }
    let mut bad = draft();
    bad.note_md = "a".repeat(128 * 1024 + 1);
    assert!(store.save(bad).is_err());
    let mut bad = draft();
    bad.book_id = "missing".into();
    assert!(store.save(bad).is_err());
    assert_eq!(store.list("a").unwrap()[0].id, original.id);
}

#[test]
fn test_pdf_bookmarks_accept_page_offsets_and_reject_invalid_pages() {
    let (store, _dir) = setup();
    let mut pdf = draft();
    pdf.file_path = "/books/a/paper.PDF".into();
    pdf.location = BookmarkLocation::Pdf {
        page: 12,
        offset: 0.65,
        x: 0.3,
    };
    let saved = store.save(pdf.clone()).unwrap();
    assert_eq!(saved.location, pdf.location);
    for location in [
        BookmarkLocation::Pdf {
            page: 0,
            offset: 0.0,
            x: 0.0,
        },
        BookmarkLocation::Pdf {
            page: 1,
            offset: 1.2,
            x: 0.0,
        },
    ] {
        pdf.location = location;
        assert!(store.save(pdf.clone()).is_err());
    }
}

#[test]
fn test_bookmarks_quota_rejects_new_rows_but_allows_editing_at_capacity() {
    let (store, _dir) = setup();
    let saved = store.save(draft()).unwrap();
    store.db.with_connection(|conn| {
        for index in 1..1000 {
            conn.execute("INSERT INTO reader_bookmarks SELECT ?1,book_id,file_path,title,note_md,location_json,revision,created_at,updated_at FROM reader_bookmarks WHERE id=?2", params![format!("quota-{index}"), saved.id])?;
        }
        Ok(())
    }).unwrap();
    assert!(store.save(draft()).is_err());
    let mut edit = draft();
    edit.id = Some(saved.id);
    edit.revision = Some(1);
    edit.note_md = "仍可编辑".into();
    assert!(store.save(edit).is_ok());
}

#[test]
fn test_bookmarks_soft_removed_books_reject_writes_and_preserve_existing_notes() {
    let (store, _dir) = setup();
    let saved = store.save(draft()).unwrap();
    store
        .db
        .with_connection(|conn| {
            conn.execute(
                "UPDATE reader_books SET shelf_state='removed' WHERE id='a'",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    assert!(store.list("a").is_err());
    assert!(store.save(draft()).is_err());
    assert!(store.delete("a", &saved.id, 1).is_err());
    store
        .db
        .with_connection(|conn| {
            conn.execute(
                "UPDATE reader_books SET shelf_state='active' WHERE id='a'",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    assert_eq!(store.list("a").unwrap()[0].note_md, saved.note_md);
}

#[test]
fn test_bookmark_migration_replay_preserves_notes() {
    let (store, _dir) = setup();
    let saved = store.save(draft()).unwrap();
    store
        .db
        .with_connection(|conn| {
            conn.execute_batch(include_str!("../../migrations/060_reader_bookmarks.sql"))?;
            Ok(())
        })
        .unwrap();
    assert_eq!(store.list("a").unwrap()[0].id, saved.id);
}
