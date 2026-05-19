-- Migration: agent thread message history
-- Stores per-turn message history for agent threads, enabling replay and
-- thread forking.

CREATE TABLE IF NOT EXISTS agent_thread_messages (
    id         TEXT    PRIMARY KEY,
    thread_id  TEXT    NOT NULL,
    turn_index INTEGER NOT NULL,
    role       TEXT    NOT NULL,
    content    TEXT    NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_atm_thread
    ON agent_thread_messages (thread_id, turn_index);
