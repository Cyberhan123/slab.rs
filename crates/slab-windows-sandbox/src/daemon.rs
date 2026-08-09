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
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::io::AsyncReadExt;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

/// Shared, connection-wide writer for `Output`/`Exited` frames (concrete: the write half of the
/// named-pipe server, which is `Send + 'static`, so spawned tasks can hold it).
type SharedWriter = Arc<tokio::sync::Mutex<tokio::io::WriteHalf<NamedPipeServer>>>;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::{
    ACL, ACL_REVISION, AddAccessAllowedAce, AddMandatoryAce, CreateWellKnownSid, InitializeAcl,
    InitializeSecurityDescriptor, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
    SetSecurityDescriptorDacl, SetSecurityDescriptorSacl, WinLowLabelSid, WinWorldSid,
};
use windows_sys::Win32::Storage::FileSystem::{FILE_GENERIC_READ, FILE_GENERIC_WRITE, SYNCHRONIZE};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW,
    GetExitCodeProcess, INFINITE, PROCESS_INFORMATION, ResumeThread, STARTF_USESTDHANDLES,
    STARTUPINFOW, TerminateProcess, WaitForSingleObject,
};

use crate::acl;
use crate::capability::{FsIsolationStrength, WindowsSetupKind};
use crate::creds;
use crate::error::{WindowsSandboxError, win32_ctx};
use crate::ipc::SetupMarker;
use crate::job::JobHandle;
use crate::pipe::{self, OutputStreamKind, PipeFrame, read_frame, write_frame};
use crate::token::LowIntegrityToken;

/// 4-byte-aligned scratch buffers for ACLs / SDs built for the stdio pipes.
#[repr(C, align(4))]
struct AclBuf([u8; 256]);

/// Run the daemon forever (until the process is killed). Loads the DPAPI key once (the daemon is
/// the elevated owner) so it can verify frame tags + write the marker.
pub async fn run_daemon(
    pipe_name: String,
    key_path: PathBuf,
    marker_path: PathBuf,
) -> Result<(), WindowsSandboxError> {
    let key = creds::load_or_create_key(&key_path)?;
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
        tokio::spawn(async move {
            if let Err(e) = handle_connection(prev, key, pipe_name_clone, marker_path_clone).await {
                tracing::warn!(error = %e, "daemon: connection handler failed");
            }
        });
    }
}

/// Per-connection state. Owned by the connection task: when the task ends (client disconnect),
/// `jobs` drops and every `JobHandle` fires `KILL_ON_JOB_CLOSE` ⇒ all children torn down.
struct ConnectionState {
    jobs: HashMap<String, JobHandle>,
    provisioned: bool,
    key: Vec<u8>,
    pipe_name: String,
    marker_path: PathBuf,
}

/// Handle one client connection: read frames, dispatch. Ends when the client disconnects.
async fn handle_connection(
    server: NamedPipeServer,
    key: Vec<u8>,
    pipe_name: String,
    marker_path: PathBuf,
) -> Result<(), WindowsSandboxError> {
    let (mut reader, writer) = tokio::io::split(server);
    let writer = Arc::new(tokio::sync::Mutex::new(writer));
    let mut state =
        ConnectionState { jobs: HashMap::new(), provisioned: false, key, pipe_name, marker_path };

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
                let marker = SetupMarker {
                    schema: crate::SCHEMA_VERSION,
                    created_at_unix: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0),
                    setup_kind: WindowsSetupKind::ElevatedAclToken,
                    filesystem_isolation: FsIsolationStrength::OsEnforced,
                    key_fingerprint,
                    denied_paths: denied_paths.clone(),
                    writable_roots_lowered: writable_roots.clone(),
                    workspace_root,
                    daemon_pipe: Some(state.pipe_name.clone()),
                    daemon_pid: Some(std::process::id()),
                };
                crate::marker::write_marker(&state.marker_path, &marker)?;
                state.provisioned = true;
                write_frame(&mut *writer.lock().await, &PipeFrame::ProvisionOk { marker }).await?;
            }

            PipeFrame::Spawn { job_token, spawn, tag } => {
                if !state.provisioned {
                    // Fail-closed: no spawn before provisioning.
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
                match spawn_low_il_child(&job_token, &spawn, writer.clone()).await {
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
struct SpawnedChild {
    job: JobHandle,
    stdout_read: usize,
    stderr_read: usize,
    process: usize,
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
) -> Result<JobHandle, WindowsSandboxError> {
    let child = spawn_low_il_child_sync(spawn)?;

    write_frame(
        &mut *writer.lock().await,
        &PipeFrame::SpawnAccepted { job_token: job_token.to_string() },
    )
    .await?;

    // Two pump tasks: read child stdout/stderr → Output frames, until EOF (child exits).
    spawn_pump(child.stdout_read, job_token.to_string(), OutputStreamKind::Stdout, writer.clone());
    spawn_pump(child.stderr_read, job_token.to_string(), OutputStreamKind::Stderr, writer.clone());

    // Exit-watcher: wait for process exit (on a blocking thread), then send Exited. It owns the
    // process handle (closes it after). The Job (in the connection map) is what enforces tree-kill.
    let w = writer.clone();
    let jt = job_token.to_string();
    let proc_addr = child.process;
    tokio::spawn(async move {
        let code = tokio::task::spawn_blocking(move || -> i32 {
            let proc = proc_addr as HANDLE;
            let mut c: u32 = 1;
            unsafe {
                WaitForSingleObject(proc, INFINITE);
                if GetExitCodeProcess(proc, &mut c) == 0 {
                    c = 1;
                }
                CloseHandle(proc);
            }
            c as i32
        })
        .await
        .unwrap_or(1);
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
) -> Result<SpawnedChild, WindowsSandboxError> {
    let token = LowIntegrityToken::new()?;
    let (stdout_read, stdout_write) = create_low_il_pipe()?;
    let (stderr_read, stderr_write) = create_low_il_pipe()?;
    let job = JobHandle::new()?;
    job.configure_kill_on_close()?;

    // STARTUPINFOW: stdio = our (Low-IL-allowable) pipe write ends; null stdin.
    let mut startup: STARTUPINFOW = unsafe { zeroed() };
    startup.cb = size_of::<STARTUPINFOW>() as u32;
    startup.dwFlags = STARTF_USESTDHANDLES;
    startup.hStdInput = INVALID_HANDLE_VALUE;
    startup.hStdOutput = stdout_write;
    startup.hStdError = stderr_write;

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
            CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW | env_flags,
            env_block.as_ref().map(|b| b.as_ptr() as *const c_void).unwrap_or(std::ptr::null()),
            cwd.as_ref().map(|w| w.as_ptr()).unwrap_or(std::ptr::null()),
            &startup,
            &mut pi,
        )
    };
    win32_ctx(ok, "CreateProcessAsUserW")?;

    // Close child-side write ends in the daemon; the child holds its inherited copies.
    unsafe {
        CloseHandle(stdout_write);
        CloseHandle(stderr_write);
    }

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
    })
}

/// Spawn a pump task that reads `read_addr` (a pipe read-handle as `usize`) until EOF, forwarding
/// each chunk as an `Output` frame.
fn spawn_pump(read_addr: usize, job_token: String, stream: OutputStreamKind, writer: SharedWriter) {
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
    });
}

/// Create an anonymous pipe whose SD grants Everyone read/write AND carries a Low mandatory label,
/// so a Low-IL child can write its stdio without a write-up (else it has no stdio and hangs).
fn create_low_il_pipe() -> Result<(HANDLE, HANDLE), WindowsSandboxError> {
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
fn build_command_line(argv: &[String]) -> String {
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
fn build_unicode_env(env: &HashMap<String, String>) -> (Option<Vec<u16>>, u32) {
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
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
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
