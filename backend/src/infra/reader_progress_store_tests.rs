use super::*;
use crate::infra::book_wiki_store::BookWikiStore;
use crate::models::book_wiki::{BookKind, ReaderBook};

fn store() -> (ReaderProgressStore, BookWikiStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(SqliteStore::new(&dir.path().join("reader.db")).unwrap());
    let books = BookWikiStore::new(db.clone());
    books
        .save_reader_books(&[ReaderBook {
            id: "book".into(),
            path: "/books/a".into(),
            kind: BookKind::Folder,
            name: "Book".into(),
            description: String::new(),
            category: String::new(),
            added_at: 1,
            progress: None,
        }])
        .unwrap();
    (ReaderProgressStore::new(db), books, dir)
}
fn state(file: &str, kind: &str, position: f64, updated: i64, read: i64) -> ReaderProgressState {
    serde_json::from_value(serde_json::json!({"lastFile":file,"lastReadAt":read,"byFile":{file:{"kind":kind,"position":position,"updatedAt":updated}}})).unwrap()
}

#[test]
fn test_progress_merges_files_and_separates_open_time_from_position() {
    let (store, books, _dir) = store();
    store
        .save("book", state("/books/a/one.md", "md", 0.6, 100, 100))
        .unwrap();
    store
        .save("book", state("/books/a/two.pdf", "pdf", 12.0, 200, 200))
        .unwrap();
    let result = store
        .save("book", state("/books/a/one.md", "md", 0.6, 100, 300))
        .unwrap();
    assert_eq!(result.by_file.len(), 2);
    assert_eq!(result.by_file["/books/a/one.md"].updated_at, 100);
    assert_eq!(result.last_read_at, 300);
    let progress = books
        .list_reader_books()
        .unwrap()
        .remove(0)
        .progress
        .unwrap();
    assert_eq!(progress.updated_at, 300);
    assert_eq!(progress.position, 0.6);
    assert_eq!(progress.last_file.as_deref(), Some("/books/a/one.md"));
}

#[test]
fn test_stale_progress_cannot_reset_latest_file_or_position_and_metadata_preserves_state() {
    let (store, books, _dir) = store();
    store
        .save("book", state("/books/a/one.md", "md", 0.7, 300, 300))
        .unwrap();
    let result = store
        .save("book", state("/books/a/two.md", "md", 0.1, 100, 100))
        .unwrap();
    assert_eq!(result.last_file, "/books/a/one.md");
    store
        .save("book", state("/books/a/one.md", "md", 0.2, 100, 100))
        .unwrap();
    let mut book = books.list_reader_books().unwrap().remove(0);
    book.name = "Renamed".into();
    book.progress = None;
    books.save_reader_books(&[book]).unwrap();
    let progress = books
        .list_reader_books()
        .unwrap()
        .remove(0)
        .progress
        .unwrap();
    assert_eq!(progress.position, 0.7);
    assert_eq!(progress.by_file.len(), 2);
}

#[test]
fn test_progress_rejects_outside_paths_invalid_positions_and_removed_books_without_writes() {
    let (store, books, _dir) = store();
    for invalid in [
        state("/books/ab/out.md", "md", 0.1, 100, 100),
        state("/books/a/../out.md", "md", 0.1, 100, 100),
        state("/books/a/one.md", "pdf", 1.0, 100, 100),
        state("/books/a/one.md", "md", 2.0, 100, 100),
        state("/books/a/two.pdf", "pdf", 1.5, 100, 100),
        state("/books/a/one.md", "md", 0.1, -1, 100),
    ] {
        assert!(store.save("book", invalid).is_err());
    }
    assert!(books.list_reader_books().unwrap()[0].progress.is_none());
    books.save_reader_books(&[]).unwrap();
    assert!(store
        .save("book", state("/books/a/one.md", "md", 0.1, 100, 100))
        .is_err());
}

#[test]
fn test_legacy_progress_survives_import_and_windows_paths_are_component_scoped() {
    let (store, books, _dir) = store();
    let mut book = books.list_reader_books().unwrap().remove(0);
    book.progress = serde_json::from_value(
        serde_json::json!({"lastFile":"/books/a/one.md","position":0.8,"updatedAt":300}),
    )
    .unwrap();
    books.save_reader_books(&[book]).unwrap();
    let result = store
        .save("book", state("/books/a/two.pdf", "pdf", 9.0, 100, 100))
        .unwrap();
    assert_eq!(result.by_file["/books/a/one.md"].position, 0.8);
    assert_eq!(result.last_file, "/books/a/one.md");
    assert!(belongs("C:\\Books\\A\\chapter.md", "c:\\books\\a", "folder").unwrap());
    assert!(!belongs("C:\\Books\\AB\\chapter.md", "C:\\Books\\A", "folder").unwrap());
}

#[tokio::test]
async fn test_reading_tool_http_updates_home_and_reloads_full_state_without_metadata_loss() {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;
    let (ctx, _dir, _legacy) = crate::AppContext::for_test();
    crate::tools::handlers::register_all_tools(&ctx.tool_registry, ctx.clone()).await;
    let books = BookWikiStore::new(ctx.db.clone());
    books
        .save_reader_books(&[ReaderBook {
            id: "book".into(),
            path: "/books/a".into(),
            kind: BookKind::Folder,
            name: "Book".into(),
            description: String::new(),
            category: String::new(),
            added_at: 1,
            progress: None,
        }])
        .unwrap();
    let app = crate::api::router::create_router(ctx);
    let reading = state("/books/a/one.md", "md", 0.65, 100, 200);
    let response=app.clone().oneshot(Request::post("/v1/tools/call").header("content-type","application/json").body(Body::from(json!({"tool":"save_reader_progress","arguments":{"book_id":"book","state":reading}}).to_string())).unwrap()).await.unwrap();
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(value["status"], "success", "{value}");
    assert_eq!(value["result"]["state"]["lastReadAt"], 200);
    let response = app
        .clone()
        .oneshot(
            Request::get("/v1/home/overview")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let home: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(home["reading"]["data"][0]["read_at"], 200);
    assert_eq!(home["reading"]["data"][0]["position"], 0.65);
    let mut book = books.list_reader_books().unwrap().remove(0);
    book.progress = None;
    book.name = "Updated".into();
    books.save_reader_books(&[book]).unwrap();
    let response = app
        .oneshot(
            Request::post("/v1/tools/call")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"tool":"get_reader_books","arguments":{}}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(
        value["result"]["books"][0]["progress"]["byFile"]["/books/a/one.md"]["position"],
        0.65
    );
    assert_eq!(value["result"]["books"][0]["name"], "Updated");
}
