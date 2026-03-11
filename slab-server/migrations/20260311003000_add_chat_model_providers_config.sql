-- Chat cloud provider configuration.
-- Value format: JSON array.
INSERT INTO config_store (key, name, value, updated_at)
VALUES (
    'chat_model_providers',
    'Chat Model Providers',
    '[]',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
)
ON CONFLICT(key) DO NOTHING;

