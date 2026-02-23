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
