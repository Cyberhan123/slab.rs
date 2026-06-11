-- Agent turn detail and memory pipeline state.

CREATE TABLE IF NOT EXISTS agent_turn_states (
    thread_id           TEXT    NOT NULL REFERENCES agent_threads(id) ON DELETE CASCADE,
    turn_index          INTEGER NOT NULL,
    status              TEXT    NOT NULL,
    input_messages_json TEXT,
    tool_specs_json     TEXT,
    llm_response_json   TEXT,
    error               TEXT,
    started_at          TEXT    NOT NULL,
    completed_at        TEXT,
    PRIMARY KEY (thread_id, turn_index)
);

CREATE INDEX IF NOT EXISTS idx_agent_turn_states_status
    ON agent_turn_states (status, started_at);

CREATE TABLE IF NOT EXISTS agent_memory_phase1_outputs (
    thread_id                             TEXT    PRIMARY KEY REFERENCES agent_threads(id) ON DELETE CASCADE,
    session_id                            TEXT    NOT NULL,
    status                                TEXT    NOT NULL DEFAULT 'pending',
    raw_memory                            TEXT,
    rollout_summary                       TEXT,
    rollout_slug                          TEXT,
    source_updated_at                     TEXT,
    generated_at                          TEXT,
    lease_owner                           TEXT,
    lease_until                           TEXT,
    attempts                              INTEGER NOT NULL DEFAULT 0,
    next_retry_at                         TEXT,
    selected_for_phase2                   INTEGER NOT NULL DEFAULT 0,
    selected_for_phase2_source_updated_at TEXT,
    last_usage                            TEXT,
    usage_count                           INTEGER NOT NULL DEFAULT 0,
    error                                 TEXT,
    updated_at                            TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_agent_memory_phase1_status
    ON agent_memory_phase1_outputs (status, next_retry_at, lease_until);
CREATE INDEX IF NOT EXISTS idx_agent_memory_phase1_selected
    ON agent_memory_phase1_outputs (selected_for_phase2, source_updated_at);
CREATE INDEX IF NOT EXISTS idx_agent_memory_phase1_usage
    ON agent_memory_phase1_outputs (last_usage, generated_at, usage_count);

CREATE TABLE IF NOT EXISTS agent_memory_phase2_lock (
    id                  INTEGER PRIMARY KEY CHECK (id = 1),
    lease_owner          TEXT,
    lease_until          TEXT,
    claimed_watermark    TEXT,
    completed_watermark  TEXT,
    status               TEXT NOT NULL DEFAULT 'idle',
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS agent_memory_phase2_runs (
    id                  TEXT PRIMARY KEY,
    status              TEXT NOT NULL,
    lease_owner          TEXT,
    claimed_watermark    TEXT,
    completed_watermark  TEXT,
    started_at           TEXT NOT NULL,
    completed_at         TEXT,
    error                TEXT
);

CREATE INDEX IF NOT EXISTS idx_agent_memory_phase2_runs_status
    ON agent_memory_phase2_runs (status, started_at);

CREATE TABLE IF NOT EXISTS agent_memory_usage_events (
    id          TEXT PRIMARY KEY,
    thread_id   TEXT,
    source      TEXT NOT NULL,
    note        TEXT,
    used_at     TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_memory_usage_events_thread
    ON agent_memory_usage_events (thread_id, used_at);
