//! Wire types + HMAC-signed file framing for the orchestrator ⇄ helper IPC.
//!
//! The orchestrator (non-elevated, in slab-server) writes a signed payload file, invokes the
//! helper elevated (`ShellExecuteExW("runas")`), and reads back a signed result file. `runas`
//! can't easily capture stdout, so the channel is file-based. Both sides serialize the SAME
//! typed structs (defined here), so `serde_json::to_vec` is deterministic without key-sorting.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::capability::{FsIsolationStrength, WindowsSetupKind};
use crate::error::WindowsSandboxError;
use crate::mac;

/// Operation requested by an elevation round-trip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadOp {
    /// One-shot provisioning (S2a smoke; in S2b applies ACLs + writes marker).
    Provision,
    /// Start the long-lived daemon on a named pipe (S2b).
    DaemonServe,
    /// Spawn one child in an already-running daemon (S2b).
    Spawn,
    /// Kill a daemon-managed job (S2b).
    Kill,
    /// Liveness probe (S2b daemon reconnect).
    Ping,
}

/// Request the orchestrator writes for the helper to consume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevationPayload {
    pub schema: u32,
    pub op: PayloadOp,
    /// Random nonce (hex); the result echoes it so the caller can correlate.
    pub nonce: String,
    pub spawned_at_unix: i64,
    /// `sha256(key)[..16]` hex — lets the helper detect a key mismatch.
    pub key_fingerprint: String,
    /// Where the helper writes its signed result.
    pub result_path: PathBuf,
    /// Named pipe for DaemonServe (S2b).
    #[serde(default)]
    pub pipe_name: Option<String>,
    /// Spawn request for Spawn op (S2b).
    #[serde(default)]
    pub spawn: Option<crate::request::SpawnRequest>,
    /// Job token for Kill op (S2b).
    #[serde(default)]
    pub job_token: Option<String>,
    /// Marker path (for Provision).
    #[serde(default)]
    pub marker_path: Option<PathBuf>,
}

impl ElevationPayload {
    pub fn new_provision(
        nonce: &str,
        spawned_at_unix: i64,
        key_fingerprint: String,
        result_path: PathBuf,
        marker_path: PathBuf,
    ) -> Self {
        Self {
            schema: crate::SCHEMA_VERSION,
            op: PayloadOp::Provision,
            nonce: nonce.to_string(),
            spawned_at_unix,
            key_fingerprint,
            result_path,
            pipe_name: None,
            spawn: None,
            job_token: None,
            marker_path: Some(marker_path),
        }
    }
}

/// What the helper writes back.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperResult {
    pub schema: u32,
    /// Echoes the payload nonce.
    pub nonce: String,
    pub ok: bool,
    pub setup_kind: WindowsSetupKind,
    pub filesystem_isolation: FsIsolationStrength,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub marker: Option<SetupMarker>,
    /// Filled by the elevated Spawn op (S2b).
    #[serde(default)]
    pub spawn_pid: Option<u32>,
}

/// Persisted record of a successful provisioning — the marker used for drift detection (S2c).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetupMarker {
    pub schema: u32,
    pub created_at_unix: i64,
    pub setup_kind: WindowsSetupKind,
    pub filesystem_isolation: FsIsolationStrength,
    pub key_fingerprint: String,
    #[serde(default)]
    pub denied_paths: Vec<PathBuf>,
    #[serde(default)]
    pub writable_roots_lowered: Vec<PathBuf>,
    #[serde(default)]
    pub workspace_root: Option<PathBuf>,
    #[serde(default)]
    pub daemon_pipe: Option<String>,
    #[serde(default)]
    pub daemon_pid: Option<u32>,
}

/// On-disk framing: the typed value plus its HMAC tag (hex).
#[derive(Debug, Serialize, Deserialize)]
pub struct SignedPayload {
    pub payload: ElevationPayload,
    pub tag: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedResult {
    pub result: HelperResult,
    pub tag: String,
}

#[derive(Debug, Error)]
pub enum FileFramingError {
    #[error("io error: {0}")]
    Io(String),
    #[error("json error: {0}")]
    Json(String),
    #[error("hmac verification failed")]
    Hmac,
    #[error("missing result file (helper did not produce one)")]
    Missing,
}

fn write_json(path: &Path, bytes: &[u8]) -> Result<(), FileFramingError> {
    std::fs::write(path, bytes).map_err(|e| FileFramingError::Io(e.to_string()))
}

fn read_json(path: &Path) -> Result<Vec<u8>, FileFramingError> {
    std::fs::read(path).map_err(|e| FileFramingError::Io(e.to_string()))
}

/// Sign + write a payload file.
pub fn write_signed_payload(
    path: &Path,
    payload: &ElevationPayload,
    key: &[u8],
) -> Result<(), WindowsSandboxError> {
    let body = serde_json::to_vec(payload).map_err(|e| FileFramingError::Json(e.to_string()))?;
    let tag = hex::encode(mac::sign(key, &body));
    let framed = SignedPayload { payload: payload.clone(), tag };
    // `body` was folded into `tag` above; the framed object carries its own `payload` so the
    // helper can recompute the tag over the same canonical bytes.
    let framed_bytes =
        serde_json::to_vec(&framed).map_err(|e| FileFramingError::Json(e.to_string()))?;
    write_json(path, &framed_bytes)?;
    Ok(())
}

/// Read + verify a payload file. Returns the inner payload only if the HMAC matches.
pub fn read_signed_payload(path: &Path, key: &[u8]) -> Result<ElevationPayload, FileFramingError> {
    let bytes = read_json(path)?;
    let framed: SignedPayload =
        serde_json::from_slice(&bytes).map_err(|e| FileFramingError::Json(e.to_string()))?;
    let body =
        serde_json::to_vec(&framed.payload).map_err(|e| FileFramingError::Json(e.to_string()))?;
    let expected_tag = mac::sign(key, &body);
    let given = hex::decode(&framed.tag).map_err(|_| FileFramingError::Hmac)?;
    if !mac::verify(key, &body, &given) || expected_tag.len() != given.len() {
        return Err(FileFramingError::Hmac);
    }
    Ok(framed.payload)
}

/// Sign + write a result file (helper side).
pub fn write_signed_result(
    path: &Path,
    result: &HelperResult,
    key: &[u8],
) -> Result<(), FileFramingError> {
    let body = serde_json::to_vec(result).map_err(|e| FileFramingError::Json(e.to_string()))?;
    let tag = hex::encode(mac::sign(key, &body));
    let framed = SignedResult { result: result.clone(), tag };
    let framed_bytes =
        serde_json::to_vec(&framed).map_err(|e| FileFramingError::Json(e.to_string()))?;
    write_json(path, &framed_bytes)
}

/// Read + verify a result file (orchestrator side).
pub fn read_signed_result(path: &Path, key: &[u8]) -> Result<HelperResult, FileFramingError> {
    if !path.exists() {
        return Err(FileFramingError::Missing);
    }
    let bytes = read_json(path)?;
    let framed: SignedResult =
        serde_json::from_slice(&bytes).map_err(|e| FileFramingError::Json(e.to_string()))?;
    let body =
        serde_json::to_vec(&framed.result).map_err(|e| FileFramingError::Json(e.to_string()))?;
    let given = hex::decode(&framed.tag).map_err(|_| FileFramingError::Hmac)?;
    if !mac::verify(key, &body, &given) {
        return Err(FileFramingError::Hmac);
    }
    Ok(framed.result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> Vec<u8> {
        b"ipc-test-key-32-bytes-_____________"[..32].to_vec()
    }

    #[test]
    fn payload_roundtrip_verifies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("payload.json");
        let payload = ElevationPayload::new_provision(
            "abc123",
            0,
            "fingerprint".into(),
            dir.path().join("result.json"),
            dir.path().join("marker.json"),
        );
        write_signed_payload(&path, &payload, &key()).unwrap();
        let read = read_signed_payload(&path, &key()).unwrap();
        assert_eq!(read.nonce, "abc123");
        assert_eq!(read.op, PayloadOp::Provision);
    }

    #[test]
    fn tampered_payload_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("payload.json");
        let payload = ElevationPayload::new_provision(
            "abc123",
            0,
            "fingerprint".into(),
            dir.path().join("result.json"),
            dir.path().join("marker.json"),
        );
        write_signed_payload(&path, &payload, &key()).unwrap();
        // Flip the schema byte in the raw file (a field the verifier recomputes over).
        let mut raw = std::fs::read(&path).unwrap();
        // Corrupt something inside the embedded payload JSON.
        if let Some(idx) = raw.windows(3).position(|w| w == b"abc") {
            raw[idx] = b'X';
        }
        std::fs::write(&path, raw).unwrap();
        let err = read_signed_payload(&path, &key()).unwrap_err();
        assert!(matches!(err, FileFramingError::Hmac));
    }

    #[test]
    fn result_roundtrip_verifies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("result.json");
        let result = HelperResult {
            schema: crate::SCHEMA_VERSION,
            nonce: "n1".into(),
            ok: true,
            setup_kind: WindowsSetupKind::JobObject,
            filesystem_isolation: FsIsolationStrength::Lexical,
            error: None,
            marker: None,
            spawn_pid: None,
        };
        write_signed_result(&path, &result, &key()).unwrap();
        let read = read_signed_result(&path, &key()).unwrap();
        assert!(read.ok);
        assert_eq!(read.nonce, "n1");
    }

    #[test]
    fn missing_result_is_missing_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        let err = read_signed_result(&path, &key()).unwrap_err();
        assert!(matches!(err, FileFramingError::Missing));
    }
}
