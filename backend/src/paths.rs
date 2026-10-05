//! Data directory resolution — all runtime data lives in one place.

use std::path::PathBuf;

/// Returns the data directory for ObsidianBrain.
///
/// Priority:
/// 1. `OBRAIN_DATA_DIR` environment variable
/// 2. `~/.obsidian-brain/`
///
/// Creates the directory if it doesn't exist. Image storage is colocated with the configured DB.
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OBRAIN_DATA_DIR") {
        let p = PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&p);
        return p;
    }

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".obsidian-brain");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Path to the SQLite database file.
pub fn db_path() -> PathBuf {
    data_dir().join("brain.db")
}

/// Originals/GC must never be shared by distinct databases in the same directory.
pub fn timeline_dir(database: &std::path::Path) -> PathBuf {
    use sha2::{Digest, Sha256};
    let parent = database
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let filename = database.file_name().unwrap_or_default();
    let directory = if filename == "brain.db" {
        "timeline".to_string()
    } else {
        format!(
            "timeline-{}",
            &hex::encode(Sha256::digest(filename.to_string_lossy().as_bytes()))[..16]
        )
    };
    parent.join(directory)
}

/// Path to the Tantivy index directory.
pub fn index_path() -> PathBuf {
    data_dir().join("tantivy_index")
}

/// Directory containing generated knowledge deliverables such as PPTX files.
pub fn artifacts_dir() -> PathBuf {
    let dir = data_dir().join("artifacts");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Path to the PID file (for daemon management).
pub fn pid_file() -> PathBuf {
    data_dir().join("obsidian-brain.pid")
}

/// Path to the log file (for daemon mode).
pub fn log_file() -> PathBuf {
    data_dir().join("obsidian-brain.log")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_timeline_directory_follows_database_and_isolates_siblings() {
        assert_eq!(
            timeline_dir(std::path::Path::new("/data/brain.db")),
            PathBuf::from("/data/timeline")
        );
        let a = timeline_dir(std::path::Path::new("/data/preview-a.db"));
        let b = timeline_dir(std::path::Path::new("/data/preview-b.db"));
        assert_ne!(a, b);
        assert_ne!(a, PathBuf::from("/data/timeline"));
        assert_eq!(a.parent(), Some(std::path::Path::new("/data")));
    }
    #[test]
    fn test_relative_database_does_not_use_global_user_storage() {
        assert_eq!(
            timeline_dir(std::path::Path::new("brain.db")),
            PathBuf::from("./timeline")
        );
        assert_eq!(
            timeline_dir(std::path::Path::new("preview.db")).parent(),
            Some(std::path::Path::new("."))
        );
    }
}
