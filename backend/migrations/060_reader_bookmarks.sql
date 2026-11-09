CREATE TABLE IF NOT EXISTS reader_bookmarks (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL REFERENCES reader_books(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    title TEXT NOT NULL,
    note_md TEXT NOT NULL DEFAULT '',
    location_json TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_reader_bookmarks_book ON reader_bookmarks(book_id, file_path, created_at);
