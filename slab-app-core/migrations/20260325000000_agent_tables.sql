-- Agent orchestration tables
-- Appended to the slab-server schema.
--
-- Depends on: chat_sessions (created in 20240101000000_initial.sql)

-- ---------------------------------------------------------------------------
-- Agent threads
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS agent_threads (
    id              TEXT    PRIMARY KEY,
    session_id      TEXT    NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    parent_id       TEXT    REFERENCES agent_threads(id) ON DELETE SET NULL,
    depth           INTEGER NOT NULL DEFAULT 0,
    status          TEXT    NOT NULL DEFAULT 'pending',
    role_name       TEXT,
    config_json     TEXT    NOT NULL DEFAULT '{}',
    completion_text TEXT,
    created_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agent_threads_session ON agent_threads(session_id);
CREATE INDEX IF NOT EXISTS idx_agent_threads_parent  ON agent_threads(parent_id);
CREATE INDEX IF NOT EXISTS idx_agent_threads_status  ON agent_threads(status);

-- ---------------------------------------------------------------------------
-- Agent tool call audit log
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS agent_tool_calls (
    id              TEXT    PRIMARY KEY,
    thread_id       TEXT    NOT NULL REFERENCES agent_threads(id) ON DELETE CASCADE,
    tool_name       TEXT    NOT NULL,
    arguments       TEXT    NOT NULL DEFAULT '{}',
    output          TEXT,
    status          TEXT    NOT NULL DEFAULT 'pending',
    created_at      TEXT    NOT NULL,
    completed_at    TEXT
);
CREATE INDEX IF NOT EXISTS idx_agent_tool_calls_thread ON agent_tool_calls(thread_id, created_at);
