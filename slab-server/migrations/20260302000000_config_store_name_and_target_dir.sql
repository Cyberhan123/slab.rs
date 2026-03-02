-- Add a human-readable name column for future config extensibility.
ALTER TABLE config_store ADD COLUMN name TEXT NOT NULL DEFAULT '';

-- Backfill missing names from key.
UPDATE config_store
SET name = key
WHERE TRIM(COALESCE(name, '')) = '';

-- Seed a model download target-dir config entry.
-- Empty value means "not configured", so model downloads fall back to hf-hub defaults.
INSERT INTO config_store (key, name, value, updated_at)
VALUES (
    'target_dir',
    'Model Target Directory',
    '',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
)
ON CONFLICT(key) DO NOTHING;
