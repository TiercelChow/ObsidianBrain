use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::error::BrainError;

/// Default debounce window in milliseconds.
#[allow(dead_code)]
pub const DEFAULT_DEBOUNCE_MS: u64 = 300;

/// Type of filesystem change.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // Public API for future tasks (Phase 1+)
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
}

/// A single file change event.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Public API for future tasks (Phase 1+)
pub struct FileChangeEvent {
    pub change_type: FileChangeType,
    pub path: PathBuf,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[allow(dead_code)] // Public API for future tasks (Phase 1+)
struct PendingEvent {
    event_type: FileChangeType,
    last_seen: Instant,
}

/// Watches a directory for `.md` file changes, debounces, and sends events.
#[allow(dead_code)] // Public API for future tasks (Phase 1+)
pub struct FileWatcher {
    _watcher: notify::RecommendedWatcher,
    pub rx: Arc<Mutex<Option<mpsc::Receiver<FileChangeEvent>>>>,
}

#[allow(dead_code)] // Public API for future tasks (Phase 1+)
impl FileWatcher {
    /// Start watching `vault_path`. Events arrive on the returned receiver.
    /// Only `.md` files are tracked. Paths matching any `exclude_patterns` substring are skipped.
    pub fn new(
        vault_path: &Path,
        exclude_patterns: Vec<String>,
        debounce_ms: u64,
    ) -> Result<Self, BrainError> {
        let (tx, rx) = mpsc::channel::<FileChangeEvent>(1024);

        let pending: Arc<Mutex<HashMap<PathBuf, PendingEvent>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Spawn flush loop
        let pending_clone = pending.clone();
        let tx_flush = tx.clone();
        let debounce_dur = Duration::from_millis(debounce_ms);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let now = Instant::now();
                let mut to_flush = Vec::new();

                {
                    let mut map = pending_clone.lock().unwrap();
                    map.retain(|path, event| {
                        if now.duration_since(event.last_seen) >= debounce_dur {
                            to_flush.push(FileChangeEvent {
                                change_type: event.event_type.clone(),
                                path: path.clone(),
                                timestamp: chrono::Utc::now(),
                            });
                            false
                        } else {
                            true
                        }
                    });
                }

                for ev in to_flush {
                    if tx_flush.send(ev).await.is_err() {
                        return; // receiver dropped
                    }
                }
            }
        });

        let pending_cb = pending.clone();
        let exclude = exclude_patterns.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                for path in &event.paths {
                    let path_str = path.to_string_lossy();

                    // Skip excluded
                    if exclude.iter().any(|p| path_str.contains(p.as_str())) {
                        continue;
                    }

                    // Only .md files
                    if path.extension().map(|e| e != "md").unwrap_or(true) {
                        continue;
                    }

                    let change_type = match event.kind {
                        EventKind::Create(_) => FileChangeType::Created,
                        EventKind::Modify(_) => FileChangeType::Modified,
                        EventKind::Remove(_) => FileChangeType::Deleted,
                        _ => continue,
                    };

                    let mut map = pending_cb.lock().unwrap();
                    let now = Instant::now();
                    map.entry(path.clone())
                        .and_modify(|e| {
                            e.last_seen = now;
                            if matches!(change_type, FileChangeType::Deleted) {
                                e.event_type = FileChangeType::Deleted;
                            }
                        })
                        .or_insert(PendingEvent {
                            event_type: change_type,
                            last_seen: now,
                        });
                }
            }
        })
        .map_err(|e| BrainError::Internal(format!("文件监控初始化失败: {e}")))?;

        watcher
            .watch(vault_path, RecursiveMode::Recursive)
            .map_err(|e| BrainError::Internal(format!("Vault 监控启动失败: {e}")))?;

        tracing::info!("文件监控启动: {:?}", vault_path);

        Ok(FileWatcher {
            _watcher: watcher,
            rx: Arc::new(Mutex::new(Some(rx))),
        })
    }

    /// Take the receiver out (can only be called once).
    pub fn take_receiver(&self) -> Option<mpsc::Receiver<FileChangeEvent>> {
        self.rx.lock().unwrap().take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_watcher_detects_md_creation() {
        let dir = TempDir::new().unwrap();
        let watcher = FileWatcher::new(
            dir.path(),
            vec![],
            100, // short debounce for testing
        )
        .unwrap();

        let mut rx = watcher.take_receiver().unwrap();

        // Create a .md file
        std::fs::write(dir.path().join("test.md"), "# Hello").unwrap();

        // Wait for debounce + flush
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("Timed out waiting for event")
            .expect("Channel closed");

        assert_eq!(event.change_type, FileChangeType::Created);
        assert!(event.path.to_string_lossy().contains("test.md"));
    }

    #[tokio::test]
    async fn test_file_watcher_ignores_non_md() {
        let dir = TempDir::new().unwrap();
        let watcher = FileWatcher::new(dir.path(), vec![], 100).unwrap();
        let mut rx = watcher.take_receiver().unwrap();

        // Create a .txt file — should be ignored
        std::fs::write(dir.path().join("test.txt"), "hello").unwrap();

        let result = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        assert!(result.is_err(), "Should not receive event for .txt file");
    }

    #[tokio::test]
    async fn test_file_watcher_excludes_patterns() {
        let dir = TempDir::new().unwrap();
        let trash = dir.path().join(".trash");
        std::fs::create_dir_all(&trash).unwrap();

        let watcher = FileWatcher::new(dir.path(), vec![".trash/".to_string()], 100).unwrap();
        let mut rx = watcher.take_receiver().unwrap();

        // Create .md in excluded dir
        std::fs::write(trash.join("deleted.md"), "gone").unwrap();

        let result = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        assert!(
            result.is_err(),
            "Should not receive event for excluded path"
        );
    }
}
