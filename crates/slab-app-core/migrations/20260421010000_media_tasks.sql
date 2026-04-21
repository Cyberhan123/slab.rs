CREATE TABLE IF NOT EXISTS image_generation_tasks (
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

CREATE INDEX IF NOT EXISTS idx_image_generation_tasks_created_at
    ON image_generation_tasks(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_image_generation_tasks_model_id
    ON image_generation_tasks(model_id);

CREATE TABLE IF NOT EXISTS video_generation_tasks (
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

CREATE INDEX IF NOT EXISTS idx_video_generation_tasks_created_at
    ON video_generation_tasks(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_video_generation_tasks_model_id
    ON video_generation_tasks(model_id);

CREATE TABLE IF NOT EXISTS audio_transcription_tasks (
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

CREATE INDEX IF NOT EXISTS idx_audio_transcription_tasks_created_at
    ON audio_transcription_tasks(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audio_transcription_tasks_model_id
    ON audio_transcription_tasks(model_id);
