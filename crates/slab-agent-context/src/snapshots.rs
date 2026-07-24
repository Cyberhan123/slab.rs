//! Lean, dependency-free value types the context hook renders into the
//! environment / permissions / memory fragments.
//!
//! These intentionally mirror types owned by other crates (`slab-exec-policy`,
//! `slab-agent-memories`, the host) so [`crate::sources::AgentContextSources`]
//! stays the only seam that depends on them. The host maps its real types into
//! these labels at the port boundary.

use serde::Serialize;

// ── Environment ───────────────────────────────────────────────────────────────

/// Resolved shell family the agent's `shell` tool launches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ShellKind {
    #[serde(rename = "bash")]
    Bash,
    #[serde(rename = "powershell")]
    PowerShell,
    #[serde(rename = "cmd")]
    Cmd,
    /// Launcher is configured to auto-detect and the resolution is unknown here.
    #[serde(rename = "unknown")]
    Unknown,
}

/// Operating-system family the agent runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OsKind {
    #[serde(rename = "windows")]
    Windows,
    #[serde(rename = "macos")]
    MacOS,
    #[serde(rename = "linux")]
    Linux,
    #[serde(rename = "unknown")]
    Unknown,
}

/// Environment facts injected once at agent start so the model knows where it
/// is working. The timestamp is computed by the host (the context crate stays
/// free of a clock dependency).
#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentSnapshot {
    /// Absolute workspace root the agent operates in, if any. Serialized as
    /// `null` when absent so the template can render an "(unset)" fallback
    /// under minijinja strict-undefined mode.
    pub cwd: Option<String>,
    pub shell: ShellKind,
    pub os: OsKind,
    /// RFC 3339 timestamp the session started at.
    pub timestamp: String,
}

// ── Permissions ───────────────────────────────────────────────────────────────

/// Permission-mode label (mirrors `slab_exec_policy::PermissionMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionModeLabel {
    RequestApproval,
    ApproveForMe,
    FullControl,
    Custom,
}

/// Baseline label (mirrors `slab_exec_policy::PermissionBaseline`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionBaselineLabel {
    ReadOnly,
    WorkspaceWrite,
    FullAccess,
}

/// Effective permission state the model should plan around. The `*_allowed`
/// flags are derived from the resolved tool exposure, so they already reflect
/// the per-session mode + global baseline.
#[derive(Debug, Clone, Serialize)]
pub struct PermissionSnapshot {
    pub mode: PermissionModeLabel,
    pub baseline: PermissionBaselineLabel,
    /// True when only read-only operations are available this turn.
    pub read_only: bool,
    pub shell_allowed: bool,
    pub file_write_allowed: bool,
    pub network_allowed: bool,
}

// ── Memory ────────────────────────────────────────────────────────────────────

/// Folded read-side memory context. `body` is the fully rendered memory
/// instruction (already wrapped by `slab-agent-memories`); the context hook
/// injects it verbatim as a `developer` message named `slab_memory`.
#[derive(Debug, Clone, Serialize)]
pub struct MemoryContext {
    pub body: String,
}
