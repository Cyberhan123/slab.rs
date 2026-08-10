//! AppContainer package-SID derivation (S3). Deterministically derives the AppContainer package
//! SID from the sandbox-helper key fingerprint so the SID is stable across daemon restarts. The
//! SAME SID is what the OS default WFP rule, our explicit [`crate::wfp`] `FWPM_CONDITION_ALE_PACKAGE_ID`
//! filter, the [`crate::acl::grant_appcontainer_write`] DACL grant, and the `SECURITY_CAPABILITIES`
//! spawn attribute all key on — so all four mechanisms target one identical identity.
//!
//! Pure `userenv` SID math: it needs NO elevation, so it is unit-testable from a plain `cargo test`.

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Isolation::DeriveAppContainerSidFromAppContainerName;
use windows_sys::Win32::Security::{CopySid, GetLengthSid, PSID};

use crate::error::WindowsSandboxError;

/// 4-byte-aligned scratch for one SID (the `SubAuthority` DWORDs want DWORD alignment; a plain
/// `Vec<u8>` would be misaligned UB). Mirrors the `acl::SidBuf` pattern.
#[repr(C, align(4))]
struct SidBuf([u8; 256]);

/// An owned, aligned AppContainer package SID derived from the key fingerprint. Plain bytes — auto
/// `Send`+`Sync` — held in the daemon's shared `WfpState` and read out as a `PSID` for the lifetime
/// of the daemon.
pub(crate) struct PackageSid {
    buf: SidBuf,
}

impl PackageSid {
    /// Derive the canonical `S-1-15-2-…` package SID for `slab-sandbox-{fingerprint}`. Deterministic:
    /// the same fingerprint always yields byte-identical SID bytes (`userenv` hashes the name).
    pub(crate) fn from_fingerprint(fingerprint: &str) -> Result<Self, WindowsSandboxError> {
        let name: Vec<u16> = format!("slab-sandbox-{fingerprint}")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut raw: PSID = std::ptr::null_mut();
        // SAFETY: `name` is NUL-terminated UTF-16; `raw` is a valid out-pointer.
        let hr = unsafe { DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut raw) };
        if hr != 0 || raw.is_null() {
            return Err(WindowsSandboxError::WindowsApi(format!(
                "DeriveAppContainerSidFromAppContainerName failed: HRESULT 0x{:08x}",
                hr as u32
            )));
        }

        // Copy the userenv-allocated SID into our aligned owned buffer, then free the allocation.
        let mut buf = SidBuf([0u8; 256]);
        let ok = unsafe {
            let len = GetLengthSid(raw);
            if len as usize > buf.0.len() {
                LocalFree(raw);
                return Err(WindowsSandboxError::WindowsApi(format!(
                    "AppContainer SID too large ({len} bytes)"
                )));
            }
            let dst = buf.0.as_mut_ptr() as PSID;
            let copied = CopySid(buf.0.len() as u32, dst, raw);
            LocalFree(raw);
            copied
        };
        if ok == 0 {
            return Err(WindowsSandboxError::WindowsApi(format!(
                "CopySid failed: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(Self { buf })
    }

    /// The raw package-SID pointer. Valid for as long as `self` lives.
    pub(crate) fn as_psid(&self) -> PSID {
        self.buf.0.as_ptr() as PSID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_sid_is_deterministic() {
        let a = PackageSid::from_fingerprint("abcdef0123456789").unwrap();
        let b = PackageSid::from_fingerprint("abcdef0123456789").unwrap();
        let la = unsafe { GetLengthSid(a.as_psid()) } as usize;
        let lb = unsafe { GetLengthSid(b.as_psid()) } as usize;
        assert_eq!(la, lb);
        assert_eq!(&a.buf.0[..la], &b.buf.0[..lb], "same fingerprint ⇒ identical SID");
    }

    #[test]
    fn package_sid_differs_for_different_fingerprint() {
        let a = PackageSid::from_fingerprint("abcdef0123456789").unwrap();
        let b = PackageSid::from_fingerprint("9876543210fedcba").unwrap();
        let la = unsafe { GetLengthSid(a.as_psid()) } as usize;
        let lb = unsafe { GetLengthSid(b.as_psid()) } as usize;
        assert_eq!(la, lb, "same authority ⇒ same length");
        assert_ne!(&a.buf.0[..la], &b.buf.0[..lb]);
    }

    #[test]
    fn package_sid_is_app_package_authority() {
        // S-1-15-2-… layout: u8 Revision(=1), u8 SubAuthorityCount, [6]u8 IdentifierAuthority,
        // then SubAuthorityCount * u32. IdentifierAuthority 15 = SECURITY_APP_PACKAGE_AUTHORITY.
        let s = PackageSid::from_fingerprint("abcdef0123456789").unwrap();
        let bytes = &s.buf.0;
        assert_eq!(bytes[0], 1, "Revision == 1");
        assert_eq!(bytes[2..8], [0, 0, 0, 0, 0, 15], "IdentifierAuthority == 15 (app package)");
        assert!(bytes[1] > 0, "at least one sub-authority");
    }
}
