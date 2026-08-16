-- Baseline schema for Slab (consolidated migration).
--
-- This single file is the squashed baseline produced by flattening the previous
-- append-only migration history (2024-01 .. 2026-07) into the current schema
-- snapshot. New deployments initialize the database from this baseline;
-- subsequent schema changes append new migration files with later timestamps.
--
-- Storage-contract rules
-- (docs/development/planning/slab-storage-contract-2026-06-17.md):
--   * json_valid() CHECK on every JSON column (sec. 2.1 / 3.1).
--   * REFERENCES ... ON DELETE CASCADE on owned child rows (sec. 2.2).
--   * Append-only (AGENTS.md, Architecture Boundaries): edit by adding a new
--     file, never by modifying this one after it has been applied.
--
-- NOTE: line endings are pinned to LF via .gitattributes (*.sql text eol=lf).
-- sqlx::migrate!() embeds the on-disk bytes at compile time and checksums
-- them; CRLF <-> LF drift between compile and apply causes a checksum
-- mismatch that aborts slab-server startup.


-- ============================================================
-- Tables
-- ============================================================

CREATE TABLE chat_sessions (
    id          TEXT    PRIMARY KEY,
    name        TEXT    NOT NULL DEFAULT '',
    state_path  TEXT,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);

CREATE TABLE config_store (
    key         TEXT    PRIMARY KEY,
    value       TEXT    NOT NULL,
    name        TEXT    NOT NULL DEFAULT '',
    updated_at  TEXT    NOT NULL
);

CREATE TABLE ui_state (
    "key"       TEXT PRIMARY KEY,
    "value"     TEXT NOT NULL,
    updated_at  TEXT NOT NULL
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

CREATE TABLE agent_memory_phase2_lock (
    id                  INTEGER PRIMARY KEY CHECK (id = 1),
    lease_owner          TEXT,
    lease_until          TEXT,
    claimed_watermark    TEXT,
    completed_watermark  TEXT,
    status               TEXT NOT NULL DEFAULT 'idle',
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE agent_memory_phase2_runs (
    id                  TEXT PRIMARY KEY,
    status              TEXT NOT NULL,
    lease_owner          TEXT,
    claimed_watermark    TEXT,
    completed_watermark  TEXT,
    started_at           TEXT NOT NULL,
    completed_at         TEXT,
    error                TEXT
);

CREATE TABLE tasks (
    id              TEXT    PRIMARY KEY,
    core_task_id    INTEGER,
    model_id        TEXT,
    task_type       TEXT    NOT NULL,
    status          TEXT    NOT NULL CHECK (
        status IN ('pending', 'running', 'succeeded', 'failed', 'cancelled', 'interrupted')
    ),
    input_data      TEXT,
    result_data     TEXT CHECK (result_data IS NULL OR json_valid(result_data)),
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
    spec                     TEXT    NOT NULL CHECK (json_valid(spec)),
    runtime_presets          TEXT CHECK (runtime_presets IS NULL OR json_valid(runtime_presets)),
    created_at               TEXT    NOT NULL,
    updated_at               TEXT    NOT NULL,
    kind                     TEXT    NOT NULL CHECK (kind IN ('local', 'cloud')),
    backend_id               TEXT,
    config_schema_version    INTEGER NOT NULL,
    config_policy_version    INTEGER NOT NULL,
    capabilities             TEXT    NOT NULL CHECK (json_valid(capabilities)),
    materialized_artifacts   TEXT    NOT NULL DEFAULT '{}' CHECK (json_valid(materialized_artifacts)),
    selected_download_source TEXT CHECK (
        selected_download_source IS NULL OR json_valid(selected_download_source)
    )
);

CREATE TABLE image_generation_tasks (
    task_id              TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    backend_id           TEXT NOT NULL,
    model_id             TEXT,
    model_path           TEXT NOT NULL,
    prompt               TEXT NOT NULL,
    negative_prompt      TEXT,
    mode                 TEXT NOT NULL,
    width                INTEGER NOT NULL CHECK (width >= 0 AND width <= 4294967295),
    height               INTEGER NOT NULL CHECK (height >= 0 AND height <= 4294967295),
    requested_count      INTEGER NOT NULL CHECK (requested_count >= 0 AND requested_count <= 4294967295),
    reference_image_path TEXT,
    primary_image_path   TEXT,
    artifact_paths       TEXT CHECK (artifact_paths IS NULL OR json_valid(artifact_paths)),
    request_data         TEXT NOT NULL CHECK (json_valid(request_data)),
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
    width                INTEGER NOT NULL CHECK (width >= 0 AND width <= 4294967295),
    height               INTEGER NOT NULL CHECK (height >= 0 AND height <= 4294967295),
    frames               INTEGER NOT NULL CHECK (frames >= 0 AND frames <= 2147483647),
    fps                  REAL NOT NULL,
    reference_image_path TEXT,
    video_path           TEXT,
    request_data         TEXT NOT NULL CHECK (json_valid(request_data)),
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
    detect_language INTEGER CHECK (detect_language IS NULL OR detect_language IN (0, 1)),
    vad_json        TEXT CHECK (vad_json IS NULL OR json_valid(vad_json)),
    decode_json     TEXT CHECK (decode_json IS NULL OR json_valid(decode_json)),
    transcript_text TEXT,
    request_data    TEXT NOT NULL CHECK (json_valid(request_data)),
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
    source_key   TEXT NOT NULL,
    hub_provider TEXT
);

CREATE TABLE model_config_state (
    model_id            TEXT PRIMARY KEY REFERENCES models(id) ON DELETE CASCADE,
    selected_preset_id  TEXT,
    selected_variant_id TEXT,
    selected_engine_id  TEXT,
    updated_at          TEXT NOT NULL
);

CREATE TABLE agent_threads (
    id              TEXT    PRIMARY KEY,
    session_id      TEXT    NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    parent_id       TEXT    REFERENCES agent_threads(id) ON DELETE SET NULL,
    depth           INTEGER NOT NULL DEFAULT 0 CHECK (depth >= 0 AND depth <= 4294967295),
    status          TEXT    NOT NULL DEFAULT 'pending' CHECK (
        status IN ('pending', 'running', 'interrupting', 'interrupted', 'completed', 'errored', 'shutdown')
    ),
    role_name       TEXT,
    config_json     TEXT    NOT NULL DEFAULT '{}' CHECK (json_valid(config_json)),
    completion_text TEXT,
    created_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL,
    archived_at     TEXT
);

CREATE TABLE agent_turn_states (
    thread_id           TEXT    NOT NULL REFERENCES agent_threads(id) ON DELETE CASCADE,
    turn_index          INTEGER NOT NULL CHECK (turn_index >= 0 AND turn_index <= 4294967295),
    status              TEXT    NOT NULL,
    input_messages_json TEXT,
    tool_specs_json     TEXT,
    llm_response_json   TEXT,
    error               TEXT,
    started_at          TEXT    NOT NULL,
    completed_at        TEXT,
    PRIMARY KEY (thread_id, turn_index)
);

CREATE TABLE agent_tool_calls (
    id              TEXT    PRIMARY KEY,
    thread_id       TEXT    NOT NULL REFERENCES agent_threads(id) ON DELETE CASCADE,
    tool_name       TEXT    NOT NULL,
    arguments       TEXT    NOT NULL DEFAULT '{}' CHECK (json_valid(arguments)),
    output          TEXT,
    status          TEXT    NOT NULL DEFAULT 'pending' CHECK (
        status IN ('pending', 'running', 'completed', 'failed')
    ),
    created_at      TEXT    NOT NULL,
    completed_at    TEXT
);

CREATE TABLE agent_thread_messages (
    id         TEXT    PRIMARY KEY,
    thread_id  TEXT    NOT NULL REFERENCES agent_threads(id) ON DELETE CASCADE,
    turn_index INTEGER NOT NULL CHECK (turn_index >= 0 AND turn_index <= 4294967295),
    role       TEXT    NOT NULL CHECK (
        role IN ('system', 'developer', 'user', 'assistant', 'tool', 'function')
    ),
    content    TEXT    NOT NULL,
    created_at TEXT    NOT NULL
);

CREATE TABLE plugin_states (
    plugin_id TEXT PRIMARY KEY NOT NULL,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('dev', 'import_pack', 'package_url')),
    source_ref TEXT,
    install_root TEXT,
    installed_version TEXT,
    manifest_hash TEXT,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    runtime_status TEXT NOT NULL DEFAULT 'stopped' CHECK (
        runtime_status IN ('running', 'stopped', 'error')
    ),
    last_error TEXT,
    installed_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_seen_at TEXT,
    last_started_at TEXT,
    last_stopped_at TEXT
);

CREATE TABLE agent_memory_phase1_outputs (
    thread_id                             TEXT    PRIMARY KEY REFERENCES agent_threads(id) ON DELETE CASCADE,
    session_id                            TEXT    NOT NULL,
    status                                TEXT    NOT NULL DEFAULT 'pending' CHECK (
        status IN ('pending', 'running', 'succeeded', 'succeeded_no_output', 'failed')
    ),
    raw_memory                            TEXT,
    rollout_summary                       TEXT,
    rollout_slug                          TEXT,
    source_updated_at                     TEXT,
    generated_at                          TEXT,
    lease_owner                           TEXT,
    lease_until                           TEXT,
    attempts                              INTEGER NOT NULL DEFAULT 0,
    next_retry_at                         TEXT,
    selected_for_phase2                   INTEGER NOT NULL DEFAULT 0 CHECK (selected_for_phase2 IN (0, 1)),
    selected_for_phase2_source_updated_at TEXT,
    last_usage                            TEXT,
    usage_count                           INTEGER NOT NULL DEFAULT 0,
    error                                 TEXT,
    updated_at                            TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE agent_memory_usage_events (
    id          TEXT PRIMARY KEY,
    thread_id   TEXT,
    source      TEXT NOT NULL,
    source_kind TEXT NOT NULL DEFAULT 'unknown' CHECK (
        source_kind IN (
            'unknown',
            'memory_summary',
            'memory_registry',
            'raw_memory',
            'rollout_summary'
        )
    ),
    note        TEXT,
    used_at     TEXT NOT NULL
);

CREATE TABLE agent_thread_responses (
    run_id           TEXT    PRIMARY KEY,
    thread_id        TEXT    NOT NULL REFERENCES agent_threads(id) ON DELETE CASCADE,
    session_id       TEXT    NOT NULL,
    turn_index_start INTEGER NOT NULL,
    status           TEXT    NOT NULL
        CHECK (status IN ('completed', 'failed', 'cancelled', 'incomplete')),
    -- Complete canonical OpenAI Response object (serialized slab_proto::openai::Response).
    response_json    TEXT    NOT NULL CHECK (json_valid(response_json)),
    created_at       TEXT    NOT NULL,
    completed_at     TEXT
);

CREATE TABLE agent_turn_items (
    id          TEXT    NOT NULL,
    thread_id   TEXT    NOT NULL REFERENCES agent_threads(id) ON DELETE CASCADE,
    turn_index  INTEGER NOT NULL CHECK (turn_index >= 0),
    seq         INTEGER NOT NULL CHECK (seq >= 0),
    item_json   TEXT    NOT NULL CHECK (json_valid(item_json)),
    created_at  TEXT    NOT NULL,
    PRIMARY KEY (thread_id, turn_index, seq)
);

-- Per-workspace exec-policy rule file mapping. Records which
-- `hash-<workspace>.rules` file corresponds to which absolute workspace path
-- so the engine can lazy-load per-workspace rules without scanning the dir.
CREATE TABLE exec_rule_workspaces (
    rules_filename  TEXT PRIMARY KEY,
    workspace_path  TEXT NOT NULL,
    created_at      TEXT NOT NULL
);

-- ============================================================
-- Indexes
-- ============================================================
CREATE INDEX idx_ui_state_updated_at
    ON ui_state(updated_at);
CREATE INDEX idx_chat_messages_session ON chat_messages(session_id, created_at);
CREATE INDEX idx_agent_memory_phase2_runs_status
    ON agent_memory_phase2_runs (status, started_at);
CREATE INDEX idx_tasks_status     ON tasks(status);
CREATE INDEX idx_tasks_task_type  ON tasks(task_type);
CREATE INDEX idx_tasks_created_at ON tasks(created_at);
CREATE INDEX idx_tasks_model_id   ON tasks(model_id);
CREATE UNIQUE INDEX idx_tasks_core_task_id
    ON tasks(core_task_id)
    WHERE core_task_id IS NOT NULL;
CREATE INDEX idx_models_status    ON models(status);
CREATE INDEX idx_models_kind      ON models(kind);
CREATE INDEX idx_models_backend_id ON models(backend_id);
CREATE INDEX idx_image_generation_tasks_created_at
    ON image_generation_tasks(created_at DESC);
CREATE INDEX idx_image_generation_tasks_model_id
    ON image_generation_tasks(model_id);
CREATE INDEX idx_video_generation_tasks_created_at
    ON video_generation_tasks(created_at DESC);
CREATE INDEX idx_video_generation_tasks_model_id
    ON video_generation_tasks(model_id);
CREATE INDEX idx_audio_transcription_tasks_created_at
    ON audio_transcription_tasks(created_at DESC);
CREATE INDEX idx_audio_transcription_tasks_model_id
    ON audio_transcription_tasks(model_id);
CREATE INDEX idx_model_downloads_model_id
    ON model_downloads(model_id);
CREATE INDEX idx_model_downloads_source
    ON model_downloads(model_id, repo_id, filename, created_at DESC);
CREATE INDEX idx_model_downloads_status
    ON model_downloads(status);
CREATE INDEX idx_model_downloads_source_key
    ON model_downloads(model_id, source_key, created_at DESC);
CREATE UNIQUE INDEX idx_model_downloads_active_source
    ON model_downloads(model_id, source_key)
    WHERE status IN ('pending', 'running');
CREATE INDEX idx_model_config_state_updated_at
    ON model_config_state(updated_at);
CREATE INDEX idx_agent_threads_session ON agent_threads(session_id);
CREATE INDEX idx_agent_threads_parent  ON agent_threads(parent_id);
CREATE INDEX idx_agent_threads_status  ON agent_threads(status);
CREATE INDEX idx_agent_turn_states_status
    ON agent_turn_states (status, started_at);
CREATE INDEX idx_agent_tool_calls_thread ON agent_tool_calls(thread_id, created_at);
CREATE INDEX idx_atm_thread
    ON agent_thread_messages (thread_id, turn_index);
CREATE INDEX idx_plugin_states_source_kind
    ON plugin_states(source_kind);
CREATE INDEX idx_agent_memory_phase1_status
    ON agent_memory_phase1_outputs (status, next_retry_at, lease_until);
CREATE INDEX idx_agent_memory_phase1_selected
    ON agent_memory_phase1_outputs (selected_for_phase2, source_updated_at);
CREATE INDEX idx_agent_memory_phase1_usage
    ON agent_memory_phase1_outputs (last_usage, generated_at, usage_count);
CREATE INDEX idx_agent_memory_usage_events_thread
    ON agent_memory_usage_events (thread_id, used_at);
CREATE INDEX idx_agent_memory_usage_events_source_kind
    ON agent_memory_usage_events (source_kind, used_at);
CREATE INDEX idx_atr_thread
    ON agent_thread_responses (thread_id, created_at);
