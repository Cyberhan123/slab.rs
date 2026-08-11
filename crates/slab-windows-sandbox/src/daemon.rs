//! The elevated daemon's accept loop. One named-pipe instance per concurrent client; each
//! connection runs in its own task. S2b1 handled only `Ping`; S2b2 drives `Provision` (apply ACLs +
//! write marker), `Spawn` (Low-IL restricted-token child via `CreateProcessAsUserW`), `Output`
//! (stdio relay), `Exited`, and `Kill`.
//!
//! Containment model: the Low-IL token is the primary boundary (kernel `NO_WRITE_UP` blocks writes
//! outside the lowered workspace). The Job (`KILL_ON_JOB_CLOSE`) guarantees process-tree cleanup on
//! `Kill` or on daemon disconnect (the connection task owns the job map; dropping it drops every
//! job). Spawn uses `CREATE_SUSPENDED` → assign-Job → `ResumeThread` so a fast fork-and-exit can
//! never escape the Job. The stdio pipes are created with a Low-IL-allowable SD or the child would
//! have no stdio and silently hang.

use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::os::windows::io::FromRawHandle;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::io::AsyncReadExt;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

/// Shared, connection-wide writer for `Output`/`Exited` frames (concrete: the write half of the
/// named-pipe server, which is `Send + 'static`, so spawned tasks can hold it).
type SharedWriter = Arc<tokio::sync::Mutex<tokio::io::WriteHalf<NamedPipeServer>>>;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::{
    ACL, ACL_REVISION, AddAccessAllowedAce, AddMandatoryAce, CreateWellKnownSid, InitializeAcl,
    InitializeSecurityDescriptor, PSID, SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES,
    SECURITY_DESCRIPTOR, SetSecurityDescriptorDacl, SetSecurityDescriptorSacl, WinLowLabelSid,
    WinWorldSid,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING, SYNCHRONIZE,
};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CREATE_NEW_CONSOLE, CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
    CreateProcessAsUserW, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, INFINITE, InitializeProcThreadAttributeList,
    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, PROCESS_INFORMATION, ResumeThread,
    STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject,
};

use crate::acl;
use crate::appcontainer::PackageSid;
use crate::capability::{FsIsolationStrength, WindowsSetupKind};
use crate::creds;
use crate::error::{WindowsSandboxError, win32_ctx};
use crate::ipc::SetupMarker;
use crate::job::JobHandle;
use crate::pipe::{self, OutputStreamKind, PipeFrame, read_frame, write_frame};
use crate::token::LowIntegrityToken;
use crate::wfp::WfpEngine;

/// 4-byte-aligned scratch buffers for ACLs / SDs built for the stdio pipes.
#[repr(C, align(4))]
struct AclBuf([u8; 256]);

/// Daemon-wide WFP + AppContainer identity state (S3). The WFP engine session + filters live for
/// the daemon's lifetime — NOT per connection — so the first `Provision` opens + registers and
/// later ones skip. The package SID is deterministic from the key fingerprint, so DACL grants +
/// filters match the spawned AppContainer child after a daemon restart. When the daemon exits the
/// `Arc<WfpState>` drops ⇒ `WfpEngine::Drop` ⇒ `FwpmEngineClose0` ⇒ the DYNAMIC session removes the
/// filters automatically.
struct WfpState {
    engine: Mutex<Option<WfpEngine>>,
    package_sid: PackageSid,
    /// Daemon-global provisioning flag. Set by the first connection's `Provision`; read by every
    /// connection's `Spawn`. This MUST be daemon-global (shared via the `Arc<WfpState>`), NOT
    /// per-connection: the orchestrator's `spawn_elevated` opens one fresh connection per spawn and
    /// sends only a `Spawn` frame — it relies on the daemon remembering PRIOR provisioning. When this
    /// lived on per-connection `ConnectionState`, every spawn hit the `!provisioned` short-circuit
    /// and returned exit 1 WITHOUT ever spawning (which masked the real spawn path for the whole
    /// elevated-spawn-debug saga).
    provisioned: std::sync::atomic::AtomicBool,
}

impl WfpState {
    fn new(key: &[u8]) -> Result<Self, WindowsSandboxError> {
        let fingerprint = creds::key_fingerprint(key);
        let package_sid = PackageSid::from_fingerprint(&fingerprint)?;
        Ok(Self {
            engine: Mutex::new(None),
            package_sid,
            provisioned: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Open the WFP engine (DYNAMIC session) + register the package-SID block filters. Idempotent:
    /// the first caller registers; later callers see `Some` and skip. Fail-closed: any error leaves
    /// the slot `None`, so a subsequent `Provision` retries.
    fn ensure_registered(&self) -> Result<(), WindowsSandboxError> {
        let mut guard = self.engine.lock().expect("WFP engine mutex poisoned");
        if guard.is_some() {
            return Ok(());
        }
        let engine = WfpEngine::open()?;
        engine.register_package_block(self.package_sid.as_psid())?;
        *guard = Some(engine);
        Ok(())
    }
}

/// Run the daemon forever (until the process is killed). Loads the DPAPI key once (the daemon is
/// the elevated owner) so it can verify frame tags + write the marker.
pub async fn run_daemon(
    pipe_name: String,
    key_path: PathBuf,
    marker_path: PathBuf,
) -> Result<(), WindowsSandboxError> {
    let key = creds::load_or_create_key(&key_path)?;
    let wfp = Arc::new(WfpState::new(&key)?);
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&pipe_name)
        .map_err(|e| WindowsSandboxError::WindowsApi(format!("create pipe: {e}")))?;

    tracing::info!(%pipe_name, "slab-sandbox-helper daemon listening");
    loop {
        if let Err(e) = server.connect().await {
            tracing::warn!(error = %e, "daemon: pipe connect failed");
            server = ServerOptions::new()
                .create(&pipe_name)
                .map_err(|e2| WindowsSandboxError::WindowsApi(format!("recreate pipe: {e2}")))?;
            continue;
        }
        let next = match ServerOptions::new().create(&pipe_name) {
            Ok(next) => next,
            Err(e) => {
                tracing::error!(error = %e, "daemon: could not create next pipe instance; stopping");
                return Err(WindowsSandboxError::WindowsApi(format!("create next pipe: {e}")));
            }
        };
        let prev = std::mem::replace(&mut server, next);
        let key = key.clone();
        let pipe_name_clone = pipe_name.clone();
        let marker_path_clone = marker_path.clone();
        let wfp_clone = wfp.clone();
        tokio::spawn(async move {
            // Compute the error-log path before `handle_connection` consumes `marker_path_clone`.
            let error_log = marker_path_clone.with_file_name("daemon-error.log");
            if let Err(e) =
                handle_connection(prev, key, pipe_name_clone, marker_path_clone, wfp_clone).await
            {
                tracing::warn!(error = %e, "daemon: connection handler failed");
                // The daemon's stderr is hidden (CREATE_NO_WINDOW), so persist the failure reason
                // next to the marker where the orchestrator/test can surface it. Captures BOTH the
                // Provision HMAC-mismatch path and any ACL/WFP error.
                let line = format!("daemon connection failed: {e}\n");
                let _ = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&error_log)
                    .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
            }
        });
    }
}

/// Per-connection state. Owned by the connection task: when the task ends (client disconnect),
/// `jobs` drops and every `JobHandle` fires `KILL_ON_JOB_CLOSE` ⇒ all children torn down.
struct ConnectionState {
    jobs: HashMap<String, JobHandle>,
    key: Vec<u8>,
    pipe_name: String,
    marker_path: PathBuf,
    wfp: Arc<WfpState>,
}

/// Handle one client connection: read frames, dispatch. Ends when the client disconnects.
async fn handle_connection(
    server: NamedPipeServer,
    key: Vec<u8>,
    pipe_name: String,
    marker_path: PathBuf,
    wfp: Arc<WfpState>,
) -> Result<(), WindowsSandboxError> {
    let (mut reader, writer) = tokio::io::split(server);
    let writer = Arc::new(tokio::sync::Mutex::new(writer));
    let mut state = ConnectionState { jobs: HashMap::new(), key, pipe_name, marker_path, wfp };

    loop {
        let frame = match read_frame(&mut reader).await {
            Ok(f) => f,
            Err(WindowsSandboxError::SetupFailed(msg)) if msg.contains("pipe closed") => break,
            Err(e) => return Err(e),
        };
        match frame {
            PipeFrame::Ping { nonce } => {
                write_frame(&mut *writer.lock().await, &PipeFrame::Pong { nonce }).await?;
            }

            PipeFrame::Provision {
                denied_paths,
                writable_roots,
                workspace_root,
                key_fingerprint,
                tag,
            } => {
                if creds::key_fingerprint(&state.key) != key_fingerprint {
                    return Err(WindowsSandboxError::HmacMismatch);
                }
                if !pipe::tag_matches(
                    &state.key,
                    &(
                        denied_paths.as_slice(),
                        writable_roots.as_slice(),
                        workspace_root.as_ref(),
                        key_fingerprint.as_str(),
                    ),
                    &tag,
                ) {
                    return Err(WindowsSandboxError::HmacMismatch);
                }
                // Apply integrity labels: lower writable roots (so the Low-IL child can write its
                // workspace) + deny-write-Low on protected paths. Fail-closed on any ACL error.
                for root in &writable_roots {
                    acl::lower_to_low_integrity(root)?;
                }
                for p in &denied_paths {
                    // Best-effort defense-in-depth; a denied path outside the workspace is already
                    // blocked by NO_WRITE_UP, but in-workspace protected names need the deny-ACE.
                    let _ = acl::deny_write_low_sid(p);
                }
                // S3: grant the AppContainer package SID write access on each writable root
                // (additive to the Low-IL SACL above) so the AppContainer child can write its
                // workspace. Fail-closed on any ACL error.
                let package_sid = state.wfp.package_sid.as_psid();
                for root in &writable_roots {
                    acl::grant_appcontainer_write(root, package_sid)?;
                }
                // S3: register the WFP package-SID outbound block filter (idempotent, daemon-lifetime).
                // Fail-closed: a registration failure ⇒ the shell stays blocked, never reports
                // OsEnforced network without an actual filter in place.
                state.wfp.ensure_registered()?;
                let marker = SetupMarker {
                    schema: crate::SCHEMA_VERSION,
                    created_at_unix: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0),
                    setup_kind: WindowsSetupKind::ElevatedAclTokenWfp,
                    filesystem_isolation: FsIsolationStrength::OsEnforced,
                    network_isolation: FsIsolationStrength::OsEnforced,
                    key_fingerprint,
                    denied_paths: denied_paths.clone(),
                    writable_roots_lowered: writable_roots.clone(),
                    workspace_root,
                    daemon_pipe: Some(state.pipe_name.clone()),
                    daemon_pid: Some(std::process::id()),
                };
                crate::marker::write_marker(&state.marker_path, &marker)?;
                // Daemon-global: any later connection's Spawn sees provisioned = true.
                state.wfp.provisioned.store(true, std::sync::atomic::Ordering::SeqCst);
                write_frame(&mut *writer.lock().await, &PipeFrame::ProvisionOk { marker }).await?;
            }

            PipeFrame::Spawn { job_token, spawn, tag } => {
                if !state.wfp.provisioned.load(std::sync::atomic::Ordering::SeqCst) {
                    // Fail-closed: no spawn before provisioning (daemon-global flag).
                    let _ = write_frame(
                        &mut *writer.lock().await,
                        &PipeFrame::Exited { job_token, code: 1, timed_out: false },
                    )
                    .await;
                    continue;
                }
                if !pipe::tag_matches(&state.key, &(job_token.as_str(), &spawn), &tag) {
                    return Err(WindowsSandboxError::HmacMismatch);
                }
                // DIAGNOSTIC (temporary): snapshot the daemon's own context before the spawn, so the
                // test can diff it against the test process's snapshot. See SpawnRequest field + the
                // elevated-spawn-debug handoff. No-op unless the request opts in. The "entered" marker
                // is written FIRST so that if the dump (or the spawn) hangs, we can still tell the
                // daemon reached the Spawn arm.
                if spawn.diagnostic_dump_context {
                    let _ = std::fs::write(
                        state.marker_path.with_file_name("daemon-spawn-entered.txt"),
                        format!("job_token={job_token} daemon_pid={}", std::process::id()),
                    );
                    dump_process_context(&state.marker_path.with_file_name("daemon-context.json"));
                }
                match spawn_low_il_child(&job_token, &spawn, writer.clone(), &state.wfp).await {
                    Ok(job) => {
                        state.jobs.insert(job_token, job);
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, job = %job_token, "daemon: spawn failed");
                        let _ = write_frame(
                            &mut *writer.lock().await,
                            &PipeFrame::Exited { job_token, code: 1, timed_out: false },
                        )
                        .await;
                    }
                }
            }

            PipeFrame::Kill { job_token, tag } => {
                if !pipe::tag_matches(&state.key, &job_token.as_str(), &tag) {
                    return Err(WindowsSandboxError::HmacMismatch);
                }
                // Drop the Job ⇒ KILL_ON_JOB_CLOSE tears the process tree down.
                state.jobs.remove(&job_token);
            }

            other => {
                tracing::warn!(?other, "daemon: unexpected frame");
            }
        }
    }
    Ok(())
}

/// Result of the synchronous spawn: the Job plus the stdio read ends + process handle as `usize`
/// (raw `HANDLE`s are not `Send`, so they cross into async code as addresses and are cast back).
/// The ConPTY fields are `0` on the piped path; on the ConPTY path `stderr_read` is `0` (the PTY
/// merges streams) and the exit-watcher tears the pseudoconsole + its consumed handles down after
/// the child exits.
pub(crate) struct SpawnedChild {
    pub(crate) job: JobHandle,
    pub(crate) stdout_read: usize,
    pub(crate) stderr_read: usize,
    pub(crate) process: usize,
    /// ConPTY only: the pseudoconsole handle (`HPCON`). Closed BEFORE the consumed handles below.
    pub(crate) pseudoconsole: usize,
    /// ConPTY only: `hInput`/`hOutput` consumed by `CreatePseudoConsole` (must outlive it).
    pub(crate) pty_input_read: usize,
    pub(crate) pty_output_write: usize,
}

/// Spawn one Low-IL restricted child. CREATE_SUSPENDED → assign Job → resume (fail-closed if
/// assignment fails). Returns the `JobHandle` (stored in the connection's job map so `Kill` /
/// disconnect tears the tree down). Pump tasks + exit-watcher own the stdio read handles + process
/// handle respectively. The Win32 calls live in [`spawn_low_il_child_sync`] (no raw pointers held
/// across `.await`s); this thin wrapper only `await`s while holding `Send` types.
async fn spawn_low_il_child(
    job_token: &str,
    spawn: &crate::request::SpawnRequest,
    writer: SharedWriter,
    wfp: &WfpState,
) -> Result<JobHandle, WindowsSandboxError> {
    // PSID is extracted here as a temporary consumed by the SYNC spawn — it never crosses an
    // `.await`, so the raw pointer does not poison the future's `Send` impl.
    let use_conpty = spawn.use_conpty;
    let child = if use_conpty {
        crate::conpty::spawn_low_il_child_conpty_sync(spawn, wfp.package_sid.as_psid())?
    } else {
        spawn_low_il_child_sync(spawn, wfp.package_sid.as_psid())?
    };

    write_frame(
        &mut *writer.lock().await,
        &PipeFrame::SpawnAccepted { job_token: job_token.to_string() },
    )
    .await?;

    // Pump child stdout → Output frames until EOF. ConPTY merges streams into one, so on the
    // ConPTY path there is only the stdout pump (no separate stderr pump). Both pump JoinHandles are
    // drained by the exit-watcher BEFORE it sends Exited — otherwise the orchestrator resolves on
    // Exited and drops Output frames (cmd's echoed stdout, or its error on stderr) arriving a hair
    // later. (This relay race left every non-ConPTY spawn showing empty stdout/stderr.)
    let stdout_pump = spawn_pump(
        child.stdout_read,
        job_token.to_string(),
        OutputStreamKind::Stdout,
        writer.clone(),
    );
    // stderr pump is piped-path-only (ConPTY merges it into stdout).
    let stderr_pump = if use_conpty {
        None
    } else {
        Some(spawn_pump(
            child.stderr_read,
            job_token.to_string(),
            OutputStreamKind::Stderr,
            writer.clone(),
        ))
    };
    let pumps_to_drain: Vec<tokio::task::JoinHandle<()>> =
        std::iter::once(stdout_pump).chain(stderr_pump).collect();

    // Exit-watcher: wait for process exit (on a blocking thread), then send Exited. It owns the
    // process handle (closes it after). On the ConPTY path it also tears down the pseudoconsole +
    // the handles it consumed (hInput/hOutput must outlive ClosePseudoConsole, so order matters),
    // then awaits the stdout pump so the output tail is drained first. The Job (in the connection
    // map) is what enforces tree-kill on disconnect.
    let w = writer.clone();
    let jt = job_token.to_string();
    let proc_addr = child.process;
    let hpc = child.pseudoconsole;
    let pty_in = child.pty_input_read;
    let pty_out = child.pty_output_write;
    tokio::spawn(async move {
        let code = tokio::task::spawn_blocking(move || -> i32 {
            let proc = proc_addr as HANDLE;
            // Sentinel default: if GetExitCodeProcess fails, the exit surfaces as -16 (i32) —
            // distinct from any real exit code — so a diagnostic run can tell a genuine exit 1 from
            // a failed exit-code read.
            let mut c: u32 = 0xFFFF_FFF0;
            unsafe {
                WaitForSingleObject(proc, INFINITE);
                if GetExitCodeProcess(proc, &mut c) == 0 {
                    c = 0xFFFF_FFF0;
                }
                CloseHandle(proc);
                if hpc != 0 {
                    windows_sys::Win32::System::Console::ClosePseudoConsole(hpc as isize);
                    if pty_in != 0 {
                        CloseHandle(pty_in as HANDLE);
                    }
                    if pty_out != 0 {
                        CloseHandle(pty_out as HANDLE);
                    }
                }
            }
            c as i32
        })
        .await
        .unwrap_or(1);
        // Drain every pump BEFORE sending Exited so the orchestrator does not drop trailing Output
        // frames (stdout output + stderr errors). The process has exited, so its inherited write
        // ends are closed and each pump is about to hit EOF; the timeout only bounds a stuck pump.
        for pump in pumps_to_drain {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(500), pump).await;
        }
        let _ = write_frame(
            &mut *w.lock().await,
            &PipeFrame::Exited { job_token: jt, code, timed_out: false },
        )
        .await;
    });

    Ok(child.job)
}

/// Synchronous Win32 spawn: build the Low-IL token + pipes + Job, `CreateProcessAsUserW` suspended,
/// assign the Job, resume. Returns handles as `usize`. Every raw pointer stays within this fn
/// (never crossing an `.await`), so the calling async future stays `Send`.
fn spawn_low_il_child_sync(
    spawn: &crate::request::SpawnRequest,
    package_sid: PSID,
) -> Result<SpawnedChild, WindowsSandboxError> {
    // DIAGNOSTIC (temporary): spawn via std::process::Command (exactly what the control experiment
    // uses in the TEST process). If this runs in the daemon but raw CreateProcessW does not, the raw
    // call has a bug; if it also fails, the daemon process itself cannot spawn children.
    if spawn.diagnostic_std_spawn {
        return spawn_std_sync(spawn);
    }
    let token = LowIntegrityToken::new()?;
    let (stdout_read, stdout_write) = create_appcontainer_pipe(package_sid)?;
    let (stderr_read, stderr_write) = create_appcontainer_pipe(package_sid)?;
    let job = JobHandle::new()?;
    job.configure_kill_on_close()?;

    // DIAGNOSTIC (temporary): bare CreateProcessW — no token, no inherited pipes, no Job assignment,
    // default STARTUPINFO. Tests whether the daemon can spawn a runnable child AT ALL. See
    // SpawnRequest::diagnostic_bare_spawn.
    if spawn.diagnostic_bare_spawn {
        return spawn_bare_sync(spawn, stdout_read, stdout_write, stderr_read, stderr_write, job);
    }

    // DIAGNOSTIC (temporary): plain Low-IL spawn — no AppContainer identity, no CREATE_NO_WINDOW,
    // plain STARTUPINFO — to isolate why SECURITY_CAPABILITIES children die on init. See
    // SpawnRequest::diagnostic_plain_spawn.
    if spawn.diagnostic_plain_spawn {
        return spawn_plain_low_il_sync(
            spawn,
            &token,
            stdout_read,
            stdout_write,
            stderr_read,
            stderr_write,
            job,
        );
    }

    // AppContainer identity overlay on top of the Low-IL restricted token. `SECURITY_CAPABILITIES`
    // with the package SID and NO capabilities ⇒ the child is an AppContainer without
    // `internetClient`, so the OS default WFP rule + our explicit package-SID filter block its
    // outbound network. (windows-sys 0.61 has no `CreateAppContainerToken`/`PROC_THREAD_ATTRIBUTE_
    // APPCONTAINER` — `PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES` + this struct is the primitive.)
    let security_capabilities = SECURITY_CAPABILITIES {
        AppContainerSid: package_sid,
        Capabilities: std::ptr::null_mut(),
        CapabilityCount: 0,
        ..Default::default()
    };

    // Two-phase InitializeProcThreadAttributeList: first call with a null list to learn the size.
    let mut attr_size: usize = 0;
    // SAFETY: null list + out-size pointer is the documented "query size" call (returns FALSE).
    unsafe {
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut attr_size);
    }
    // Pointer-width-aligned backing store (the attribute list holds pointers internally; a plain
    // `Vec<u8>` would be 1-byte aligned and may misalign).
    let attr_words = attr_size.div_ceil(size_of::<u64>()).max(1);
    let mut attr_buf: Vec<u64> = vec![0u64; attr_words];
    let attr_list = attr_buf.as_mut_ptr() as *mut c_void;
    win32_ctx(
        // SAFETY: attr_list is a sufficiently-large, properly-aligned buffer.
        unsafe { InitializeProcThreadAttributeList(attr_list, 1, 0, &mut attr_size) },
        "InitializeProcThreadAttributeList",
    )?;
    win32_ctx(
        // SAFETY: bind the SECURITY_CAPABILITIES (one attribute) into the list.
        unsafe {
            UpdateProcThreadAttribute(
                attr_list,
                0,
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                &security_capabilities as *const SECURITY_CAPABILITIES as *const c_void,
                size_of::<SECURITY_CAPABILITIES>(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        },
        "UpdateProcThreadAttribute(SECURITY_CAPABILITIES)",
    )?;

    // STARTUPINFOEXW: the existing stdio handle setup (cb sized to the EX struct) + the attribute
    // list carrying the AppContainer identity.
    let null_stdin = create_null_stdin()?;
    let mut startup_ex: STARTUPINFOEXW = unsafe { zeroed() };
    startup_ex.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup_ex.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup_ex.StartupInfo.hStdInput = null_stdin;
    startup_ex.StartupInfo.hStdOutput = stdout_write;
    startup_ex.StartupInfo.hStdError = stderr_write;
    startup_ex.lpAttributeList = attr_list;

    let mut cmd_line = wide(&build_command_line(&spawn.argv));
    let cwd = spawn.cwd.as_ref().map(|p| wide(&p.to_string_lossy()));
    let (env_block, env_flags) = build_unicode_env(&spawn.env);

    let mut pi: PROCESS_INFORMATION = unsafe { zeroed() };
    let ok = unsafe {
        CreateProcessAsUserW(
            token.raw(),
            // lpApplicationName = NULL: Windows parses the first token of `cmd_line` and searches
            // the system dirs (so `cmd` / `bash` resolve even with a minimal env), matching how
            // shell commands name their program.
            std::ptr::null(),
            cmd_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1, // bInheritHandles = TRUE (only the inheritable stdio pipes inherit)
            CREATE_SUSPENDED
                | CREATE_UNICODE_ENVIRONMENT
                | CREATE_NO_WINDOW
                | EXTENDED_STARTUPINFO_PRESENT
                | env_flags,
            env_block.as_ref().map(|b| b.as_ptr() as *const c_void).unwrap_or(std::ptr::null()),
            cwd.as_ref().map(|w| w.as_ptr()).unwrap_or(std::ptr::null()),
            &startup_ex.StartupInfo,
            &mut pi,
        )
    };
    // The child captured the attribute list; release our copy regardless of spawn outcome.
    // SAFETY: the list was initialized above and is no longer referenced after this.
    unsafe { DeleteProcThreadAttributeList(attr_list) };
    // Close child-side write ends + the null stdin in the daemon; the child holds its inherited
    // copies.
    unsafe {
        CloseHandle(stdout_write);
        CloseHandle(stderr_write);
        CloseHandle(null_stdin);
    }
    // Fail-closed on spawn failure AFTER cleanup (no untracked process was created on failure).
    win32_ctx(ok, "CreateProcessAsUserW")?;

    // Assign the Job WHILE SUSPENDED. On failure, terminate the suspended child (never resume an
    // untracked process) and fail closed.
    if let Err(e) = job.assign_process(pi.hProcess) {
        unsafe {
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);
        }
        return Err(e);
    }
    // Resume the main thread. 0xffffffff ⇒ ResumeThread failed ⇒ terminate + fail closed.
    let prev = unsafe { ResumeThread(pi.hThread) };
    unsafe {
        CloseHandle(pi.hThread);
    }
    if prev == u32::MAX {
        unsafe {
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hProcess);
        }
        return Err(WindowsSandboxError::SpawnFailed("ResumeThread failed".into()));
    }

    Ok(SpawnedChild {
        job,
        stdout_read: stdout_read as usize,
        stderr_read: stderr_read as usize,
        process: pi.hProcess as usize,
        pseudoconsole: 0,
        pty_input_read: 0,
        pty_output_write: 0,
    })
}

/// DIAGNOSTIC (temporary): spawn via `std::process::Command` — exactly what the control experiment
/// uses in the TEST process (which successfully ran `cmd /c exit 42` ⇒ 42). If this runs in the
/// daemon but the raw CreateProcessW variants do not, the raw call has a bug; if it also fails, the
/// daemon process itself cannot spawn children. Opens a fresh process handle (by PID) for the exit-
/// watcher and leaks the std Child + its pipes so their handles stay open for the relay plumbing.
fn spawn_std_sync(
    spawn: &crate::request::SpawnRequest,
) -> Result<SpawnedChild, WindowsSandboxError> {
    use std::os::windows::io::AsRawHandle;
    use std::process::{Command, Stdio};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    let prog = spawn.argv.first().ok_or(WindowsSandboxError::EmptyCommand)?;
    let mut cmd = Command::new(prog);
    cmd.args(&spawn.argv[1..]);
    if let Some(cwd) = &spawn.cwd {
        cmd.current_dir(cwd);
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child =
        cmd.spawn().map_err(|e| WindowsSandboxError::SpawnFailed(format!("std spawn: {e}")))?;
    let pid = child.id();
    // Fresh handle for the exit-watcher: SYNCHRONIZE (WaitForSingleObject) + QUERY_LIMITED
    // (GetExitCodeProcess). The std Child's own handle is leaked below so Drop doesn't kill it.
    let proc_handle =
        unsafe { OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if proc_handle.is_null() {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "OpenProcess(std child pid {pid}) failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout_read = stdout_pipe.as_ref().map(|p| p.as_raw_handle() as usize).unwrap_or(0);
    let stderr_read = stderr_pipe.as_ref().map(|p| p.as_raw_handle() as usize).unwrap_or(0);
    // Leak the std Child + pipes so their handles stay open (the exit-watcher + pumps own them now).
    std::mem::forget(child);
    if let Some(p) = stdout_pipe {
        std::mem::forget(p);
    }
    if let Some(p) = stderr_pipe {
        std::mem::forget(p);
    }
    Ok(SpawnedChild {
        job: JobHandle::new()?, // not assigned; dropping is a no-op
        stdout_read,
        stderr_read,
        process: proc_handle as usize,
        pseudoconsole: 0,
        pty_input_read: 0,
        pty_output_write: 0,
    })
}

/// DIAGNOSTIC (temporary): BARE `CreateProcessW` — no token, no inherited pipes (bInheritHandles =
/// FALSE), no Job assignment, default STARTUPINFO. The child is headless. With `cmd /c exit 42` this
/// tests whether the daemon can spawn a runnable child AT ALL: exit 42 ⇒ yes (so the
/// pipes/Job/token/STARTUPINFO setup is the init-failure cause); exit 1 ⇒ the daemon context itself
/// cannot run children. The pipes + job are created only to satisfy the SpawnedChild plumbing; they
/// are not connected to the child.
fn spawn_bare_sync(
    spawn: &crate::request::SpawnRequest,
    stdout_read: HANDLE,
    stdout_write: HANDLE,
    stderr_read: HANDLE,
    stderr_write: HANDLE,
    job: JobHandle,
) -> Result<SpawnedChild, WindowsSandboxError> {
    let mut cmd_line = wide(&build_command_line(&spawn.argv));
    let mut startup: STARTUPINFOW = unsafe { zeroed() };
    startup.cb = size_of::<STARTUPINFOW>() as u32;
    let mut pi: PROCESS_INFORMATION = unsafe { zeroed() };
    // SAFETY: bare CreateProcessW — NULL app name (parsed from cmd line), default STARTUPINFO.
    let ok = unsafe {
        CreateProcessW(
            std::ptr::null(),
            cmd_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,                // bInheritHandles = FALSE — headless child, no inherited stdio
            0,                // no creation flags — run immediately, no Job, no suspend
            std::ptr::null(), // inherit the daemon's environment
            std::ptr::null(), // inherit the daemon's cwd
            &startup,
            &mut pi,
        )
    };
    // The child did not inherit these; close the daemon's copies.
    unsafe {
        CloseHandle(stdout_write);
        CloseHandle(stderr_write);
    }
    win32_ctx(ok, "CreateProcessW(bare diag)")?;
    unsafe { CloseHandle(pi.hThread) };
    Ok(SpawnedChild {
        job, // not assigned to the child; dropping it is a no-op
        stdout_read: stdout_read as usize,
        stderr_read: stderr_read as usize,
        process: pi.hProcess as usize,
        pseudoconsole: 0,
        pty_input_read: 0,
        pty_output_write: 0,
    })
}

/// DIAGNOSTIC (temporary): spawn with the Low-IL token but NO AppContainer identity, NO
/// `CREATE_NO_WINDOW`, and a plain STARTUPINFO. If a binary runs here but dies under the
/// SECURITY_CAPABILITIES path, the AppContainer identity (or CREATE_NO_WINDOW) is the culprit; if it
/// still dies, the Low-IL token itself is unusable for console apps. Shares the token/pipe/Job setup
/// with [`spawn_low_il_child_sync`]; only the CreateProcess call differs.
fn spawn_plain_low_il_sync(
    spawn: &crate::request::SpawnRequest,
    token: &LowIntegrityToken,
    stdout_read: HANDLE,
    stdout_write: HANDLE,
    stderr_read: HANDLE,
    stderr_write: HANDLE,
    job: JobHandle,
) -> Result<SpawnedChild, WindowsSandboxError> {
    let null_stdin = create_null_stdin()?;
    let mut startup: STARTUPINFOW = unsafe { zeroed() };
    startup.cb = size_of::<STARTUPINFOW>() as u32;
    startup.dwFlags = STARTF_USESTDHANDLES;
    startup.hStdInput = null_stdin;
    startup.hStdOutput = stdout_write;
    startup.hStdError = stderr_write;

    let mut cmd_line = wide(&build_command_line(&spawn.argv));
    let cwd = spawn.cwd.as_ref().map(|p| wide(&p.to_string_lossy()));
    let (env_block, env_flags) = build_unicode_env(&spawn.env);

    // DIAGNOSTIC: optionally give the child its own console (CREATE_NEW_CONSOLE) instead of
    // inheriting the daemon's none, to test whether the no-console condition aborts console-app init.
    let mut creation_flags = CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | env_flags;
    if spawn.diagnostic_new_console {
        creation_flags |= CREATE_NEW_CONSOLE;
    }

    let mut pi: PROCESS_INFORMATION = unsafe { zeroed() };
    // DIAGNOSTIC: diagnostic_no_low_il_token ⇒ CreateProcessW (the daemon's own token, NO Low-IL
    // restriction) instead of CreateProcessAsUserW(LowIntegrityToken). Isolates whether the
    // LowIntegrityToken is the init-failure cause.
    let ok = if spawn.diagnostic_no_low_il_token {
        unsafe {
            CreateProcessW(
                std::ptr::null(),
                cmd_line.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
                creation_flags,
                env_block.as_ref().map(|b| b.as_ptr() as *const c_void).unwrap_or(std::ptr::null()),
                cwd.as_ref().map(|w| w.as_ptr()).unwrap_or(std::ptr::null()),
                &startup,
                &mut pi,
            )
        }
    } else {
        unsafe {
            CreateProcessAsUserW(
                token.raw(),
                std::ptr::null(),
                cmd_line.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1, // bInheritHandles = TRUE (the inheritable stdio pipes inherit)
                creation_flags,
                env_block.as_ref().map(|b| b.as_ptr() as *const c_void).unwrap_or(std::ptr::null()),
                cwd.as_ref().map(|w| w.as_ptr()).unwrap_or(std::ptr::null()),
                &startup,
                &mut pi,
            )
        }
    };
    // Close the daemon's write ends + null stdin; the child holds its inherited copies.
    unsafe {
        CloseHandle(stdout_write);
        CloseHandle(stderr_write);
        CloseHandle(null_stdin);
    }
    win32_ctx(ok, "CreateProcess(plain diag)")?;

    if let Err(e) = job.assign_process(pi.hProcess) {
        unsafe {
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);
        }
        return Err(e);
    }
    let prev = unsafe { ResumeThread(pi.hThread) };
    unsafe {
        CloseHandle(pi.hThread);
    }
    if prev == u32::MAX {
        unsafe {
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hProcess);
        }
        return Err(WindowsSandboxError::SpawnFailed("ResumeThread failed".into()));
    }

    Ok(SpawnedChild {
        job,
        stdout_read: stdout_read as usize,
        stderr_read: stderr_read as usize,
        process: pi.hProcess as usize,
        pseudoconsole: 0,
        pty_input_read: 0,
        pty_output_write: 0,
    })
}

/// Spawn a pump task that reads `read_addr` (a pipe read-handle as `usize`) until EOF, forwarding
/// each chunk as an `Output` frame. Returns the task's `JoinHandle` so a caller can await EOF (used
/// by the ConPTY exit-watcher to drain the output tail before sending `Exited`).
fn spawn_pump(
    read_addr: usize,
    job_token: String,
    stream: OutputStreamKind,
    writer: SharedWriter,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let read_handle = read_addr as HANDLE;
        // Wrap the raw anonymous-pipe handle in an async file. tokio::fs::File runs reads on a
        // blocking-pool thread, so a non-OVERLAPPED pipe handle works.
        let file = unsafe { std::fs::File::from_raw_handle(read_handle as _) };
        let mut file = tokio::fs::File::from_std(file);
        let mut buf = [0u8; 8192];
        loop {
            match file.read(&mut buf).await {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let frame = PipeFrame::Output {
                        job_token: job_token.clone(),
                        stream,
                        bytes: buf[..n].to_vec(),
                    };
                    if write_frame(&mut *writer.lock().await, &frame).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    })
}

/// Create an anonymous pipe whose SD grants Everyone AND the AppContainer package SID read/write,
/// and carries a Low mandatory label, so the AppContainer (Low-IL) child can write its stdio
/// without a write-up or a package-SID access denial (else it has no stdio and silently hangs).
fn create_appcontainer_pipe(package_sid: PSID) -> Result<(HANDLE, HANDLE), WindowsSandboxError> {
    // The SID buffers (`_everyone_sid_buf` / `_low_sid_buf`) MUST outlive the SD built below — the
    // pointers point into them. The `_` prefix only suppresses the unused-by-name lint; the
    // bindings still live to end of scope (only a bare `_` would drop early).
    let (_everyone_sid_buf, everyone_sid) = make_well_known(WinWorldSid)?;
    let (_low_sid_buf, low_sid) = make_well_known(WinLowLabelSid)?;

    let mut dacl_buf = AclBuf([0u8; 256]);
    let pdacl = dacl_buf.0.as_mut_ptr() as *mut ACL;
    let mut sacl_buf = AclBuf([0u8; 256]);
    let psacl = sacl_buf.0.as_mut_ptr() as *mut ACL;
    let mut sd: SECURITY_DESCRIPTOR = unsafe { zeroed() };
    let psd = &mut sd as *mut SECURITY_DESCRIPTOR as *mut c_void;

    unsafe {
        if InitializeAcl(pdacl, dacl_buf.0.len() as u32, ACL_REVISION) == 0
            || InitializeAcl(psacl, sacl_buf.0.len() as u32, ACL_REVISION) == 0
        {
            return Err(WindowsSandboxError::WindowsApi("InitializeAcl(pipe) failed".into()));
        }
        let pipe_access = FILE_GENERIC_READ | FILE_GENERIC_WRITE | SYNCHRONIZE;
        if AddAccessAllowedAce(pdacl, ACL_REVISION, pipe_access, everyone_sid) == 0 {
            return Err(WindowsSandboxError::WindowsApi("AddAccessAllowedAce failed".into()));
        }
        // S3: also grant the AppContainer package SID — AppContainer access checks need the package
        // SID (or "All Application Packages") explicitly granted; "Everyone" alone is not reliable.
        if AddAccessAllowedAce(pdacl, ACL_REVISION, pipe_access, package_sid) == 0 {
            return Err(WindowsSandboxError::WindowsApi(
                "AddAccessAllowedAce(package) failed".into(),
            ));
        }
        if AddMandatoryAce(
            psacl,
            ACL_REVISION,
            0,
            windows_sys::Win32::System::SystemServices::SYSTEM_MANDATORY_LABEL_NO_WRITE_UP,
            low_sid,
        ) == 0
        {
            return Err(WindowsSandboxError::WindowsApi("AddMandatoryAce(pipe) failed".into()));
        }
        if InitializeSecurityDescriptor(psd, 1) == 0 {
            return Err(WindowsSandboxError::WindowsApi(
                "InitializeSecurityDescriptor(pipe) failed".into(),
            ));
        }
        // DACL present (Everyone read/write), not defaulted; SACL present (Low label), not defaulted.
        if SetSecurityDescriptorDacl(psd, 1, pdacl, 0) == 0
            || SetSecurityDescriptorSacl(psd, 1, psacl, 0) == 0
        {
            return Err(WindowsSandboxError::WindowsApi(
                "SetSecurityDescriptor*(pipe) failed".into(),
            ));
        }
    }

    let sa = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        bInheritHandle: 1, // the write end inherits into the child
        lpSecurityDescriptor: psd,
    };
    let mut read: HANDLE = std::ptr::null_mut();
    let mut write: HANDLE = std::ptr::null_mut();
    let ok = unsafe { CreatePipe(&mut read, &mut write, &sa, 0) };
    win32_ctx(ok, "CreatePipe")?;
    Ok((read, write))
}

/// Create an inheritable read handle to the null device (`NUL`) for the child's stdin. A REAL null
/// handle — NOT `INVALID_HANDLE_VALUE`: under `STARTF_USESTDHANDLES`, an `INVALID_HANDLE_VALUE`
/// stdin can abort console-app CRT initialization (the child then exits 1 with no output, which is
/// exactly the symptom seen for whoami/curl/cmd). The handle is inheritable so the child gets a
/// copy via `bInheritHandles`; the daemon closes its own copy after `CreateProcess`.
fn create_null_stdin() -> Result<HANDLE, WindowsSandboxError> {
    let nul = wide("NUL");
    let sa = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        bInheritHandle: 1,
        lpSecurityDescriptor: std::ptr::null_mut(),
    };
    // SAFETY: `NUL` is NUL-terminated UTF-16; `sa` is a valid pointer; the null device always exists.
    let h = unsafe {
        CreateFileW(
            nul.as_ptr(),
            FILE_GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            &sa,
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if h == INVALID_HANDLE_VALUE {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "CreateFileW(NUL stdin) failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(h)
}

/// Build a well-known SID into a stack buffer; returns (buffer, pointer). The buffer must outlive
/// any use of the pointer in the SAME scope.
fn make_well_known(sid_kind: i32) -> Result<([u8; 256], HANDLE), WindowsSandboxError> {
    let mut buf = [0u8; 256];
    let mut len = buf.len() as u32;
    let ptr = buf.as_mut_ptr() as *mut c_void;
    let ok = unsafe { CreateWellKnownSid(sid_kind, std::ptr::null_mut(), ptr, &mut len) };
    if ok == 0 {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "CreateWellKnownSid({sid_kind}) failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok((buf, ptr))
}

/// Quote + space-join argv into a single command line for `lpCommandLine` (caller must free-quote
/// conservatively; argv[0] is also passed separately as lpApplicationName).
pub(crate) fn build_command_line(argv: &[String]) -> String {
    argv.iter()
        .map(|a| {
            if a.contains(' ') || a.is_empty() {
                format!("\"{}\"", a.replace('"', "\\\""))
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build a unicode (`CREATE_UNICODE_ENVIRONMENT`) env block from a map. Returns (block, flags); if
/// the map is empty the block is `None` and the child inherits the daemon's environment.
pub(crate) fn build_unicode_env(env: &HashMap<String, String>) -> (Option<Vec<u16>>, u32) {
    if env.is_empty() {
        return (None, 0);
    }
    let mut keys: Vec<&String> = env.keys().collect();
    keys.sort();
    let mut block: Vec<u16> = Vec::new();
    for k in keys {
        let entry = format!("{k}={}", env[k]);
        entry.encode_utf16().for_each(|c| block.push(c));
        block.push(0);
    }
    block.push(0);
    (Some(block), 0)
}

/// Encode a string as a NUL-terminated UTF-16 buffer.
pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ============================================================================
// DIAGNOSTIC (temporary): `dump_process_context`. Remove this whole block — and
// the `Win32_System_StationsAndDesktops` feature, the `SpawnRequest::diagnostic_
// dump_context` field, the re-export in `lib.rs`, and the `os_diagnostic_context_
// diff` test — once the elevated-daemon-can't-spawn-children saga is resolved.
// ============================================================================

/// Snapshot this process's spawn-relevant context — Job membership + limits (candidate a),
/// window-station + desktop (candidate b), console presence, token integrity + elevation
/// (candidate c) — to `path` as JSON. Called from BOTH the test process (which CAN spawn
/// children — the control) and the elevated daemon (which CANNOT), so the two can be diffed to
/// find why the daemon's children abort during CRT init. Best-effort: each section is guarded so a
/// failure in one does not abort the others.
pub fn dump_process_context(path: &std::path::Path) {
    use windows_sys::Win32::System::JobObjects::{
        IsProcessInJob, JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        QueryInformationJobObject,
    };
    use windows_sys::Win32::System::StationsAndDesktops::{
        GetProcessWindowStation, GetThreadDesktop,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetCurrentThreadId};

    let mut m = serde_json::Map::new();
    m.insert("pid".into(), json_num_u32(std::process::id()));

    // (a) Job membership + the immediate Job's limit flags. NULL hJob ⇒ query the calling process's
    // associated Job (nested Jobs: the closest one). The limit flags reveal an active-process cap or
    // a kill-on-close/breakaway policy that could block child creation.
    let mut job = serde_json::Map::new();
    let mut in_job: i32 = 0;
    unsafe { IsProcessInJob(GetCurrentProcess(), std::ptr::null_mut(), &mut in_job) };
    job.insert("in_job".into(), serde_json::Value::Bool(in_job != 0));
    if in_job != 0 {
        let mut ext: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        let mut ret = 0u32;
        let ok = unsafe {
            QueryInformationJobObject(
                std::ptr::null_mut(),
                JobObjectExtendedLimitInformation,
                &mut ext as *mut _ as *mut c_void,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                &mut ret,
            )
        };
        job.insert("query_ok".into(), serde_json::Value::Bool(ok != 0));
        if ok != 0 {
            let flags = ext.BasicLimitInformation.LimitFlags;
            job.insert("limit_flags_hex".into(), serde_json::Value::String(format!("{flags:#x}")));
            job.insert(
                "active_process_limit".into(),
                json_num_u32(ext.BasicLimitInformation.ActiveProcessLimit),
            );
            job.insert(
                "limit_kill_on_job_close".into(),
                serde_json::Value::Bool(flags & JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE != 0),
            );
            job.insert(
                "limit_active_process".into(),
                serde_json::Value::Bool(flags & JOB_OBJECT_LIMIT_ACTIVE_PROCESS != 0),
            );
            job.insert(
                "limit_breakaway_ok".into(),
                serde_json::Value::Bool(flags & JOB_OBJECT_LIMIT_BREAKAWAY_OK != 0),
            );
            job.insert(
                "limit_silent_breakaway_ok".into(),
                serde_json::Value::Bool(flags & JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK != 0),
            );
        }
    }
    m.insert("job".into(), serde_json::Value::Object(job));

    // (b) Window-station + desktop names. A daemon bound to a non-interactive window-station or a
    // desktop it cannot access would produce children that fail console/CRT init.
    let mut ws = serde_json::Map::new();
    ws.insert(
        "window_station".into(),
        user_object_name(unsafe { GetProcessWindowStation() })
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null),
    );
    ws.insert(
        "desktop".into(),
        user_object_name(unsafe { GetThreadDesktop(GetCurrentThreadId()) })
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null),
    );
    m.insert("winsta_desktop".into(), serde_json::Value::Object(ws));

    // Console presence (NULL with CREATE_NO_WINDOW — expected). Included only as a sanity signal.
    m.insert(
        "has_console".into(),
        serde_json::Value::Bool(
            !unsafe { windows_sys::Win32::System::Console::GetConsoleWindow() }.is_null(),
        ),
    );

    // (c) Token integrity level + elevation. The daemon should inherit the elevated High-IL token; a
    // surprise Low/Medium-IL or non-elevated token would explain init failures.
    let mut tok = serde_json::Map::new();
    tok.insert("elevated".into(), serde_json::Value::Bool(crate::token::is_process_elevated()));
    if let Some((label, rid)) = read_integrity() {
        tok.insert("integrity".into(), serde_json::Value::String(label.to_string()));
        tok.insert("integrity_rid".into(), json_num_u32(rid));
    } else {
        tok.insert("integrity".into(), serde_json::Value::Null);
    }
    m.insert("token".into(), serde_json::Value::Object(tok));

    let val = serde_json::Value::Object(m);
    let _ = std::fs::write(path, serde_json::to_string_pretty(&val).unwrap_or_default());
}

/// Read the user object (window-station / desktop) name for the given handle via
/// `GetUserObjectInformationW(UOI_NAME)`. Returns None on any failure.
fn user_object_name(handle: *mut c_void) -> Option<String> {
    use windows_sys::Win32::System::StationsAndDesktops::{GetUserObjectInformationW, UOI_NAME};
    if handle.is_null() {
        return None;
    }
    let mut buf = [0u16; 256];
    let mut needed = 0u32;
    let ok = unsafe {
        GetUserObjectInformationW(
            handle,
            UOI_NAME,
            buf.as_mut_ptr() as *mut c_void,
            (buf.len() * 2) as u32,
            &mut needed,
        )
    };
    if ok == 0 {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

/// Read this process's token integrity label. Returns (name, rid). None on any failure.
fn read_integrity() -> Option<(&'static str, u32)> {
    use windows_sys::Win32::Security::{
        GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_MANDATORY_LABEL,
        TOKEN_QUERY, TokenIntegrityLevel,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut htok: HANDLE = std::ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut htok) } == 0 {
        return None;
    }
    // TOKEN_MANDATORY_LABEL holds a pointer to a SID; use a generous, 8-byte-aligned byte buffer
    // (the struct needs pointer alignment — a plain `[u8; N]` is 1-byte aligned and would misalign).
    #[repr(C, align(8))]
    struct TokenInfoBuf([u8; 64]);
    let mut buf = TokenInfoBuf([0u8; 64]);
    let mut ret = 0u32;
    let ok = unsafe {
        GetTokenInformation(
            htok,
            TokenIntegrityLevel,
            buf.0.as_mut_ptr() as *mut c_void,
            buf.0.len() as u32,
            &mut ret,
        )
    };
    unsafe { CloseHandle(htok) };
    if ok == 0 {
        return None;
    }
    let label = unsafe { &*(buf.0.as_ptr() as *const TOKEN_MANDATORY_LABEL) };
    let sid = label.Label.Sid;
    if sid.is_null() {
        return None;
    }
    let count = unsafe { *GetSidSubAuthorityCount(sid) } as usize;
    if count == 0 {
        return None;
    }
    let rid = unsafe { *GetSidSubAuthority(sid, (count - 1) as u32) };
    let name = match rid {
        0x0000_0000 => "Untrusted",
        0x0000_1000 => "Low",
        0x0000_2000 => "Medium",
        0x0000_3000 => "High",
        0x0000_4000 => "System",
        0x0000_5000 => "Protected",
        _ => "Other",
    };
    Some((name, rid))
}

fn json_num_u32(n: u32) -> serde_json::Value {
    serde_json::Value::Number(serde_json::Number::from(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_quotes_spaces() {
        assert_eq!(build_command_line(&["cmd".into(), "/c".into()]), "cmd /c");
        assert_eq!(
            build_command_line(&["echo".into(), "hello world".into()]),
            "echo \"hello world\""
        );
    }

    #[test]
    fn unicode_env_empty_inherits() {
        let (block, flags) = build_unicode_env(&HashMap::new());
        assert!(block.is_none());
        assert_eq!(flags, 0);
    }

    #[test]
    fn unicode_env_sorted_and_terminated() {
        let mut env = HashMap::new();
        env.insert("B".into(), "2".into());
        env.insert("A".into(), "1".into());
        let (block, _) = build_unicode_env(&env);
        let block = block.unwrap();
        let s: String = block
            .iter()
            .filter(|&&c| c != 0)
            .map(|&c| char::from_u32(c as u32).unwrap_or('?'))
            .collect();
        assert!(s.contains("A=1") && s.contains("B=2"), "{s}");
        // Two trailing NULs (one per entry + final).
        assert_eq!(*block.last().unwrap(), 0u16);
    }
}
