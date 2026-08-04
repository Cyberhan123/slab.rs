//! `session_index.jsonl` — the L2' text reverse-lookup index for rollout files.
//!
//! When a rollout file moves to the date-partitioned layout (`sessions/YYYY/MM/DD/
//! rollout-<ts>-<thread_id>.jsonl`), the file name alone is enough to find it by
//! scan, but a small append-only text index makes the common case (resolve a
//! thread to its file) a single bounded tail-read instead of a directory walk.
//! It is a PURE OPTIMIZATION with a graceful fallback: [`crate::store`] always
//! also does a date-dir scan + a legacy-flat check, so a missing/stale entry
//! never makes a file unreadable.
//!
//! One line per create/migrate event, JSON-encoded [`SessionIndexEntry`]. Readers
//! that want "the latest entry for a thread" seek to the last 8 KiB and scan
//! backwards — the last append wins. Duplicate appends (e.g. a re-create that
//! raced) are harmless: the reverse scan takes the newest matching entry and
//! verifies the path still exists on disk before using it.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One record appended to `session_index.jsonl`.
///
/// `name` is populated lazily (D2 list backfill); create/migrate write `None`
/// here today because the rollout `SessionMeta` does not carry the session
/// display name.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionIndexEntry {
    /// Owning thread id (sanitized form matches the file-name segment).
    pub thread_id: String,
    /// Session the thread belongs to.
    pub session_id: String,
    /// Optional session display name (populated by D2 list backfill).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// RFC-3339 timestamp of this index update.
    pub updated_at: String,
    /// Absolute (or sessions_dir-relative-resolvable) rollout file path.
    pub file_path: String,
}

/// The canonical index path: `<sessions_dir>/session_index.jsonl`.
pub fn index_path(sessions_dir: &Path) -> PathBuf {
    sessions_dir.join("session_index.jsonl")
}

/// Append one entry to the index. Best-effort: a write failure is warned and
/// does NOT propagate — the index is an optimization, the scan + flat fallback
/// in [`crate::store::RolloutFileStore::lookup_path`] still finds the file.
pub fn append_entry(sessions_dir: &Path, entry: &SessionIndexEntry) {
    let path = index_path(sessions_dir);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut bytes = match serde_json::to_vec(entry) {
        Ok(b) => b,
        Err(error) => {
            tracing::warn!(error = %error, "session_index serialize failed; entry dropped");
            return;
        }
    };
    if !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    use std::io::Write;
    match std::fs::OpenOptions::new().append(true).create(true).open(&path) {
        Ok(mut f) => {
            let _ = f.write_all(&bytes);
            let _ = f.sync_all();
        }
        Err(error) => tracing::warn!(
            error = %error,
            path = %path.display(),
            "session_index append failed; lookup will fall back to scan + flat",
        ),
    }
}

/// The NEWEST index entry whose `thread_id` matches, or `None` when the index
/// is absent / has no match. Seeks to the last 8 KiB and scans backwards so a
/// large index is still O(1)-ish to query for recent threads.
pub fn find_latest_for_thread(sessions_dir: &Path, thread_id: &str) -> Option<SessionIndexEntry> {
    let path = index_path(sessions_dir);
    scan_from_end(&path, |e| e.thread_id == thread_id)
}

/// Bulk name/entry lookup by thread id (forward scan). Returns the NEWEST entry
/// per requested id. Used by the D2 list backfill to recover session display
/// names without a per-thread reverse scan.
pub fn find_entries_by_ids(
    sessions_dir: &Path,
    ids: &HashSet<String>,
) -> HashMap<String, SessionIndexEntry> {
    let mut out: HashMap<String, SessionIndexEntry> = HashMap::new();
    let path = index_path(sessions_dir);
    if !path.exists() {
        return out;
    }
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return out;
    };
    for line in raw.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<SessionIndexEntry>(trimmed) else {
            continue;
        };
        if ids.contains(&entry.thread_id) {
            // Last write wins (newest entry is at the tail of the file).
            out.insert(entry.thread_id.clone(), entry);
        }
    }
    out
}

/// Reverse-scan the tail of `path` (up to 8 KiB) for the newest entry matching
/// `predicate`. The first 8 KiB-boundary straddler line is dropped (it may be
/// partial) so we only ever parse complete lines.
fn scan_from_end<F>(path: &Path, predicate: F) -> Option<SessionIndexEntry>
where
    F: Fn(&SessionIndexEntry) -> bool,
{
    if !path.exists() {
        return None;
    }
    let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if len == 0 {
        return None;
    }
    const TAIL: u64 = 8 * 1024;
    let start = len.saturating_sub(TAIL);
    let mut file = std::fs::File::open(path).ok()?;
    file.seek(SeekFrom::Start(start)).ok()?;
    // Read the tail as RAW BYTES and decode each line INDEPENDENTLY. A whole-block
    // `read_to_string` requires the entire 8 KiB window to be valid UTF-8, so a
    // multi-byte char (CJK session name / non-ASCII file_path segment) straddling
    // the seek boundary makes it return `InvalidData` and the whole lookup
    // silently degrades to a full scan. Decoding line-by-line lets us skip just
    // the (already-dropped) partial first chunk and any genuinely corrupt line,
    // matching the existing "skip bad lines" semantics.
    let mut buf: Vec<u8> = Vec::new();
    file.read_to_end(&mut buf).ok()?;

    let mut lines: Vec<&[u8]> = buf.split(|b| *b == b'\n').collect();
    // When we sought into the middle of the file, the first chunk is a partial
    // line — drop it. (At offset 0 the first chunk is a real line, but splitting
    // an empty leading is harmless: it just becomes "" which is skipped below.)
    if start > 0 {
        lines.remove(0);
    }
    for line in lines.into_iter().rev() {
        let trimmed = line.trim_ascii();
        if trimmed.is_empty() {
            continue;
        }
        // Skip lines that are not valid UTF-8 (a boundary-straddling multi-byte
        // char or genuine corruption) instead of failing the whole read.
        let Ok(s) = std::str::from_utf8(trimmed) else {
            continue;
        };
        if let Ok(entry) = serde_json::from_str::<SessionIndexEntry>(s)
            && predicate(&entry)
        {
            return Some(entry);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn entry(thread_id: &str, file_path: &str, updated_at: &str) -> SessionIndexEntry {
        SessionIndexEntry {
            thread_id: thread_id.to_owned(),
            session_id: "s".to_owned(),
            name: None,
            updated_at: updated_at.to_owned(),
            file_path: file_path.to_owned(),
        }
    }

    #[test]
    fn missing_index_returns_none() {
        let d = dir();
        assert!(find_latest_for_thread(d.path(), "t").is_none());
        assert!(find_entries_by_ids(d.path(), &HashSet::from(["t".to_owned()])).is_empty());
    }

    #[test]
    fn append_then_find_latest_for_thread() {
        let d = dir();
        append_entry(d.path(), &entry("t1", "/a/t1.jsonl", "2026-08-03T00:00:00Z"));
        append_entry(d.path(), &entry("t2", "/a/t2.jsonl", "2026-08-03T00:00:01Z"));
        let found = find_latest_for_thread(d.path(), "t1").expect("found t1");
        assert_eq!(found.thread_id, "t1");
        assert_eq!(found.file_path, "/a/t1.jsonl");
        let found2 = find_latest_for_thread(d.path(), "t2").expect("found t2");
        assert_eq!(found2.file_path, "/a/t2.jsonl");
        assert!(find_latest_for_thread(d.path(), "ghost").is_none());
    }

    #[test]
    fn duplicate_appends_take_newest_on_reverse_scan() {
        let d = dir();
        append_entry(d.path(), &entry("t", "/old/t.jsonl", "2026-08-03T00:00:00Z"));
        append_entry(d.path(), &entry("t", "/new/t.jsonl", "2026-08-03T00:00:05Z"));
        let found = find_latest_for_thread(d.path(), "t").expect("found");
        assert_eq!(found.file_path, "/new/t.jsonl", "reverse scan returns the newest entry");
    }

    #[test]
    fn reverse_scan_reads_only_tail_when_index_is_large() {
        let d = dir();
        let path = index_path(d.path());
        // Pad the head with entries for OTHER threads so the 8 KiB tail only
        // covers the most recent thread. The lookup must still find t-target.
        {
            let mut f = std::fs::File::create(&path).unwrap();
            for i in 0..200 {
                let e =
                    entry(&format!("pad-{i}"), &format!("/p/{i}.jsonl"), "2026-01-01T00:00:00Z");
                writeln!(f, "{}", serde_json::to_string(&e).unwrap()).unwrap();
            }
        }
        append_entry(d.path(), &entry("t-target", "/t/target.jsonl", "2026-08-03T00:00:00Z"));
        let found = find_latest_for_thread(d.path(), "t-target").expect("found in tail");
        assert_eq!(found.thread_id, "t-target");
    }

    #[test]
    fn find_entries_by_ids_returns_newest_per_id() {
        let d = dir();
        append_entry(d.path(), &entry("a", "/a1.jsonl", "2026-08-03T00:00:00Z"));
        append_entry(d.path(), &entry("b", "/b1.jsonl", "2026-08-03T00:00:00Z"));
        append_entry(d.path(), &entry("a", "/a2.jsonl", "2026-08-03T00:00:05Z"));
        let map = find_entries_by_ids(
            d.path(),
            &HashSet::from(["a".to_owned(), "b".to_owned(), "z".to_owned()]),
        );
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("a").unwrap().file_path, "/a2.jsonl");
        assert_eq!(map.get("b").unwrap().file_path, "/b1.jsonl");
        assert!(!map.contains_key("z"));
    }

    #[test]
    fn unparseable_lines_are_skipped() {
        let d = dir();
        let path = index_path(d.path());
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "{{not json").unwrap();
            writeln!(f, "{}", serde_json::to_string(&entry("t", "/t.jsonl", "x")).unwrap())
                .unwrap();
            writeln!(f, "also not json").unwrap();
        }
        let found = find_latest_for_thread(d.path(), "t").expect("parseable line found");
        assert_eq!(found.file_path, "/t.jsonl");
    }

    // F3: scan_from_end must read the tail as RAW BYTES and decode each line
    // independently, NOT read_to_string the whole 8 KiB window. When the seek
    // boundary lands inside a multi-byte UTF-8 char (a CJK session name / non-
    // ASCII file_path segment later on), a whole-block read_to_string rejects the
    // ENTIRE tail as InvalidData and find_latest_for_thread returns None → the
    // lookup silently degrades to a full date-tree scan. The current D1 entries
    // have ASCII-only file_paths so this never triggers today; D2 (Chinese
    // session display names) or a non-ASCII app_home path will. This test forces
    // the boundary into a CJK run and asserts the target line is still found.
    #[test]
    fn reverse_scan_skips_invalid_utf8_at_boundary() {
        let d = dir();
        let path = index_path(d.path());
        // One giant padding line whose file_path is a long run of multi-byte CJK
        // chars, then the target line. The 8 KiB reverse-scan window starts
        // inside the CJK run (mid-character): a whole-block read_to_string would
        // reject the whole tail as InvalidData; per-line byte decoding drops the
        // partial first chunk and still finds the target.
        let cjk = "脚本".repeat(3000); // 6000 CJK chars ≈ 18 KiB UTF-8
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&path).unwrap();
            let pad = entry("pad", &cjk, "2026-01-01T00:00:00Z");
            writeln!(f, "{}", serde_json::to_string(&pad).unwrap()).unwrap();
        }
        append_entry(d.path(), &entry("t-target", "/t/target.jsonl", "2026-08-03T00:00:00Z"));

        // Sanity: the file is larger than the 8 KiB tail window so the boundary
        // actually falls inside the CJK padding line.
        let len = std::fs::metadata(&path).unwrap().len();
        assert!(len > 8 * 1024, "file large enough to exercise the tail window: {len} bytes");

        // Mutation: revert scan_from_end to `file.read_to_string(&mut buf).ok()?`
        // → the boundary-straddling CJK makes read_to_string return InvalidData →
        // `.ok()?` yields None → this expect panics.
        let found = find_latest_for_thread(d.path(), "t-target")
            .expect("per-line byte decode skips boundary CJK; read_to_string mutation -> None");
        assert_eq!(found.thread_id, "t-target");
        assert_eq!(found.file_path, "/t/target.jsonl");
    }
}
