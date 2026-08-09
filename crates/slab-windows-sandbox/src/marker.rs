//! `SetupMarker` read/write + drift detection. The marker is an advisory cache of the "last
//! good provision"; the source of truth is re-derived on each `prepare()` (S2c). S2a ships a
//! minimal write/read + a conservative drift stub (always treats a present, schema-valid marker
//! with a matching key fingerprint as non-drifting); the full drift matrix lands in S2c.

use std::path::Path;

use crate::error::WindowsSandboxError;
use crate::ipc::SetupMarker;

/// Write the marker (plain JSON — it carries no secret; the key fingerprint is a digest).
pub fn write_marker(path: &Path, marker: &SetupMarker) -> Result<(), WindowsSandboxError> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let bytes = serde_json::to_vec_pretty(marker)
        .map_err(|e| WindowsSandboxError::SetupFailed(e.to_string()))?;
    std::fs::write(path, bytes).map_err(|e| WindowsSandboxError::KeyIo(e.to_string()))
}

/// Read the marker. `None` if absent.
pub fn read_marker(path: &Path) -> Result<Option<SetupMarker>, WindowsSandboxError> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|e| WindowsSandboxError::KeyIo(e.to_string()))?;
    let marker: SetupMarker = serde_json::from_slice(&bytes)
        .map_err(|e| WindowsSandboxError::SetupFailed(e.to_string()))?;
    Ok(Some(marker))
}

/// Whether the marker satisfies a request with the given key fingerprint. Conservative: any
/// structural problem (missing, wrong schema, wrong key) ⇒ drift ⇒ the caller re-provisions.
/// Full drift (denied-path coverage, workspace-root change, daemon liveness) lands in S2c.
pub fn has_drift(marker: Option<&SetupMarker>, expected_key_fingerprint: &str) -> bool {
    match marker {
        None => true,
        Some(m) => {
            m.schema != crate::SCHEMA_VERSION || m.key_fingerprint != expected_key_fingerprint
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{FsIsolationStrength, WindowsSetupKind};

    fn sample_marker() -> SetupMarker {
        SetupMarker {
            schema: crate::SCHEMA_VERSION,
            created_at_unix: 1,
            setup_kind: WindowsSetupKind::JobObject,
            filesystem_isolation: FsIsolationStrength::Lexical,
            key_fingerprint: "abcd".into(),
            denied_paths: vec![],
            writable_roots_lowered: vec![],
            workspace_root: None,
            daemon_pipe: None,
            daemon_pid: None,
        }
    }

    #[test]
    fn write_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("marker.json");
        let m = sample_marker();
        write_marker(&path, &m).unwrap();
        let read = read_marker(&path).unwrap().unwrap();
        assert_eq!(read.schema, m.schema);
        assert_eq!(read.key_fingerprint, m.key_fingerprint);
    }

    #[test]
    fn missing_marker_is_drift() {
        assert!(has_drift(None, "anything"));
    }

    #[test]
    fn wrong_fingerprint_is_drift() {
        let m = sample_marker();
        assert!(has_drift(Some(&m), "different"));
    }

    #[test]
    fn matching_marker_no_drift() {
        let m = sample_marker();
        assert!(!has_drift(Some(&m), "abcd"));
    }
}
