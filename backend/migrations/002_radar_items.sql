CREATE TABLE IF NOT EXISTS radar_items (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    summary     TEXT,
    source      TEXT NOT NULL,
    url         TEXT NOT NULL UNIQUE,
    embedding_id TEXT,
    status      TEXT DEFAULT 'new' CHECK(status IN ('new','read','saved','dismissed')),
    relevance_score REAL,
    related_notes TEXT,
    fetched_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
    published_at DATETIME
);

CREATE INDEX IF NOT EXISTS idx_radar_items_status ON radar_items(status);
CREATE INDEX IF NOT EXISTS idx_radar_items_score ON radar_items(relevance_score DESC);
CREATE INDEX IF NOT EXISTS idx_radar_items_source ON radar_items(source);