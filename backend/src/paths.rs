//! Data directory resolution — all runtime data lives in one place.

use std::path::PathBuf;

/// Returns the data directory for ObsidianBrain.
///
/// Priority:
/// 1. `OBRAIN_DATA_DIR` environment variable
/// 2. `~/.obsidian-brain/`
///
/// Creates the directory (and `thumbnails/` subdirectory) if it doesn't exist.
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OBRAIN_DATA_DIR") {
        let p = PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&p);
        let _ = std::fs::create_dir_all(p.join("thumbnails"));
        return p;
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".obsidian-brain");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("thumbnails"));
    dir
}

/// Path to the SQLite database file.
pub fn db_path() -> PathBuf {
    data_dir().join("brain.db")
}

/// Path to the Tantivy index directory.
pub fn index_path() -> PathBuf {
    data_dir().join("tantivy_index")
}

/// Path to the thumbnails directory.
pub fn thumbnails_dir() -> PathBuf {
    data_dir().join("thumbnails")
}

/// Path to the PID file (for daemon management).
pub fn pid_file() -> PathBuf {
    data_dir().join("obsidian-brain.pid")
}

/// Path to the log file (for daemon mode).
pub fn log_file() -> PathBuf {
    data_dir().join("obsidian-brain.log")
}
