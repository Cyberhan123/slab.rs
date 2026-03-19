// This file is intentionally excluded from compilation (removed from mod.rs).
//
// All functionality previously in `ExecutionKernel` has been merged directly
// into `Orchestrator` (see `orchestrator.rs`):
//
//   - `wait_terminal` / `wait_result` / `wait_stream` → `Orchestrator`
//   - `DEFAULT_WAIT_TIMEOUT` / `STREAM_INIT_TIMEOUT` constants → `orchestrator.rs`
//   - `snapshot` / `take_result` / `take_stream` / `purge` → pre-existing methods on `Orchestrator`
//
// Callers that previously held `ExecutionKernel` now hold `Orchestrator` directly.
// This file can be safely deleted.
