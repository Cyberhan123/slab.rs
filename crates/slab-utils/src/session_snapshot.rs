//! Atomic session snapshot for workspace sidecar migration (INFRA-01 / ADR-012).
//!
//! When the host switches workspace it must enumerate active agent threads,
//! interrupt them, and persist a snapshot so the new sidecar can restore them
//! scoped to the originating project. The snapshot is written atomically
//! (write to a sibling tmp file, then rename) so a crash mid-write cannot leave
//! a partial/truncated snapshot that would be mistaken for "no pending work".

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One interrupted thread recorded in a snapshot (restorable on the new sidecar).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSnapshotEntry {
    pub thread_id: String,
    /// Terminal status at snapshot time (typically `interrupted`).
    pub status: String,
}

/// A project-scoped snapshot of restorable agent threads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSnapshot {
    /// Workspace-derived project id; the new sidecar restores only threads whose
    /// project id matches (red-team boundary: no cross-workpoint leakage).
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub threads: Vec<SessionSnapshotEntry>,
}

impl SessionSnapshot {
    pub fn new(project_id: impl Into<String>) -> Self {
        Self { project_id: project_id.into(), session_id: None, threads: Vec::new() }
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        self.session_id = if session_id.is_empty() { None } else { Some(session_id) };
        self
    }

    pub fn with_thread(mut self, thread_id: impl Into<String>, status: impl Into<String>) -> Self {
        self.threads
            .push(SessionSnapshotEntry { thread_id: thread_id.into(), status: status.into() });
        self
    }

    /// File name for a project's snapshot inside a session-state dir.
    pub fn file_name(project_id: &str) -> String {
        let safe = project_id
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' { ch } else { '_' })
            .collect::<String>();
        format!("migration-{}.json", safe)
    }
}

/// Write a snapshot atomically: serialize to `<dir>/<file>.tmp`, then rename to
/// `<dir>/<file>`. Returns the final path. A pre-existing tmp file is overwritten.
pub fn write_session_snapshot_atomic(
    dir: &Path,
    snapshot: &SessionSnapshot,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|error| format!("create snapshot dir failed: {error}"))?;

    let final_path = dir.join(SessionSnapshot::file_name(&snapshot.project_id));
    let tmp_path = final_path.with_extension("json.tmp");

    let bytes = serde_json::to_vec(snapshot)
        .map_err(|error| format!("serialize session snapshot failed: {error}"))?;

    std::fs::write(&tmp_path, bytes)
        .map_err(|error| format!("write snapshot tmp failed: {error}"))?;
    std::fs::rename(&tmp_path, &final_path)
        .map_err(|error| format!("rename snapshot into place failed: {error}"))?;

    Ok(final_path)
}

/// Read a project's snapshot. Returns `Ok(None)` when no snapshot exists.
pub fn read_session_snapshot(
    dir: &Path,
    project_id: &str,
) -> Result<Option<SessionSnapshot>, String> {
    let path = dir.join(SessionSnapshot::file_name(project_id));
    match std::fs::read(&path) {
        Ok(bytes) => {
            let snapshot = serde_json::from_slice(&bytes)
                .map_err(|error| format!("parse session snapshot failed: {error}"))?;
            Ok(Some(snapshot))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("read session snapshot failed: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_round_trips_with_project_binding() {
        let temp = tempfile::tempdir().expect("temp dir");
        let snapshot = SessionSnapshot::new("proj-1")
            .with_session_id("sess-7")
            .with_thread("thread-a", "interrupted")
            .with_thread("thread-b", "interrupted");

        let path = write_session_snapshot_atomic(temp.path(), &snapshot).expect("write");
        assert!(path.ends_with("migration-proj-1.json"));
        // Atomic: no leftover tmp file.
        assert!(!path.with_extension("json.tmp").exists());

        let loaded = read_session_snapshot(temp.path(), "proj-1").expect("read").expect("present");
        assert_eq!(loaded, snapshot);
        assert_eq!(loaded.threads.len(), 2);
        assert_eq!(loaded.threads[0].thread_id, "thread-a");
    }

    #[test]
    fn read_returns_none_when_no_snapshot() {
        let temp = tempfile::tempdir().expect("temp dir");
        assert!(read_session_snapshot(temp.path(), "absent").unwrap().is_none());
    }

    #[test]
    fn snapshots_are_isolated_per_project() {
        let temp = tempfile::tempdir().expect("temp dir");
        write_session_snapshot_atomic(
            temp.path(),
            &SessionSnapshot::new("proj-a").with_thread("t1", "interrupted"),
        )
        .unwrap();
        write_session_snapshot_atomic(
            temp.path(),
            &SessionSnapshot::new("proj-b").with_thread("t2", "interrupted"),
        )
        .unwrap();

        // proj-b's snapshot must not surface proj-a's threads (no cross-project leakage).
        let b = read_session_snapshot(temp.path(), "proj-b").unwrap().unwrap();
        assert_eq!(b.threads.len(), 1);
        assert_eq!(b.threads[0].thread_id, "t2");
    }

    #[test]
    fn overwrites_previous_snapshot_for_same_project() {
        let temp = tempfile::tempdir().expect("temp dir");
        write_session_snapshot_atomic(
            temp.path(),
            &SessionSnapshot::new("proj-x").with_thread("old", "interrupted"),
        )
        .unwrap();
        write_session_snapshot_atomic(
            temp.path(),
            &SessionSnapshot::new("proj-x").with_thread("new", "interrupted"),
        )
        .unwrap();

        let loaded = read_session_snapshot(temp.path(), "proj-x").unwrap().unwrap();
        assert_eq!(loaded.threads.len(), 1);
        assert_eq!(loaded.threads[0].thread_id, "new");
    }

    #[test]
    fn file_name_sanitizes_project_id() {
        assert_eq!(SessionSnapshot::file_name("proj 1/2"), "migration-proj_1_2.json");
        assert_eq!(SessionSnapshot::file_name("safe-id"), "migration-safe-id.json");
    }
}
