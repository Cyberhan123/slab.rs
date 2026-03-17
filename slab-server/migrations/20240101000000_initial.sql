-- Squashed initial migration.
-- Consolidates all previous migrations into a single file.
-- Replaces model_catalog / model_catalog_backend with the new unified `models` table.

-- ---------------------------------------------------------------------------
-- Async task queue (whisper, image generation, ffmpeg conversion, downloads)
-- ---------------------------------------------------------------------------
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
CREATE INDEX IF NOT EXISTS idx_tasks_status     ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_task_type  ON tasks(task_type);
CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at);
CREATE INDEX IF NOT EXISTS idx_tasks_model_id   ON tasks(model_id);

-- ---------------------------------------------------------------------------
-- Chat sessions
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS chat_sessions (
    id          TEXT    PRIMARY KEY,
    name        TEXT    NOT NULL DEFAULT '',
    state_path  TEXT,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);

-- ---------------------------------------------------------------------------
-- Key-value config store
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS config_store (
    key         TEXT    PRIMARY KEY,
    value       TEXT    NOT NULL,
    name        TEXT    NOT NULL DEFAULT '',
    updated_at  TEXT    NOT NULL
);

-- Seed: model cache directory (empty = use hf-hub default)
INSERT INTO config_store (key, name, value, updated_at)
VALUES ('model_cache_dir', 'Model Cache Directory', '', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT(key) DO NOTHING;

-- Seed: per-backend worker counts (empty = use server default of 1)
INSERT INTO config_store (key, name, value, updated_at)
VALUES ('llama_num_workers', 'Llama Workers', '', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT(key) DO NOTHING;

INSERT INTO config_store (key, name, value, updated_at)
VALUES ('whisper_num_workers', 'Whisper Workers', '', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT(key) DO NOTHING;

INSERT INTO config_store (key, name, value, updated_at)
VALUES ('diffusion_num_workers', 'Diffusion Workers', '', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT(key) DO NOTHING;

-- Seed: llama context length (empty = use backend default)
INSERT INTO config_store (key, name, value, updated_at)
VALUES ('llama_context_length', 'Llama Context Length', '', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT(key) DO NOTHING;

-- Seed: model auto-unload settings
INSERT INTO config_store (key, name, value, updated_at)
VALUES ('model_auto_unload_enabled', 'Model Auto Unload Enabled', 'false', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT(key) DO NOTHING;

INSERT INTO config_store (key, name, value, updated_at)
VALUES ('model_auto_unload_idle_minutes', 'Model Auto Unload Idle Minutes', '10', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT(key) DO NOTHING;

-- Seed: cloud chat provider configuration (kept for backward-compat with existing installs)
INSERT INTO config_store (key, name, value, updated_at)
VALUES ('chat_model_providers', 'Chat Model Providers', '[]', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT(key) DO NOTHING;

-- ---------------------------------------------------------------------------
-- Chat message history (per-session)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS chat_messages (
    id          TEXT    PRIMARY KEY,
    session_id  TEXT    NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role        TEXT    NOT NULL,   -- 'user' | 'assistant' | 'system'
    content     TEXT    NOT NULL,
    created_at  TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id, created_at);

-- ---------------------------------------------------------------------------
-- Unified models table
-- Replaces the old model_catalog + model_catalog_backend tables.
-- Both local (provider = "local.ggml.llama", etc.) and cloud
-- (provider = "cloud.openai", etc.) models share this table.
-- spec and runtime_presets are stored as JSON strings.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS models (
    id              TEXT    PRIMARY KEY,
    display_name    TEXT    NOT NULL,
    provider        TEXT    NOT NULL,
    status          TEXT    NOT NULL,   -- 'ready' | 'not_downloaded' | 'downloading' | 'error'
    spec            TEXT    NOT NULL,   -- JSON: ModelSpec
    runtime_presets TEXT,               -- JSON: RuntimePresets (optional)
    created_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_models_provider ON models(provider);
CREATE INDEX IF NOT EXISTS idx_models_status   ON models(status);
