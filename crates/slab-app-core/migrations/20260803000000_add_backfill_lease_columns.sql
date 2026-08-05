-- Backfill lease columns (concurrency-safe CAS + crash recovery).
--
-- The startup backfill (`backfill_all_threads`) is a single-worker-per-process
-- task, but a process crash mid-backfill used to leave a thread stuck in
-- `backfill_status = 'in_progress'` until a manual recovery. A crash-looping
-- process would also re-enter the same thread's atomic rewrite on every boot.
--
-- The lease is an advisory lock on `rollout_backfill_state`: a worker takes it
-- via a compare-and-swap (`try_acquire_backfill_lease`) before backfilling a
-- thread, and releases it (clears the columns) on success or failure. A worker
-- that crashes holds a non-expired lease; the next worker sees it held and
-- SKIPS that thread until the lease expires (default 15 min), after which the
-- CAS treats it as stale and re-acquires.
--
-- Both columns are nullable with no default. NULL means "no lease held" (the
-- existing rows and any row created by older code). This is fully backward
-- compatible: an older binary that never reads these columns still works, and a
-- fresh DB simply has NULL for every row until a lease-aware worker writes one.
-- `mark_backfill_state` (the existing writer) does not touch these columns, so
-- its INSERTs leave them NULL — which the CAS reads as "no lease held".

ALTER TABLE rollout_backfill_state ADD COLUMN lease_owner TEXT;
ALTER TABLE rollout_backfill_state ADD COLUMN lease_expires_at TEXT;
