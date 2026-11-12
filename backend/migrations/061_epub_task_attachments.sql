-- The runner disables FK enforcement outside this transaction and checks it
-- before commit. Do not DROP the parent with foreign_keys enabled: bookmarks
-- would be cascaded even with defer_foreign_keys.
CREATE TABLE reader_books_epub (
    id TEXT PRIMARY KEY, path TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL CHECK (kind IN ('folder','pdf','epub')),
    name TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
    category TEXT NOT NULL DEFAULT '', added_at INTEGER NOT NULL,
    progress_json TEXT, shelf_state TEXT NOT NULL DEFAULT 'active' CHECK(shelf_state IN ('active','removed')),
    removed_at TEXT, revision INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO reader_books_epub SELECT * FROM reader_books;
DROP TABLE reader_books;
ALTER TABLE reader_books_epub RENAME TO reader_books;
CREATE INDEX idx_reader_books_shelf ON reader_books(shelf_state, added_at DESC);

-- No FK cascade to the transient task rows (replace_document rebuilds them).
CREATE TABLE IF NOT EXISTS task_attachments (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    progress_id TEXT,
    filename TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
    state TEXT NOT NULL CHECK(state IN ('uploading','active','deleted')),
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_task_attachments_scope ON task_attachments(task_id, progress_id, state);
