//! Process-token helper for the elevated Windows sandbox.

use core::ffi::c_void;
use std::mem::size_of;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_INFORMATION_CLASS, TOKEN_QUERY, TokenElevation,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Whether the current process is already elevated (skips the UAC `runas` prompt when starting the
/// daemon). Used by the orchestrator side, not the daemon.
pub(crate) fn is_process_elevated() -> bool {
    let mut source: HANDLE = core::ptr::null_mut();
    let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut source) };
    if opened == 0 {
        return false;
    }
    let mut elevation: TOKEN_ELEVATION = unsafe { core::mem::zeroed() };
    let mut ret_len = 0u32;
    let ok = unsafe {
        GetTokenInformation(
            source,
            TokenElevation as TOKEN_INFORMATION_CLASS,
            &mut elevation as *mut TOKEN_ELEVATION as *mut c_void,
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        )
    };
    unsafe { CloseHandle(source) };
    ok != 0 && elevation.TokenIsElevated != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_process_elevated_does_not_panic() {
        // The non-elevated test runner should report false. (Gated OS tests run elevated.)
        let _ = is_process_elevated();
    }
}
