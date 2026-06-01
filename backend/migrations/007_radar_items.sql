-- 雷达条目表
CREATE TABLE IF NOT EXISTS radar_items (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    summary         TEXT,
    source_name     TEXT NOT NULL,
    url             TEXT NOT NULL UNIQUE,
    status          TEXT NOT NULL DEFAULT 'new',  -- new, read, saved, dismissed
    relevance_score REAL,
    published_at    DATETIME,
    saved_path      TEXT,
    fetched_at      DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_radar_status ON radar_items(status);
CREATE INDEX IF NOT EXISTS idx_radar_fetched_at ON radar_items(fetched_at DESC);
CREATE INDEX IF NOT EXISTS idx_radar_relevance ON radar_items(relevance_score DESC);
