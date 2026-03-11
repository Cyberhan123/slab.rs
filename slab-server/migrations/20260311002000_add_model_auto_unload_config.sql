-- Seed model auto-unload config entries.
-- Disabled by default. When enabled, models are auto-unloaded after idle timeout.
INSERT INTO config_store (key, name, value, updated_at)
VALUES (
    'model_auto_unload_enabled',
    'Model Auto Unload Enabled',
    'false',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
)
ON CONFLICT(key) DO NOTHING;

INSERT INTO config_store (key, name, value, updated_at)
VALUES (
    'model_auto_unload_idle_minutes',
    'Model Auto Unload Idle Minutes',
    '10',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
)
ON CONFLICT(key) DO NOTHING;
