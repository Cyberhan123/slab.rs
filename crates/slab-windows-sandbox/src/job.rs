//! Windows Job Object handle, lifted from `slab_sandboxing::platform::windows`. Used for
//! process-tree cleanup (`KILL_ON_JOB_CLOSE`): dropping the handle tears down the whole tree,
//! which is what releases inherited stdout/stderr pipes a backgrounded grandchild may hold.

use crate::error::{WindowsSandboxError, win32_ctx};

/// RAII handle to a Windows Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
pub(crate) struct JobHandle(windows_sys::Win32::Foundation::HANDLE);

// SAFETY: the handle is owned solely by this wrapper and never shared across threads while live.
unsafe impl Send for JobHandle {}

impl JobHandle {
    /// Create a new Job Object.
    pub(crate) fn new() -> Result<Self, WindowsSandboxError> {
        use windows_sys::Win32::System::JobObjects::CreateJobObjectW;
        // SAFETY: null name/attrs ⇒ anonymous job; returned handle is owned by `Self`.
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(WindowsSandboxError::SetupFailed(format!(
                "CreateJobObjectW failed: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(Self(handle))
    }

    /// Configure the job so closing its handle kills every assigned process.
    pub(crate) fn configure_kill_on_close(&self) -> Result<(), WindowsSandboxError> {
        use windows_sys::Win32::System::JobObjects::{
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectExtendedLimitInformation, SetInformationJobObject,
        };
        // SAFETY: zeroed struct is valid; we only set LimitFlags.
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: handle is valid; info pointer + size match the JobObjectExtendedLimitInformation class.
        let ok = unsafe {
            SetInformationJobObject(
                self.0,
                JobObjectExtendedLimitInformation,
                &mut info as *mut _ as *mut _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        win32_ctx(ok, "SetInformationJobObject")
    }

    /// Assign a process (by its OS handle) to this job.
    pub(crate) fn assign_process(
        &self,
        process: windows_sys::Win32::Foundation::HANDLE,
    ) -> Result<(), WindowsSandboxError> {
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        // SAFETY: handle + process handle are valid; assignment is the documented usage.
        let ok = unsafe { AssignProcessToJobObject(self.0, process) };
        if ok == 0 {
            tracing::warn!(error = %std::io::Error::last_os_error(), "failed to assign process to Windows Job Object");
        }
        win32_ctx(ok, "AssignProcessToJobObject")
    }
}

impl Drop for JobHandle {
    fn drop(&mut self) {
        // SAFETY: dropping a Job handle whose limit flags include KILL_ON_JOB_CLOSE is the
        // documented way to tear down the whole process tree.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
