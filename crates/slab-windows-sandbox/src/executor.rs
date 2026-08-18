//! The executor trait + the non-elevated `JobOnlyExecutor` (today's behavior, moved here) and the
//! elevated `ElevatedAclTokenExecutor` (restricted token + ACL + daemon) added in S2b2.

use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::net::windows::named_pipe::ClientOptions;
use tokio::sync::oneshot;

use crate::capability::{CapabilitySnapshot, FsIsolationStrength, WindowsSetupKind};
use crate::elevation::Elevator;
use crate::error::WindowsSandboxError;
use crate::ipc::SetupMarker;
use crate::job::JobHandle;
use crate::request::{
    ElevatedExit, ElevatedRun, ErasedOutputSink, PrepareContext, SpawnRequest, SpawnedChild,
};

/// Produces isolated child processes on Windows. The thin shim in
/// `slab_sandboxing::platform::windows` holds one of these and delegates to it.
///
/// Trait evolution is additive: S2a only needs `capabilities` + `spawn_job_only`;
/// S2b2 adds `prepare` (elevation round-trip + ACL provisioning) and `spawn_elevated`
/// (Low-IL restricted-token child via the elevated daemon).
pub trait WindowsSandboxExecutor: Send + Sync {
    /// Honest report of what this executor currently enforces.
    fn capabilities(&self) -> CapabilitySnapshot;

    /// Non-elevated spawn: build a `tokio::process::Child`, assign it to a Job Object, and
    /// return it with a tree-kill closure. The shim feeds both into the shared `wait_for_child`.
    fn spawn_job_only(&self, req: &SpawnRequest) -> Result<SpawnedChild, WindowsSandboxError>;

    /// Drive elevation to completion + apply integrity-label ACLs (idempotent). After Ok,
    /// `capabilities().provisioned` flips true and `spawn_elevated` is usable. Default: not
    /// supported (non-elevated executors).
    fn prepare(&self, _ctx: &PrepareContext) -> Result<(), WindowsSandboxError> {
        Err(WindowsSandboxError::SetupFailed("prepare not supported by this executor".into()))
    }

    /// Elevated spawn: returns buffers + exit_future + kill_tree. Only valid after `prepare()`.
    /// Default: not supported.
    fn spawn_elevated(
        &self,
        _req: &SpawnRequest,
        _sink: Option<Arc<dyn ErasedOutputSink>>,
    ) -> Result<ElevatedRun, WindowsSandboxError> {
        Err(WindowsSandboxError::SetupFailed(
            "spawn_elevated not supported by this executor".into(),
        ))
    }

    /// Whether this executor can reach the OS-enforced (elevated) path.
    fn is_elevated_capable(&self) -> bool {
        false
    }
}

/// The non-elevated baseline executor: Job-Object tree-cleanup + (caller-applied) lexical guard.
/// Behaviorally identical to the pre-S2 `WindowsSandboxDriver`. This is the default until the
/// user opts into elevation (S2b).
pub struct JobOnlyExecutor {
    setup_required: bool,
}

impl JobOnlyExecutor {
    pub fn new(setup_required: bool) -> Self {
        Self { setup_required }
    }
}

impl WindowsSandboxExecutor for JobOnlyExecutor {
    fn capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot::job_only(self.setup_required)
    }

    fn spawn_job_only(&self, req: &SpawnRequest) -> Result<SpawnedChild, WindowsSandboxError> {
        let program = req.argv.first().ok_or(WindowsSandboxError::EmptyCommand)?;
        let mut command = tokio::process::Command::new(program);
        command.args(&req.argv[1..]);
        for (key, value) in &req.env {
            command.env(key, value);
        }
        if let Some(ref cwd) = req.cwd {
            command.current_dir(cwd);
        }
        command.kill_on_drop(true);
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let spawned =
            command.spawn().map_err(|e| WindowsSandboxError::SpawnFailed(e.to_string()))?;
        let job = JobHandle::new()?;
        job.configure_kill_on_close()?;
        let process_handle = spawned.raw_handle().ok_or_else(|| {
            WindowsSandboxError::SetupFailed("spawned child has no process handle".to_string())
        })?;
        job.assign_process(process_handle as windows_sys::Win32::Foundation::HANDLE)?;

        tracing::debug!(pid = spawned.id(), "spawned process in Windows Job Object");
        // Dropping `job` fires KILL_ON_JOB_CLOSE → tree dies → pipes released. The shim's
        // `wait_for_child` invokes this closure right after the direct child exits.
        let kill_tree: Box<dyn FnOnce() + Send + 'static> = Box::new(move || drop(job));
        Ok(SpawnedChild { child: spawned, kill_tree: Some(kill_tree) })
    }
}

/// The elevated executor: starts (once, via UAC) a long-lived daemon that provisions Low-IL
/// integrity-label ACLs and spawns each sandboxed child under a restricted token. The orchestrator
/// (non-elevated) drives the daemon over a named pipe; the daemon dies with slab-server (owner-PID
/// watchdog), so every server start pays one UAC.
pub struct ElevatedAclTokenExecutor {
    cfg: PrepareContext,
    state: Mutex<ElevatedState>,
}

struct ElevatedState {
    provisioned: bool,
    /// Provisioning mechanism the daemon actually applied (S3 distinguishes `ElevatedAclTokenWfp`
    /// from the S2 `ElevatedAclToken` baseline). Captured from the `ProvisionOk` marker.
    setup_kind: WindowsSetupKind,
    /// Honest network-isolation strength the daemon reported in the marker (S3).
    network_isolation: FsIsolationStrength,
}

impl ElevatedAclTokenExecutor {
    pub fn new(cfg: PrepareContext) -> Self {
        Self {
            cfg,
            state: Mutex::new(ElevatedState {
                provisioned: false,
                setup_kind: WindowsSetupKind::ElevatedAclToken,
                network_isolation: FsIsolationStrength::None,
            }),
        }
    }

    /// Pipe name, derived from the key fingerprint so it is per-user + unguessable to other users.
    fn pipe_name(&self, fingerprint: &str) -> String {
        daemon_pipe_name(fingerprint)
    }
}

impl WindowsSandboxExecutor for ElevatedAclTokenExecutor {
    fn capabilities(&self) -> CapabilitySnapshot {
        // Marker-derived honest report: the daemon writes `setup_kind` + `network_isolation` into
        // the ProvisionOk marker, so the orchestrator reports exactly what was enforced — not a
        // hardcoded guess.
        let s = self.state.lock().unwrap();
        if !s.provisioned {
            return CapabilitySnapshot::degraded_required();
        }
        match s.setup_kind {
            WindowsSetupKind::ElevatedAclTokenWfp => CapabilitySnapshot::elevated_wfp(),
            _ => CapabilitySnapshot::elevated(),
        }
    }

    fn spawn_job_only(&self, _req: &SpawnRequest) -> Result<SpawnedChild, WindowsSandboxError> {
        Err(WindowsSandboxError::SetupFailed("elevated executor does not spawn job-only".into()))
    }

    fn is_elevated_capable(&self) -> bool {
        true
    }

    fn prepare(&self, ctx: &PrepareContext) -> Result<(), WindowsSandboxError> {
        if self.state.lock().map(|s| s.provisioned).unwrap_or(false) {
            return Ok(());
        }

        let key = crate::creds::load_or_create_key(&ctx.key_path)?;
        let fingerprint = crate::creds::key_fingerprint(&key);
        let pipe_name = self.pipe_name(&fingerprint);
        let helper_exe = ctx.helper_exe.clone();
        let cfg = ctx.clone();

        // Run the daemon IPC (connect/ping/provision — async) on a dedicated OS thread with its
        // own current-thread runtime. This avoids the `block_on`-inside-tokio panic regardless of
        // whether `prepare` is called from a runtime thread (bootstrap) or not.
        let marker = std::thread::spawn(move || -> Result<SetupMarker, WindowsSandboxError> {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| WindowsSandboxError::SetupFailed(format!("temp runtime: {e}")))?;
            rt.block_on(prepare_daemon(pipe_name.clone(), key, helper_exe, cfg))
        })
        .join()
        .map_err(|_| WindowsSandboxError::SetupFailed("provision thread panicked".into()))??;
        {
            // Capture what the daemon actually enforced (S3: WFP + AppContainer ⇒ ElevatedAclTokenWfp
            // + OsEnforced network) so `capabilities()` reports the truth, marker-derived.
            let mut state = self.state.lock().unwrap();
            state.provisioned = true;
            state.setup_kind = marker.setup_kind;
            state.network_isolation = marker.network_isolation;
        }
        Ok(())
    }

    fn spawn_elevated(
        &self,
        req: &SpawnRequest,
        sink: Option<Arc<dyn ErasedOutputSink>>,
    ) -> Result<ElevatedRun, WindowsSandboxError> {
        if !self.state.lock().map(|s| s.provisioned).unwrap_or(false) {
            return Err(WindowsSandboxError::SetupFailed(
                "elevated executor not provisioned (call prepare first)".into(),
            ));
        }
        let key = crate::creds::load_or_create_key(&self.cfg.key_path)?;
        let fingerprint = crate::creds::key_fingerprint(&key);
        let pipe_name = self.pipe_name(&fingerprint);
        let job_token = uuid::Uuid::new_v4().simple().to_string();
        let tag = crate::pipe::spawn_tag(&key, &job_token, req)?;

        let stdout_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let (exit_tx, exit_rx) = oneshot::channel::<ElevatedExit>();

        // One fresh connection per spawn (the daemon is globally provisioned). `ClientOptions::open`
        // is synchronous; all async I/O (send Spawn, relay Output/Exited) happens in the reader task.
        let client = ClientOptions::new().open(&pipe_name).map_err(|e| {
            WindowsSandboxError::WindowsApi(format!("connect to sandbox daemon: {e}"))
        })?;

        let stdout_for_task = Arc::clone(&stdout_buf);
        let stderr_for_task = Arc::clone(&stderr_buf);
        let job_token_for_task = job_token.clone();
        let req_owned = req.clone();
        let reader_task = tokio::spawn(async move {
            use crate::pipe::{OutputStreamKind, PipeFrame, read_frame, write_frame};
            let (mut reader, mut writer) = tokio::io::split(client);
            // Send the Spawn frame first.
            if write_frame(
                &mut writer,
                &PipeFrame::Spawn { job_token: job_token_for_task, spawn: req_owned, tag },
            )
            .await
            .is_err()
            {
                return;
            }
            loop {
                match read_frame(&mut reader).await {
                    Ok(PipeFrame::Output { stream, bytes, .. }) => {
                        let buf = match stream {
                            OutputStreamKind::Stdout => &stdout_for_task,
                            OutputStreamKind::Stderr => &stderr_for_task,
                        };
                        buf.lock().unwrap().extend_from_slice(&bytes);
                        if let Some(s) = &sink {
                            s.on_output(stream, &String::from_utf8_lossy(&bytes));
                        }
                    }
                    Ok(PipeFrame::Exited { code, timed_out, .. }) => {
                        let _ = exit_tx.send(ElevatedExit { exit_code: code, timed_out });
                        break;
                    }
                    Ok(_) => {}      // SpawnAccepted / unexpected — ignore
                    Err(_) => break, // pipe closed
                }
            }
        });

        let exit_future = Box::pin(async move {
            exit_rx
                .await
                .map_err(|_| WindowsSandboxError::SpawnFailed("elevated exit stream closed".into()))
        });

        // kill_tree: dropping the guard aborts the reader task (which owns the connection) → the
        // daemon sees the disconnect and tears the Job down (KILL_ON_JOB_CLOSE).
        let kill_tree: Box<dyn FnOnce() + Send + 'static> = Box::new(move || {
            drop(ConnGuard { _reader_task: reader_task });
        });

        Ok(ElevatedRun { stdout_buf, stderr_buf, exit_future, kill_tree: Some(kill_tree) })
    }
}

/// Owns the reader task (which owns the connection) so dropping it closes the pipe.
struct ConnGuard {
    _reader_task: tokio::task::JoinHandle<()>,
}

/// Named pipe path for a given key fingerprint.
fn daemon_pipe_name(fingerprint: &str) -> String {
    format!(r"\\.\pipe\slab-sandbox-helper-{fingerprint}")
}

/// Connect/ping/start/provision the daemon (async; run via a temp runtime in `prepare`). Returns
/// the `SetupMarker` the daemon writes back in `ProvisionOk` so `prepare` can capture the enforced
/// `setup_kind` + `network_isolation`.
async fn prepare_daemon(
    pipe_name: String,
    key: Vec<u8>,
    helper_exe: std::path::PathBuf,
    cfg: PrepareContext,
) -> Result<SetupMarker, WindowsSandboxError> {
    use crate::pipe::{PipeFrame, ping_with_timeout, read_frame, write_frame};

    // If no daemon is alive, start one (one UAC at enable-time; reconnect is no-UAC). Thread the
    // key + marker paths so the daemon loads the SAME key the orchestrator signs with (HMAC must
    // match) and writes the marker where the orchestrator expects.
    if !daemon_alive(&pipe_name).await {
        if crate::token::is_process_elevated() {
            crate::elevation::launch_daemon_direct(
                &helper_exe,
                &pipe_name,
                &cfg.key_path,
                &cfg.marker_path,
            )?;
        } else {
            crate::elevation::ShellElevator::default()
                .run_serve(&helper_exe, &pipe_name, &cfg.key_path, &cfg.marker_path)
                .map_err(|e| match e {
                    crate::elevation::HelperLaunchError::Declined => {
                        WindowsSandboxError::ElevationDeclined
                    }
                    crate::elevation::HelperLaunchError::Timeout => {
                        WindowsSandboxError::ElevationTimeout
                    }
                    crate::elevation::HelperLaunchError::Failed(m) => {
                        WindowsSandboxError::ElevationFailed(m)
                    }
                })?;
        }
        // Wait up to 15s for the daemon to create its pipe + answer a ping.
        let _ = ping_with_timeout(&pipe_name, "prepare", Duration::from_secs(15)).await?;
    }

    // Provision: open a connection, send the Provision frame, read ProvisionOk.
    let client = ClientOptions::new()
        .open(&pipe_name)
        .map_err(|e| WindowsSandboxError::WindowsApi(format!("connect for provision: {e}")))?;
    let (mut reader, mut writer) = tokio::io::split(client);
    let fingerprint = crate::creds::key_fingerprint(&key);
    let tag = crate::pipe::provision_tag(
        &key,
        &cfg.denied_paths,
        &cfg.writable_roots,
        cfg.workspace_root.as_ref(),
        &fingerprint,
    )?;
    write_frame(
        &mut writer,
        &PipeFrame::Provision {
            denied_paths: cfg.denied_paths.clone(),
            writable_roots: cfg.writable_roots.clone(),
            workspace_root: cfg.workspace_root.clone(),
            key_fingerprint: fingerprint,
            tag,
        },
    )
    .await?;
    let reply = tokio::time::timeout(Duration::from_secs(30), read_frame(&mut reader))
        .await
        .map_err(|_| WindowsSandboxError::ElevationTimeout)?
        .map_err(|e| WindowsSandboxError::SetupFailed(format!("provision read: {e}")))?;
    match reply {
        PipeFrame::ProvisionOk { marker } => Ok(marker),
        other => Err(WindowsSandboxError::SetupFailed(format!(
            "unexpected reply to Provision: {other:?}"
        ))),
    }
}

/// Whether the daemon answers a ping (short timeout).
async fn daemon_alive(pipe_name: &str) -> bool {
    crate::pipe::ping_with_timeout(pipe_name, "alive", Duration::from_millis(300)).await.is_ok()
}
