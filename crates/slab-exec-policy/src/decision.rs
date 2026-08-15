//! Permission decision vocabulary: the engine's verdict, the per-session mode,
//! the popup persistence scope, and the global baseline.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The engine's verdict for a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecDecision {
    /// Run without prompting.
    Allow,
    /// Pause and ask the host/user before running.
    RequireApproval,
    /// Refuse without prompting.
    Deny,
}

/// Per-session permission mode (flows via `ThreadStartParams`/`TurnStartParams`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Prompt for command-line / file-edit / network operations.
    #[default]
    RequestApproval,
    /// acceptEdits semantics: auto-allow what the active [`PermissionBaseline`]
    /// already permits (workspace file edits + non-destructive shell under
    /// `WorkspaceWrite`; reads under `ReadOnly`), prompt the rest. Hard-deny
    /// safety and `Block` rules still apply.
    ApproveForMe,
    /// Run everything (hard-deny safety patterns still apply).
    FullControl,
    /// Defer to the global `agent.permissions` baseline (`read_only` /
    /// `work_space_write` / `full_access`).
    Custom,
}

impl PermissionMode {
    /// Effective mode after resolving any stub. `ApproveForMe` is no longer a
    /// stub — it now carries acceptEdits semantics (auto-allow what the active
    /// baseline permits, prompt the rest), so this is the identity. Retained as
    /// a chokepoint in case a future mode needs pre-resolution.
    pub fn effective(self) -> Self {
        self
    }
}

/// Persistence scope chosen by the user when approving a prompt.
#[derive(TS, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ApprovalScope {
    /// Allow this run only; do not persist a rule.
    RunOnce,
    /// Persist a rule scoped to the current workspace (`hash-<workspace>.rules`).
    AlwaysInWorkspace,
    /// Persist a global rule (`default.rules`).
    Always,
    /// Deny this run only; do not persist.
    Deny,
}

impl ApprovalScope {
    /// Whether this scope should persist a rule.
    pub fn persists(self) -> bool {
        matches!(self, Self::AlwaysInWorkspace | Self::Always)
    }

    /// Whether this scope approves the immediate run.
    pub fn approves(self) -> bool {
        matches!(self, Self::RunOnce | Self::AlwaysInWorkspace | Self::Always)
    }

    /// The default scope old clients implicitly use (no persistence).
    pub fn default_for_approval() -> Self {
        Self::RunOnce
    }
}

impl Default for ApprovalScope {
    fn default() -> Self {
        Self::default_for_approval()
    }
}

/// Global baseline (the `agent.permissions` setting), maps 1:1 onto
/// [`slab_sandboxing::SandboxPolicy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionBaseline {
    #[default]
    ReadOnly,
    WorkspaceWrite,
    FullAccess,
}

impl PermissionBaseline {
    pub fn to_sandbox_policy(self) -> slab_sandboxing::SandboxPolicy {
        match self {
            Self::ReadOnly => slab_sandboxing::SandboxPolicy::ReadOnly,
            Self::WorkspaceWrite => slab_sandboxing::SandboxPolicy::WorkspaceWrite,
            Self::FullAccess => slab_sandboxing::SandboxPolicy::DangerFullAccess,
        }
    }
}
