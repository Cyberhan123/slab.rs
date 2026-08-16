//! Seam transport types crossing the `slab_sandboxing` ⇄ `slab_linux_sandbox` boundary.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Mirror of `slab_sandboxing::SandboxPolicy` (this crate MUST NOT depend on `slab_sandboxing`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SandboxPolicyMirror {
    #[default]
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

/// A child process the sandbox should spawn, with the resolved policy attached. The shim in
/// `slab_sandboxing::platform::linux` builds this from `SandboxEnvironment` + `SandboxedCommand`.
#[derive(Debug, Clone)]
pub struct SpawnRequest {
    pub argv: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
    /// `true` when `env.permissions.network == NetworkPolicy::Blocked`.
    pub network_blocked: bool,
    /// `true` when `env.permissions.managed_proxy.is_some()`. The network seccomp filter (and
    /// bwrap `--unshare-net`) are skipped when a managed proxy is active — it needs outbound.
    pub managed_proxy_active: bool,
    pub sandbox_policy: SandboxPolicyMirror,
    pub workspace_root: Option<PathBuf>,
    /// Roots the child may write to (plus workspace_root + temp_dir at spawn time).
    pub writable_roots: Vec<PathBuf>,
    pub readable_roots: Vec<PathBuf>,
    pub denied_paths: Vec<PathBuf>,
    /// Protected metadata dir names kept read-only inside writable roots (`.git`/`.slab`/`.agents`).
    /// Honored by the bwrap path via read-only bind mounts; landlock cannot express this (documented).
    pub protected_path_names: Vec<String>,
}

impl SpawnRequest {
    /// The network-isolation predicate shared by bwrap `--unshare-net` and the seccomp network
    /// filter: block outbound only when network is blocked AND no managed proxy is active.
    pub fn network_enforced(&self) -> bool {
        self.network_blocked && !self.managed_proxy_active
    }
}

/// Result of a spawn: a local `tokio::process::Child` plus a tree-kill closure. The shim feeds both
/// into `slab_sandboxing`'s shared `wait_for_child`. Identical shape to the Windows sub-crate's
/// `SpawnedChild` so the shared `wait_for_child` accepts it unchanged.
pub struct SpawnedChild {
    pub child: tokio::process::Child,
    pub kill_tree: Option<Box<dyn FnOnce() + Send + 'static>>,
}
