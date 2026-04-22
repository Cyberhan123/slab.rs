UPDATE models
SET config_schema_version = 2
WHERE config_schema_version < 2;

UPDATE models
SET config_policy_version = 3
WHERE config_policy_version < 3;
