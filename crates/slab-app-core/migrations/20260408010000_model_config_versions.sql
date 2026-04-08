-- Track model config upgrades in two dimensions:
-- 1. `config_schema_version` for structural/storage shape changes.
-- 2. `config_policy_version` for semantic/value merge changes.

ALTER TABLE models ADD COLUMN config_schema_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE models ADD COLUMN config_policy_version INTEGER NOT NULL DEFAULT 1;
