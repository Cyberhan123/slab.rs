-- Async tasks table (whisper, image generation, ffmpeg conversion)
CREATE TABLE IF NOT EXISTS tasks (
    id          TEXT    PRIMARY KEY,
    task_type   TEXT    NOT NULL,
    status      TEXT    NOT NULL,
    input_data  TEXT,
    result_data TEXT,
    error_msg   TEXT,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tasks_status      ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_task_type   ON tasks(task_type);
CREATE INDEX IF NOT EXISTS idx_tasks_created_at  ON tasks(created_at);

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
    updated_at  TEXT    NOT NULL
);

-- Store per-session chat message history.
CREATE TABLE IF NOT EXISTS chat_messages (
    id          TEXT    PRIMARY KEY,
    session_id  TEXT    NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role        TEXT    NOT NULL,    -- 'user' | 'assistant' | 'system'
    content     TEXT    NOT NULL,
    created_at  TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id, created_at);

-- Add core_task_id column for tasks backed by slab-core runtime
ALTER TABLE tasks ADD COLUMN core_task_id INTEGER;
