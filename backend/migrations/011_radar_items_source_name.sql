-- 修复雷达条目表的结构冲突。
--
-- 002（version 2）用 `source` 建了 radar_items，007（version 7）想改用
-- `source_name` + `saved_path`，但它同样是 `CREATE TABLE IF NOT EXISTS`，
-- 于是 007 的建表语句成了静默空操作：`source_name` 与 `saved_path` 从未被创建，
-- 而 sqlite_store 的 INSERT / SELECT 一直在用 `source_name`，导致 get_radar
-- 在任何安装上都失败（no such column: source_name）。
--
-- 迁移文件是历史记录，不要修改 002/007，改为在这里按 007 的目标结构重建该表，
-- 并把历史数据从 `source` 回填到 `source_name`。
CREATE TABLE IF NOT EXISTS radar_items_rebuilt (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    summary         TEXT,
    source_name     TEXT NOT NULL,
    url             TEXT NOT NULL UNIQUE,
    status          TEXT NOT NULL DEFAULT 'new',
    relevance_score REAL,
    published_at    DATETIME,
    saved_path      TEXT,
    fetched_at      DATETIME DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO radar_items_rebuilt
    (id, title, summary, source_name, url, status, relevance_score, published_at, fetched_at)
SELECT
    id,
    title,
    summary,
    COALESCE(NULLIF(source, ''), 'unknown'),
    url,
    COALESCE(status, 'new'),
    relevance_score,
    published_at,
    fetched_at
FROM radar_items;

DROP TABLE radar_items;

ALTER TABLE radar_items_rebuilt RENAME TO radar_items;

CREATE INDEX IF NOT EXISTS idx_radar_status ON radar_items(status);
CREATE INDEX IF NOT EXISTS idx_radar_fetched_at ON radar_items(fetched_at DESC);
CREATE INDEX IF NOT EXISTS idx_radar_relevance ON radar_items(relevance_score DESC);
