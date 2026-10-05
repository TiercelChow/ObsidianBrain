-- The revision column is conditionally added by the migration hook for retry-safe legacy upgrades.
CREATE TABLE IF NOT EXISTS timeline_images (
    path TEXT PRIMARY KEY,
    filename TEXT NOT NULL UNIQUE,
    content_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    pending INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS timeline_image_gc (path TEXT PRIMARY KEY);
CREATE TABLE IF NOT EXISTS timeline_image_cache (
    key TEXT PRIMARY KEY,
    size_bytes INTEGER NOT NULL,
    last_used INTEGER NOT NULL
);
