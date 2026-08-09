//! Seam transport types crossing the `slab_sandboxing` ⇄ `slab_windows_sandbox` boundary.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::capability::{FsIsolationStrength, WindowsSetupKind};
use crate::ipc::SetupMarker;

/// A child process the sandbox should spawn, with the resolved policy attached. The shim in
/// `slab_sandboxing::platform::windows` builds this from `SandboxEnvironment` + `SandboxedCommand`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnRequest {
    pub argv: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
    /// Absolute protected paths (deny-write targets).
    #[serde(default)]
    pub denied_paths: Vec<PathBuf>,
    #[serde(default)]
    pub denied_globs: Vec<String>,
    /// Roots the child may write to (lowered to Low integrity in S2b).
    #[serde(default)]
    pub writable_roots: Vec<PathBuf>,
    pub workspace_root: Option<PathBuf>,
    /// Advisory only in S2 (Low-IL does not block sockets); S3 (WFP) enforces it.
    #[serde(default)]
    pub network_blocked: bool,
}

/// Result of the non-elevated spawn path: a local `tokio::process::Child` plus a tree-kill
/// closure. The shim feeds both into `slab_sandboxing`'s shared `wait_for_child`.
pub struct SpawnedChild {
    pub child: tokio::process::Child,
    pub kill_tree: Option<Box<dyn FnOnce() + Send + 'static>>,
}

/// Result of the elevated spawn path (S2b): the child lives in the elevated daemon, so bytes
/// arrive over the named pipe. The buffers are filled by the pipe-relay reader; `exit_future`
/// resolves when the daemon reports process exit; `kill_tree` sends a fire-and-forget Kill RPC.
pub struct ElevatedRun {
    pub stdout_buf: Arc<Mutex<Vec<u8>>>,
    pub stderr_buf: Arc<Mutex<Vec<u8>>>,
    pub exit_future: std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<ElevatedExit, crate::error::WindowsSandboxError>,
                > + Send,
        >,
    >,
    pub kill_tree: Option<Box<dyn FnOnce() + Send + 'static>>,
}

#[derive(Debug, Clone)]
pub struct ElevatedExit {
    pub exit_code: i32,
    pub timed_out: bool,
}

/// What kind of operation an elevation round-trip performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupMode {
    /// One-shot provisioning (apply ACLs, write marker); helper exits after.
    OneShotProvision,
    /// Start the long-lived daemon serving a named pipe (S2b).
    DaemonServe,
    /// Spawn a single child in an already-running daemon (S2b).
    SpawnOnly,
}

/// Outcome of `prepare()`.
#[derive(Debug, Clone)]
pub struct ProvisionReport {
    pub setup_kind: WindowsSetupKind,
    pub filesystem_isolation: FsIsolationStrength,
    pub marker: SetupMarker,
}
