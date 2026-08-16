//! Rollout JSONL reader — line-by-line parsing with fault tolerance + seek.
//!
//! Reading opens a fresh, read-only handle (the recorder owns the write handle).
//! Unparseable lines are skipped with a `tracing::warn` and a line number, never
//! aborting the whole replay — a single corrupt line should not make an entire
//! session unrecoverable.

use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

use crate::item::RolloutLine;

/// Fault-tolerant convenience: read every parseable [`RolloutLine`] from `path`.
///
/// Unparseable lines are logged and skipped. Returns an empty vec when the file
/// does not exist (a thread with no rollout yet).
pub fn read_rollout_lines(path: &Path) -> Vec<RolloutLine> {
    if !path.exists() {
        return Vec::new();
    }
    let mut reader = match RolloutReader::open(path) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "failed to open rollout file");
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    while let Some(res) = reader.next_line() {
        match res {
            Ok(line) => out.push(line),
            Err(e) => tracing::warn!(
                line = reader.line_number(),
                error = %e,
                "skipping unparseable rollout line"
            ),
        }
    }
    out
}

/// Seek-capable streaming reader over a rollout file.
pub struct RolloutReader {
    reader: BufReader<File>,
    /// 1-indexed number of the last line consumed.
    line: usize,
}

impl RolloutReader {
    /// Open a file for reading from the start.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let file = File::open(path)?;
        Ok(Self { reader: BufReader::new(file), line: 0 })
    }

    /// 1-indexed number of the last line read (0 before any read).
    pub fn line_number(&self) -> usize {
        self.line
    }

    /// Read and parse the next line, skipping blank lines. Returns `None` at EOF.
    pub fn next_line(&mut self) -> Option<std::result::Result<RolloutLine, serde_json::Error>> {
        let mut buf = String::new();
        loop {
            buf.clear();
            let n = match self.reader.read_line(&mut buf) {
                Ok(n) => n,
                Err(e) => {
                    // Treat an I/O failure as a soft EOF for the iterator contract;
                    // the caller already has a Result channel per line.
                    tracing::warn!(error = %e, "io error while reading rollout line");
                    return None;
                }
            };
            if n == 0 {
                return None;
            }
            self.line += 1;
            let trimmed = buf.trim_end_matches(['\n', '\r']);
            if trimmed.is_empty() {
                continue;
            }
            return Some(serde_json::from_str(trimmed));
        }
    }

    /// Seek to byte `pos` from the start of the file (for index-assisted replay).
    pub fn seek(&mut self, pos: u64) -> std::io::Result<()> {
        self.reader.seek(SeekFrom::Start(pos))?;
        Ok(())
    }

    /// Current byte offset from the start of the file.
    pub fn stream_position(&mut self) -> std::io::Result<u64> {
        self.reader.stream_position()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{RolloutItem, RolloutLine, SessionMeta};
    use std::io::Write;

    fn meta_line() -> RolloutLine {
        RolloutLine::with_timestamp(
            "2026-08-02T00:00:00Z",
            RolloutItem::SessionMeta(SessionMeta {
                thread_id: "t".to_owned(),
                session_id: "s".to_owned(),
                parent_id: None,
                started_at: "x".to_owned(),
                config_json: serde_json::json!({}),
                rollout_version: 1,
                role_name: None,
                trace_path: None,
            }),
        )
    }

    #[test]
    fn reads_lines_back_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.rollout.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            for i in 0..3 {
                let line = RolloutLine::with_timestamp(format!("t{i}"), meta_line().item.clone());
                writeln!(f, "{}", serde_json::to_string(&line).unwrap()).unwrap();
            }
        }
        let lines = read_rollout_lines(&path);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].timestamp, "t0");
        assert_eq!(lines[2].timestamp, "t2");
    }

    #[test]
    fn missing_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.rollout.jsonl");
        assert!(read_rollout_lines(&path).is_empty());
    }

    #[test]
    fn skips_unparseable_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.rollout.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            let good = serde_json::to_string(&meta_line()).unwrap();
            writeln!(f, "{good}").unwrap();
            writeln!(f, "{{ this is not valid json").unwrap();
            writeln!(f, "{good}").unwrap();
            writeln!(f).unwrap(); // blank line
        }
        let lines = read_rollout_lines(&path);
        // Two good lines survive; the bad + blank are skipped.
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn reader_reports_line_number() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.rollout.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "{}", serde_json::to_string(&meta_line()).unwrap()).unwrap();
            writeln!(f, "{{ bad").unwrap();
        }
        let mut reader = RolloutReader::open(&path).unwrap();
        assert!(reader.next_line().unwrap().is_ok());
        assert_eq!(reader.line_number(), 1);
        // bad line at index 2 returns Some(Err)
        assert!(reader.next_line().unwrap().is_err());
        assert_eq!(reader.line_number(), 2);
        assert!(reader.next_line().is_none());
    }

    #[test]
    fn seek_skips_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.rollout.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "{}", serde_json::to_string(&meta_line()).unwrap()).unwrap();
        }
        let mut reader = RolloutReader::open(&path).unwrap();
        let pos = reader.stream_position().unwrap();
        reader.seek(pos).unwrap();
        assert!(reader.next_line().is_some());
    }
}
