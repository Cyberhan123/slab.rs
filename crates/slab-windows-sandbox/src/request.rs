//! Seam transport types crossing the `slab_sandboxing` ⇄ `slab_windows_sandbox` boundary.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::capability::{FsIsolationStrength, WindowsSetupKind};
use crate::ipc::SetupMarker;
use crate::pipe::OutputStreamKind;

/// Inputs the executor needs to provision (the shim has the `SandboxEnvironment`; it builds this
/// and hands it to `prepare()`). Carries the session-stable path set the daemon lowers to Low /
/// denies, plus the runtime paths (helper exe, DPAPI key, IPC dir, marker).
#[derive(Clone)]
pub struct PrepareContext {
    pub workspace_root: Option<PathBuf>,
    pub denied_paths: Vec<PathBuf>,
    pub denied_globs: Vec<String>,
    pub writable_roots: Vec<PathBuf>,
    pub network_blocked: bool,
    pub helper_exe: PathBuf,
    pub key_path: PathBuf,
    pub ipc_dir: PathBuf,
    pub marker_path: PathBuf,
}

/// The sub-crate's mirror of `slab_sandboxing::OutputSink`. This crate MUST NOT depend on
/// `slab_sandboxing`, so the elevated relay accepts this erased trait; the shim wraps its own
/// `OutputSink` in a 5-line adapter (see `SinkAdapter` in `platform/windows.rs`).
pub trait ErasedOutputSink: Send + Sync {
    fn on_output(&self, stream: OutputStreamKind, delta: &str);
}

/// A child process the sandbox should spawn, with the resolved policy attached. The shim in
/// `slab_sandboxing::platform::windows` builds this from `SandboxEnvironment` + `SandboxedCommand`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    /// S6a: run the elevated child under a Windows pseudoconsole (ConPTY) instead of piped stdio,
    /// giving the child a real terminal (ANSI/TUI fidelity). Default `false` (piped). Meaningful
    /// only on the elevated AppContainer path; ignored by the job-only path. Opt-in via the
    /// `windows_use_conpty` config knob. Covered by the Spawn frame HMAC (it is a field of `spawn`).
    #[serde(default)]
    pub use_conpty: bool,
    /// DIAGNOSTIC ONLY (temporary): spawn with the Low-IL token but NO AppContainer identity, NO
    /// `CREATE_NO_WINDOW`, and a plain STARTUPINFO. Used to isolate why AppContainer children die on
    /// init — if a binary runs here but not under SECURITY_CAPABILITIES, the AppContainer identity
    /// (or CREATE_NO_WINDOW) is the culprit; if it still dies, the Low-IL token itself is unusable.
    /// Not wired to any production config surface.
    #[serde(default)]
    pub diagnostic_plain_spawn: bool,
    /// DIAGNOSTIC ONLY (temporary): when set WITH `diagnostic_plain_spawn`, spawn via `CreateProcessW`
    /// (the daemon's own token) instead of `CreateProcessAsUserW(LowIntegrityToken)` — i.e. drop the
    /// Low-IL restriction entirely. If a binary runs here, the LowIntegrityToken is the init-failure
    /// cause. Not wired to any production config surface.
    #[serde(default)]
    pub diagnostic_no_low_il_token: bool,
    /// DIAGNOSTIC ONLY (temporary): when set, add `CREATE_NEW_CONSOLE` to the creation flags (give the
    /// child its own console instead of inheriting the daemon's none). Tests whether the no-console
    /// condition aborts console-app init. Not wired to any production config surface.
    #[serde(default)]
    pub diagnostic_new_console: bool,
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
