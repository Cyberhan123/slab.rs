//! Owner-PID watchdog for the elevated daemon. The daemon must never outlive the process that
//! requested it (slab-server): an idle elevated process holding a WFP dynamic session is a
//! liability, so the daemon watches its owner's process handle and signals shutdown the moment
//! that handle is signaled (owner exited — clean shutdown, crash, or taskkill).
//!
//! PID-reuse safety: the owner handle is opened ONCE at daemon start. A Windows process handle
//! stays bound to the original process object even after the pid is recycled, so the watchdog can
//! never end up watching an unrelated process.
//!
//! Fail-closed: if the owner handle cannot be opened (owner already dead, bogus pid), startup
//! fails — there is no owner to serve. From an elevated process, `OpenProcess(PROCESS_SYNCHRONIZE)`
//! on a same-user process essentially always succeeds, so failure means the owner is gone.

use tokio::sync::watch;

use crate::error::WindowsSandboxError;

/// Guard for the watchdog thread. Dropping it stops the thread (bounded: the stop event releases
/// the wait) so an aborted daemon future (unit tests) never leaks it.
pub(crate) struct OwnerWatchdog {
    /// Shutdown signal; flips to `true` when the owner process handle is signaled.
    exited: watch::Receiver<bool>,
    /// Auto-reset event that releases the wait thread on drop.
    stop_event: windows_sys::Win32::Foundation::HANDLE,
    /// `Option` so `Drop` can `join()` (which consumes the handle).
    thread: Option<std::thread::JoinHandle<()>>,
}

// SAFETY: the stop-event handle is owned solely by this guard (the watchdog thread holds only a
// usize copy for the wait), so moving the guard across threads cannot race its close.
unsafe impl Send for OwnerWatchdog {}

impl OwnerWatchdog {
    /// Clone of the shutdown signal for the accept loop's `select!`.
    pub(crate) fn subscribe(&self) -> watch::Receiver<bool> {
        self.exited.clone()
    }
}

impl Drop for OwnerWatchdog {
    fn drop(&mut self) {
        use windows_sys::Win32::System::Threading::SetEvent;
        // SAFETY: the stop event is owned by this guard and stays valid until closed below. Harmless
        // noise if the thread already exited on owner death: an auto-reset event simply stays
        // signaled until consumed (or closed).
        unsafe {
            SetEvent(self.stop_event);
        }
        // Bounded: the thread always returns from the two-handle wait once either handle is
        // signaled. Join before closing so the thread never touches a closed handle.
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // SAFETY: our handle; the thread only closes the OWNER handle, never this one.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.stop_event);
        }
    }
}

/// Start watching `owner_pid`. The returned guard's [`OwnerWatchdog::subscribe`] receiver resolves
/// once the owner process terminates.
pub(crate) fn start(owner_pid: u32) -> Result<OwnerWatchdog, WindowsSandboxError> {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_FAILED, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        CreateEventW, INFINITE, OpenProcess, PROCESS_SYNCHRONIZE, WaitForMultipleObjects,
    };

    // SAFETY: read-only open of an existing process by pid; the returned handle is owned by us.
    let owner = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, owner_pid) };
    if owner.is_null() {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "OpenProcess(owner pid {owner_pid}, PROCESS_SYNCHRONIZE) failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    // SAFETY: unnamed auto-reset event, initially nonsignaled; the handle is owned by the guard.
    let stop_event = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
    if stop_event.is_null() {
        // SAFETY: undo the open above; nothing else references the owner handle yet.
        unsafe { CloseHandle(owner) };
        return Err(WindowsSandboxError::WindowsApi(format!(
            "CreateEventW(owner watchdog, pid {owner_pid}) failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    let (exited_tx, exited) = watch::channel(false);
    // Two-handle wait instead of a plain WaitForSingleObject(owner): the stop event lets Drop
    // release the thread (aborted daemon futures must not leak it) while owner-death detection
    // stays immediate.
    //
    // HANDLE is an opaque pointer-sized kernel value, not a Rust pointer we dereference — but
    // windows-sys types it as `*mut c_void`, which is not `Send`. Pass the two handles into the
    // thread as `usize` copies (lossless on Windows) so the originals stay closable here if the
    // spawn itself fails.
    let owner_raw = owner as usize;
    let stop_raw = stop_event as usize;
    let thread =
        std::thread::Builder::new().name("slab-owner-watchdog".to_string()).spawn(move || {
            let owner = owner_raw as windows_sys::Win32::Foundation::HANDLE;
            let stop_event = stop_raw as windows_sys::Win32::Foundation::HANDLE;
            let handles = [owner, stop_event];
            // SAFETY: both handles are valid and distinct; bWaitAll = FALSE; INFINITE timeout.
            let waited = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE) };
            // Index 0 is the owner. WAIT_FAILED is indeterminate — fail closed as owner-gone.
            let owner_gone = waited == WAIT_OBJECT_0 || waited == WAIT_FAILED;
            if owner_gone {
                let _ = exited_tx.send(true);
            }
            // SAFETY: our copy of the owner handle; nobody references it after this point.
            unsafe { CloseHandle(owner) };
        });
    let thread = match thread {
        Ok(thread) => thread,
        Err(e) => {
            // SAFETY: undo both opens; the failed closure captured only the usize copies above,
            // so the original handles are still ours to close.
            unsafe {
                CloseHandle(owner);
                CloseHandle(stop_event);
            }
            return Err(WindowsSandboxError::WindowsApi(format!(
                "spawn owner-watchdog thread (pid {owner_pid}) failed: {e}"
            )));
        }
    };
    Ok(OwnerWatchdog { exited, stop_event, thread: Some(thread) })
}

/// Resolve once the owner process has terminated. `None` (no watchdog — in-process tests) never
/// resolves. A dropped sender (watchdog gone) resolves immediately: fail closed.
pub(crate) async fn owner_signal(rx: &mut Option<watch::Receiver<bool>>) {
    match rx {
        Some(rx) => loop {
            if *rx.borrow() {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        },
        None => std::future::pending::<()>().await,
    }
}
