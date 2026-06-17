DROP INDEX IF EXISTS idx_model_config_state_updated_at;

CREATE TABLE model_config_state_rebuild (
    model_id            TEXT PRIMARY KEY REFERENCES models(id) ON DELETE CASCADE,
    selected_preset_id  TEXT,
    selected_variant_id TEXT,
    updated_at          TEXT NOT NULL
);

INSERT INTO model_config_state_rebuild (
    model_id,
    selected_preset_id,
    selected_variant_id,
    updated_at
)
SELECT
    state.model_id,
    state.selected_preset_id,
    state.selected_variant_id,
    state.updated_at
FROM model_config_state AS state
WHERE EXISTS (
    SELECT 1
    FROM models
    WHERE models.id = state.model_id
);

DROP TABLE model_config_state;

ALTER TABLE model_config_state_rebuild RENAME TO model_config_state;

CREATE INDEX IF NOT EXISTS idx_model_config_state_updated_at
    ON model_config_state(updated_at);
