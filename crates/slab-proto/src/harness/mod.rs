//! Slab agent harness protocol.
//!
//! Wire contract for the `/v1/agents/harness` WebSocket endpoint, spoken as
//! standard JSON-RPC 2.0 (envelope types from `slab-jsonrpc`). The model is a
//! clean thread/turn/item hierarchy with an explicit lifecycle per submission:
//!
//! ```text
//! UserSubmissionOp::X → Event::XStarted → Event::XDelta* → Event::XCompleted | Error
//! ```
//!
//! The `example/` subdirectory holds the original design pseudocode (`.rs` +
//! `.ts`); the compiled types in this module are the authoritative contract.

pub mod messages;
pub mod model;
pub mod notification;
pub mod operation;
pub mod user_input;

pub use messages::{
    ApprovalResolveParams, ApprovalResolveResult, ApprovalScope, InitializeParams,
    InitializeResult, OperationCategory, PermissionMode, ReasoningEffort, ShutdownParams,
    ShutdownResult, SkillInfo, SkillSource, SkillsListParams, SkillsListResult,
    ThreadArchiveParams, ThreadArchiveResult, ThreadCompactStartParams, ThreadCompactStartResult,
    ThreadForkParams, ThreadForkResult, ThreadListParams, ThreadListResult, ThreadResumeParams,
    ThreadResumeResult, ThreadRollbackParams, ThreadRollbackResult, ThreadStartParams,
    ThreadStartResult, TurnInterruptParams, TurnInterruptResult, TurnStartParams, TurnStartResult,
};
pub use model::{ModelInfo, ModelListParams, ModelListResult, ReasoningEffortOption};
pub use notification::{
    ModelLoadCompletedParams, ModelLoadDeltaParams, ModelLoadError, ModelLoadPhase,
    ServerNotification,
};
pub use operation::{AdditionalContextEntry, ThreadSettingsOverrides, UserSubmissionOp};
pub use user_input::UserInput;

/// JSON-RPC method-name constants — single source of truth for the dispatcher
/// and any TypeScript binding generator.
pub mod method {
    // --- Requests (client -> server, expect a response) ---
    pub const INITIALIZE: &str = "initialize";
    pub const THREAD_START: &str = "thread/start";
    pub const THREAD_RESUME: &str = "thread/resume";
    pub const THREAD_FORK: &str = "thread/fork";
    pub const THREAD_ROLLBACK: &str = "thread/rollback";
    pub const THREAD_COMPACT_START: &str = "thread/compact/start";
    pub const THREAD_ARCHIVE: &str = "thread/archive";
    pub const THREAD_LIST: &str = "thread/list";
    pub const TURN_START: &str = "turn/start";
    pub const TURN_INTERRUPT: &str = "turn/interrupt";
    pub const MODEL_LIST: &str = "model/list";
    pub const SKILLS_LIST: &str = "skills/list";
    pub const APPROVAL_RESOLVE: &str = "approval/resolve";
    pub const SHUTDOWN: &str = "shutdown";
    pub const WORKSPACE_MIGRATE: &str = "workspace/migrate";

    // --- Notifications (server -> client) ---
    pub const THREAD_STARTED: &str = "thread/started";
    pub const TURN_STARTED: &str = "turn/started";
    pub const TURN_COMPLETED: &str = "turn/completed";
    pub const ITEM_STARTED: &str = "item/started";
    pub const ITEM_COMPLETED: &str = "item/completed";
    pub const ITEM_AGENT_MESSAGE_DELTA: &str = "item/agentMessage/delta";
    pub const ITEM_REASONING_TEXT_DELTA: &str = "item/reasoning/textDelta";
    pub const ITEM_REASONING_SUMMARY_TEXT_DELTA: &str = "item/reasoning/summaryTextDelta";
    pub const ITEM_COMMAND_EXECUTION_OUTPUT_DELTA: &str = "item/commandExecution/outputDelta";
    pub const ITEM_FILE_CHANGE_OUTPUT_DELTA: &str = "item/fileChange/outputDelta";
    pub const ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL: &str =
        "item/commandExecution/requestApproval";
    pub const ITEM_FILE_CHANGE_REQUEST_APPROVAL: &str = "item/fileChange/requestApproval";
    pub const ERROR: &str = "error";
    pub const ACCOUNT_UPDATED: &str = "account/updated";
    pub const ACCOUNT_LOGIN_COMPLETED: &str = "account/loginCompleted";
    // Model load lifecycle, emitted directly from the `turn/start` handler (NOT
    // projected from `EventMsg`). `<noun>/delta* → <noun>/completed` convention.
    pub const MODEL_LOAD_DELTA: &str = "model/load/delta";
    pub const MODEL_LOAD_COMPLETED: &str = "model/load/completed";
    // Context-compaction lifecycle, emitted from the agent turn loop via
    // `EventMsg` (projected like turn/item events). `<noun>/ing → <noun>/ed`.
    pub const CONTEXT_COMPACTING: &str = "context/compacting";
    pub const CONTEXT_COMPACTED: &str = "context/compacted";
}

/// Harness-scoped JSON-RPC error codes (application-errors below the standard
/// `-32000` band reserved by `slab-jsonrpc::APPLICATION_ERROR`).
pub mod error_code {
    /// Server has not yet received a successful `initialize` on this socket.
    pub const NOT_INITIALIZED: i64 = -32001;
    /// No thread exists for the given id / thread is unknown to this socket.
    pub const THREAD_NOT_FOUND: i64 = -32002;
    /// A turn is already in progress and the op cannot be accepted.
    pub const TURN_IN_PROGRESS: i64 = -32003;
    /// The requested method is recognized by the protocol but not yet wired.
    pub const NOT_IMPLEMENTED: i64 = -32004;
}
