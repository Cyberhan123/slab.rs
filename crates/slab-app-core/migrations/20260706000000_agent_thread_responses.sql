-- Migration: per-run OpenAI-Responses-canonical Response storage.
-- Replaces the per-message agent_thread_messages representation for the
-- /v1/agents/responses flow: one row per agent run holding the COMPLETE
-- serialized slab_proto::openai::Response JSON (the same shape as
-- testdata/fixtures/openai-compatible/responses/*.json), not per-message
-- chunks.
--
-- Storage-contract rules (docs/development/planning/slab-storage-contract-2026-06-17.md):
--   - json_valid() CHECK on the JSON column (§2.1/§3.1).
--   - CHECK (status IN (...)) on the enum column (§3.6).
--   - append-only migration (AGENTS.md §32).

CREATE TABLE IF NOT EXISTS agent_thread_responses (
    run_id           TEXT    PRIMARY KEY,
    thread_id        TEXT    NOT NULL REFERENCES agent_threads(id) ON DELETE CASCADE,
    session_id       TEXT    NOT NULL,
    turn_index_start INTEGER NOT NULL,
    status           TEXT    NOT NULL
        CHECK (status IN ('completed', 'failed', 'cancelled', 'incomplete')),
    -- Complete canonical OpenAI Response object (serialized slab_proto::openai::Response).
    response_json    TEXT    NOT NULL CHECK (json_valid(response_json)),
    created_at       TEXT    NOT NULL,
    completed_at     TEXT
);

CREATE INDEX IF NOT EXISTS idx_atr_thread
    ON agent_thread_responses (thread_id, created_at);
