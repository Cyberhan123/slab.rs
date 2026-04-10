CREATE TABLE IF NOT EXISTS model_downloads (
    task_id      TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    model_id     TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
    repo_id      TEXT NOT NULL,
    filename     TEXT NOT NULL,
    status       TEXT NOT NULL,
    error_msg    TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_model_downloads_model_id
    ON model_downloads(model_id);

CREATE INDEX IF NOT EXISTS idx_model_downloads_source
    ON model_downloads(model_id, repo_id, filename, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_model_downloads_status
    ON model_downloads(status);

CREATE UNIQUE INDEX IF NOT EXISTS idx_model_downloads_active_source
    ON model_downloads(model_id, repo_id, filename)
    WHERE status IN ('pending', 'running');
