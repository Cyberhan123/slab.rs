//! ConPTY (Windows pseudoconsole) spawn path for the elevated Low-IL AppContainer child (S6a).
//!
//! The elevated shell today runs over piped stdio (one anonymous pipe per stream). That works for
//! one-shot `bash -lc "<cmd>"` captures, but the child sees a pipe — not a terminal — so ANSI
//! color, progress bars, and TUI apps behave as if non-interactive. ConPTY gives the child a real
//! pseudoconsole: it attaches via `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`, and the daemon reads the
//! rendered stream from the PTY's output pipe.
//!
//! Opt-in: [`crate::request::SpawnRequest::use_conpty`] (default `false`), surfaced through the
//! `windows_use_conpty` config knob. The piped path stays the default; ConPTY is an explicit choice
//! for terminal-aware fidelity. Because it is opt-in, a ConPTY failure is fail-closed (return the
//! error) rather than silently falling back to piped.
//!
//! The novel combination is the attribute list: `SECURITY_CAPABILITIES` (AppContainer identity,
//! from S3) AND `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` in one `STARTUPINFOEXW`. `STARTF_USESTDHANDLES`
//! is NOT set — it is mutually exclusive with the pseudoconsole attribute (the PTY is the console).
//! `bInheritHandles = FALSE`: the child attaches to the pseudoconsole via the attribute, not handle
//! inheritance, so no daemon handle leaks into the AppContainer child (no `HANDLE_LIST` needed).
//!
//! ConPTY merges stdout+stderr into one stream, so the daemon pumps a single output stream tagged
//! `Stdout` (no separate stderr). Interactive stdin is not driven in S6a (the shell tool is
//! one-shot); the input write-end is closed right after spawn so the child sees stdin EOF.
//!
//! ⚠ **Unvalidated combination:** spawning an AppContainer (Low-IL) child under a pseudoconsole is
//! not a documented Win32 scenario. ConPTY internally spawns a non-AppContainer `conhost` that hosts
//! the PTY; the AppContainer child attaches via a console ALPC port that may not be reachable across
//! the AppContainer silo. If attachment fails the child typically hangs or emits no output — a
//! functionality gap, NOT a security hole (the path stays fail-closed). This combination MUST be
//! empirically validated on the target Windows build (the gated `os_conpty_restricted_child_echo_
//! roundtrip` test under `SLAB_SANDBOX_ELEVATED=1`) before it is relied on; if it does not work,
//! drop the AppContainer overlay on the ConPTY path (spawn under the bare Low-IL token) or document
//! the version requirement.

#![cfg(target_os = "windows")]

use std::ffi::c_void;
use std::mem::{size_of, zeroed};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{PSID, SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES};
use windows_sys::Win32::System::Console::{COORD, ClosePseudoConsole, CreatePseudoConsole, HPCON};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW,
    DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, InitializeProcThreadAttributeList,
    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
    PROCESS_INFORMATION, ResumeThread, STARTUPINFOEXW, TerminateProcess, UpdateProcThreadAttribute,
};

use crate::daemon::{SpawnedChild, build_command_line, build_unicode_env, wide};
use crate::error::{WindowsSandboxError, win32_ctx};
use crate::job::JobHandle;
use crate::token::LowIntegrityToken;

/// RAII teardown for the pseudoconsole + its four pipe handles + the proc-thread attribute list.
/// Created immediately after `CreatePseudoConsole` succeeds. On ANY error between there and the
/// successful spawn, `Drop` closes everything in the load-bearing order (`ClosePseudoConsole` BEFORE
/// the `hInput`/`hOutput` handles it consumed). On the success path the live handles are read out
/// and `disarmed = true`, so `Drop` is a no-op and the consumed handles transfer to [`SpawnedChild`]
/// (the daemon's exit-watcher closes them later). This closes the handle-leak window flagged in the
/// S6a review (previously five `?` points between `CreatePseudoConsole` and `CreateProcessAsUserW`
/// leaked the PTY + pipes + attribute list).
struct ConptyCleanup {
    disarmed: bool,
    hpc: HPCON,
    input_read: Option<HANDLE>,
    output_write: Option<HANDLE>,
    input_write: Option<HANDLE>,
    output_read: Option<HANDLE>,
    /// `Some` once the attribute list is initialized (so `Drop` calls `DeleteProcThreadAttributeList`);
    /// taken back to `None` once the success path deletes it explicitly.
    attr_list: Option<*mut c_void>,
}

impl Drop for ConptyCleanup {
    fn drop(&mut self) {
        if self.disarmed {
            return;
        }
        // SAFETY: close order is load-bearing — the pseudoconsole consumed input_read/output_write
        // and they must outlive ClosePseudoConsole.
        unsafe {
            ClosePseudoConsole(self.hpc);
            for handle in [
                self.input_read.take(),
                self.output_write.take(),
                self.input_write.take(),
                self.output_read.take(),
            ]
            .into_iter()
            .flatten()
            {
                CloseHandle(handle);
            }
            if let Some(attr_list) = self.attr_list.take() {
                DeleteProcThreadAttributeList(attr_list);
            }
        }
    }
}

/// Spawn the Low-IL AppContainer child under a pseudoconsole. Returns the same [`SpawnedChild`]
/// shape as the piped path so the daemon's pump/exit machinery is shared: the PTY output stream is
/// pumped as `Stdout`, and the PTY is torn down by the daemon's exit-watcher after the child exits.
pub(crate) fn spawn_low_il_child_conpty_sync(
    spawn: &crate::request::SpawnRequest,
    package_sid: PSID,
) -> Result<SpawnedChild, WindowsSandboxError> {
    let token = LowIntegrityToken::new()?;

    // Two daemon-owned anonymous pipes form the PTY's input/output channels. The child never opens
    // these — it attaches to the pseudoconsole — so they need no AppContainer package-SID grant and
    // are non-inheritable.
    let (input_read, input_write) = create_daemon_pipe()?;
    let (output_read, output_write) = create_daemon_pipe()?;

    // CreatePseudoConsole consumes `input_read` (hInput) + `output_write` (hOutput); they must stay
    // valid until ClosePseudoConsole. From this point on, `cleanup` owns their teardown on any error.
    let size = COORD { X: 80, Y: 24 };
    let mut hpc: HPCON = 0;
    // CreatePseudoConsole returns an HRESULT (S_OK == 0), not a Win32 error — format it directly
    // (last_os_error would be stale/unrelated).
    let pc_result = unsafe { CreatePseudoConsole(size, input_read, output_write, 0, &mut hpc) };
    if pc_result != 0 {
        unsafe {
            CloseHandle(input_read);
            CloseHandle(output_write);
            CloseHandle(input_write);
            CloseHandle(output_read);
        }
        return Err(WindowsSandboxError::WindowsApi(format!(
            "CreatePseudoConsole failed: HRESULT 0x{pc_result:08X}"
        )));
    }
    let mut cleanup = ConptyCleanup {
        disarmed: false,
        hpc,
        input_read: Some(input_read),
        output_write: Some(output_write),
        input_write: Some(input_write),
        output_read: Some(output_read),
        attr_list: None,
    };

    let job = JobHandle::new()?;
    job.configure_kill_on_close()?;

    // AppContainer identity overlay on top of the Low-IL restricted token (same as the piped path).
    let security_capabilities = SECURITY_CAPABILITIES {
        AppContainerSid: package_sid,
        Capabilities: std::ptr::null_mut(),
        CapabilityCount: 0,
        ..Default::default()
    };

    // Attribute list with TWO attributes: AppContainer identity + the pseudoconsole.
    let mut attr_size: usize = 0;
    unsafe {
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 2, 0, &mut attr_size);
    }
    // Pointer-width-aligned backing store (the attribute list holds pointers internally). Outlives
    // the function (and thus `cleanup`'s `DeleteProcThreadAttributeList` on the error path).
    let attr_words = attr_size.div_ceil(size_of::<u64>()).max(1);
    let mut attr_buf: Vec<u64> = vec![0u64; attr_words];
    let attr_list = attr_buf.as_mut_ptr() as *mut c_void;
    win32_ctx(
        // SAFETY: attr_list is a sufficiently-large, properly-aligned buffer.
        unsafe { InitializeProcThreadAttributeList(attr_list, 2, 0, &mut attr_size) },
        "InitializeProcThreadAttributeList(conpty,2)",
    )?;
    // The list is now initialized — let `cleanup` delete it if a later step fails.
    cleanup.attr_list = Some(attr_list);
    win32_ctx(
        // SAFETY: bind the SECURITY_CAPABILITIES into the list (first attribute).
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
        "UpdateProcThreadAttribute(SECURITY_CAPABILITIES,conpty)",
    )?;
    // SAFETY: PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE is the documented exception — `lpValue` is the
    // HPCON value itself (pointer-sized), not a pointer-to-HPCON.
    win32_ctx(
        unsafe {
            UpdateProcThreadAttribute(
                attr_list,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                hpc as *const c_void,
                size_of::<HPCON>(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        },
        "UpdateProcThreadAttribute(PSEUDOCONSOLE)",
    )?;

    // NO STARTF_USESTDHANDLES — mutually exclusive with the pseudoconsole attribute (the PTY is the
    // console). dwFlags stays 0; hStdInput/Output/Error stay zeroed/ignored.
    let mut startup_ex: STARTUPINFOEXW = unsafe { zeroed() };
    startup_ex.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup_ex.lpAttributeList = attr_list;

    let mut cmd_line = wide(&build_command_line(&spawn.argv));
    let cwd = spawn.cwd.as_ref().map(|p| wide(&p.to_string_lossy()));
    let (env_block, env_flags) = build_unicode_env(&spawn.env);

    let mut pi: PROCESS_INFORMATION = unsafe { zeroed() };
    let ok = unsafe {
        CreateProcessAsUserW(
            token.raw(),
            std::ptr::null(),
            cmd_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0, // bInheritHandles = FALSE: the child attaches to the pseudoconsole via the attribute,
            // not handle inheritance; avoids leaking daemon handles into the AppContainer child.
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
    // The child captured the attribute list; release our copy. Take it back from `cleanup` so its
    // Drop does not double-delete.
    // SAFETY: the list was initialized above and is no longer referenced after this.
    unsafe { DeleteProcThreadAttributeList(attr_list) };
    cleanup.attr_list = None;
    // S6a does not drive interactive stdin: close the input write-end now (the child sees stdin EOF,
    // correct for one-shot `bash -lc "<cmd>"`). Take it from `cleanup` so its Drop does not re-close.
    if let Some(input_write) = cleanup.input_write.take() {
        unsafe { CloseHandle(input_write) };
    }

    // Fail-closed on spawn failure AFTER cleanup (no untracked process was created on failure). The
    // `?`-free explicit return lets `cleanup` (still armed) tear down hpc + the consumed handles.
    if ok == 0 {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "CreateProcessAsUserW(conpty) failed: {}",
            std::io::Error::last_os_error()
        )));
    }

    // Assign the Job WHILE SUSPENDED. On failure, terminate the suspended child (never resume an
    // untracked process) + close the process/thread handles, then let `cleanup` tear down the PTY.
    if let Err(e) = job.assign_process(pi.hProcess) {
        unsafe {
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);
        }
        return Err(e);
    }
    // Resume the main thread. (u32::MAX) ⇒ ResumeThread failed ⇒ terminate + fail closed.
    let prev = unsafe { ResumeThread(pi.hThread) };
    unsafe { CloseHandle(pi.hThread) };
    if prev == u32::MAX {
        unsafe {
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hProcess);
        }
        return Err(WindowsSandboxError::SpawnFailed("ResumeThread failed".into()));
    }

    // Success: disarm `cleanup` (its Drop would otherwise close the handles we are transferring to
    // SpawnedChild for the daemon's exit-watcher to close after the child exits).
    let stdout_read = cleanup.output_read.take().unwrap() as usize;
    let pty_input_read = cleanup.input_read.take().unwrap() as usize;
    let pty_output_write = cleanup.output_write.take().unwrap() as usize;
    let pseudoconsole = cleanup.hpc as usize;
    cleanup.disarmed = true;
    Ok(SpawnedChild {
        job,
        stdout_read,    // the PTY output stream (pumped as Stdout)
        stderr_read: 0, // ConPTY merges streams; no separate stderr
        process: pi.hProcess as usize,
        pseudoconsole,
        pty_input_read,
        pty_output_write,
    })
}

/// A plain anonymous pipe (non-inheritable, default security) for the pseudoconsole's channels.
/// Unlike the piped path's `create_appcontainer_pipe`, no AppContainer package-SID grant is needed
/// because the child never opens these handles — the pseudoconsole does.
fn create_daemon_pipe() -> Result<(HANDLE, HANDLE), WindowsSandboxError> {
    let sa = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        bInheritHandle: 0,
        lpSecurityDescriptor: std::ptr::null_mut(),
    };
    let mut read: HANDLE = std::ptr::null_mut();
    let mut write: HANDLE = std::ptr::null_mut();
    let ok = unsafe { CreatePipe(&mut read, &mut write, &sa, 0) };
    win32_ctx(ok, "CreatePipe(daemon)")?;
    Ok((read, write))
}
