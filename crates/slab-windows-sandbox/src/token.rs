//! Low-integrity restricted primary token builder (S2b2).
//!
//! Builds a primary token the daemon hands to `CreateProcessAsUserW` for each sandboxed child. The
//! token is a filtered copy of the daemon's own (elevated) token:
//!
//! 1. `OpenProcessToken(GetCurrentProcess, TOKEN_DUPLICATE|TOKEN_QUERY)` — note this lives in
//!    `Win32::System::Threading`, not `Win32::Security` (windows-sys 0.61).
//! 2. `DuplicateTokenEx(TokenPrimary)` — mask grants what the next calls + `CreateProcessAsUserW`
//!    need (`TOKEN_ASSIGN_PRIMARY | TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ADJUST_DEFAULT`).
//! 3. `CreateRestrictedToken(DISABLE_MAX_PRIVILEGE | LUA_TOKEN)` — strips every privilege and
//!    produces a filtered UAC-style token. (Note: the constant is `DISABLE_MAX_PRIVILEGE`,
//!    singular, in windows-sys 0.61.)
//! 4. `CreateWellKnownSid(WinLowLabelSid)` + `SetTokenInformation(TokenIntegrityLevel)` — sets the
//!    mandatory integrity to Low so the kernel's `NO_WRITE_UP` blocks writes to any Medium-IL
//!    object (the user's home, `C:\`, ...). This is the core filesystem containment.
//!
//! Every step is fail-closed via [`win32_ctx`].

use core::ffi::c_void;
use std::mem::size_of;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{
    CreateRestrictedToken, CreateWellKnownSid, DISABLE_MAX_PRIVILEGE, DuplicateTokenEx,
    GetTokenInformation, LUA_TOKEN, SECURITY_IMPERSONATION_LEVEL, SID_AND_ATTRIBUTES,
    SecurityImpersonation, SetTokenInformation, TOKEN_ADJUST_DEFAULT, TOKEN_ASSIGN_PRIMARY,
    TOKEN_DUPLICATE, TOKEN_ELEVATION, TOKEN_INFORMATION_CLASS, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
    TOKEN_TYPE, TokenElevation, TokenIntegrityLevel, TokenPrimary, WinLowLabelSid,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use crate::error::{WindowsSandboxError, win32_ctx};

/// Buffer large enough for any well-known SID (`SECURITY_MAX_SID_SIZE` = 68).
const SID_BUF_LEN: usize = 256;

/// RAII handle to a Low-IL restricted primary token. `Drop` closes the handle.
pub(crate) struct LowIntegrityToken(HANDLE);

// The handle is owned and not shared across threads once built (each spawn builds its own).
unsafe impl Send for LowIntegrityToken {}

impl LowIntegrityToken {
    /// Build a Low-IL restricted primary token from the daemon's own (elevated) token.
    pub(crate) fn new() -> Result<Self, WindowsSandboxError> {
        // 1. Open the current process token.
        let mut source: HANDLE = core::ptr::null_mut();
        unsafe {
            win32_ctx(
                OpenProcessToken(GetCurrentProcess(), TOKEN_DUPLICATE | TOKEN_QUERY, &mut source),
                "OpenProcessToken",
            )?;
        }

        // 2. Duplicate as a primary token with the rights the following calls need.
        let mut dup: HANDLE = core::ptr::null_mut();
        let dup_mask = TOKEN_ASSIGN_PRIMARY | TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ADJUST_DEFAULT;
        unsafe {
            win32_ctx(
                DuplicateTokenEx(
                    source,
                    dup_mask,
                    core::ptr::null(),
                    SecurityImpersonation as SECURITY_IMPERSONATION_LEVEL,
                    TokenPrimary as TOKEN_TYPE,
                    &mut dup,
                ),
                "DuplicateTokenEx",
            )?;
            CloseHandle(source);
        }

        // 3. Strip privileges + filter to a UAC-style token (no SIDs/privileges explicitly disabled
        //    — the flags do the full strip). Count params are 0 and pointers null.
        let mut restricted: HANDLE = core::ptr::null_mut();
        let create_ok = unsafe {
            CreateRestrictedToken(
                dup,
                DISABLE_MAX_PRIVILEGE | LUA_TOKEN,
                0,
                core::ptr::null(),
                0,
                core::ptr::null(),
                0,
                core::ptr::null(),
                &mut restricted,
            )
        };
        unsafe {
            win32_ctx(create_ok, "CreateRestrictedToken")?;
            CloseHandle(dup);
        }

        // 4. Lower the integrity level to Low: build the Low SID, then set TokenIntegrityLevel.
        let mut sid_buf = [0u8; SID_BUF_LEN];
        let mut sid_len = sid_buf.len() as u32;
        let low_sid = sid_buf.as_mut_ptr() as *mut c_void;
        unsafe {
            win32_ctx(
                CreateWellKnownSid(WinLowLabelSid, core::ptr::null_mut(), low_sid, &mut sid_len),
                "CreateWellKnownSid(WinLowLabelSid)",
            )?;
        }

        // The TOKEN_MANDATORY_LABEL holds a POINTER to the SID (the SID itself stays in `sid_buf`,
        // valid for the duration of the SetTokenInformation call). Use the real type so the buffer
        // is correctly aligned (a Vec<u8> would only be 1-byte aligned).
        let mut label: TOKEN_MANDATORY_LABEL = unsafe { core::mem::zeroed() };
        label.Label = SID_AND_ATTRIBUTES {
            Sid: low_sid,
            Attributes: 0, // SE_GROUP_INTEGRITY — presence of the Low SID is what matters.
        };
        unsafe {
            win32_ctx(
                SetTokenInformation(
                    restricted,
                    TokenIntegrityLevel as TOKEN_INFORMATION_CLASS,
                    &label as *const TOKEN_MANDATORY_LABEL as *mut c_void,
                    size_of::<TOKEN_MANDATORY_LABEL>() as u32,
                ),
                "SetTokenInformation(TokenIntegrityLevel)",
            )?;
        }

        Ok(Self(restricted))
    }

    /// Raw handle for `CreateProcessAsUserW`. The token remains owned by this guard.
    pub(crate) fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for LowIntegrityToken {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

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
