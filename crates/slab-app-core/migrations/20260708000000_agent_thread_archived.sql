-- Migration: add archived_at to agent_threads.
-- Backs the harness `thread/archive` method. Archived threads are excluded
-- from `thread/list` unless the caller opts in via `include_archived`.
-- `archived_at` is NULL for live threads and an RFC 3339 timestamp once
-- archived; existing rows default to NULL (live).
--
-- append-only migration (AGENTS.md §32).

ALTER TABLE agent_threads ADD COLUMN archived_at TEXT;
