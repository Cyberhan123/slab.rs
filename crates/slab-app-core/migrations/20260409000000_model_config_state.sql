CREATE TABLE IF NOT EXISTS model_config_state (
    model_id             TEXT PRIMARY KEY REFERENCES models(id) ON DELETE CASCADE,
    selected_preset_id   TEXT,
    selected_variant_id  TEXT,
    updated_at           TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_model_config_state_updated_at
    ON model_config_state(updated_at);
