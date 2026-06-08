DROP INDEX IF EXISTS idx_model_downloads_active_source;
DROP INDEX IF EXISTS idx_model_downloads_source_key;
DROP INDEX IF EXISTS idx_model_downloads_status;
DROP INDEX IF EXISTS idx_model_downloads_source;
DROP INDEX IF EXISTS idx_model_downloads_model_id;
DROP INDEX IF EXISTS idx_audio_transcription_tasks_model_id;
DROP INDEX IF EXISTS idx_audio_transcription_tasks_created_at;
DROP INDEX IF EXISTS idx_video_generation_tasks_model_id;
DROP INDEX IF EXISTS idx_video_generation_tasks_created_at;
DROP INDEX IF EXISTS idx_image_generation_tasks_model_id;
DROP INDEX IF EXISTS idx_image_generation_tasks_created_at;
DROP INDEX IF EXISTS idx_chat_messages_session;
DROP INDEX IF EXISTS idx_models_backend_id;
DROP INDEX IF EXISTS idx_models_kind;
DROP INDEX IF EXISTS idx_models_status;
DROP INDEX IF EXISTS idx_tasks_model_id;
DROP INDEX IF EXISTS idx_tasks_created_at;
DROP INDEX IF EXISTS idx_tasks_task_type;
DROP INDEX IF EXISTS idx_tasks_status;

ALTER TABLE model_downloads RENAME TO model_downloads_old;
ALTER TABLE image_generation_tasks RENAME TO image_generation_tasks_old;
ALTER TABLE video_generation_tasks RENAME TO video_generation_tasks_old;
ALTER TABLE audio_transcription_tasks RENAME TO audio_transcription_tasks_old;
ALTER TABLE chat_messages RENAME TO chat_messages_old;
ALTER TABLE models RENAME TO models_old;
ALTER TABLE tasks RENAME TO tasks_old;

CREATE TABLE tasks (
    id              TEXT    PRIMARY KEY,
    core_task_id    INTEGER,
    model_id        TEXT,
    task_type       TEXT    NOT NULL,
    status          TEXT    NOT NULL CHECK (
        status IN ('pending', 'running', 'succeeded', 'failed', 'cancelled', 'interrupted')
    ),
    input_data      TEXT,
    result_data     TEXT,
    error_msg       TEXT,
    created_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL
);

CREATE TABLE models (
    id                       TEXT    PRIMARY KEY,
    display_name             TEXT    NOT NULL,
    status                   TEXT    NOT NULL CHECK (
        status IN ('ready', 'not_downloaded', 'downloading', 'error')
    ),
    spec                     TEXT    NOT NULL,
    runtime_presets          TEXT,
    created_at               TEXT    NOT NULL,
    updated_at               TEXT    NOT NULL,
    kind                     TEXT    NOT NULL CHECK (kind IN ('local', 'cloud')),
    backend_id               TEXT,
    config_schema_version    INTEGER NOT NULL,
    config_policy_version    INTEGER NOT NULL,
    capabilities             TEXT    NOT NULL,
    materialized_artifacts   TEXT    NOT NULL DEFAULT '{}',
    selected_download_source TEXT
);

CREATE TABLE chat_messages (
    id          TEXT    PRIMARY KEY,
    session_id  TEXT    NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role        TEXT    NOT NULL CHECK (
        role IN ('system', 'developer', 'user', 'assistant', 'tool', 'function')
    ),
    content     TEXT    NOT NULL,
    created_at  TEXT    NOT NULL
);

CREATE TABLE image_generation_tasks (
    task_id              TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    backend_id           TEXT NOT NULL,
    model_id             TEXT,
    model_path           TEXT NOT NULL,
    prompt               TEXT NOT NULL,
    negative_prompt      TEXT,
    mode                 TEXT NOT NULL,
    width                INTEGER NOT NULL,
    height               INTEGER NOT NULL,
    requested_count      INTEGER NOT NULL,
    reference_image_path TEXT,
    primary_image_path   TEXT,
    artifact_paths       TEXT,
    request_data         TEXT NOT NULL,
    result_data          TEXT,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL
);

CREATE TABLE video_generation_tasks (
    task_id              TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    backend_id           TEXT NOT NULL,
    model_id             TEXT,
    model_path           TEXT NOT NULL,
    prompt               TEXT NOT NULL,
    negative_prompt      TEXT,
    width                INTEGER NOT NULL,
    height               INTEGER NOT NULL,
    frames               INTEGER NOT NULL,
    fps                  REAL NOT NULL,
    reference_image_path TEXT,
    video_path           TEXT,
    request_data         TEXT NOT NULL,
    result_data          TEXT,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL
);

CREATE TABLE audio_transcription_tasks (
    task_id         TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    backend_id      TEXT NOT NULL,
    model_id        TEXT,
    source_path     TEXT NOT NULL,
    language        TEXT,
    prompt          TEXT,
    detect_language INTEGER,
    vad_json        TEXT,
    decode_json     TEXT,
    transcript_text TEXT,
    request_data    TEXT NOT NULL,
    result_data     TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE model_downloads (
    task_id      TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    model_id     TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
    repo_id      TEXT NOT NULL,
    filename     TEXT NOT NULL,
    status       TEXT NOT NULL CHECK (
        status IN ('pending', 'running', 'succeeded', 'failed', 'cancelled', 'interrupted')
    ),
    error_msg    TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    source_key   TEXT,
    hub_provider TEXT
);

INSERT INTO tasks (
    id, core_task_id, model_id, task_type, status, input_data, result_data, error_msg, created_at,
    updated_at
)
SELECT
    id, core_task_id, model_id, task_type, status, input_data, result_data, error_msg, created_at,
    updated_at
FROM tasks_old;

INSERT INTO models (
    id, display_name, status, spec, runtime_presets, created_at, updated_at, kind, backend_id,
    config_schema_version, config_policy_version, capabilities, materialized_artifacts,
    selected_download_source
)
SELECT
    id, display_name, status, spec, runtime_presets, created_at, updated_at, kind, backend_id,
    config_schema_version, config_policy_version, capabilities, materialized_artifacts,
    selected_download_source
FROM models_old;

INSERT INTO chat_messages (id, session_id, role, content, created_at)
SELECT id, session_id, role, content, created_at
FROM chat_messages_old;

INSERT INTO image_generation_tasks (
    task_id, backend_id, model_id, model_path, prompt, negative_prompt, mode, width, height,
    requested_count, reference_image_path, primary_image_path, artifact_paths, request_data,
    result_data, created_at, updated_at
)
SELECT
    task_id, backend_id, model_id, model_path, prompt, negative_prompt, mode, width, height,
    requested_count, reference_image_path, primary_image_path, artifact_paths, request_data,
    result_data, created_at, updated_at
FROM image_generation_tasks_old;

INSERT INTO video_generation_tasks (
    task_id, backend_id, model_id, model_path, prompt, negative_prompt, width, height, frames, fps,
    reference_image_path, video_path, request_data, result_data, created_at, updated_at
)
SELECT
    task_id, backend_id, model_id, model_path, prompt, negative_prompt, width, height, frames, fps,
    reference_image_path, video_path, request_data, result_data, created_at, updated_at
FROM video_generation_tasks_old;

INSERT INTO audio_transcription_tasks (
    task_id, backend_id, model_id, source_path, language, prompt, detect_language, vad_json,
    decode_json, transcript_text, request_data, result_data, created_at, updated_at
)
SELECT
    task_id, backend_id, model_id, source_path, language, prompt, detect_language, vad_json,
    decode_json, transcript_text, request_data, result_data, created_at, updated_at
FROM audio_transcription_tasks_old;

INSERT INTO model_downloads (
    task_id, model_id, repo_id, filename, status, error_msg, created_at, updated_at, source_key,
    hub_provider
)
SELECT
    task_id, model_id, repo_id, filename, status, error_msg, created_at, updated_at, source_key,
    hub_provider
FROM model_downloads_old;

DROP TABLE model_downloads_old;
DROP TABLE image_generation_tasks_old;
DROP TABLE video_generation_tasks_old;
DROP TABLE audio_transcription_tasks_old;
DROP TABLE chat_messages_old;
DROP TABLE models_old;
DROP TABLE tasks_old;

CREATE INDEX IF NOT EXISTS idx_tasks_status     ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_task_type  ON tasks(task_type);
CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at);
CREATE INDEX IF NOT EXISTS idx_tasks_model_id   ON tasks(model_id);
CREATE INDEX IF NOT EXISTS idx_models_status    ON models(status);
CREATE INDEX IF NOT EXISTS idx_models_kind      ON models(kind);
CREATE INDEX IF NOT EXISTS idx_models_backend_id ON models(backend_id);
CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id, created_at);
CREATE INDEX IF NOT EXISTS idx_image_generation_tasks_created_at
    ON image_generation_tasks(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_image_generation_tasks_model_id
    ON image_generation_tasks(model_id);
CREATE INDEX IF NOT EXISTS idx_video_generation_tasks_created_at
    ON video_generation_tasks(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_video_generation_tasks_model_id
    ON video_generation_tasks(model_id);
CREATE INDEX IF NOT EXISTS idx_audio_transcription_tasks_created_at
    ON audio_transcription_tasks(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audio_transcription_tasks_model_id
    ON audio_transcription_tasks(model_id);
CREATE INDEX IF NOT EXISTS idx_model_downloads_model_id
    ON model_downloads(model_id);
CREATE INDEX IF NOT EXISTS idx_model_downloads_source
    ON model_downloads(model_id, repo_id, filename, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_model_downloads_status
    ON model_downloads(status);
CREATE INDEX IF NOT EXISTS idx_model_downloads_source_key
    ON model_downloads(model_id, source_key, created_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_model_downloads_active_source
    ON model_downloads(model_id, source_key)
    WHERE status IN ('pending', 'running');
