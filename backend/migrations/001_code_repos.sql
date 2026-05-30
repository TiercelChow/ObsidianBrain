CREATE TABLE IF NOT EXISTS code_repos (
    name        TEXT PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    registered_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    metadata    TEXT
);

CREATE TABLE IF NOT EXISTS note_repo_links (
    note_path   TEXT NOT NULL,
    repo_name   TEXT NOT NULL REFERENCES code_repos(name) ON DELETE CASCADE,
    linked_at   DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (note_path, repo_name)
);

CREATE INDEX IF NOT EXISTS idx_note_repo_links_repo ON note_repo_links(repo_name);