//! Helper-side logic, invoked by the `slab-sandbox-helper` binary. Lives in the library so it is
//! unit-testable without the binary. The S2a `--payload` path performs a one-shot Provision: it
//! loads the key, verifies the payload, writes an honest (still Job-only) marker + result, and
//! exits 0. S2b's Provision applies real ACLs and returns `ElevatedAclToken`/`OsEnforced`.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::capability::{FsIsolationStrength, WindowsSetupKind};
use crate::creds;
use crate::error::WindowsSandboxError;
use crate::ipc::{self, ElevationPayload, HelperResult, PayloadOp, SetupMarker};
use crate::marker;

/// Exit codes used by the helper (the orchestrator treats non-zero as fail-closed).
pub mod exit_code {
    /// Success.
    pub const OK: i32 = 0;
    /// Internal error (key/IPC failure).
    pub const ERROR: i32 = 1;
    /// Schema/key mismatch — refused, no result written.
    pub const SCHEMA_KEY_MISMATCH: i32 = 2;
    /// Unsupported op for this build (Spawn/Kill/DaemonServe in S2a).
    pub const UNSUPPORTED_OP: i32 = 3;
}

/// Process a one-shot `--payload <path>` invocation. Returns the exit code.
pub fn run_payload(payload_path: &Path, key_path: &Path) -> i32 {
    match run_payload_inner(payload_path, key_path) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("slab-sandbox-helper: {e}");
            exit_code::ERROR
        }
    }
}

fn run_payload_inner(payload_path: &Path, key_path: &Path) -> Result<i32, WindowsSandboxError> {
    let key = creds::load_or_create_key(key_path)?;
    let payload = ipc::read_signed_payload(payload_path, &key)?;
    let fingerprint = creds::key_fingerprint(&key);
    // Fail-closed on schema/key mismatch: write no result, exit non-zero.
    if payload.schema != crate::SCHEMA_VERSION || payload.key_fingerprint != fingerprint {
        return Ok(exit_code::SCHEMA_KEY_MISMATCH);
    }

    let result = match payload.op {
        PayloadOp::Provision => provision(&payload)?,
        PayloadOp::Ping => ping(&payload),
        // Spawn / Kill / DaemonServe land in S2b.
        PayloadOp::Spawn | PayloadOp::Kill | PayloadOp::DaemonServe => {
            return Ok(exit_code::UNSUPPORTED_OP);
        }
    };

    ipc::write_signed_result(&payload.result_path, &result, &key)?;
    Ok(exit_code::OK)
}

fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// S2a Provision: prove the elevation + IPC round-trip works. Honest about still being Job-only
/// (no ACLs/token applied yet); S2b flips this to `ElevatedAclToken`/`OsEnforced`.
fn provision(payload: &ElevationPayload) -> Result<HelperResult, WindowsSandboxError> {
    let marker = SetupMarker {
        schema: crate::SCHEMA_VERSION,
        created_at_unix: now_unix(),
        setup_kind: WindowsSetupKind::JobObject,
        filesystem_isolation: FsIsolationStrength::Lexical,
        key_fingerprint: payload.key_fingerprint.clone(),
        denied_paths: Vec::new(),
        writable_roots_lowered: Vec::new(),
        workspace_root: None,
        daemon_pipe: None,
        daemon_pid: None,
    };
    if let Some(marker_path) = &payload.marker_path {
        marker::write_marker(marker_path, &marker)?;
    }
    Ok(HelperResult {
        schema: crate::SCHEMA_VERSION,
        nonce: payload.nonce.clone(),
        ok: true,
        setup_kind: WindowsSetupKind::JobObject,
        filesystem_isolation: FsIsolationStrength::Lexical,
        error: None,
        marker: Some(marker),
        spawn_pid: None,
    })
}

fn ping(payload: &ElevationPayload) -> HelperResult {
    HelperResult {
        schema: crate::SCHEMA_VERSION,
        nonce: payload.nonce.clone(),
        ok: true,
        setup_kind: WindowsSetupKind::JobObject,
        filesystem_isolation: FsIsolationStrength::Lexical,
        error: None,
        marker: None,
        spawn_pid: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::SignedPayload;
    use std::path::{Path, PathBuf};

    fn write_payload(dir: &Path, key: &[u8], payload: &ElevationPayload) -> PathBuf {
        let path = dir.join("payload.json");
        let body = serde_json::to_vec(payload).unwrap();
        let tag = hex::encode(crate::mac::sign(key, &body));
        let framed = SignedPayload { payload: payload.clone(), tag };
        std::fs::write(&path, serde_json::to_vec(&framed).unwrap()).unwrap();
        path
    }

    #[test]
    fn provision_roundtrip_writes_result_and_marker() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("helper.key");
        let key = creds::load_or_create_key(&key_path).unwrap();
        let fp = creds::key_fingerprint(&key);

        let result_path = dir.path().join("result.json");
        let marker_path = dir.path().join("marker.json");
        let payload = ElevationPayload::new_provision(
            "nonce-1",
            0,
            fp.clone(),
            result_path.clone(),
            marker_path.clone(),
        );
        let payload_path = write_payload(dir.path(), &key, &payload);

        let code = run_payload(&payload_path, &key_path);
        assert_eq!(code, exit_code::OK);
        assert!(result_path.exists());
        assert!(marker_path.exists());

        let result = ipc::read_signed_result(&result_path, &key).unwrap();
        assert!(result.ok);
        assert_eq!(result.nonce, "nonce-1");
        assert_eq!(result.setup_kind, WindowsSetupKind::JobObject);

        let marker = marker::read_marker(&marker_path).unwrap().unwrap();
        assert_eq!(marker.key_fingerprint, fp);
    }

    #[test]
    fn key_mismatch_exits_nonzero_without_result() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("helper.key");
        let key = creds::load_or_create_key(&key_path).unwrap();

        // Sign the payload with a DIFFERENT key than the one on disk.
        let other = b"other-key-32-bytes-_______________"[..32].to_vec();
        let result_path = dir.path().join("result.json");
        let payload = ElevationPayload::new_provision(
            "n",
            0,
            creds::key_fingerprint(&other),
            result_path.clone(),
            dir.path().join("marker.json"),
        );
        let payload_path = write_payload(dir.path(), &other, &payload);

        let code = run_payload(&payload_path, &key_path);
        // read_signed_payload fails HMAC ⇒ run_payload returns ERROR (1).
        assert_ne!(code, exit_code::OK);
        assert!(!result_path.exists());
        let _ = key; // silence unused on non-windows
    }
}
