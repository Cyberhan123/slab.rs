CREATE TABLE IF NOT EXISTS ui_state (
    "key"       TEXT PRIMARY KEY,
    "value"     TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ui_state_updated_at
    ON ui_state(updated_at);
