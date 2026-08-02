-- Slice 5: rollout event-source L2 index + backfill state.
--
-- Introduces the index tables that back the rollout read gate
-- (`rollout_session_index.backfill_status`) and the legacy-to-rollout backfill
-- (`rollout_backfill_state`). The legacy three conversation tables
-- (`agent_thread_messages` / `agent_turn_states` / `agent_turn_items`) are kept
-- as a read-only backfill source (dropped in a later slice); the zombie
-- `agent_thread_responses` table — which has had no live writer since the
-- 2026-07-08 removal of response persistence — is dropped now.

-- One row per thread that owns a rollout JSONL file. `backfill_status` is the
-- read gate: reads go rollout-first ONLY when it is 'completed' (a legacy
-- thread stays on the SQL fallback until the startup backfill copies its rows
-- into the rollout file and flips this to 'completed'). The default 'completed'
-- makes NEW threads (inserted by the rollout adapter's upsert_thread) rollout-
-- native from birth — no backfill needed.
CREATE TABLE rollout_session_index (
    thread_id        TEXT    PRIMARY KEY REFERENCES agent_threads(id) ON DELETE CASCADE,
    session_id       TEXT    NOT NULL,
    file_path        TEXT    NOT NULL,
    line_count       INTEGER NOT NULL DEFAULT 0,
    last_turn_index  INTEGER NOT NULL DEFAULT 0,
    last_item_id     TEXT,
    last_updated_at  TEXT    NOT NULL,
    created_at       TEXT    NOT NULL,
    backfill_status  TEXT    NOT NULL DEFAULT 'completed' CHECK (
        backfill_status IN ('pending', 'in_progress', 'completed', 'failed')
    )
);

CREATE INDEX idx_rollout_session ON rollout_session_index(session_id, last_updated_at DESC);

-- Detailed progress for the startup backfill task (started_at / completed_at +
-- an error column for diagnostics). Orthogonal to rollout_session_index, which
-- is the load-bearing read gate.
CREATE TABLE rollout_backfill_state (
    thread_id      TEXT    PRIMARY KEY REFERENCES agent_threads(id) ON DELETE CASCADE,
    status         TEXT    NOT NULL DEFAULT 'pending' CHECK (
        status IN ('pending', 'in_progress', 'completed', 'failed')
    ),
    lines_written  INTEGER NOT NULL DEFAULT 0,
    error          TEXT,
    started_at     TEXT,
    completed_at   TEXT
);

-- Zombie table: stored a full canonical OpenAI Response JSON per run. No live
-- writer since 2026-07-08; the rollout true source plus SQL metadata replace it.
DROP TABLE IF EXISTS agent_thread_responses;
DROP INDEX IF EXISTS idx_atr_thread;
