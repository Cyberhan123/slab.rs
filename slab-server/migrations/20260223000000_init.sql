-- Async tasks table (whisper, image generation, ffmpeg conversion)
CREATE TABLE IF NOT EXISTS tasks (
    id              TEXT    PRIMARY KEY,
    core_task_id    INTEGER,
    model_id        TEXT,
    task_type       TEXT    NOT NULL,
    status          TEXT    NOT NULL,
    input_data      TEXT,
    result_data     TEXT,
    error_msg       TEXT,
    created_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tasks_status      ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_task_type   ON tasks(task_type);
CREATE INDEX IF NOT EXISTS idx_tasks_created_at  ON tasks(created_at);
CREATE INDEX IF NOT EXISTS idx_tasks_model_id ON tasks(model_id);

CREATE TABLE IF NOT EXISTS chat_sessions (
    id          TEXT    PRIMARY KEY,
    name        TEXT    NOT NULL DEFAULT '',
    state_path  TEXT,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS config_store (
    key         TEXT    PRIMARY KEY,
    value       TEXT    NOT NULL,
    name        TEXT    NOT NULL DEFAULT '',
    updated_at  TEXT    NOT NULL
);

-- Seed a model cache directory config entry.
-- Empty value means "not configured", so model downloads fall back to hf-hub defaults.
INSERT INTO config_store (key, name, value, updated_at)
VALUES (
    'model_cache_dir',
    'Model Cache Directory',
    '',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
)
ON CONFLICT(key) DO NOTHING;

-- Store per-session chat message history.
CREATE TABLE IF NOT EXISTS chat_messages (
    id          TEXT    PRIMARY KEY,
    session_id  TEXT    NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role        TEXT    NOT NULL,    -- 'user' | 'assistant' | 'system'
    content     TEXT    NOT NULL,
    created_at  TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id, created_at);

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

CREATE INDEX IF NOT EXISTS idx_model_catalog_repo_filename
    ON model_catalog(repo_id, filename);

CREATE TABLE IF NOT EXISTS model_catalog_backend (
    model_id   TEXT NOT NULL REFERENCES model_catalog(id) ON DELETE CASCADE,
    backend_id TEXT NOT NULL,
    PRIMARY KEY (model_id, backend_id)
);

CREATE INDEX IF NOT EXISTS idx_model_catalog_backend_backend_id
    ON model_catalog_backend(backend_id);

