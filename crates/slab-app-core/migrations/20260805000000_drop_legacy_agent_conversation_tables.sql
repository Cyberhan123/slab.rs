-- Slice E: drop the legacy agent conversation tables + agent_tool_calls audit
-- table + the rollout backfill state table.
--
-- After the rollout JSONL event-source refactor (Part 1 + Part 2 + slices
-- 0/C/F/D1/D2a/D2b) the rollout true source is the ONLY live writer/reader for
-- agent conversation, turn state, finalized turn items, and tool-call capture
-- (tool calls are now captured as `TurnItem::CommandExecution` / `McpToolCall`
-- in the rollout stream). The four legacy tables below are no longer written
-- and have no live reader; `rollout_backfill_state` tracked the one-shot
-- startup backfill that is itself being removed this slice (rollout is now the
-- only source, so there is nothing to backfill).
--
-- `rollout_session_index` is KEPT (its `backfill_status` / `line_count` columns
-- still back the D2a list ghost-gate; the `backfill_status` name is now a
-- misnomer — it is effectively a rollout-native marker — but renaming the
-- column is out of scope for this append-only migration).
--
-- Append-only: drops only. Order is indexes first, then tables. All statements
-- use IF EXISTS so re-running against a DB that already dropped a table is a
-- no-op (mirrors the 20260802 zombie-table drop).

-- Legacy indexes (initial.sql).
DROP INDEX IF EXISTS idx_atm_thread;
DROP INDEX IF EXISTS idx_agent_turn_states_status;
DROP INDEX IF EXISTS idx_agent_tool_calls_thread;

-- Legacy conversation + audit + backfill tables.
DROP TABLE IF EXISTS agent_thread_messages;
DROP TABLE IF EXISTS agent_turn_states;
DROP TABLE IF EXISTS agent_turn_items;
DROP TABLE IF EXISTS agent_tool_calls;
DROP TABLE IF EXISTS rollout_backfill_state;
