/**
 * Harness method / notification / error-code constants.
 *
 * Mirrors `slab_proto::harness::{method, error_code}` (ts-rs generates no
 * constants, so these stay hand-written). `bun run gen:harness` fails when the
 * values drift from the Rust source.
 */

// ── Method + error-code constants (mirror `slab_proto::harness::method`) ────

/** Client → server requests. */
export const HARNESS_METHOD = {
  INITIALIZE: "initialize",
  THREAD_START: "thread/start",
  THREAD_RESUME: "thread/resume",
  THREAD_FORK: "thread/fork",
  THREAD_ROLLBACK: "thread/rollback",
  THREAD_COMPACT_START: "thread/compact/start",
  THREAD_ARCHIVE: "thread/archive",
  THREAD_LIST: "thread/list",
  TURN_START: "turn/start",
  TURN_INTERRUPT: "turn/interrupt",
  MODEL_LIST: "model/list",
  SKILLS_LIST: "skills/list",
  COMMAND_LIST: "command/list",
  APPROVAL_RESOLVE: "approval/resolve",
  SHUTDOWN: "shutdown",
  WORKSPACE_MIGRATE: "workspace/migrate",
} as const

/** Server → client notifications (agent event delivery). */
export const HARNESS_NOTIFICATION = {
  THREAD_STATUS_CHANGED: "thread/statusChanged",
  TURN_STARTED: "turn/started",
  TURN_COMPLETED: "turn/completed",
  ITEM_STARTED: "item/started",
  ITEM_COMPLETED: "item/completed",
  ITEM_AGENT_MESSAGE_DELTA: "item/agentMessage/delta",
  ITEM_REASONING_TEXT_DELTA: "item/reasoning/textDelta",
  ITEM_REASONING_SUMMARY_TEXT_DELTA: "item/reasoning/summaryTextDelta",
  ITEM_COMMAND_EXECUTION_OUTPUT_DELTA: "item/commandExecution/outputDelta",
  ITEM_FILE_CHANGE_OUTPUT_DELTA: "item/fileChange/outputDelta",
  ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL: "item/commandExecution/requestApproval",
  ITEM_FILE_CHANGE_REQUEST_APPROVAL: "item/fileChange/requestApproval",
  ERROR: "error",
  ACCOUNT_UPDATED: "account/updated",
  ACCOUNT_LOGIN_COMPLETED: "account/loginCompleted",
  // Model load lifecycle, emitted directly from the turn/start handler.
  MODEL_LOAD_DELTA: "model/load/delta",
  MODEL_LOAD_COMPLETED: "model/load/completed",
  // Context-compaction lifecycle, emitted from the agent turn loop via EventMsg.
  CONTEXT_COMPACTING: "context/compacting",
  CONTEXT_COMPACTED: "context/compacted",
} as const

/** Harness-specific error codes (reserved; the dispatcher currently emits `-32000`). */
export const HARNESS_ERROR_CODE = {
  APPLICATION_ERROR: -32000,
  NOT_INITIALIZED: -32001,
  THREAD_NOT_FOUND: -32002,
  NOT_IMPLEMENTED: -32004,
} as const
