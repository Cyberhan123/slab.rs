CREATE TABLE IF NOT EXISTS plugin_states (
    plugin_id TEXT PRIMARY KEY NOT NULL,
    source_kind TEXT NOT NULL,
    source_ref TEXT,
    install_root TEXT,
    installed_version TEXT,
    manifest_hash TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    runtime_status TEXT NOT NULL DEFAULT 'stopped',
    last_error TEXT,
    installed_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_seen_at TEXT,
    last_started_at TEXT,
    last_stopped_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_plugin_states_source_kind
    ON plugin_states(source_kind);

