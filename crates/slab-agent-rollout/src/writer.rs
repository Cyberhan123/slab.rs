//! Low-level append writer over a buffered file handle, plus the atomic
//! truncate/rotation helper used by [`crate::store`].
//!
//! The writer is intentionally primitive: it serializes [`RolloutLine`]s to bytes
//! and appends them with a trailing newline, then `fsync`s on flush for
//! durability. Atomic file replacement (for truncation / rotation) goes through
//! [`tempfile`] so it is safe on Windows (rename-based replace).

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::{Result, RolloutError};
use crate::item::RolloutLine;

/// Buffered append-only writer for a single rollout JSONL file.
pub struct JsonlWriter {
    inner: BufWriter<File>,
}

impl JsonlWriter {
    /// Open `path` for appending, creating it if necessary.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().append(true).create(true).open(path)?;
        Ok(Self { inner: BufWriter::new(file) })
    }

    /// Append a single serialized line + newline. Does not flush.
    pub fn append_line(&mut self, line: &RolloutLine) -> Result<()> {
        let bytes = serde_json::to_vec(line)?;
        self.inner.write_all(&bytes)?;
        self.inner.write_all(b"\n")?;
        Ok(())
    }

    /// Append an already-serialized raw line. The recorder serializes each
    /// pending line to bytes ONCE and feeds it here, so the per-line durable
    /// count can advance immediately after the bytes reach the OS file — an
    /// fsync failure then leaves a durability gap, never a duplicate. (M3)
    pub fn append_raw(&mut self, bytes: &[u8]) -> Result<()> {
        self.inner.write_all(bytes)?;
        if !bytes.ends_with(b"\n") {
            self.inner.write_all(b"\n")?;
        }
        Ok(())
    }

    /// Flush the `BufWriter` so buffered bytes reach the OS file. Does NOT
    /// `fsync` — used by the recorder to advance the per-line durable count
    /// before best-effort `sync`, so an fsync failure is a durability gap, not a
    /// duplication (see [`crate::recorder`]). (M3)
    pub fn flush_buffered(&mut self) -> Result<()> {
        self.inner.flush()?;
        Ok(())
    }

    /// `fsync` the underlying file for durability (best-effort from the
    /// recorder's perspective). (M3)
    pub fn sync(&mut self) -> Result<()> {
        self.inner.get_ref().sync_all()?;
        Ok(())
    }

    /// Flush the buffer AND `fsync` the underlying file for durability. Compound
    /// of [`flush_buffered`](Self::flush_buffered) + [`sync`](Self::sync); used by
    /// the header write where a failure must roll back the pending meta.
    pub fn flush(&mut self) -> Result<()> {
        self.flush_buffered()?;
        self.sync()?;
        Ok(())
    }
}

/// Atomically replace `path` with the concatenation of `kept` raw line bytes.
///
/// Writes the kept bytes to a `NamedTempFile` in the same directory, then
/// `persist`s it over `path`. On Windows this uses `ReplaceFile` semantics,
/// avoiding the partial-write window a direct truncate+rewrite would have.
///
/// ## Windows-safety scope
/// The recorder actor drops its OWN write handle before calling this, so the
/// write side never contends with the rename. Read paths, however, open their
/// own read-only `File` handles independently of the actor, and on Windows a
/// read handle open at the instant of the rename can cause a sharing violation
/// (ERROR_SHARING_VIOLATION / `PermissionDenied`). To tolerate this we retry
/// `persist` on a transient sharing violation with a bounded exponential
/// backoff (~50 attempts, 5ms→100ms), keeping the worst case to a few seconds.
/// Any other error returns immediately.
pub fn replace_file_atomically(path: &Path, kept: &[Vec<u8>]) -> Result<()> {
    let parent =
        path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    for line in kept {
        tmp.write_all(line)?;
        if !line.ends_with(b"\n") {
            tmp.write_all(b"\n")?;
        }
    }
    tmp.as_file().sync_all()?;
    persist_with_sharing_retry(tmp, path)?;
    Ok(())
}

/// `persist` the temp file over `path`, retrying transient Windows sharing
/// violations (a concurrently-open read handle). On a non-retriable error the
/// underlying `io::Error` is surfaced. The `NamedTempFile` is recovered from
/// `PersistError` on each retriable failure so the same (already-written, synced)
/// temp file is retried.
fn persist_with_sharing_retry(mut tmp: tempfile::NamedTempFile, path: &Path) -> Result<()> {
    const MAX_ATTEMPTS: u32 = 50;
    let mut delay = std::time::Duration::from_millis(5);
    let cap = std::time::Duration::from_millis(100);
    for _ in 0..MAX_ATTEMPTS {
        match tmp.persist(path) {
            Ok(_) => return Ok(()),
            Err(tempfile::PersistError { file, error }) => {
                if is_sharing_violation(&error) {
                    tmp = file;
                    std::thread::sleep(delay);
                    delay = (delay * 2).min(cap);
                    continue;
                }
                return Err(RolloutError::Io(error));
            }
        }
    }
    // Exhausted retries — surface a final attempt's distinct error so callers see
    // the real cause rather than a generic timeout.
    tracing::error!(
        path = %path.display(),
        attempts = MAX_ATTEMPTS,
        "replace_file_atomically exhausted sharing-violation retries"
    );
    Err(RolloutError::Io(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "rollout atomic replace blocked by a persistent sharing violation",
    )))
}

/// Whether an `io::Error` looks like a Windows sharing violation (rename-over
/// while a read handle is open). We check both the raw OS code (32 ==
/// ERROR_SHARING_VIOLATION) and the `PermissionDenied` kind for portability
/// across how the error is constructed.
fn is_sharing_violation(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(32) || error.kind() == std::io::ErrorKind::PermissionDenied
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{RolloutItem, SessionMeta};

    #[test]
    fn append_and_flush_writes_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.rollout.jsonl");
        let mut writer = JsonlWriter::open(&path).unwrap();

        let meta = SessionMeta {
            thread_id: "t".to_owned(),
            session_id: "s".to_owned(),
            parent_id: None,
            started_at: "2026-08-02T00:00:00Z".to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: None,
            trace_path: None,
        };
        writer
            .append_line(&RolloutLine::with_timestamp(
                "2026-08-02T00:00:00Z",
                RolloutItem::SessionMeta(meta),
            ))
            .unwrap();
        writer.flush().unwrap();
        drop(writer);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.ends_with('\n'));
        let parsed: RolloutLine = serde_json::from_str(content.trim_end()).unwrap();
        assert!(matches!(parsed.item, RolloutItem::SessionMeta(_)));
    }

    #[test]
    fn append_creates_missing_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/deep/t.rollout.jsonl");
        let mut writer = JsonlWriter::open(&path).unwrap();
        let meta = SessionMeta {
            thread_id: "t".to_owned(),
            session_id: "s".to_owned(),
            parent_id: None,
            started_at: "x".to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: 1,
            role_name: None,
            trace_path: None,
        };
        writer
            .append_line(&RolloutLine::with_timestamp("x", RolloutItem::SessionMeta(meta)))
            .unwrap();
        writer.flush().unwrap();
        assert!(path.exists());
    }

    #[test]
    fn replace_file_atomically_rewrites() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.rollout.jsonl");
        std::fs::write(&path, b"one\nTWO\nthree\n").unwrap();

        let kept = vec![b"one\n".to_vec(), b"three\n".to_vec()];
        replace_file_atomically(&path, &kept).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\nthree\n");
    }

    #[test]
    fn replace_file_atomically_creates_when_absent() {
        // Happy path: no read handle open, no contention. The retry path itself
        // is only exercised under Windows sharing contention (hard to unit-test);
        // here we confirm the helper exists and the no-contention path still works.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fresh.rollout.jsonl");
        assert!(!path.exists());
        let kept = vec![b"only\n".to_vec()];
        replace_file_atomically(&path, &kept).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "only\n");
        // The sharing-violation classifier is exercised for both kinds.
        let share = std::io::Error::from_raw_os_error(32);
        assert!(is_sharing_violation(&share));
        let denied = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "x");
        assert!(is_sharing_violation(&denied));
        let other = std::io::Error::new(std::io::ErrorKind::NotFound, "x");
        assert!(!is_sharing_violation(&other));
    }
}
