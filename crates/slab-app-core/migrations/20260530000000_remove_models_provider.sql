DROP INDEX IF EXISTS idx_models_provider;

ALTER TABLE models DROP COLUMN provider;
