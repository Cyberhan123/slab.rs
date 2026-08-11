//! Process-tree containment for spawned MCP stdio servers (S6c).
//!
//! Today [`crate::stdio::StdioMcpClient`] spawns the server with a bare `tokio::process::Command`
//! and stores the `Child` in a field that is never killed — so when the client drops, the server
//! (and any process it forked) is orphaned and keeps running. This module fixes that with reliable
//! tree teardown on drop:
//!
//! - **Windows**: a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` is created and the child is
//!   assigned to it; dropping the job handle tears the whole tree down.
//! - **Unix**: the child is made a process-group leader (`process_group(0)`); the guard sends
//!   `SIGKILL` to the negative pgid on drop.
//!
//! `kill_on_drop(true)` is set on the command in both cases as belt-and-suspenders (it only kills
//! the direct child; the Job/group is what reaches grandchildren).
//!
//! Network policy is intentionally NOT changed here — MCP servers are long-lived and almost always
//! need outbound network (a GitHub MCP server fetches GitHub, etc.). This is tree-kill + containment,
//! not restriction. An inline minimal Job Object helper avoids pulling the heavyweight
//! `slab-windows-sandbox` crate (and its WFP/ACL/elevation deps) into this lightweight client.
//!
//! Windows note: the Job is assigned after `spawn()`, so a grandchild forked in the tiny window
//! before assignment escapes the Job. tokio offers no `CREATE_SUSPENDED` hook, and MCP servers do
//! not fork before the `initialize` handshake, so this race is negligible for the orphan-fix goal.

#[cfg(target_os = "windows")]
mod windows_job {
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::OpenProcess;

    // PROCESS_SET_QUOTA | PROCESS_TERMINATE = 0x0100 | 0x0001
    const PROCESS_ASSIGN_FLAGS: u32 = 0x0100 | 0x0001;

    /// A Job Object handle that tears its process tree down on drop (`KILL_ON_JOB_CLOSE`).
    pub(super) struct JobHandle(HANDLE);

    unsafe impl Send for JobHandle {}

    impl JobHandle {
        pub(super) fn new() -> std::io::Result<Self> {
            // SAFETY: both pointers null ⇒ unnamed, unsecured job (standard creation).
            let h = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if h.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Self(h))
        }

        pub(super) fn configure_kill_on_close(&self) -> std::io::Result<()> {
            // SAFETY: zeroed struct with only LimitFlags set is the documented kill-on-close setup.
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = unsafe {
                SetInformationJobObject(
                    self.0,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const c_void,
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if ok == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        }

        pub(super) fn assign(&self, process: HANDLE) -> std::io::Result<()> {
            // SAFETY: `process` is a live process handle opened with PROCESS_ASSIGN_FLAGS.
            if unsafe { AssignProcessToJobObject(self.0, process) } == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        }
    }

    impl Drop for JobHandle {
        fn drop(&mut self) {
            // SAFETY: handle was created by CreateJobObjectW and is owned solely by this struct.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    pub(super) fn build_guard(child: &tokio::process::Child) -> Option<Box<dyn FnOnce() + Send>> {
        let pid = child.id()?;
        let job = JobHandle::new().ok()?;
        job.configure_kill_on_close().ok()?;
        // SAFETY: PROCESS_ASSIGN_FLAGS grants the access AssignProcessToJobObject needs.
        let proc = unsafe { OpenProcess(PROCESS_ASSIGN_FLAGS, 0, pid) };
        if proc.is_null() {
            return None;
        }
        let assigned = job.assign(proc).is_ok();
        // SAFETY: the handle is no longer needed after assignment; the Job tracks the process.
        unsafe {
            CloseHandle(proc);
        }
        if !assigned {
            return None;
        }
        Some(Box::new(move || drop(job)))
    }
}

/// Configure the command for containment BEFORE spawn (called by `StdioMcpClient::connect`). No-op
/// on platforms without a tree-kill mechanism.
pub(crate) fn pre_spawn(command: &mut tokio::process::Command) {
    command.kill_on_drop(true);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
}

/// Build a tree-teardown guard from the spawned child (called after `spawn()`). The guard's Drop
/// kills the whole process tree. `None` on platforms without a mechanism or if setup failed
/// (the `kill_on_drop(true)` from [`pre_spawn`] still kills the direct child).
pub(crate) fn post_spawn(
    child: &tokio::process::Child,
) -> Option<Box<dyn FnOnce() + Send + 'static>> {
    #[cfg(target_os = "windows")]
    {
        windows_job::build_guard(child)
    }
    #[cfg(unix)]
    {
        // process_group(0) made the child a group leader, so PGID == PID. Negative pid signals the
        // whole group.
        let pgid = child.id()?;
        Some(Box::new(move || {
            // SAFETY: libc::kill with a negative pid signals the process group; SIGKILL is safe.
            unsafe {
                libc::kill(-(pgid as i32), libc::SIGKILL);
            }
        }))
    }
    #[cfg(not(any(target_os = "windows", unix)))]
    {
        let _ = child;
        None
    }
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::windows_job::JobHandle;

    #[test]
    fn windows_job_creates_and_configures_kill_on_close() {
        // Creating a Job Object + setting KILL_ON_JOB_CLOSE does NOT require elevation; verify the
        // machinery works on this host. The handle drops at end of scope (CloseHandle) without a
        // process assigned, which is a no-op on the kernel side.
        let job = JobHandle::new().expect("CreateJobObjectW succeeds (non-elevated)");
        job.configure_kill_on_close().expect("SetInformationJobObject(KILL_ON_JOB_CLOSE)");
        // Drop runs here — must not panic.
    }
}

#[cfg(test)]
mod behavior {
    use super::{post_spawn, pre_spawn};

    // Behavioral coverage for the orphan fix: spawn a long-lived child, build the containment guard,
    // drop ONLY the guard (not the child), and assert the child is torn down. This is the only
    // runnable end-to-end check of tree-kill (no elevation needed); it runs on every host.
    #[tokio::test]
    async fn containment_guard_kills_child_tree_on_drop() {
        let mut command = if cfg!(windows) {
            // `cmd /c ping` makes cmd fork ping as a grandchild, so this also exercises tree-kill
            // (the Job/process-group must reach ping, not just cmd).
            let mut c = tokio::process::Command::new("cmd");
            c.args(["/c", "ping", "-n", "30", "127.0.0.1"]);
            c
        } else {
            let mut c = tokio::process::Command::new("sh");
            c.args(["-c", "sleep 30"]);
            c
        };
        pre_spawn(&mut command);
        let mut child = command.spawn().expect("spawn long-lived child");
        let guard = post_spawn(&child);
        assert!(guard.is_some(), "containment guard should be created");

        // Drop ONLY the guard — the Job close / process-group SIGKILL must kill the tree. The child
        // handle is still held, so `kill_on_drop` is NOT what fires here.
        drop(guard);

        // Poll for exit without depending on the tokio `time` feature. If the guard failed to kill,
        // the deadline trips long before the child's natural ~30s exit.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if child.try_wait().expect("try_wait").is_some() {
                break;
            }
            if std::time::Instant::now() > deadline {
                panic!("containment guard drop did not kill the child within 5s");
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }
}
