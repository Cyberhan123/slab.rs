-- Per-project memory stores (Claude memdir model).
--
-- Memories used to live in ONE global workspace (`<memory_root>/MEMORY.md`,
-- `memory_summary.md`, `rollout_summaries/`, ...) shared by every project a
-- user touched, so project A's rollout experience leaked into project B's
-- consolidated memory. The filesystem now shards by project:
-- `<memory_root>/projects/<project-key>/...` where the key is the sanitized
-- canonical git root of the workspace (worktrees share one store), falling
-- back to the sanitized workspace root for non-git directories.
--
-- The DB follows the shard:
--   * `agent_memory_phase1_outputs.project_key` routes each extracted
--     rollout to the project it ran in (the workspace root at claim time).
--     `''` is the sentinel for "no workspace bound".
--   * `agent_memory_phase2_locks` replaces the `CHECK (id = 1)` singleton
--     lock with a keyed lock (job_key = project_key) so consolidations for
--     different projects never contend; each project keeps its own
--     claimed/completed watermark pair for the delta-skip gate.
--   * `agent_memory_phase2_runs.project_key` attributes each consolidation
--     run to its project.
--   * `agent_memory_usage_events.project_key` stamps the project a citation
--     or tool-read happened in.
--
-- Legacy rows default to project_key = '' and are backfilled by the startup
-- adoption step (`adopt_legacy_layout` moving the old flat files into the
-- current project's store followed by a one-shot UPDATE), so an upgrade
-- never orphans pre-existing memories.

ALTER TABLE agent_memory_phase1_outputs ADD COLUMN project_key TEXT NOT NULL DEFAULT '';

CREATE INDEX idx_agent_memory_phase1_project
    ON agent_memory_phase1_outputs (project_key, status, next_retry_at, lease_until);

ALTER TABLE agent_memory_phase2_runs ADD COLUMN project_key TEXT NOT NULL DEFAULT '';

CREATE INDEX idx_agent_memory_phase2_runs_project
    ON agent_memory_phase2_runs (project_key, started_at);

ALTER TABLE agent_memory_usage_events ADD COLUMN project_key TEXT NOT NULL DEFAULT '';

CREATE TABLE agent_memory_phase2_locks (
    job_key              TEXT PRIMARY KEY,
    lease_owner          TEXT,
    lease_until          TEXT,
    claimed_watermark    TEXT,
    completed_watermark  TEXT,
    status               TEXT NOT NULL DEFAULT 'idle',
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

DROP TABLE agent_memory_phase2_lock;
