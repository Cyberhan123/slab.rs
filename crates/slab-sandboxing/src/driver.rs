use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
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

        let mut spawned = child.spawn().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
        // Unix: kill the child's whole process group. Windows: assign a
        // KILL_ON_JOB_CLOSE Job Object so the tree dies on drop (a bare
        // `child.kill` only terminates the direct child). Other platforms fall
        // back to the grace backstop in `wait_for_child`.
        #[cfg(unix)]
        let kill_tree = unix_kill_tree(spawned.id());
        #[cfg(windows)]
        let kill_tree = windows_job_kill_tree(&mut spawned);
        #[cfg(not(any(unix, windows)))]
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

/// Initial probe inside [`POST_KILL_WAIT_GRACE`] (Windows): if the child has
/// not exited by this point the first kill did not land, and we escalate to
/// `taskkill /PID <pid> /T /F` before waiting out the remaining grace.
#[cfg(target_os = "windows")]
const KILL_ESCALATION_PROBE: Duration = Duration::from_secs(2);

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
    // Deadline seal: set by the timeout path BEFORE killing. Once sealed, the
    // read tasks drop every chunk they pick up (neither the sink nor the
    // buffer sees it) so output produced after the deadline can never be
    // reported — the exact `slow_command; echo marker` leak.
    let sealed = Arc::new(AtomicBool::new(false));
    let mut stdout_task = Some(tokio::spawn(read_stream(
        stdout,
        stdout_sink,
        OutputStream::Stdout,
        stdout_buf.clone(),
        sealed.clone(),
    )));
    let mut stderr_task = Some(tokio::spawn(read_stream(
        stderr,
        stderr_sink,
        OutputStream::Stderr,
        stderr_buf.clone(),
        sealed.clone(),
    )));

    let (exit_code, timed_out, reads_aborted) = if let Some(timeout) = timeout {
        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(Ok(status)) => (status.code().unwrap_or(-1), false, false),
            Ok(Err(error)) => return Err(SandboxError::SpawnFailed(error.to_string())),
            Err(_) => {
                // Timed out. Seal the streams FIRST so no chunk read from here
                // on can reach the sink or the buffer, then kill the whole tree
                // (job close / process group — a bare child.kill only
                // terminates the direct child and can even fail silently),
                // best-effort kill the direct child, and reap it within a
                // bounded grace (escalating on Windows when the kill did not
                // land). Whatever happens, report the fixed timeout exit code —
                // the real status is untrustworthy once killing has started.
                sealed.store(true, Ordering::SeqCst);
                if kill_tree.fire() {
                    tracing::info!(?pid, "wait_for_child: timeout, process tree killed");
                }
                if let Err(error) = child.kill().await {
                    tracing::warn!(?pid, error = %error, "wait_for_child: kill after timeout failed");
                }
                match reap_after_kill(&mut child, pid).await {
                    ReapOutcome::Reaped => {
                        tracing::info!(?pid, "wait_for_child: reaped child after timeout kill");
                    }
                    ReapOutcome::WaitFailed(error) => {
                        tracing::warn!(?pid, error = %error, "wait_for_child: wait after timeout kill failed");
                    }
                    ReapOutcome::Abandoned => {
                        tracing::warn!(
                            ?pid,
                            "wait_for_child: child did not exit within grace after kill; abandoning wait"
                        );
                    }
                }
                // Abort the read tasks immediately instead of draining: the
                // tree is dead so EOF is imminent, and waiting would only
                // invite bytes from a writer that survived the kill back into
                // the buffer. Accepted trade-off: up to one pipe-buffer of
                // pre-deadline bytes not yet read by the task may be lost.
                if let Some(task) = stdout_task.take() {
                    task.abort();
                }
                if let Some(task) = stderr_task.take() {
                    task.abort();
                }
                (TIMEOUT_EXIT_CODE, true, true)
            }
        }
    } else {
        let status = child.wait().await.map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
        (status.code().unwrap_or(-1), false, false)
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

    // Drain the pipes within a grace window (timeout path already aborted its
    // reads above). If a read still hasn't hit EOF, abort it and keep whatever
    // reached the shared buffer.
    if !reads_aborted {
        if let Some(task) = stdout_task.take() {
            drain_with_grace(task, READ_DRAIN_GRACE).await;
        }
        if let Some(task) = stderr_task.take() {
            drain_with_grace(task, READ_DRAIN_GRACE).await;
        }
    }
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
    match timeout {
        Some(duration) => format!("command timed out after {}s", duration.as_secs()),
        None => "command timed out".to_string(),
    }
}

/// Outcome of the post-kill reap in [`wait_for_child`].
enum ReapOutcome {
    Reaped,
    WaitFailed(std::io::Error),
    Abandoned,
}

/// Reap the child after a timeout kill within a bounded grace. On Windows,
/// probes for [`KILL_ESCALATION_PROBE`] first: a child that survived the tree
/// kill (job close failed, group kill missed it) is escalated to
/// `taskkill /PID <pid> /T /F` — `/T` enumerates descendants by walking the
/// parent chain of live processes, which is only reliable while the root is
/// still alive, and the probe failing is exactly the signal that it is.
#[cfg(target_os = "windows")]
async fn reap_after_kill(child: &mut tokio::process::Child, pid: Option<u32>) -> ReapOutcome {
    match tokio::time::timeout(KILL_ESCALATION_PROBE, child.wait()).await {
        Ok(Ok(_)) => ReapOutcome::Reaped,
        Ok(Err(error)) => ReapOutcome::WaitFailed(error),
        Err(_) => {
            if let Some(pid) = pid {
                tracing::warn!(
                    ?pid,
                    "wait_for_child: child survived kill; escalating to taskkill /T /F"
                );
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
            match tokio::time::timeout(POST_KILL_WAIT_GRACE - KILL_ESCALATION_PROBE, child.wait())
                .await
            {
                Ok(Ok(_)) => ReapOutcome::Reaped,
                Ok(Err(error)) => ReapOutcome::WaitFailed(error),
                Err(_) => ReapOutcome::Abandoned,
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
async fn reap_after_kill(child: &mut tokio::process::Child, _pid: Option<u32>) -> ReapOutcome {
    match tokio::time::timeout(POST_KILL_WAIT_GRACE, child.wait()).await {
        Ok(Ok(_)) => ReapOutcome::Reaped,
        Ok(Err(error)) => ReapOutcome::WaitFailed(error),
        Err(_) => ReapOutcome::Abandoned,
    }
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
                // Snapshot the buffers NOW, before the grace wait: the
                // daemon-side relay may still deliver in-flight frames for a
                // moment after the drop, and any frame that lands after this
                // snapshot belongs to a process that outlived the kill —
                // post-deadline output that must not be reported.
                let stdout = stdout_buf
                    .lock()
                    .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?
                    .clone();
                let mut stderr = stderr_buf
                    .lock()
                    .map_err(|e| SandboxError::SpawnFailed(e.to_string()))?
                    .clone();
                let _ = tokio::time::timeout(POST_KILL_WAIT_GRACE, &mut exit_future).await;
                if stderr.is_empty() {
                    stderr.extend_from_slice(timeout_stderr_note(timeout).as_bytes());
                }
                return Ok(SandboxedOutput {
                    stdout,
                    stderr,
                    exit_code: TIMEOUT_EXIT_CODE,
                    timed_out: true,
                });
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
/// Once `sealed` is set (timeout path, before the kill), every chunk read from
/// that point on is dropped — output produced after the deadline must reach
/// neither the live sink nor the returned buffer.
async fn read_stream<R: AsyncRead + Unpin>(
    reader: Option<R>,
    sink: Option<Arc<dyn OutputSink>>,
    stream: OutputStream,
    buffer: Arc<Mutex<Vec<u8>>>,
    sealed: Arc<AtomicBool>,
) -> std::io::Result<()> {
    let Some(mut reader) = reader else { return Ok(()) };
    let mut buf = [0u8; 8192];
    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        if sealed.load(Ordering::Acquire) {
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

/// Windows counterpart of [`unix_kill_tree`]: attach a `KILL_ON_JOB_CLOSE`
/// Job Object to the freshly spawned child and return a tree-kill closure —
/// closing the job handle kills every assigned process and their descendants.
/// `None` when the Job API is unavailable (then the grace backstop in
/// [`wait_for_child`] still applies).
#[cfg(target_os = "windows")]
fn windows_job_kill_tree(
    child: &mut tokio::process::Child,
) -> Option<Box<dyn FnOnce() + Send + 'static>> {
    let job = slab_windows_sandbox::JobHandle::new_kill_on_close().ok()?;
    let raw = child.raw_handle()?;
    // SAFETY: `raw` is the freshly spawned child's valid, open process handle.
    unsafe { job.assign_process(raw) }.ok()?;
    Some(Box::new(move || drop(job)))
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

    /// Deadline-seal semantics, pinned deterministically (no process, no
    /// kill): bytes written before the seal must land in the buffer, bytes
    /// written after it must be dropped.
    #[tokio::test]
    async fn read_stream_drops_chunks_after_seal() {
        use tokio::io::AsyncWriteExt;

        let buffer: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let sealed = Arc::new(AtomicBool::new(false));
        let (mut writer, reader) = tokio::io::duplex(64);

        let task_buffer = Arc::clone(&buffer);
        let task_sealed = Arc::clone(&sealed);
        let task = tokio::spawn(async move {
            read_stream(Some(reader), None, OutputStream::Stdout, task_buffer, task_sealed)
                .await
                .expect("read_stream");
        });

        writer.write_all(b"pre-deadline").await.expect("write pre");
        // Give the read task a beat to pick the chunk up before sealing.
        tokio::time::sleep(Duration::from_millis(100)).await;
        sealed.store(true, Ordering::SeqCst);
        writer.write_all(b"POST-DEADLINE-LEAK").await.expect("write post");
        // Let the read task observe the post-seal chunk and bail out.
        tokio::time::sleep(Duration::from_millis(100)).await;
        task.abort();

        let collected = buffer.lock().unwrap().clone();
        let text = String::from_utf8_lossy(&collected).into_owned();
        assert!(text.contains("pre-deadline"), "pre-seal bytes missing: {text:?}");
        assert!(!text.contains("POST-DEADLINE-LEAK"), "post-seal bytes leaked: {text:?}");
    }

    #[test]
    fn timeout_stderr_note_without_duration_is_generic() {
        assert_eq!(timeout_stderr_note(None), "command timed out");
    }
}
