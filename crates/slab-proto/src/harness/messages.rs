//! Harness JSON-RPC request params / results and the protocol policy enums.
//!
//! Field naming is camelCase on the wire (matching the public TS contract);
//! Rust fields stay snake_case and rely on `#[serde(rename_all = "camelCase")]`.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::harness::user_input::UserInput;

// `Thread` / `Turn` are owned by `slab_agent::protocol`; import them here so the
// request/response DTOs below (e.g. `ThreadStartResult.thread`) can reference
// them. They are no longer re-exported — consumers use `slab_agent::protocol`.
use slab_agent::protocol::{Thread, Turn};

// ============ Reasoning effort ============

/// Reasoning effort selector.
#[derive(TS, Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ReasoningEffort {
    Off,
    Low,
    Medium,
    High,
    Xhigh,
}

// ============ Policies ============

#[derive(TS, Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[ts(export)]
pub enum ApprovalPolicy {
    #[serde(rename = "never")]
    Never,
    #[serde(rename = "on-request")]
    OnRequest,
    #[serde(rename = "on-failure")]
    OnFailure,
    #[serde(rename = "untrusted")]
    Untrusted,
}

#[derive(TS, Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[ts(export)]
pub enum SandboxMode {
    #[serde(rename = "read-only")]
    ReadOnly,
    #[serde(rename = "workspace-write")]
    WorkspaceWrite,
    #[serde(rename = "danger-full-access")]
    DangerFullAccess,
}

#[derive(TS, Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum NetworkAccess {
    Restricted,
    Enabled,
}

/// Per-session permission mode (flows via `ThreadStartParams`/`TurnStartParams`).
/// Mirrors `slab_exec_policy::PermissionMode`.
#[derive(TS, Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PermissionMode {
    #[default]
    RequestApproval,
    ApproveForMe,
    FullControl,
    Custom,
}

/// Persistence scope chosen by the user when approving a prompt. Mirrors
/// `slab_exec_policy::ApprovalScope`.
#[derive(TS, Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ApprovalScope {
    RunOnce,
    AlwaysInWorkspace,
    Always,
    Deny,
}

/// Operation category for an approval prompt. Mirrors
/// `slab_exec_policy::OperationCategory`.
#[derive(TS, Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OperationCategory {
    Shell,
    FileEdit,
    Network,
    ReadOnly,
}

/// Sandbox policy — a `type`-discriminated union mirroring the TS contract.
#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum SandboxPolicy {
    DangerFullAccess,
    ReadOnly,
    ExternalSandbox {
        #[serde(default, rename = "networkAccess", skip_serializing_if = "Option::is_none")]
        network_access: Option<NetworkAccess>,
    },
    WorkspaceWrite {
        #[serde(default, rename = "writableRoots", skip_serializing_if = "Option::is_none")]
        writable_roots: Option<Vec<String>>,
        #[serde(default, rename = "networkAccess", skip_serializing_if = "Option::is_none")]
        network_access: Option<bool>,
        #[serde(default, rename = "excludeTmpdirEnvVar", skip_serializing_if = "Option::is_none")]
        exclude_tmpdir_env_var: Option<bool>,
        #[serde(default, rename = "excludeSlashTmp", skip_serializing_if = "Option::is_none")]
        exclude_slash_tmp: Option<bool>,
    },
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self::WorkspaceWrite {
            writable_roots: None,
            network_access: None,
            exclude_tmpdir_env_var: None,
            exclude_slash_tmp: None,
        }
    }
}

// ============ Initialize ============

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InitializeParams {
    pub client_info: ClientInfo,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClientInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub version: String,
}

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InitializeResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_info: Option<ServerInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ServerCapabilities>,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Advertised server capabilities (extensible).
#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[ts(export)]
pub struct ServerCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Value>,
}

// ============ Thread / Turn ============
// `Thread` / `GitInfo` / `Turn` are re-exported above from `slab_agent::protocol`.

// ============ thread/start ============

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadStartParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<ApprovalPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    /// Per-session permission mode (request-approval / approve-for-me /
    /// full-control / custom). When unset the server uses `RequestApproval`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<PermissionMode>,
    /// Built-in agent type to run the turn as (e.g. `"plan"`). When set, the
    /// server resolves the agent definition (tool constraint + system prompt)
    /// and applies it for the turn. Unset = default agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental_raw_events: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadStartResult {
    pub thread: Thread,
    pub model: String,
    pub model_provider: String,
    pub cwd: String,
    pub approval_policy: ApprovalPolicy,
    pub sandbox: SandboxPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
}

// ============ thread/resume ============

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadResumeParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadResumeResult {
    pub thread: Thread,
}

// ============ thread/fork ============

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadForkParams {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_override: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_override: Option<SandboxMode>,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadForkResult {
    pub thread: Thread,
}

// ============ thread/rollback ============

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadRollbackParams {
    pub thread_id: String,
    pub to_turn_id: String,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadRollbackResult {
    pub thread: Thread,
}

// ============ thread/compact/start ============

/// Manually compact a thread's persisted history: summarize older turns into a
/// single recap (with a trailing-window trim fallback) and keep the recent
/// window verbatim. Refuses while the thread is running.
#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadCompactStartParams {
    pub thread_id: String,
    /// Optional override of the summarization model; defaults to the thread's
    /// configured model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_override: Option<String>,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadCompactStartResult {
    pub thread: Thread,
    /// Number of persisted messages removed by the compaction.
    pub removed_messages: u32,
    /// Estimated token count of the compacted message set.
    pub output_tokens: u32,
}

// ============ thread/archive ============

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadArchiveParams {
    pub thread_id: String,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadArchiveResult {
    pub thread: Thread,
}

// ============ thread/list ============

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_providers: Option<Vec<String>>,
}

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThreadListResult {
    pub data: Vec<Thread>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

// ============ skills/list ============

/// Where a skill was discovered (workspace `.agents/skills` vs global app-home).
#[derive(TS, Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SkillSource {
    #[default]
    Workspace,
    Global,
}

/// A discoverable skill surfaced by `skills/list`.
#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkillInfo {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub path: PathBuf,
    pub source: SkillSource,
}

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkillsListParams {}

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkillsListResult {
    pub data: Vec<SkillInfo>,
}

// ============ command/list ============

/// How a user-facing `/`-command dispatches on the client.
#[derive(TS, Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CommandKind {
    #[default]
    /// Intercepts submission and runs a host action; never reaches the model.
    Control,
    /// Expands into prompt text that is sent to the model.
    Prompt,
}

/// Where a command was declared.
#[derive(TS, Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CommandSource {
    #[default]
    Builtin,
    Skill,
}

/// A user-facing `/`-command surfaced by `command/list`.
#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommandInfo {
    /// Trigger name, without the leading `/`.
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub description: String,
    pub kind: CommandKind,
    pub source: CommandSource,
    /// `Control`-kind action key the client maps to a host callback
    /// (e.g. `"compact"`, `"fork"`). Absent for non-`Control` kinds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_action: Option<String>,
}

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommandListParams {}

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommandListResult {
    pub data: Vec<CommandInfo>,
}

// ============ turn/start ============

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TurnStartParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<ApprovalPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<SandboxPolicy>,
    /// Per-session permission mode override for this turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<PermissionMode>,
    /// Built-in agent type to run this turn as (e.g. `"plan"`). When set, the
    /// server resolves the agent definition (tool constraint + system prompt)
    /// and applies it for this turn only. Unset = default agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<ReasoningEffort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
}

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TurnStartResult {
    pub turn: Turn,
}

// ============ turn/interrupt ============

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TurnInterruptParams {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TurnInterruptResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

// ============ approval/resolve ============

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApprovalResolveParams {
    pub thread_id: String,
    /// The pending item / tool-call id awaiting approval.
    pub item_id: String,
    pub approved: bool,
    /// Persistence scope chosen by the user. Older clients omit this; the
    /// server treats a missing scope as `RunOnce` (no persistence).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ApprovalScope>,
}

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApprovalResolveResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivered: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

// ============ shutdown ============

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShutdownParams {
    pub thread_id: String,
}

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShutdownResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

// ============ workspace/migrate ============

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkspaceMigrateParams {
    /// Target workspace root. When omitted the server resolves the root from
    /// its own configuration. This is the canonical workspace-migration method:
    /// the old `POST /v1/agents/migrate` REST endpoint was removed, and the
    /// server-side `POST /v1/workspace/open` runs the same migration internally
    /// when switching away from a different active workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<PathBuf>,
}

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkspaceMigrateResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub suspended_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_start_params_round_trips_camel_case() {
        let json = serde_json::json!({
            "model": "gpt-oss",
            "approvalPolicy": "on-request",
            "sandbox": "workspace-write",
            "experimentalRawEvents": true
        });
        let params: ThreadStartParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.model.as_deref(), Some("gpt-oss"));
        assert_eq!(params.approval_policy, Some(ApprovalPolicy::OnRequest));
        assert_eq!(params.sandbox, Some(SandboxMode::WorkspaceWrite));
        assert_eq!(params.experimental_raw_events, Some(true));
    }

    #[test]
    fn sandbox_policy_serializes_tagged_union() {
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: Some(vec!["/a".to_owned()]),
            network_access: Some(true),
            exclude_tmpdir_env_var: None,
            exclude_slash_tmp: None,
        };
        let json = serde_json::to_value(&policy).unwrap();
        assert_eq!(json["type"], "workspaceWrite");
        assert_eq!(json["writableRoots"][0], "/a");
        assert_eq!(json["networkAccess"], true);
    }

    #[test]
    fn reasoning_effort_xhigh_lowercases() {
        let json = serde_json::to_value(ReasoningEffort::Xhigh).unwrap();
        assert_eq!(json, "xhigh");
    }

    #[test]
    fn turn_start_params_agent_type_round_trips() {
        // Absent agent_type deserializes to None (default agent).
        let params: TurnStartParams =
            serde_json::from_str(r#"{"threadId":"t","input":[]}"#).unwrap();
        assert_eq!(params.agent_type, None);

        // An explicit agent_type round-trips through the params.
        let params: TurnStartParams =
            serde_json::from_str(r#"{"threadId":"t","input":[],"agentType":"plan"}"#).unwrap();
        assert_eq!(params.agent_type, Some("plan".to_owned()));
    }
}
