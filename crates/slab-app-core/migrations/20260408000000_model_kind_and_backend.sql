-- Canonical model routing fields.
-- `provider` remains for legacy storage compatibility, but runtime logic now
-- uses explicit `kind` and `backend_id`.

ALTER TABLE models ADD COLUMN kind TEXT NOT NULL DEFAULT 'local';
ALTER TABLE models ADD COLUMN backend_id TEXT;

UPDATE models
SET kind = CASE
    WHEN provider LIKE 'cloud.%' OR provider = 'cloud' THEN 'cloud'
    ELSE 'local'
END;

UPDATE models
SET backend_id = CASE
    WHEN provider LIKE 'local.%' THEN substr(provider, 7)
    ELSE NULL
END;

CREATE INDEX IF NOT EXISTS idx_models_kind ON models(kind);
CREATE INDEX IF NOT EXISTS idx_models_backend_id ON models(backend_id);
