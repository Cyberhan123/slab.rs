use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::SandboxError;
use crate::policy::SandboxEnvironment;

/// Which process stream an output chunk came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// Receiver for incremental process output. While a command runs the driver
/// forwards each chunk (e.g. to the harness display); it still returns the fully
/// accumulated output when the process exits.
pub trait OutputSink: Send + Sync {
    fn on_output(&self, stream: OutputStream, delta: &str);
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SandboxedCommand {
    pub argv: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub timeout: Option<Duration>,
    /// Optional live-output receiver. `#[serde(skip)]` (closures aren't
    /// serializable); defaults to `None` when deserialized.
    #[serde(skip)]
    pub output_sink: Option<Arc<dyn OutputSink>>,
}

impl std::fmt::Debug for SandboxedCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SandboxedCommand")
            .field("argv", &self.argv)
            .field("env", &self.env)
            .field("cwd", &self.cwd)
            .field("timeout", &self.timeout)
            .field("output_sink", &self.output_sink.as_ref().map(|_| "<sink>"))
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct SandboxedOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SandboxPlatform {
    Windows,
    Linux,
    Macos,
    #[default]
    Unsupported,
}

/// Coarse isolation level reported by a driver. New variants are additive so
/// existing discriminants stay stable (the smoke test casts this to `u8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SandboxIsolation {
    Full,
    #[default]
    Degraded,
    Passthrough,
    Unsupported,
    /// Lexical/in-process guard only (no OS enforcement) but stronger than raw
    /// passthrough — e.g. an allowlist gate before spawn.
    Guard,
    /// OS-enforced but partial (e.g. restricted token + ACL on Windows without
    /// WFP, or firewall-only network blocking).
    Elevated,
    /// Kernel-level filtering (landlock, WFP callout, seccomp).
    KernelFiltered,
}

/// What mechanism actually backs one dimension of isolation — the honest
/// counterpart to the legacy `filesystem` / `network` booleans on
/// [`SandboxCapabilities`]. `Lexical` means an in-process text/path check
/// (`validate_command`) that is defense-in-depth and bypassable; `OsEnforced`
/// means the OS kernel enforces it. The booleans stay `false` unless a kernel
/// mechanism is active, so callers cannot be lied to again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IsolationStrength {
    #[default]
    None,
    Lexical,
    OsEnforced,
}

/// How the sandbox is (or would be) provisioned. Maps 1:1 to the honest
/// `capabilities()` report so callers branch on the real mechanism instead of
/// guessing from booleans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SetupKind {
    #[default]
    None,
    /// Windows: Job Object (tree-kill) + lexical guard only — today's state.
    JobObject,
    /// Lexical guard only (no Job Object, no OS isolation).
    Guard,
    /// Windows: restricted token + ACL filesystem containment, no WFP.
    ElevatedAclToken,
    /// Windows: full — restricted token + ACL + WFP/firewall network blocking.
    ElevatedAclTokenWfp,
    /// Linux: bubblewrap for the filesystem view.
    Bwrap,
    /// Linux: bubblewrap + seccomp (network syscalls blocked except AF_UNIX).
    BwrapSeccomp,
    /// Linux: bubblewrap + landlock fallback.
    BwrapLandlock,
    /// macOS: seatbelt (`sandbox-exec`).
    Seatbelt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SandboxCapabilities {
    pub platform: SandboxPlatform,
    pub isolation: SandboxIsolation,
    /// OS-enforced filesystem write containment. `false` unless a kernel
    /// mechanism (ACL/seatbelt/bwrap bind) actually enforces it — a lexical
    /// guard alone does NOT set this. See [`IsolationStrength`] for nuance.
    #[serde(default)]
    pub filesystem: bool,
    /// OS-enforced network blocking. `false` unless a kernel mechanism
    /// (WFP/seccomp/`--unshare-net`) actually enforces it.
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub process_cleanup: bool,
    #[serde(default)]
    pub setup_required: bool,
    /// Honest strength of the filesystem dimension.
    #[serde(default)]
    pub filesystem_isolation: IsolationStrength,
    /// Honest strength of the network dimension.
    #[serde(default)]
    pub network_isolation: IsolationStrength,
    /// The provisioning mechanism in effect.
    #[serde(default)]
    pub setup_kind: SetupKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxSetupStatus {
    pub available: bool,
    pub prepared: bool,
    pub degraded: bool,
    pub details: String,
}

impl SandboxSetupStatus {
    pub fn ready(details: impl Into<String>) -> Self {
        Self { available: true, prepared: true, degraded: false, details: details.into() }
    }

    pub fn degraded(details: impl Into<String>) -> Self {
        Self { available: true, prepared: false, degraded: true, details: details.into() }
    }

    pub fn unavailable(details: impl Into<String>) -> Self {
        Self { available: false, prepared: false, degraded: false, details: details.into() }
    }
}

impl SandboxedOutput {
    pub fn stdout_str(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
    pub fn stderr_str(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
    /// `true` when the process exited cleanly (zero status) without timing out.
    pub fn success(&self) -> bool {
        self.exit_code == 0 && !self.timed_out
    }
}

#[async_trait]
pub trait SandboxDriver: Send + Sync {
    async fn run(&self, cmd: SandboxedCommand) -> Result<SandboxedOutput, SandboxError>;
    fn name(&self) -> &str;

    async fn prepare(&self) -> Result<SandboxSetupStatus, SandboxError> {
        Ok(self.setup_status())
    }

    fn capabilities(&self) -> SandboxCapabilities {
        // Conservative default: no platform, degraded, nothing OS-enforced.
        // Matches `SandboxCapabilities::default()` exactly.
        SandboxCapabilities::default()
    }

    fn setup_status(&self) -> SandboxSetupStatus {
        SandboxSetupStatus::ready(format!("{} is ready", self.name()))
    }
}

/// A pass-through sandbox driver that executes commands directly without isolation.
/// Use only in development/test environments or when DangerFullAccess policy is set.
pub struct PassThroughDriver;

#[async_trait]
impl SandboxDriver for PassThroughDriver {
    fn name(&self) -> &str {
        "passthrough"
    }

    async fn run(&self, cmd: SandboxedCommand) -> Result<SandboxedOutput, SandboxError> {
        use tokio::process::Command;
        let program = cmd.argv.first().ok_or(SandboxError::EmptyCommand)?;
        let mut child = Command::new(program);
        child.args(&cmd.argv[1..]);
        for (k, v) in &cmd.env {
            child.env(k, v);
        }
        if let Some(ref cwd) = cmd.cwd {
            child.current_dir(cwd);
        }
        child.kill_on_drop(true);
        child.stdin(std::process::Stdio::null());
        child.stdout(std::process::Stdio::piped());
        child.stderr(std::process::Stdio::piped());
        // Run the command in its own process group so a backgrounded child that
        // inherits the stdout/stderr pipes can be tree-killed after it exits
        // (otherwise the read tasks wait for pipe EOF forever).
        #[cfg(unix)]
        {
            child.process_group(0);
        }

        let spawned = child.spawn().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
        // PassThrough has no Windows Job Object: on Unix kill the child's process
        // group; on Windows the grace-timeout backstop in `wait_for_child` is the
        // only protection (this driver is dev/test-only).
        #[cfg(unix)]
        let kill_tree = unix_kill_tree(spawned.id());
        #[cfg(not(unix))]
        let kill_tree: Option<Box<dyn FnOnce() + Send + 'static>> = None;
        wait_for_child(spawned, cmd.timeout, cmd.output_sink.clone(), kill_tree).await
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities { isolation: SandboxIsolation::Passthrough, ..Default::default() }
    }
}

pub(crate) fn command_env(
    env: &SandboxEnvironment,
    cmd: &SandboxedCommand,
) -> HashMap<String, String> {
    let mut merged = cmd.env.clone();
    if let Some(proxy) = &env.permissions.managed_proxy {
        if let Some(http_proxy) = &proxy.http_proxy {
            merged.insert("HTTP_PROXY".to_string(), http_proxy.clone());
            merged.insert("http_proxy".to_string(), http_proxy.clone());
        }
        if let Some(https_proxy) = &proxy.https_proxy {
            merged.insert("HTTPS_PROXY".to_string(), https_proxy.clone());
            merged.insert("https_proxy".to_string(), https_proxy.clone());
        }
        if !proxy.no_proxy.is_empty() {
            let no_proxy = proxy.no_proxy.join(",");
            merged.insert("NO_PROXY".to_string(), no_proxy.clone());
            merged.insert("no_proxy".to_string(), no_proxy);
        }
    }
    merged
}

/// After the direct child exits, how long to keep draining its stdout/stderr
/// pipes before aborting the read tasks. Normally the pipes close immediately
/// once the process tree is killed; this backstop guards pathological cases
/// (e.g. a reparented descendant that kept a handle) so the run can never hang.
const READ_DRAIN_GRACE: Duration = Duration::from_secs(5);

/// How long to wait for the child to actually exit after a timeout kill before
/// abandoning the reap. Once we have started killing the process its exit
/// status is untrustworthy anyway — this only bounds how long a stuck kill can
/// delay the run (the old code waited indefinitely, so a failed kill let the
/// command run to completion while still being reported as timed out).
const POST_KILL_WAIT_GRACE: Duration = Duration::from_secs(5);

/// Fixed exit code reported for a command killed by its timeout (GNU `timeout`
/// convention). Never derived from the process: after a kill race the real
/// status is meaningless, and a synthetic `1` is indistinguishable from a
/// command that genuinely failed with exit 1.
const TIMEOUT_EXIT_CODE: i32 = 124;

/// Fires a tree-kill closure exactly once — including when the owning future
/// is dropped before completion. Turn cancellation (`tokio::select!` around
/// tool execution) drops the `wait_for_child`/`wait_for_elevated` future
/// mid-`child.wait()`, skipping every explicit kill site below; without this
/// guard, backgrounded grandchildren would outlive the cancelled turn (the
/// direct child still dies via `kill_on_drop`, but its descendants do not).
/// Normal paths call [`TreeKillGuard::fire`] explicitly, which disarms the
/// closure so the eventual `Drop` is a no-op.
struct TreeKillGuard(Option<Box<dyn FnOnce() + Send + 'static>>);

impl TreeKillGuard {
    fn new(kill: Option<Box<dyn FnOnce() + Send + 'static>>) -> Self {
        Self(kill)
    }

    /// `true` while a closure is still armed (present and not yet fired).
    fn is_armed(&self) -> bool {
        self.0.is_some()
    }

    /// Invoke and disarm the closure. Returns `true` when a closure actually
    /// ran (i.e. one was present and had not fired yet).
    fn fire(&mut self) -> bool {
        match self.0.take() {
            Some(kill) => {
                kill();
                true
            }
            None => false,
        }
    }
}

impl Drop for TreeKillGuard {
    fn drop(&mut self) {
        self.fire();
    }
}

#[allow(clippy::type_complexity)]
pub(crate) async fn wait_for_child(
    mut child: tokio::process::Child,
    timeout: Option<Duration>,
    sink: Option<Arc<dyn OutputSink>>,
    // Invoked once the direct child has exited, to tear down any leftover
    // descendants so they release the stdout/stderr pipes. `None` when the
    // caller has no tree-kill mechanism (then the grace backstop below applies).
    kill_tree: Option<Box<dyn FnOnce() + Send + 'static>>,
) -> Result<SandboxedOutput, SandboxError> {
    // Wrapped in a guard so an early drop (turn cancellation) still kills the
    // tree; every explicit kill below goes through `fire`, which disarms it.
    let mut kill_tree = TreeKillGuard::new(kill_tree);
    let pid = child.id();
    tracing::info!(
        ?pid,
        ?timeout,
        has_sink = sink.is_some(),
        has_kill_tree = kill_tree.is_armed(),
        "wait_for_child: spawned, awaiting child.wait"
    );
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_sink = sink.clone();
    let stderr_sink = sink.clone();
    // Shared accumulators: read tasks append here as they go, so the partial
    // output survives even if a task is aborted before EOF (see drain grace).
    let stdout_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let stdout_task =
        tokio::spawn(read_stream(stdout, stdout_sink, OutputStream::Stdout, stdout_buf.clone()));
    let stderr_task =
        tokio::spawn(read_stream(stderr, stderr_sink, OutputStream::Stderr, stderr_buf.clone()));

    let (exit_code, timed_out) = if let Some(timeout) = timeout {
        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(Ok(status)) => (status.code().unwrap_or(-1), false),
            Ok(Err(error)) => return Err(SandboxError::SpawnFailed(error.to_string())),
            Err(_) => {
                // Timed out: kill the whole tree FIRST (job close / process
                // group — a bare child.kill only terminates the direct child
                // and can even fail silently), then best-effort kill the
                // direct child and reap it within a bounded grace. Whatever
                // happens, report the fixed timeout exit code — the real
                // status is untrustworthy once killing has started.
                if kill_tree.fire() {
                    tracing::info!(?pid, "wait_for_child: timeout, process tree killed");
                }
                if let Err(error) = child.kill().await {
                    tracing::warn!(?pid, error = %error, "wait_for_child: kill after timeout failed");
                }
                match tokio::time::timeout(POST_KILL_WAIT_GRACE, child.wait()).await {
                    Ok(Ok(status)) => {
                        tracing::info!(
                            ?pid,
                            ?status,
                            "wait_for_child: reaped child after timeout kill"
                        );
                    }
                    Ok(Err(error)) => {
                        tracing::warn!(?pid, error = %error, "wait_for_child: wait after timeout kill failed");
                    }
                    Err(_) => {
                        tracing::warn!(
                            ?pid,
                            "wait_for_child: child did not exit within grace after kill; abandoning wait"
                        );
                    }
                }
                (TIMEOUT_EXIT_CODE, true)
            }
        }
    } else {
        let status = child.wait().await.map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
        (status.code().unwrap_or(-1), false)
    };
    tracing::info!(?pid, exit_code, timed_out, "wait_for_child: child exited, killing tree");

    // The direct child has exited. Kill the entire process tree so any live
    // grandchildren release the inherited stdout/stderr pipes. Without this the
    // read tasks below wait for pipe EOF forever — the exact cause of a turn
    // hanging indefinitely after a shell approval when the command backgrounds a
    // long-lived process. Must happen BEFORE awaiting the read tasks.
    if kill_tree.fire() {
        tracing::info!(?pid, "wait_for_child: process tree killed");
    }

    // Drain the pipes within a grace window. If a read still hasn't hit EOF,
    // abort it and keep whatever reached the shared buffer.
    drain_with_grace(stdout_task, READ_DRAIN_GRACE).await;
    drain_with_grace(stderr_task, READ_DRAIN_GRACE).await;
    tracing::info!(
        ?pid,
        stdout_len = stdout_buf.lock().map(|b| b.len()).unwrap_or(0),
        stderr_len = stderr_buf.lock().map(|b| b.len()).unwrap_or(0),
        "wait_for_child: read tasks drained, returning"
    );

    let stdout = stdout_buf.lock().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?.clone();
    let mut stderr =
        stderr_buf.lock().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?.clone();
    if timed_out && stderr.is_empty() {
        stderr.extend_from_slice(timeout_stderr_note(timeout).as_bytes());
    }

    Ok(SandboxedOutput { stdout, stderr, exit_code, timed_out })
}

/// Marker written into stderr when a run timed out and produced no stderr of
/// its own, so the model gets an explicit signal instead of inferring the
/// timeout from `timed_out` alone.
fn timeout_stderr_note(timeout: Option<Duration>) -> String {
    let secs = timeout.map(|t| t.as_secs()).unwrap_or_default();
    format!("command timed out after {secs}s")
}

/// Await a read task for at most `grace`; on timeout abort it. The shared
/// accumulator buffer retains whatever was read before the abort.
pub(crate) async fn drain_with_grace(
    mut handle: tokio::task::JoinHandle<std::io::Result<()>>,
    grace: Duration,
) {
    if tokio::time::timeout(grace, &mut handle).await.is_err() {
        handle.abort();
        let _ = (&mut handle).await;
    }
}

/// Elevated-path sibling of [`wait_for_child`]: the child lives in the elevated daemon, so bytes
/// arrive in pre-filled `Arc<Mutex<Vec<u8>>>` buffers and the exit is an `exit_future`. Mirrors
/// `wait_for_child`'s semantics: on timeout, fire `kill_tree` (drops the daemon connection ⇒
/// `KILL_ON_JOB_CLOSE`) then best-effort await; fire `kill_tree` before the final buffer snapshot.
/// `wait_for_child` itself stays untouched (still the Job-only path).
#[cfg(target_os = "windows")]
pub(crate) async fn wait_for_elevated(
    run: slab_windows_sandbox::ElevatedRun,
    timeout: Option<Duration>,
) -> Result<SandboxedOutput, SandboxError> {
    let slab_windows_sandbox::ElevatedRun { stdout_buf, stderr_buf, mut exit_future, kill_tree } =
        run;
    // Guarded so an early drop (turn cancellation mid-await) still tears the
    // daemon connection down; the explicit fires below disarm it.
    let mut kill_tree = TreeKillGuard::new(kill_tree);

    let (exit_code, timed_out) = match timeout {
        Some(t) => match tokio::time::timeout(t, &mut exit_future).await {
            Ok(Ok(elev)) => (elev.exit_code, elev.timed_out),
            Ok(Err(_)) => {
                kill_tree.fire();
                return Err(SandboxError::SpawnFailed("elevated exit stream errored".into()));
            }
            Err(_) => {
                // Timed out: drop the daemon connection (kills the Job), then
                // best-effort await Exited within a bounded grace so the
                // pre-filled buffers settle. Report the fixed timeout exit
                // code either way — the real status is untrustworthy once the
                // kill has started (previously a natural exit inside the old
                // 3s window returned the command's real code with timed_out
                // stapled on top).
                kill_tree.fire();
                let _ = tokio::time::timeout(POST_KILL_WAIT_GRACE, &mut exit_future).await;
                (TIMEOUT_EXIT_CODE, true)
            }
        },
        None => {
            let elev = exit_future.await.map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
            (elev.exit_code, elev.timed_out)
        }
    };

    // Free the daemon connection now that we have the exit (the child already exited in the
    // non-timeout path; in the timeout path it was already fired above).
    kill_tree.fire();

    let stdout = stdout_buf.lock().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?.clone();
    let mut stderr =
        stderr_buf.lock().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?.clone();
    if timed_out && stderr.is_empty() {
        stderr.extend_from_slice(timeout_stderr_note(timeout).as_bytes());
    }

    Ok(SandboxedOutput { stdout, stderr, exit_code, timed_out })
}

/// Read a child pipe to EOF. When a sink is present, forward each chunk
/// incrementally (lossy UTF-8); chunks are always accumulated into the shared
/// `buffer` so the final output is available even if this task is aborted.
async fn read_stream<R: AsyncRead + Unpin>(
    reader: Option<R>,
    sink: Option<Arc<dyn OutputSink>>,
    stream: OutputStream,
    buffer: Arc<Mutex<Vec<u8>>>,
) -> std::io::Result<()> {
    let Some(mut reader) = reader else { return Ok(()) };
    let mut buf = [0u8; 8192];
    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        if let Some(sink) = sink.as_ref() {
            let delta = String::from_utf8_lossy(&buf[..n]);
            sink.on_output(stream, &delta);
        }
        if let Ok(mut guard) = buffer.lock() {
            guard.extend_from_slice(&buf[..n]);
        }
    }
    Ok(())
}

/// Build a tree-kill closure for a Unix child: send `SIGKILL` to the child's
/// process group so backgrounded descendants die and release the pipes. `pgid`
/// is the spawned child's id (it must be a group leader — ensured by a new
/// session or `process_group(0)`).
#[cfg(unix)]
pub(crate) fn unix_kill_tree(pgid: Option<u32>) -> Option<Box<dyn FnOnce() + Send + 'static>> {
    pgid.map(|p| {
        Box::new(move || {
            // A negative pid targets the whole process group.
            let _ = unsafe { libc::kill(-(p as i32), libc::SIGKILL) };
        }) as Box<dyn FnOnce() + Send + 'static>
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn tree_kill_guard_fires_on_drop() {
        // Simulates cancellation: the future is dropped without reaching any
        // explicit kill site, and the guard still fires the closure once.
        let fired = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&fired);
        {
            let _guard = TreeKillGuard::new(Some(Box::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            })));
        } // dropped without fire()
        assert_eq!(fired.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn tree_kill_guard_fire_then_drop_fires_once() {
        let fired = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&fired);
        let mut guard = TreeKillGuard::new(Some(Box::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        })));
        assert!(guard.fire(), "first fire should run the closure");
        assert!(!guard.fire(), "second fire should be a no-op");
        drop(guard);
        assert_eq!(fired.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn tree_kill_guard_without_closure_is_noop() {
        let mut guard = TreeKillGuard::new(None);
        assert!(!guard.fire());
    }

    /// The P4 fix: a timeout must actually terminate the child (instead of
    /// waiting for it to finish and stapling `timed_out` on top), and report
    /// the fixed 124 code rather than a fabricated 1. Guards against the
    /// function hanging past the grace window.
    #[tokio::test]
    async fn wait_for_child_kills_on_timeout_and_reports_124() {
        #[cfg(windows)]
        let child = tokio::process::Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 30",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn powershell");
        #[cfg(not(windows))]
        let child = tokio::process::Command::new("sleep")
            .arg("30")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn sleep");

        let started = std::time::Instant::now();
        let output = wait_for_child(child, Some(Duration::from_secs(1)), None, None)
            .await
            .expect("wait_for_child returns after timeout");

        assert!(output.timed_out);
        assert_eq!(output.exit_code, TIMEOUT_EXIT_CODE);
        assert!(String::from_utf8_lossy(&output.stderr).contains("command timed out after 1s"));
        // 1s timeout + kill + ≤5s grace + ≤5s drain grace must stay well
        // under the 30s the command would run if the kill did not land.
        assert!(started.elapsed() < Duration::from_secs(20), "elapsed: {:?}", started.elapsed());
    }
}
