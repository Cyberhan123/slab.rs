//! Harness JSON-RPC request params / results and the protocol policy enums.
//!
//! Field naming is camelCase on the wire (matching the public TS contract);
//! Rust fields stay snake_case and rely on `#[serde(rename_all = "camelCase")]`.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::harness::error::TurnError;
use crate::harness::item::TurnItem;
use crate::harness::user_input::UserInput;

// ============ Reasoning effort ============

/// Reasoning effort selector.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Off,
    Low,
    Medium,
    High,
    Xhigh,
}

// ============ Policies ============

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
pub enum SandboxMode {
    #[serde(rename = "read-only")]
    ReadOnly,
    #[serde(rename = "workspace-write")]
    WorkspaceWrite,
    #[serde(rename = "danger-full-access")]
    DangerFullAccess,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum NetworkAccess {
    Restricted,
    Enabled,
}

/// Per-session permission mode (flows via `ThreadStartParams`/`TurnStartParams`).
/// Mirrors `slab_exec_policy::PermissionMode`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    #[default]
    RequestApproval,
    ApproveForMe,
    FullControl,
    Custom,
}

/// Persistence scope chosen by the user when approving a prompt. Mirrors
/// `slab_exec_policy::ApprovalScope`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalScope {
    RunOnce,
    AlwaysInWorkspace,
    Always,
    Deny,
}

/// Operation category for an approval prompt. Mirrors
/// `slab_exec_policy::OperationCategory`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationCategory {
    Shell,
    FileEdit,
    Network,
    ReadOnly,
}

/// Sandbox policy — a `type`-discriminated union mirroring the TS contract.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub version: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_info: Option<ServerInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ServerCapabilities>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Advertised server capabilities (extensible).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
pub struct ServerCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Value>,
}

// ============ Thread / Turn ============

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub id: String,
    pub preview: String,
    pub model_provider: String,
    /// Unix epoch milliseconds.
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_info: Option<GitInfo>,
    #[serde(default)]
    pub turns: Vec<Turn>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GitInfo {
    pub branch: String,
    pub sha: String,
    pub is_dirty: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub id: String,
    #[serde(default)]
    pub items: Vec<TurnItem>,
    /// Open string set: `completed` / `interrupted` / `failed` / `inProgress`
    /// (plus PascalCase aliases accepted on decode).
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<TurnError>,
}

// ============ thread/start ============

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental_raw_events: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadResumeParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadResumeResult {
    pub thread: Thread,
}

// ============ thread/fork ============

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadForkParams {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_override: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_override: Option<SandboxMode>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadForkResult {
    pub thread: Thread,
}

// ============ thread/rollback ============

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRollbackParams {
    pub thread_id: String,
    pub to_turn_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRollbackResult {
    pub thread: Thread,
}

// ============ thread/archive ============

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadArchiveParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadArchiveResult {
    pub thread: Thread,
}

// ============ thread/list ============

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_providers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListResult {
    pub data: Vec<Thread>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

// ============ turn/start ============

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<ReasoningEffort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartResult {
    pub turn: Turn,
}

// ============ turn/interrupt ============

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnInterruptParams {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnInterruptResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

// ============ approval/resolve ============

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalResolveResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivered: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

// ============ shutdown ============

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

// ============ workspace/migrate ============

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMigrateParams {
    /// Target workspace root. When omitted the server resolves the root from
    /// its own configuration. This is the canonical workspace-migration method:
    /// the old `POST /v1/agents/migrate` REST endpoint was removed, and the
    /// server-side `POST /v1/workspace/open` runs the same migration internally
    /// when switching away from a different active workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
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
    fn thread_serializes_camel_case_fields() {
        let thread = Thread {
            id: "t1".to_owned(),
            preview: "hi".to_owned(),
            model_provider: "openai".to_owned(),
            created_at: 1_700_000_000_000,
            turns: vec![],
            ..Default::default()
        };
        let json = serde_json::to_value(&thread).unwrap();
        assert_eq!(json["modelProvider"], "openai");
        assert_eq!(json["createdAt"], 1_700_000_000_000_i64);
    }

    #[test]
    fn reasoning_effort_xhigh_lowercases() {
        let json = serde_json::to_value(ReasoningEffort::Xhigh).unwrap();
        assert_eq!(json, "xhigh");
    }
}
