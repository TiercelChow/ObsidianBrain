-- 时光机小记表
CREATE TABLE IF NOT EXISTS memos (
    id          TEXT PRIMARY KEY,
    timestamp   DATETIME NOT NULL,
    date        TEXT NOT NULL,
    content     TEXT NOT NULL,
    images      TEXT,
    tags        TEXT,
    file_path   TEXT NOT NULL,
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_memos_timestamp ON memos(timestamp DESC);
CREATE INDEX idx_memos_date ON memos(date);
CREATE INDEX idx_memos_tags ON memos(tags);
