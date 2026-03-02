CREATE TABLE IF NOT EXISTS model_catalog (
    id                    TEXT PRIMARY KEY,
    display_name          TEXT NOT NULL,
    repo_id               TEXT NOT NULL,
    filename              TEXT NOT NULL,
    local_path            TEXT,
    last_download_task_id TEXT,
    last_downloaded_at    TEXT,
    created_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS model_catalog_backend (
    model_id   TEXT NOT NULL REFERENCES model_catalog(id) ON DELETE CASCADE,
    backend_id TEXT NOT NULL,
    PRIMARY KEY (model_id, backend_id)
);

CREATE INDEX IF NOT EXISTS idx_model_catalog_repo_filename
    ON model_catalog(repo_id, filename);
CREATE INDEX IF NOT EXISTS idx_model_catalog_backend_backend_id
    ON model_catalog_backend(backend_id);

ALTER TABLE tasks ADD COLUMN model_id TEXT;
CREATE INDEX IF NOT EXISTS idx_tasks_model_id ON tasks(model_id);
