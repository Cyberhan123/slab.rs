//! DPAPI-sealed HMAC key. The orchestrator (non-elevated) and the helper (elevated) both call
//! `load_or_create_key`: same-user DPAPI unprotects across integrity levels, so both obtain the
//! identical key. The key file lives at `<app_home>/sandbox-helper.key` (path passed in by the
//! caller so this stays unit-testable with a temp dir).
//!
//! Threat model: the HMAC proves *integrity* (the payload came from a key-holder), not
//! caller-authentication. Same-user compromise is game over regardless; the SACL integrity labels
//! (the real filesystem boundary, applied in S2b) are not gated by this key.

use std::path::Path;

use ring::rand::SecureRandom;
use windows_sys::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};

use crate::error::WindowsSandboxError;

/// DPAPI entropy — raw bytes, identical on both sides.
const DPAPI_ENTROPY: &[u8] = b"cn.cyberhan.slab";

/// HMAC-SHA256 key length.
const KEY_LEN: usize = 32;

/// Load the key from `key_path`, or generate + seal a new one if absent. Fail-closed if the file
/// exists but cannot be unsealed (corrupt, or a different OS user) — never silently regenerate,
/// since regenerating would let an attacker who deleted the file forge payloads.
pub fn load_or_create_key(key_path: &Path) -> Result<Vec<u8>, WindowsSandboxError> {
    if key_path.exists() {
        let blob =
            std::fs::read(key_path).map_err(|e| WindowsSandboxError::KeyIo(e.to_string()))?;
        // The file is DPAPI user-scope sealed; only the same user (at any integrity level) can
        // unprotect it. Unseal failure ⇒ corrupt or different user ⇒ fail-closed.
        unseal(&blob).map_err(|_| WindowsSandboxError::KeyUnsealFailed)
    } else {
        let rng = ring::rand::SystemRandom::new();
        let mut key = vec![0u8; KEY_LEN];
        rng.fill(&mut key)
            .map_err(|_| WindowsSandboxError::SetupFailed("rng key generation failed".into()))?;
        let blob = seal(&key)?;
        if let Some(parent) = key_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(key_path, &blob).map_err(|e| WindowsSandboxError::KeyIo(e.to_string()))?;
        Ok(key)
    }
}

/// SHA-256 fingerprint of the key (first 16 bytes, hex) — stored in the marker to detect
/// key rotation / a different key across provisions.
pub fn key_fingerprint(key: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, key);
    hex::encode(&digest.as_ref()[..16])
}

fn seal(key: &[u8]) -> Result<Vec<u8>, WindowsSandboxError> {
    // SAFETY: input/entropy blobs point to local byte buffers that outlive the synchronous call;
    // out_blob is zeroed then filled by DPAPI; the returned pbData is freed via LocalFree below.
    unsafe {
        let in_blob = blob_from_bytes(key);
        let entropy_blob = blob_from_bytes(DPAPI_ENTROPY);
        let mut out_blob: CRYPT_INTEGER_BLOB = std::mem::zeroed();
        let ok = CryptProtectData(
            &in_blob,
            std::ptr::null(),
            &entropy_blob,
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        );
        if ok == 0 {
            return Err(WindowsSandboxError::WindowsApi(format!(
                "CryptProtectData failed: {}",
                std::io::Error::last_os_error()
            )));
        }
        let out = slice_from_blob(&out_blob).to_vec();
        free_blob(&out_blob);
        Ok(out)
    }
}

fn unseal(blob: &[u8]) -> Result<Vec<u8>, WindowsSandboxError> {
    // SAFETY: same reasoning as `seal`.
    unsafe {
        let in_blob = blob_from_bytes(blob);
        let entropy_blob = blob_from_bytes(DPAPI_ENTROPY);
        let mut out_blob: CRYPT_INTEGER_BLOB = std::mem::zeroed();
        let ok = CryptUnprotectData(
            &in_blob,
            std::ptr::null_mut(),
            &entropy_blob,
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        );
        if ok == 0 {
            return Err(WindowsSandboxError::WindowsApi(format!(
                "CryptUnprotectData failed: {}",
                std::io::Error::last_os_error()
            )));
        }
        let out = slice_from_blob(&out_blob).to_vec();
        free_blob(&out_blob);
        Ok(out)
    }
}

/// Build a `DATA_BLOB` (`CRYPT_INTEGER_BLOB`) referencing the given bytes. The bytes must outlive
/// any FFI call made with the returned blob.
fn blob_from_bytes(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_ptr() as *mut u8 }
}

/// SAFETY: caller ensures the blob points to a live DPAPI-allocated buffer of `cbData` bytes.
unsafe fn slice_from_blob(blob: &CRYPT_INTEGER_BLOB) -> &[u8] {
    if blob.cbData == 0 || blob.pbData.is_null() {
        &[]
    } else {
        // SAFETY: caller guarantees pbData points to cbData valid bytes.
        unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize) }
    }
}

/// SAFETY: frees a DPAPI-allocated buffer. DPAPI allocates with `LocalAlloc`; free with
/// `LocalFree` (which lives in `Win32::Foundation` in windows-sys 0.61).
unsafe fn free_blob(blob: &CRYPT_INTEGER_BLOB) {
    if !blob.pbData.is_null() {
        // SAFETY: pbData was allocated by DPAPI via LocalAlloc.
        unsafe {
            windows_sys::Win32::Foundation::LocalFree(blob.pbData as _);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_unseal_roundtrip() {
        // DPAPI user-scope: same test process can unprotect what it protected.
        let key = b"roundtrip-test-key-32-bytes-______"[..32].to_vec();
        let sealed = seal(&key).expect("seal");
        let opened = unseal(&sealed).expect("unseal");
        assert_eq!(opened, key);
    }

    #[test]
    fn fingerprint_is_stable() {
        let key = b"fingerprint-test-key-32-bytes-____"[..32].to_vec();
        assert_eq!(key_fingerprint(&key), key_fingerprint(&key));
    }

    #[test]
    fn load_or_create_then_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sandbox-helper.key");
        let created = load_or_create_key(&path).expect("create");
        assert!(path.exists());
        // Second load must return the SAME key (unseal the existing blob), not regenerate.
        let reloaded = load_or_create_key(&path).expect("reload");
        assert_eq!(created, reloaded);
    }
}
