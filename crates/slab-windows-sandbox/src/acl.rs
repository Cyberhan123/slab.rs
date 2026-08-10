//! Integrity-label ACL application (S2b2). Pure kernel SACL/DACL manipulation — unit-testable
//! WITHOUT elevation (the test process can relabel files it owns).
//!
//! Two complementary mechanisms:
//!
//! 1. [`lower_to_low_integrity`] — apply a Low-integrity mandatory-label SACL ACE (inheritable) on
//!    a directory so a Low-IL child can WRITE there without a write-up. This is the ONLY mechanism
//!    that grants the child write access to its workspace.
//! 2. [`deny_write_low_sid`] — a DACL deny-write ACE for the Low SID on protected/denied paths.
//!    Load-bearing for paths INSIDE the lowered workspace (e.g. `.git/config`): because the
//!    workspace is Low, the Low token alone would let the child write them; the deny-ACE
//!    re-protects them. For paths already at Medium-IL it is harmless belt-and-suspenders (kernel
//!    `NO_WRITE_UP` already blocks the Low-IL writer).
//!
//! Key Win32 facts (windows-sys 0.61):
//! - SACL mandatory labels need [`AddMandatoryAce`] directly — `SetEntriesInAclW`/
//!   `BuildExplicitAccessWithNameW` are DACL-only and do NOT support label ACEs.
//! - ACLs require DWORD alignment; buffers are declared `#[repr(C, align(4))]` (a `Vec<u8>` is only
//!   1-byte aligned and would be misaligned UB).
//! - [`SetNamedSecurityInfoW`] returns `WIN32_ERROR` (u32), NOT a BOOL — check `!= 0`, do NOT route
//!   through [`win32_ctx`].

use core::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::path::Path;

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::{
    BuildTrusteeWithSidW, DENY_ACCESS, EXPLICIT_ACCESS_W, GRANT_ACCESS, GetNamedSecurityInfoW,
    SE_FILE_OBJECT, SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_W,
};
use windows_sys::Win32::Security::{
    ACE_HEADER, ACL, ACL_REVISION, ACL_SIZE_INFORMATION, AclSizeInformation, AddMandatoryAce,
    CreateWellKnownSid, DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetAclInformation,
    InitializeAcl, InitializeSecurityDescriptor, LABEL_SECURITY_INFORMATION, PSID,
    SECURITY_DESCRIPTOR, SUB_CONTAINERS_AND_OBJECTS_INHERIT, SYSTEM_MANDATORY_LABEL_ACE,
    SetSecurityDescriptorSacl, WinLowLabelSid,
};
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_APPEND_DATA, FILE_DELETE_CHILD, FILE_WRITE_ATTRIBUTES, FILE_WRITE_DATA,
    FILE_WRITE_EA,
};
use windows_sys::Win32::System::SystemServices::{
    SECURITY_DESCRIPTOR_REVISION, SYSTEM_MANDATORY_LABEL_ACE_TYPE,
};

use crate::error::WindowsSandboxError;

/// Buffer large enough for any well-known SID.
const SID_BUF_LEN: usize = 256;

/// The deny-write mask applied to protected paths (covers all write avenues + delete).
const DENY_WRITE_MASK: u32 = FILE_WRITE_DATA
    | FILE_APPEND_DATA
    | FILE_WRITE_EA
    | FILE_DELETE_CHILD
    | FILE_WRITE_ATTRIBUTES
    | DELETE;

/// Aligned scratch buffer for a single-ACE ACL (DWORD alignment required by `ACL`).
#[repr(C, align(4))]
struct AclBuf([u8; 256]);

/// 4-byte-aligned scratch for a well-known SID (the `SubAuthority` DWORDs want DWORD alignment).
#[repr(C, align(4))]
struct SidBuf([u8; SID_BUF_LEN]);

/// Build the well-known Low-integrity SID into a stack buffer. IMPORTANT: compute the `PSID` in
/// the CALLER via `buf.0.as_ptr() as PSID` — a pointer captured inside this fn would dangle,
/// because the buffer is MOVED on return (the classic return-of-pointer-into-moved-local).
fn make_low_sid() -> Result<SidBuf, WindowsSandboxError> {
    let mut buf = SidBuf([0u8; SID_BUF_LEN]);
    let mut len = buf.0.len() as u32;
    let sid = buf.0.as_mut_ptr() as PSID;
    let ok = unsafe { CreateWellKnownSid(WinLowLabelSid, std::ptr::null_mut(), sid, &mut len) };
    if ok == 0 {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "CreateWellKnownSid(WinLowLabelSid) failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(buf)
}

/// Lower `path` to Low integrity: apply an inheritable Low mandatory-label SACL ACE so a Low-IL
/// child can write here without a write-up. Creates the directory first if it does not exist.
pub(crate) fn lower_to_low_integrity(path: &Path) -> Result<(), WindowsSandboxError> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| WindowsSandboxError::SetupFailed(format!("create_dir_all: {e}")))?;
    }

    // Compute the SID pointer HERE, in the scope that owns the buffer (a pointer captured in
    // `make_low_sid` would dangle once its buffer is moved on return).
    let sid_buf = make_low_sid()?;
    let low_sid = sid_buf.0.as_ptr() as PSID;

    // Build a SACL with one inheritable Low mandatory-label ACE.
    let mut acl_buf = AclBuf([0u8; 256]);
    let pacl = acl_buf.0.as_mut_ptr() as *mut ACL;
    unsafe {
        if InitializeAcl(pacl, acl_buf.0.len() as u32, ACL_REVISION) == 0 {
            return Err(WindowsSandboxError::WindowsApi(format!(
                "InitializeAcl failed: {}",
                std::io::Error::last_os_error()
            )));
        }
        // SUB_CONTAINERS_AND_OBJECTS_INHERIT is load-bearing: without it, new files/subdirs inside
        // the lowered root default to Medium-IL and the child cannot write its own output.
        if AddMandatoryAce(
            pacl,
            ACL_REVISION,
            SUB_CONTAINERS_AND_OBJECTS_INHERIT,
            windows_sys::Win32::System::SystemServices::SYSTEM_MANDATORY_LABEL_NO_WRITE_UP,
            low_sid,
        ) == 0
        {
            return Err(WindowsSandboxError::WindowsApi(format!(
                "AddMandatoryAce failed: {}",
                std::io::Error::last_os_error()
            )));
        }
    }

    // Wrap the SACL in a self-relative security descriptor.
    let mut sd: SECURITY_DESCRIPTOR = unsafe { zeroed() };
    let psd = &mut sd as *mut SECURITY_DESCRIPTOR as *mut c_void;
    unsafe {
        if InitializeSecurityDescriptor(psd, SECURITY_DESCRIPTOR_REVISION) == 0 {
            return Err(WindowsSandboxError::WindowsApi(format!(
                "InitializeSecurityDescriptor failed: {}",
                std::io::Error::last_os_error()
            )));
        }
        // SACL present = TRUE, not defaulted = FALSE.
        if SetSecurityDescriptorSacl(psd, 1, pacl, 0) == 0 {
            return Err(WindowsSandboxError::WindowsApi(format!(
                "SetSecurityDescriptorSacl failed: {}",
                std::io::Error::last_os_error()
            )));
        }
    }

    set_label_security_info(path, psd as *const ACL)
}

/// Defense-in-depth (and load-bearing for in-workspace protected paths): deny the Low SID write
/// access on `path` via a DACL deny-ACE, MERGED into the existing DACL (the existing owner ACL is
/// preserved — a deny-only DACL would strip the owner's access). Creates the directory first if it
/// does not exist.
pub(crate) fn deny_write_low_sid(path: &Path) -> Result<(), WindowsSandboxError> {
    if !path.exists() {
        // A deny on a not-yet-existing path is best-effort; create it so the label has a target.
        let _ = std::fs::create_dir_all(path);
    }

    let sid_buf = make_low_sid()?;
    let low_sid = sid_buf.0.as_ptr() as PSID;

    // Read the existing DACL (owner-readable; no SE_SECURITY_NAME needed) so the deny ACE merges in
    // without stripping the owner's access.
    let wide = wide_path(path);
    let mut existing_dacl: *mut ACL = std::ptr::null_mut();
    let mut existing_sd: *mut c_void = std::ptr::null_mut();
    let err = unsafe {
        GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut existing_dacl,
            std::ptr::null_mut(),
            &mut existing_sd,
        )
    };
    if err != 0 {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "GetNamedSecurityInfoW(DACL) failed: code {err}"
        )));
    }

    // SID-based trustee + inheritable DENY_ACCESS entry with the write mask.
    let mut trustee: TRUSTEE_W = unsafe { zeroed() };
    unsafe { BuildTrusteeWithSidW(&mut trustee, low_sid) };
    let ea = EXPLICIT_ACCESS_W {
        grfAccessPermissions: DENY_WRITE_MASK,
        grfAccessMode: DENY_ACCESS,
        grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT,
        Trustee: trustee,
    };

    let old_acl = if existing_dacl.is_null() { std::ptr::null() } else { existing_dacl };
    let mut new_dacl: *mut ACL = std::ptr::null_mut();
    let err = unsafe { SetEntriesInAclW(1, &ea, old_acl, &mut new_dacl) };
    unsafe {
        if !existing_sd.is_null() {
            LocalFree(existing_sd as _);
        }
    }
    if err != 0 {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "SetEntriesInAclW failed: code {err}"
        )));
    }

    let err = unsafe {
        SetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            new_dacl,
            std::ptr::null_mut(),
        )
    };
    unsafe { LocalFree(new_dacl as _) };
    if err != 0 {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "SetNamedSecurityInfoW(DACL) failed: code {err}"
        )));
    }
    Ok(())
}

/// Grant the AppContainer package SID write access on `path` (S3). AppContainer processes must be
/// explicitly granted their package SID in the DACL (in addition to passing the Low-IL SACL
/// write-up check) before they can create/edit files. MERGED into the existing DACL so the owner's
/// access is preserved. Additive to [`lower_to_low_integrity`] (SACL) — run both on `writable_roots`.
pub(crate) fn grant_appcontainer_write(
    path: &Path,
    package_sid: PSID,
) -> Result<(), WindowsSandboxError> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| WindowsSandboxError::SetupFailed(format!("create_dir_all: {e}")))?;
    }

    // Read the existing DACL so the grant-ACE merges in without stripping the owner's access.
    let wide = wide_path(path);
    let mut existing_dacl: *mut ACL = std::ptr::null_mut();
    let mut existing_sd: *mut c_void = std::ptr::null_mut();
    let err = unsafe {
        GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut existing_dacl,
            std::ptr::null_mut(),
            &mut existing_sd,
        )
    };
    if err != 0 {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "GetNamedSecurityInfoW(DACL) failed: code {err}"
        )));
    }

    // SID-based trustee + inheritable GRANT_ACCESS entry with the write mask.
    let mut trustee: TRUSTEE_W = unsafe { zeroed() };
    unsafe { BuildTrusteeWithSidW(&mut trustee, package_sid) };
    let ea = EXPLICIT_ACCESS_W {
        grfAccessPermissions: DENY_WRITE_MASK,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT,
        Trustee: trustee,
    };

    let old_acl = if existing_dacl.is_null() { std::ptr::null() } else { existing_dacl };
    let mut new_dacl: *mut ACL = std::ptr::null_mut();
    let err = unsafe { SetEntriesInAclW(1, &ea, old_acl, &mut new_dacl) };
    unsafe {
        if !existing_sd.is_null() {
            LocalFree(existing_sd as _);
        }
    }
    if err != 0 {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "SetEntriesInAclW failed: code {err}"
        )));
    }

    let err = unsafe {
        SetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            new_dacl,
            std::ptr::null_mut(),
        )
    };
    unsafe { LocalFree(new_dacl as _) };
    if err != 0 {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "SetNamedSecurityInfoW(DACL grant) failed: code {err}"
        )));
    }
    Ok(())
}

/// Apply LABEL_SECURITY_INFORMATION with the given SACL onto `path`. Returns the Win32 error code
/// as an error on non-zero (SetNamedSecurityInfoW returns u32, not BOOL).
fn set_label_security_info(path: &Path, sacl: *const ACL) -> Result<(), WindowsSandboxError> {
    let wide = wide_path(path);
    let err = unsafe {
        SetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            LABEL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            sacl as *mut ACL,
        )
    };
    if err != 0 {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "SetNamedSecurityInfoW(LABEL) failed: code {err}"
        )));
    }
    Ok(())
}

/// Whether `path`'s SACL contains a mandatory-label ACE naming the Low integrity SID.
#[allow(dead_code)] // exercised by the cfg(test) read-back tests + the elevated OS test suite
pub(crate) fn has_low_integrity_label(path: &Path) -> Result<bool, WindowsSandboxError> {
    let sid_buf = make_low_sid()?;
    let low_sid = sid_buf.0.as_ptr() as PSID;

    let wide = wide_path(path);
    let mut psacl: *mut ACL = std::ptr::null_mut();
    let mut psd: *mut c_void = std::ptr::null_mut();
    let err = unsafe {
        GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            LABEL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut psacl,
            &mut psd,
        )
    };
    if err != 0 {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "GetNamedSecurityInfoW failed: code {err}"
        )));
    }

    let result = scan_sacl_for_low_label(psacl, low_sid);

    if !psd.is_null() {
        unsafe { LocalFree(psd as _) };
    }
    result
}

/// Walk `sacl` for a SYSTEM_MANDATORY_LABEL_ACE whose SID equals `low_sid`.
#[allow(dead_code)]
fn scan_sacl_for_low_label(sacl: *mut ACL, low_sid: PSID) -> Result<bool, WindowsSandboxError> {
    if sacl.is_null() {
        return Ok(false);
    }
    let mut size_info: ACL_SIZE_INFORMATION = unsafe { zeroed() };
    let ok = unsafe {
        GetAclInformation(
            sacl,
            &mut size_info as *mut ACL_SIZE_INFORMATION as *mut c_void,
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    };
    if ok == 0 {
        return Err(WindowsSandboxError::WindowsApi(format!(
            "GetAclInformation failed: {}",
            std::io::Error::last_os_error()
        )));
    }

    for i in 0..size_info.AceCount {
        let mut ace: *mut c_void = std::ptr::null_mut();
        let ok = unsafe { GetAce(sacl, i, &mut ace) };
        if ok == 0 || ace.is_null() {
            continue;
        }
        let header = unsafe { &*(ace as *const ACE_HEADER) };
        if header.AceType != SYSTEM_MANDATORY_LABEL_ACE_TYPE as u8 {
            continue;
        }
        // The SID begins at the SidStart field of SYSTEM_MANDATORY_LABEL_ACE (offset 8: header 4 +
        // mask 4). Point at it and compare.
        let label = unsafe { &*(ace as *const SYSTEM_MANDATORY_LABEL_ACE) };
        let ace_sid = &label.SidStart as *const u32 as PSID;
        if unsafe { EqualSid(ace_sid, low_sid) != 0 } {
            return Ok(true);
        }
    }
    Ok(false)
}

fn wide_path(p: &Path) -> Vec<u16> {
    p.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether the current process can read a file's SACL (`SE_SECURITY_NAME` is required to read
    /// back the mandatory label via `GetNamedSecurityInfoW`). Setting the label succeeds without
    /// elevation (owner suffices), but the read-back verification only works elevated. The non-
    /// elevated `cargo test` run therefore verifies the APPLY call sequence (the Win32 calls accept
    /// the constructed SACL/DACL); the elevated OS tests verify presence + inheritance.
    fn can_read_sacl(path: &Path) -> bool {
        let wide = wide_path(path);
        let mut psacl: *mut ACL = std::ptr::null_mut();
        let mut psd: *mut c_void = std::ptr::null_mut();
        let err = unsafe {
            GetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                LABEL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut psacl,
                &mut psd,
            )
        };
        let readable = err == 0 && !psacl.is_null();
        if !psd.is_null() {
            unsafe { LocalFree(psd as _) };
        }
        readable
    }

    #[test]
    fn lower_to_low_integrity_applies() {
        // The APPLY call sequence (InitializeAcl + AddMandatoryAce + SD + SetNamedSecurityInfoW)
        // returning Ok proves the constructed SACL is well-formed and accepted by the kernel.
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("workspace");
        std::fs::create_dir_all(&target).unwrap();
        lower_to_low_integrity(&target).expect("apply Low IL label");
    }

    #[test]
    fn lower_to_low_integrity_label_present_when_readable() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("workspace");
        std::fs::create_dir_all(&target).unwrap();
        lower_to_low_integrity(&target).expect("apply");
        if !can_read_sacl(&target) {
            eprintln!("skipping read-back — SACL read needs SE_SECURITY_NAME (elevated only)");
            return;
        }
        assert!(has_low_integrity_label(&target).expect("read back"), "Low IL label present");
    }

    #[test]
    fn inherited_by_subdir_when_readable() {
        // Pins SUB_CONTAINERS_AND_OBJECTS_INHERIT (read-back gated).
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("root");
        lower_to_low_integrity(&target).expect("apply");
        if !can_read_sacl(&target) {
            eprintln!("skipping read-back — SACL read needs SE_SECURITY_NAME (elevated only)");
            return;
        }
        let sub = target.join("nested");
        std::fs::create_dir_all(&sub).unwrap();
        assert!(has_low_integrity_label(&sub).expect("read sub"), "inherited label on child dir");
    }

    #[test]
    fn inherited_by_new_file_when_readable() {
        // Pins the silent-breakage gotcha: a new file under a lowered dir must inherit Low-IL.
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("root");
        lower_to_low_integrity(&target).expect("apply");
        if !can_read_sacl(&target) {
            eprintln!("skipping read-back — SACL read needs SE_SECURITY_NAME (elevated only)");
            return;
        }
        let file = target.join("out.txt");
        std::fs::write(&file, b"data").unwrap();
        assert!(has_low_integrity_label(&file).expect("read file"), "inherited label on new file");
    }

    #[test]
    fn apply_on_nonexistent_path_creates_then_labels() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("auto-created");
        lower_to_low_integrity(&target).expect("auto-create + label");
        assert!(target.is_dir(), "directory was created before labeling");
    }

    #[test]
    fn deny_write_low_sid_applies() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("protected");
        std::fs::create_dir_all(&target).unwrap();
        deny_write_low_sid(&target).expect("apply deny-write-Low ACE");
    }

    #[test]
    fn grant_appcontainer_write_applies() {
        // The APPLY call sequence (GetNamedSecurityInfoW + BuildTrusteeWithSidW + SetEntriesInAclW
        // + SetNamedSecurityInfoW) returning Ok proves the constructed grant-ACE merges cleanly.
        let sid = crate::appcontainer::PackageSid::from_fingerprint("test-grant").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("workspace");
        std::fs::create_dir_all(&target).unwrap();
        grant_appcontainer_write(&target, sid.as_psid()).expect("grant AppContainer package SID");
    }
}
