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
DROP INDEX IF EXISTS idx_models_backend_id;
DROP INDEX IF EXISTS idx_models_kind;
DROP INDEX IF EXISTS idx_models_status;
DROP INDEX IF EXISTS idx_model_config_state_updated_at;
DROP INDEX IF EXISTS idx_tasks_model_id;
DROP INDEX IF EXISTS idx_tasks_created_at;
DROP INDEX IF EXISTS idx_tasks_task_type;
DROP INDEX IF EXISTS idx_tasks_status;
DROP INDEX IF EXISTS idx_agent_threads_session;
DROP INDEX IF EXISTS idx_agent_threads_parent;
DROP INDEX IF EXISTS idx_agent_threads_status;
DROP INDEX IF EXISTS idx_agent_turn_states_status;
DROP INDEX IF EXISTS idx_agent_tool_calls_thread;
DROP INDEX IF EXISTS idx_atm_thread;
DROP INDEX IF EXISTS idx_plugin_states_source_kind;
DROP INDEX IF EXISTS idx_agent_memory_phase1_status;
DROP INDEX IF EXISTS idx_agent_memory_phase1_selected;
DROP INDEX IF EXISTS idx_agent_memory_phase1_usage;
DROP INDEX IF EXISTS idx_agent_memory_usage_events_thread;
DROP INDEX IF EXISTS idx_agent_memory_usage_events_source_kind;

UPDATE model_downloads
SET source_key = 'auto::' || repo_id || '::' || filename
WHERE source_key IS NULL OR TRIM(source_key) = '';

UPDATE agent_threads
SET config_json = '{}'
WHERE config_json IS NULL OR TRIM(config_json) = '';

CREATE TABLE _storage_contract_guard (
    ok INTEGER NOT NULL CHECK (ok = 1)
);

INSERT INTO _storage_contract_guard (ok)
SELECT 0 WHERE EXISTS (SELECT 1 FROM models WHERE NOT json_valid(spec));
INSERT INTO _storage_contract_guard (ok)
SELECT 0 WHERE EXISTS (
    SELECT 1 FROM models WHERE materialized_artifacts IS NULL OR NOT json_valid(materialized_artifacts)
);
INSERT INTO _storage_contract_guard (ok)
SELECT 0 WHERE EXISTS (
    SELECT 1 FROM models WHERE selected_download_source IS NOT NULL AND NOT json_valid(selected_download_source)
);
INSERT INTO _storage_contract_guard (ok)
SELECT 0 WHERE EXISTS (
    SELECT 1 FROM models WHERE runtime_presets IS NOT NULL AND NOT json_valid(runtime_presets)
);
INSERT INTO _storage_contract_guard (ok)
SELECT 0 WHERE EXISTS (SELECT 1 FROM models WHERE NOT json_valid(capabilities));
INSERT INTO _storage_contract_guard (ok)
SELECT 0 WHERE EXISTS (SELECT 1 FROM agent_threads WHERE NOT json_valid(config_json));
INSERT INTO _storage_contract_guard (ok)
SELECT 0 WHERE EXISTS (
    SELECT 1 FROM image_generation_tasks WHERE artifact_paths IS NOT NULL AND NOT json_valid(artifact_paths)
);
INSERT INTO _storage_contract_guard (ok)
SELECT 0 WHERE EXISTS (SELECT 1 FROM tasks WHERE result_data IS NOT NULL AND NOT json_valid(result_data));
INSERT INTO _storage_contract_guard (ok)
SELECT 0 WHERE EXISTS (
    SELECT 1 FROM tasks WHERE core_task_id IS NOT NULL GROUP BY core_task_id HAVING COUNT(*) > 1
);

UPDATE tasks
SET result_data = (
    SELECT CASE
        WHEN json_valid(media.result_data) THEN json_object(
            'kind', 'task_result',
            'version', 1,
            'data', json(media.result_data)
        )
        ELSE json_object(
            'kind', 'task_result',
            'version', 1,
            'data', media.result_data
        )
    END
    FROM image_generation_tasks AS media
    WHERE media.task_id = tasks.id
      AND media.result_data IS NOT NULL
)
WHERE result_data IS NULL
  AND EXISTS (
      SELECT 1
      FROM image_generation_tasks AS media
      WHERE media.task_id = tasks.id
        AND media.result_data IS NOT NULL
  );

UPDATE tasks
SET result_data = (
    SELECT CASE
        WHEN json_valid(media.result_data) THEN json_object(
            'kind', 'task_result',
            'version', 1,
            'data', json(media.result_data)
        )
        ELSE json_object(
            'kind', 'task_result',
            'version', 1,
            'data', media.result_data
        )
    END
    FROM video_generation_tasks AS media
    WHERE media.task_id = tasks.id
      AND media.result_data IS NOT NULL
)
WHERE result_data IS NULL
  AND EXISTS (
      SELECT 1
      FROM video_generation_tasks AS media
      WHERE media.task_id = tasks.id
        AND media.result_data IS NOT NULL
  );

UPDATE tasks
SET result_data = (
    SELECT CASE
        WHEN json_valid(media.result_data) THEN json_object(
            'kind', 'task_result',
            'version', 1,
            'data', json(media.result_data)
        )
        ELSE json_object(
            'kind', 'task_result',
            'version', 1,
            'data', media.result_data
        )
    END
    FROM audio_transcription_tasks AS media
    WHERE media.task_id = tasks.id
      AND media.result_data IS NOT NULL
)
WHERE result_data IS NULL
  AND EXISTS (
      SELECT 1
      FROM audio_transcription_tasks AS media
      WHERE media.task_id = tasks.id
        AND media.result_data IS NOT NULL
  );

DROP TABLE _storage_contract_guard;

ALTER TABLE tasks RENAME TO tasks_old_contract;
ALTER TABLE models RENAME TO models_old_contract;
ALTER TABLE image_generation_tasks RENAME TO image_generation_tasks_old_contract;
ALTER TABLE video_generation_tasks RENAME TO video_generation_tasks_old_contract;
ALTER TABLE audio_transcription_tasks RENAME TO audio_transcription_tasks_old_contract;
ALTER TABLE model_downloads RENAME TO model_downloads_old_contract;
ALTER TABLE model_config_state RENAME TO model_config_state_old_contract;
ALTER TABLE agent_threads RENAME TO agent_threads_old_contract;
ALTER TABLE agent_turn_states RENAME TO agent_turn_states_old_contract;
ALTER TABLE agent_tool_calls RENAME TO agent_tool_calls_old_contract;
ALTER TABLE agent_thread_messages RENAME TO agent_thread_messages_old_contract;
ALTER TABLE plugin_states RENAME TO plugin_states_old_contract;
ALTER TABLE agent_memory_phase1_outputs RENAME TO agent_memory_phase1_outputs_old_contract;
ALTER TABLE agent_memory_usage_events RENAME TO agent_memory_usage_events_old_contract;

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
    updated_at      TEXT    NOT NULL
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

INSERT INTO tasks (
    id, core_task_id, model_id, task_type, status, input_data, result_data, error_msg, created_at,
    updated_at
)
SELECT
    id, core_task_id, model_id, task_type, status, input_data, result_data, error_msg, created_at,
    updated_at
FROM tasks_old_contract;

INSERT INTO models (
    id, display_name, status, spec, runtime_presets, created_at, updated_at, kind, backend_id,
    config_schema_version, config_policy_version, capabilities, materialized_artifacts,
    selected_download_source
)
SELECT
    id, display_name, status, spec, runtime_presets, created_at, updated_at, kind, backend_id,
    config_schema_version, config_policy_version, capabilities, materialized_artifacts,
    selected_download_source
FROM models_old_contract;

INSERT INTO image_generation_tasks (
    task_id, backend_id, model_id, model_path, prompt, negative_prompt, mode, width, height,
    requested_count, reference_image_path, primary_image_path, artifact_paths, request_data,
    created_at, updated_at
)
SELECT
    task_id, backend_id, model_id, model_path, prompt, negative_prompt, mode, width, height,
    requested_count, reference_image_path, primary_image_path, artifact_paths, request_data,
    created_at, updated_at
FROM image_generation_tasks_old_contract;

INSERT INTO video_generation_tasks (
    task_id, backend_id, model_id, model_path, prompt, negative_prompt, width, height, frames, fps,
    reference_image_path, video_path, request_data, created_at, updated_at
)
SELECT
    task_id, backend_id, model_id, model_path, prompt, negative_prompt, width, height, frames, fps,
    reference_image_path, video_path, request_data, created_at, updated_at
FROM video_generation_tasks_old_contract;

INSERT INTO audio_transcription_tasks (
    task_id, backend_id, model_id, source_path, language, prompt, detect_language, vad_json,
    decode_json, transcript_text, request_data, created_at, updated_at
)
SELECT
    task_id, backend_id, model_id, source_path, language, prompt, detect_language, vad_json,
    decode_json, transcript_text, request_data, created_at, updated_at
FROM audio_transcription_tasks_old_contract;

INSERT INTO model_downloads (
    task_id, model_id, repo_id, filename, status, error_msg, created_at, updated_at, source_key,
    hub_provider
)
SELECT
    task_id, model_id, repo_id, filename, status, error_msg, created_at, updated_at, source_key,
    hub_provider
FROM model_downloads_old_contract;

INSERT INTO model_config_state (
    model_id, selected_preset_id, selected_variant_id, selected_engine_id, updated_at
)
SELECT
    model_id, selected_preset_id, selected_variant_id, selected_engine_id, updated_at
FROM model_config_state_old_contract;

INSERT INTO agent_threads (
    id, session_id, parent_id, depth, status, role_name, config_json, completion_text, created_at,
    updated_at
)
SELECT
    id, session_id, parent_id, depth, status, role_name, config_json, completion_text, created_at,
    updated_at
FROM agent_threads_old_contract;

INSERT INTO agent_turn_states (
    thread_id, turn_index, status, input_messages_json, tool_specs_json, llm_response_json, error,
    started_at, completed_at
)
SELECT
    thread_id, turn_index, status, input_messages_json, tool_specs_json, llm_response_json, error,
    started_at, completed_at
FROM agent_turn_states_old_contract;

INSERT INTO agent_tool_calls (
    id, thread_id, tool_name, arguments, output, status, created_at, completed_at
)
SELECT
    id, thread_id, tool_name, arguments, output, status, created_at, completed_at
FROM agent_tool_calls_old_contract;

INSERT INTO agent_thread_messages (id, thread_id, turn_index, role, content, created_at)
SELECT id, thread_id, turn_index, role, content, created_at
FROM agent_thread_messages_old_contract;

INSERT INTO plugin_states (
    plugin_id, source_kind, source_ref, install_root, installed_version, manifest_hash, enabled,
    runtime_status, last_error, installed_at, updated_at, last_seen_at, last_started_at,
    last_stopped_at
)
SELECT
    plugin_id, source_kind, source_ref, install_root, installed_version, manifest_hash, enabled,
    runtime_status, last_error, installed_at, updated_at, last_seen_at, last_started_at,
    last_stopped_at
FROM plugin_states_old_contract;

INSERT INTO agent_memory_phase1_outputs (
    thread_id, session_id, status, raw_memory, rollout_summary, rollout_slug, source_updated_at,
    generated_at, lease_owner, lease_until, attempts, next_retry_at, selected_for_phase2,
    selected_for_phase2_source_updated_at, last_usage, usage_count, error, updated_at
)
SELECT
    thread_id, session_id, status, raw_memory, rollout_summary, rollout_slug, source_updated_at,
    generated_at, lease_owner, lease_until, attempts, next_retry_at, selected_for_phase2,
    selected_for_phase2_source_updated_at, last_usage, usage_count, error, updated_at
FROM agent_memory_phase1_outputs_old_contract;

INSERT INTO agent_memory_usage_events (id, thread_id, source, source_kind, note, used_at)
SELECT id, thread_id, source, source_kind, note, used_at
FROM agent_memory_usage_events_old_contract;

DROP TABLE agent_memory_usage_events_old_contract;
DROP TABLE agent_memory_phase1_outputs_old_contract;
DROP TABLE plugin_states_old_contract;
DROP TABLE agent_thread_messages_old_contract;
DROP TABLE agent_tool_calls_old_contract;
DROP TABLE agent_turn_states_old_contract;
DROP TABLE agent_threads_old_contract;
DROP TABLE model_config_state_old_contract;
DROP TABLE model_downloads_old_contract;
DROP TABLE audio_transcription_tasks_old_contract;
DROP TABLE video_generation_tasks_old_contract;
DROP TABLE image_generation_tasks_old_contract;
DROP TABLE models_old_contract;
DROP TABLE tasks_old_contract;

CREATE INDEX IF NOT EXISTS idx_tasks_status     ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_task_type  ON tasks(task_type);
CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at);
CREATE INDEX IF NOT EXISTS idx_tasks_model_id   ON tasks(model_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_core_task_id
    ON tasks(core_task_id)
    WHERE core_task_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_models_status    ON models(status);
CREATE INDEX IF NOT EXISTS idx_models_kind      ON models(kind);
CREATE INDEX IF NOT EXISTS idx_models_backend_id ON models(backend_id);
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
CREATE INDEX IF NOT EXISTS idx_model_config_state_updated_at
    ON model_config_state(updated_at);
CREATE INDEX IF NOT EXISTS idx_agent_threads_session ON agent_threads(session_id);
CREATE INDEX IF NOT EXISTS idx_agent_threads_parent  ON agent_threads(parent_id);
CREATE INDEX IF NOT EXISTS idx_agent_threads_status  ON agent_threads(status);
CREATE INDEX IF NOT EXISTS idx_agent_turn_states_status
    ON agent_turn_states (status, started_at);
CREATE INDEX IF NOT EXISTS idx_agent_tool_calls_thread ON agent_tool_calls(thread_id, created_at);
CREATE INDEX IF NOT EXISTS idx_atm_thread
    ON agent_thread_messages (thread_id, turn_index);
CREATE INDEX IF NOT EXISTS idx_plugin_states_source_kind
    ON plugin_states(source_kind);
CREATE INDEX IF NOT EXISTS idx_agent_memory_phase1_status
    ON agent_memory_phase1_outputs (status, next_retry_at, lease_until);
CREATE INDEX IF NOT EXISTS idx_agent_memory_phase1_selected
    ON agent_memory_phase1_outputs (selected_for_phase2, source_updated_at);
CREATE INDEX IF NOT EXISTS idx_agent_memory_phase1_usage
    ON agent_memory_phase1_outputs (last_usage, generated_at, usage_count);
CREATE INDEX IF NOT EXISTS idx_agent_memory_usage_events_thread
    ON agent_memory_usage_events (thread_id, used_at);
CREATE INDEX IF NOT EXISTS idx_agent_memory_usage_events_source_kind
    ON agent_memory_usage_events (source_kind, used_at);
