-- Migration: full-fidelity harness TurnItem snapshots.
--
-- The live HarnessProjection emits rich `item/completed` TurnItems (agentMessage
-- text, reasoning summary+content, commandExecution command/cwd/output/exit_code,
-- fileChange diffs, mcpToolCall result/error, webSearch query) but none were
-- persisted, so `thread/resume` re-synthesized lossy items from
-- ConversationMessages. This table stores one finalized (item/completed)
-- TurnItem per row so resume can replay full-fidelity history. Delta events are
-- NOT persisted — completed items carry the full content.
--
-- `seq` is a per-thread monotonic orderer assigned by the persistence observer
-- and is stable across replays (events are delivered in order, so the same
-- ItemCompleted stream yields the same seq assignments). The PK is therefore
-- (thread_id, turn_index, seq): it is the natural exactly-once key, immune to
-- TurnItem.id collisions (item ids are turn-index-based for text, UUIDs for
-- tool calls, but LLM-provided tool_call ids are not guaranteed unique), and
-- lets forked children reuse the same logical item id without remapping (the
-- child thread_id distinguishes them). The PK index also serves the resume
-- `(thread_id, turn_index, seq)` ordering lookup.
--
-- Storage-contract rules (docs/development/planning/slab-storage-contract-2026-06-17.md):
--   - json_valid() CHECK on the JSON column (§2.1/§3.1).
--   - FK thread_id -> agent_threads ON DELETE CASCADE.
--   - append-only migration (AGENTS.md §32).

CREATE TABLE IF NOT EXISTS agent_turn_items (
    id          TEXT    NOT NULL,
    thread_id   TEXT    NOT NULL REFERENCES agent_threads(id) ON DELETE CASCADE,
    turn_index  INTEGER NOT NULL CHECK (turn_index >= 0),
    seq         INTEGER NOT NULL CHECK (seq >= 0),
    item_json   TEXT    NOT NULL CHECK (json_valid(item_json)),
    created_at  TEXT    NOT NULL,
    PRIMARY KEY (thread_id, turn_index, seq)
);
