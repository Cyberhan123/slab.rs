-- Seed global worker config entries for each backend.
-- Empty value means "use server default (1)".
INSERT INTO config_store (key, name, value, updated_at)
VALUES (
    'llama_num_workers',
    'Llama Workers',
    '',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
)
ON CONFLICT(key) DO NOTHING;

INSERT INTO config_store (key, name, value, updated_at)
VALUES (
    'whisper_num_workers',
    'Whisper Workers',
    '',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
)
ON CONFLICT(key) DO NOTHING;

INSERT INTO config_store (key, name, value, updated_at)
VALUES (
    'diffusion_num_workers',
    'Diffusion Workers',
    '',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
)
ON CONFLICT(key) DO NOTHING;
