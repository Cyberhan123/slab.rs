ALTER TABLE model_downloads
    ADD COLUMN source_key TEXT;

ALTER TABLE model_downloads
    ADD COLUMN hub_provider TEXT;

UPDATE model_downloads
SET source_key = 'auto::' || repo_id || '::' || filename
WHERE source_key IS NULL OR TRIM(source_key) = '';

CREATE INDEX IF NOT EXISTS idx_model_downloads_source_key
    ON model_downloads(model_id, source_key, created_at DESC);

DROP INDEX IF EXISTS idx_model_downloads_active_source;

CREATE UNIQUE INDEX IF NOT EXISTS idx_model_downloads_active_source
    ON model_downloads(model_id, source_key)
    WHERE status IN ('pending', 'running');
