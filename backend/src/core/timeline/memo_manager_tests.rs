use super::*;

async fn setup() -> (MemoManager, tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(SqliteStore::new(&dir.path().join("brain.db")).unwrap());
    let images =
        Arc::new(TimelineImages::new(db.clone(), dir.path().join("timeline"), 1024).unwrap());
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(10, 10)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let path = images.upload(&png.into_inner()).await.unwrap();
    (MemoManager::new(db, images), dir, path)
}

fn request(content: &str, paths: Vec<String>) -> MemoCreateRequest {
    MemoCreateRequest {
        content: content.into(),
        images: paths,
        tags: vec!["test".into()],
    }
}

#[tokio::test]
async fn test_edit_preserves_dates_and_detects_stale_revision() {
    let (manager, _dir, path) = setup().await;
    let memo = manager
        .create_memo(request("before", vec![path.clone()]))
        .await
        .unwrap();
    let updated = manager
        .update_memo(&memo.id, 1, request("after", vec![path]))
        .await
        .unwrap();
    assert_eq!(memo.timestamp, updated.timestamp);
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.content, "after");
    assert!(manager
        .update_memo(&memo.id, 1, request("stale", vec![]))
        .await
        .is_err());
    assert_eq!(manager.get_memo(&memo.id).unwrap().content, "after");
}

#[tokio::test]
async fn test_shared_image_deleted_only_after_last_reference() {
    let (manager, _dir, path) = setup().await;
    let a = manager
        .create_memo(request("one", vec![path.clone()]))
        .await
        .unwrap();
    let b = manager
        .create_memo(request("two", vec![path.clone()]))
        .await
        .unwrap();
    manager.images.thumbnail(&path).await.unwrap();
    manager.delete_memo(&a.id, 1).await.unwrap();
    assert!(manager.images.original(&path).await.is_ok());
    assert!(manager.delete_memo(&b.id, 99).await.is_err());
    assert!(manager.get_memo(&b.id).is_ok());
    assert_eq!(manager.delete_memo(&b.id, 1).await.unwrap(), 0);
    assert!(manager.images.original(&path).await.is_err());
    assert_eq!(manager.images.stats().await.unwrap().cache_bytes, 0);
}

#[tokio::test]
async fn test_edit_image_removal_cleanup_and_invalid_attachment_rollback() {
    let (manager, _dir, path) = setup().await;
    let memo = manager
        .create_memo(request("before", vec![path.clone()]))
        .await
        .unwrap();
    assert!(manager
        .update_memo(&memo.id, 1, request("bad", vec!["../missing".into()]))
        .await
        .is_err());
    assert_eq!(manager.get_memo(&memo.id).unwrap().content, "before");
    manager
        .update_memo(&memo.id, 1, request("after", vec![]))
        .await
        .unwrap();
    assert!(manager.images.original(&path).await.is_err());
    assert!(manager.create_memo(request("  ", vec![])).await.is_err());
    assert!(manager.create_memo(request("", vec![])).await.is_err());
}

#[tokio::test]
async fn test_failed_cleanup_persists_and_retries_after_restart() {
    let (manager, dir, path) = setup().await;
    let memo = manager
        .create_memo(request("delete", vec![path.clone()]))
        .await
        .unwrap();
    let (file, _) = manager.images.asset(&path).unwrap();
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap(); // deterministic failure, even running as root
    assert_eq!(manager.delete_memo(&memo.id, 1).await.unwrap(), 1);
    assert!(manager.get_memo(&memo.id).is_err());
    std::fs::remove_dir(&file).unwrap();
    drop(manager);
    let db = Arc::new(SqliteStore::new(&dir.path().join("brain.db")).unwrap());
    let images = TimelineImages::new(db, dir.path().join("timeline"), 1024).unwrap();
    assert_eq!(images.collect().await.unwrap(), 0);
    assert_eq!(images.stats().await.unwrap().originals_bytes, 0);
}

#[tokio::test]
async fn test_inline_image_reference_is_not_collected() {
    let (manager, _dir, path) = setup().await;
    let a = manager
        .create_memo(request("one", vec![path.clone()]))
        .await
        .unwrap();
    let b = manager
        .create_memo(request(&format!("![[{path}]]"), vec![]))
        .await
        .unwrap();
    manager.delete_memo(&a.id, 1).await.unwrap();
    assert!(manager.images.original(&path).await.is_ok());
    manager.delete_memo(&b.id, 1).await.unwrap();
    assert!(manager.images.original(&path).await.is_err());
}
