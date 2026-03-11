-- Seed global llama context length config entry.
-- Empty value means "use backend default context length".
INSERT INTO config_store (key, name, value, updated_at)
VALUES (
    'llama_context_length',
    'Llama Context Length',
    '',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
)
ON CONFLICT(key) DO NOTHING;
