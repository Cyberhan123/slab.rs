//! [`RolloutStore`] trait + the file-backed implementation.
//!
//! [`RolloutFileStore`] owns one [`RolloutRecorderHandle`] per thread (guarded
//! by a [`DashMap`]). Writes go through the recorder actor; reads open a fresh
//! read-only handle (flushing the writer first so pending items are durable).

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, Datelike};
use dashmap::DashMap;
use tokio::sync::{mpsc, oneshot};

use crate::error::{Result, RolloutError};
use crate::item::{RolloutItem, RolloutLine, SessionMeta, TurnContextPayload};
use crate::reader::read_rollout_lines;
use crate::recorder::{RolloutCmd, RolloutRecorderHandle, RolloutRecorderParams};
use crate::writer::replace_file_atomically;

/// One entry of the interleaved turn timeline produced by
/// [`RolloutStore::read_turn_timeline`] — full-fidelity UI artifacts and
/// LLM-grade conversation messages **in the order their rollout lines were
/// written**, each carrying its turn affiliation.
///
/// Unlike the three separate replay reads (`read_messages` /
/// `read_turn_items` / `read_events`), the timeline preserves the relative
/// order between `TurnItem` and `MessageAppend` lines, so a consumer
/// rebuilding the UI history (`thread/resume`) renders the exact sequence the
/// live event stream produced instead of re-merging per-turn buckets.
#[derive(Debug, Clone)]
pub enum TurnTimelineEntry {
    /// A finalized UI artifact (`RolloutItem::TurnItem`), attributed by the
    /// running-turn heuristic (identical to [`RolloutStore::read_turn_items`]).
    Item(slab_agent::port::TurnItemRecord),
    /// An appended conversation message (`TurnContext::MessageAppend`) or a
    /// post-compaction baseline message (`Compacted`), carrying its own
    /// explicit turn affiliation.
    Message(slab_agent::port::ThreadMessageRecord),
}

/// Abstract read/write surface over the rollout true source.
#[async_trait]
pub trait RolloutStore: Send + Sync {
    /// Replay the LLM-visible conversation (from `TurnContext::MessageAppend`,
    /// reset at each `Compacted` baseline).
    async fn read_messages(&self, thread_id: &str) -> Vec<slab_types::ConversationMessage>;
    /// Replay finalized items (from `RolloutItem::TurnItem`), attaching the
    /// currently-tracked turn index and a monotonic `seq`.
    async fn read_turn_items(&self, thread_id: &str) -> Vec<slab_agent::port::TurnItemRecord>;
    /// Replay the interleaved `TurnItem` + `MessageAppend` timeline in file
    /// (write) order, each entry attributed to its turn. The single ordered
    /// source for history-restore projections — see [`TurnTimelineEntry`].
    async fn read_turn_timeline(&self, thread_id: &str) -> Vec<TurnTimelineEntry>;
    /// Replay persisted events (from `RolloutItem::EventMsg`).
    async fn read_events(&self, thread_id: &str) -> Vec<slab_agent::protocol::EventMsg>;
    /// Append one rollout item.
    async fn append(&self, thread_id: &str, item: RolloutItem) -> Result<()>;
    /// Append several rollout items at once.
    async fn append_batch(&self, thread_id: &str, items: Vec<RolloutItem>) -> Result<()>;
    /// Flush pending items for `thread_id` and wait for durability.
    async fn flush(&self, thread_id: &str) -> Result<()>;
    /// Atomically drop every line belonging to turn `from_turn` and later.
    async fn truncate_from_turn(&self, thread_id: &str, from_turn: u32) -> Result<()>;
    /// Atomically replace the thread rollout file with exactly these lines
    /// (preserving each line timestamp), routed through the recorder actor so it
    /// is serialized against concurrent appends and the writer handle is dropped
    /// before the on-disk replace (Windows-safe, like
    /// [`RolloutStore::truncate_from_turn`]). The next write reopens the file
    /// lazily and lands after the rewritten content.
    async fn rewrite_session(&self, thread_id: &str, lines: Vec<RolloutLine>) -> Result<()>;
    /// Whether the rollout file exists for `thread_id`.
    async fn file_exists(&self, thread_id: &str) -> bool;
    /// The session header, if the file exists and has one.
    async fn read_session_meta(&self, thread_id: &str) -> Option<SessionMeta>;
}

/// File-backed rollout store: one recorder per thread, files under `sessions_dir`.
pub struct RolloutFileStore {
    sessions_dir: PathBuf,
    recorders: DashMap<String, RolloutRecorderHandle>,
}

impl RolloutFileStore {
    /// Create a store rooted at `sessions_dir`.
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir, recorders: DashMap::new() }
    }

    /// The date-partitioned write path for a NEW session:
    /// `sessions/YYYY/MM/DD/rollout-<ts>-<sanitized_thread_id>.jsonl`. `<ts>` is
    /// a compact, fixed-width RFC-3339 timestamp (`YYYYMMDDTHHMMSSZ`) so
    /// file-name dictionary order equals chronological order within a day
    /// directory — the D2 watermark backfill relies on this to find the newest
    /// file for a thread by a simple reverse-scan. `YYYY/MM/DD` is extracted
    /// from `started_at` (RFC-3339); an unparseable `started_at` falls back to
    /// the epoch (`1970/01/01`, ts `19700101T000000Z`).
    ///
    /// Use this when materializing a NEW rollout file (you have the
    /// [`SessionMeta`] with its `started_at`). For reads, use
    /// [`Self::resolve_path`] which reverse-looks-up the EXISTING file across
    /// all layouts (date-partitioned, `session_index.jsonl`, legacy flat).
    ///
    /// The thread id is sanitized to a single path-safe segment: any char that
    /// is not ascii-alphanumeric, `-` or `_` becomes `_`. This prevents path
    /// traversal (`..`, `/`, `\`) from a hostile or malformed thread id, and is a
    /// no-op for normal UUID-like ids. (L5)
    pub fn path_for_new(&self, thread_id: &str, started_at: &str) -> PathBuf {
        let (year, month, day) = date_parts_from_started_at(started_at);
        let ts = compact_ts_from_started_at(started_at);
        let stem = sanitize_thread_id(thread_id);
        self.sessions_dir
            .join(format!("{year:04}"))
            .join(format!("{month:02}"))
            .join(format!("{day:02}"))
            .join(format!("rollout-{ts}-{stem}.jsonl"))
    }

    /// The pre-migration flat path `sessions/<sanitized_thread_id>.rollout.jsonl`.
    /// Kept as the read fallback ([`Self::resolve_path`]) so a rollout file
    /// written by a previous version (or skipped by the startup migration) is
    /// still readable.
    fn legacy_flat_path(&self, thread_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{}.rollout.jsonl", sanitize_thread_id(thread_id)))
    }

    /// Reverse-look-up the EXISTING rollout file for `thread_id` across all
    /// layouts (no fallback to a non-existent path). Returns `None` when no file
    /// is found anywhere.
    ///
    /// Chain order (cheapest + most-authoritative first):
    /// 1. `session_index.jsonl` text index (L2') — newest entry whose
    ///    `file_path` still exists on disk (stale entries are skipped).
    /// 2. Date-partitioned scan — `sessions/YYYY/MM/DD/
    ///    rollout-*-<sanitized>.jsonl`; the lexicographically-largest (newest)
    ///    match wins.
    /// 3. Legacy flat path — `sessions/<sanitized>.rollout.jsonl`.
    pub fn lookup_path(&self, thread_id: &str) -> Option<PathBuf> {
        let sanitized = sanitize_thread_id(thread_id);
        // (1) session_index.jsonl (L2' text reverse-lookup).
        if let Some(entry) =
            crate::session_index::find_latest_for_thread(&self.sessions_dir, &sanitized)
        {
            let indexed = PathBuf::from(&entry.file_path);
            if indexed.exists() {
                return Some(indexed);
            }
        }
        // (2) date-partitioned scan (newest match by file-name dictionary order).
        if let Some(found) = scan_date_dirs_for_thread(&self.sessions_dir, &sanitized) {
            return Some(found);
        }
        // (3) legacy flat fallback (pre-migration / skipped-by-migration files).
        let flat = self.legacy_flat_path(thread_id);
        if flat.exists() {
            return Some(flat);
        }
        None
    }

    /// The read path for `thread_id`: the first existing file in the
    /// [`Self::lookup_path`] chain, or — when no file exists yet — the legacy
    /// flat path (non-existent, so a subsequent `read_rollout_lines` returns an
    /// empty vec, matching the pre-migration behaviour for a thread with no
    /// rollout file).
    pub fn resolve_path(&self, thread_id: &str) -> PathBuf {
        self.lookup_path(thread_id).unwrap_or_else(|| self.legacy_flat_path(thread_id))
    }

    /// List the [`SessionMeta`] headers of EVERY rollout file discoverable under
    /// `sessions_dir` — the date-partitioned tree (`YYYY/MM/DD/rollout-*.jsonl`)
    /// AND the legacy top-level flat files (`*.rollout.jsonl`). Used by the
    /// `RolloutBackedAgentStore` list DB-unavailable fallback to reconstruct a
    /// best-effort thread listing purely from the rollout true source when the
    /// metadata DB cannot be queried.
    ///
    /// One `SessionMeta` per thread id. The precedence MIRRORS
    /// [`Self::lookup_path`]'s reverse chain: the date-partitioned tree wins
    /// over the legacy flat layout (the tree is the canonical new layout; a flat
    /// file is at best a stale pre-migration duplicate). Lexicographic path
    /// order is NOT a valid cross-layout chronological signal — `dir/2026/...`
    /// sorts before `dir/t.rollout.jsonl` purely on the `2` < `t` byte — so the
    /// two layouts are folded SEPARATELY: within the tree the newest path per
    /// thread wins (dict-order == chronological there, since the fixed-width ts
    /// prefix is the only variable segment under a shared YYYY/MM/DD/ parent),
    /// then a flat file fills in ONLY threads absent from the tree. Files whose
    /// first line does not parse as a `SessionMeta`-bearing rollout line are
    /// skipped (best-effort).
    pub fn list_all_session_metas(&self) -> Vec<SessionMeta> {
        // (1) Date-partitioned tree: newest path per thread wins (dict-order ==
        // chronological within the tree).
        let mut by_thread: std::collections::HashMap<String, SessionMeta> =
            std::collections::HashMap::new();
        let mut newest_path: std::collections::HashMap<String, PathBuf> =
            std::collections::HashMap::new();
        for path in scan_all_date_dirs(&self.sessions_dir) {
            let Some(meta) = read_first_line_session_meta(&path) else {
                continue;
            };
            let better = !matches!(
                newest_path.get(&meta.thread_id),
                Some(cur) if path.as_path() <= cur.as_path()
            );
            if better {
                newest_path.insert(meta.thread_id.clone(), path);
                by_thread.insert(meta.thread_id.clone(), meta);
            }
        }
        // (2) Legacy flat files: fill in ONLY threads absent from the tree.
        for path in list_dir_entries(&self.sessions_dir).into_iter().map(|e| e.path()) {
            let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !path.is_file() || !fname.ends_with(".rollout.jsonl") {
                continue;
            }
            let Some(meta) = read_first_line_session_meta(&path) else {
                continue;
            };
            by_thread.entry(meta.thread_id.clone()).or_insert(meta);
        }
        by_thread.into_values().collect()
    }

    /// One-shot startup migration: move every top-level
    /// `<thread_id>.rollout.jsonl` (the pre-migration flat layout) into the
    /// date-partitioned layout derived from its own `SessionMeta.started_at`,
    /// and append a `session_index.jsonl` entry for each move.
    ///
    /// Idempotent: a second run finds no top-level `*.rollout.jsonl` files and
    /// is a no-op. Crash-safe: `std::fs::rename` is atomic within the same
    /// volume, so a crash leaves the file at either the old or the new path —
    /// re-running picks it up from wherever it is. Returns the number of files
    /// moved. Best-effort: a single rename failure is warned and skipped (the
    /// [`Self::lookup_path`] flat fallback still finds the unmoved file).
    pub fn migrate_flat_rollouts(&self) -> usize {
        // Collect the candidate entries BEFORE renaming: renaming during a live
        // `read_dir` iteration can skip or revisit entries on Windows
        // (FindFirstFile/FindNextFile observe the directory lazily). A snapshot
        // Vec makes the migration deterministic and idempotent across platforms.
        let candidates: Vec<PathBuf> = match std::fs::read_dir(&self.sessions_dir) {
            Ok(r) => r
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.ends_with(".rollout.jsonl"))
                            .unwrap_or(false)
                })
                .collect(),
            Err(_) => return 0,
        };
        let mut migrated = 0usize;
        for path in candidates {
            let Some(meta) = read_first_line_session_meta(&path) else {
                // Not a rollout file we recognize (or empty/corrupt) — leave it.
                continue;
            };
            let new_path = self.path_for_new(&meta.thread_id, &meta.started_at);
            if new_path == path || new_path.exists() {
                // Already migrated (or a duplicate); leave the flat file for
                // resolve_path's flat fallback rather than risk clobbering.
                continue;
            }
            if let Some(parent) = new_path.parent()
                && std::fs::create_dir_all(parent).is_err()
            {
                continue;
            }
            match std::fs::rename(&path, &new_path) {
                Ok(()) => {
                    let entry = crate::session_index::SessionIndexEntry {
                        // Key the index by the SANITIZED id — the same form used
                        // for the file-name stem AND the lookup_path query — so
                        // `find_latest_for_thread(sanitized)` hits. Writing the
                        // raw id here would miss for any thread id whose raw form
                        // differs from its sanitized form (path separators, etc.).
                        thread_id: sanitize_thread_id(&meta.thread_id),
                        session_id: meta.session_id.clone(),
                        name: None,
                        updated_at: chrono::Utc::now().to_rfc3339(),
                        file_path: new_path.to_string_lossy().into_owned(),
                    };
                    crate::session_index::append_entry(&self.sessions_dir, &entry);
                    migrated += 1;
                }
                Err(error) => tracing::warn!(
                    error = %error,
                    from = %path.display(),
                    to = %new_path.display(),
                    "rollout flat-to-date migration: rename failed; leaving the flat file in place",
                ),
            }
        }
        migrated
    }

    /// Explicitly create a session (writes the `SessionMeta` header lazily on the
    /// first write). Returns without writing.
    ///
    /// If a rollout file for `meta.thread_id` already exists anywhere in the
    /// [`Self::lookup_path`] chain (e.g. on process restart, or when the adapter
    /// re-sends meta for an existing thread), spawn with `Resume` on the EXISTING
    /// path so a fresh `Create` recorder does not append a SECOND `SessionMeta`
    /// header (M2). Otherwise spawn `Create` at the new date-partitioned path
    /// derived from `meta.started_at`, and append a `session_index.jsonl` entry
    /// pointing at it (the L2' text reverse-lookup index).
    pub fn create_session(&self, meta: SessionMeta) {
        let thread_id = meta.thread_id.clone();
        self.recorders.entry(thread_id.clone()).or_insert_with(|| {
            if let Some(existing) = self.lookup_path(&thread_id) {
                RolloutRecorderHandle::spawn(
                    RolloutRecorderParams::Resume { thread_id: thread_id.clone() },
                    existing,
                )
            } else {
                let path = self.path_for_new(&thread_id, &meta.started_at);
                let entry = crate::session_index::SessionIndexEntry {
                    // Keyed by the SANITIZED id (matches the file-name stem and
                    // the lookup_path query); see migrate_flat_rollouts.
                    thread_id: sanitize_thread_id(&thread_id),
                    session_id: meta.session_id.clone(),
                    name: None,
                    updated_at: chrono::Utc::now().to_rfc3339(),
                    file_path: path.to_string_lossy().into_owned(),
                };
                crate::session_index::append_entry(&self.sessions_dir, &entry);
                RolloutRecorderHandle::spawn(RolloutRecorderParams::Create { meta }, path)
            }
        });
    }

    /// Get or spawn the recorder for `thread_id`. Auto-creates with a default
    /// `SessionMeta` (started_at = now) when neither a recorder nor a file
    /// exists. The default meta's `started_at` seeds the date-partitioned write
    /// path so even an auto-created session lands in the new layout.
    fn recorder_for(&self, thread_id: &str) -> Option<mpsc::UnboundedSender<RolloutCmd>> {
        // Fast path: existing recorder.
        if let Some(handle) = self.recorders.get(thread_id) {
            return Some(handle.sender());
        }
        // Slow path: insert a new recorder (Create vs Resume by file existence).
        let (params, path) = if let Some(existing) = self.lookup_path(thread_id) {
            (RolloutRecorderParams::Resume { thread_id: thread_id.to_owned() }, existing)
        } else {
            let meta = default_meta(thread_id);
            let path = self.path_for_new(thread_id, &meta.started_at);
            let entry = crate::session_index::SessionIndexEntry {
                // Keyed by the SANITIZED id (matches the file-name stem and the
                // lookup_path query); see migrate_flat_rollouts.
                thread_id: sanitize_thread_id(thread_id),
                session_id: meta.session_id.clone(),
                name: None,
                updated_at: chrono::Utc::now().to_rfc3339(),
                file_path: path.to_string_lossy().into_owned(),
            };
            crate::session_index::append_entry(&self.sessions_dir, &entry);
            (RolloutRecorderParams::Create { meta }, path)
        };
        let entry = self
            .recorders
            .entry(thread_id.to_owned())
            .or_insert_with(|| RolloutRecorderHandle::spawn(params, path));
        Some(entry.sender())
    }

    async fn flush_via_sender(&self, tx: &mpsc::UnboundedSender<RolloutCmd>) -> Result<()> {
        let (ack, rx) = oneshot::channel();
        tx.send(RolloutCmd::Persist(Some(ack))).map_err(|_| RolloutError::RecorderClosed)?;
        rx.await.map_err(|_| RolloutError::RecorderClosed)?
    }
}

fn default_meta(thread_id: &str) -> SessionMeta {
    SessionMeta {
        thread_id: thread_id.to_owned(),
        session_id: thread_id.to_owned(),
        parent_id: None,
        started_at: chrono::Utc::now().to_rfc3339(),
        config_json: serde_json::json!({}),
        rollout_version: SessionMeta::CURRENT_VERSION,
        role_name: None,
        trace_path: None,
    }
}

/// Map a thread id to a path-safe file-name segment. Any char that is not
/// ascii-alphanumeric, `-` or `_` becomes `_`. The mapping is deterministic and
/// a no-op for normal UUID-like ids. Applied inside every
/// [`RolloutFileStore`] path builder ([`RolloutFileStore::path_for_new`] +
/// [`RolloutFileStore::legacy_flat_path`] + the date-dir scan suffix) so a
/// hostile thread id cannot traverse out of `sessions_dir` and — critically —
/// the write path (computed once by the store and passed verbatim to
/// [`RolloutRecorderHandle::spawn`]) and the store's read path resolve to the
/// SAME file (no read/write split-brain). (L5)
pub(crate) fn sanitize_thread_id(thread_id: &str) -> String {
    thread_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

/// `(year, month, day)` for the date partition, derived from an RFC-3339
/// `started_at`. Falls back to the epoch (`1970-01-01`) when the timestamp
/// cannot be parsed — the file still lands at a deterministic path
/// (`sessions/1970/01/01/...`) rather than panicking.
fn date_parts_from_started_at(started_at: &str) -> (i32, u32, u32) {
    match DateTime::parse_from_rfc3339(started_at) {
        // Project to UTC BEFORE extracting the calendar fields: `parse_from_rfc3339`
        // yields a `DateTime<FixedOffset>`, and `.year()/.month()/.day()` reflect
        // the offset's LOCAL wall-clock, not the UTC instant. Formatting the local
        // fields would put the date partition on the offset's calendar day (and,
        // for `compact_ts_from_started_at`, emit a 'Z'-suffixed ts that encodes a
        // non-UTC instant), which lets two distinct UTC instants collapse to the
        // same sort key and breaks the dict-order == chronological-order invariant.
        Ok(dt) => {
            let utc = dt.with_timezone(&chrono::Utc);
            (utc.year(), utc.month(), utc.day())
        }
        Err(_) => (1970, 1, 1),
    }
}

/// Compact, fixed-width timestamp `YYYYMMDDTHHMMSSZ` for a rollout file name.
/// Dictionary order equals chronological order. Falls back to
/// `19700101T000000Z` when `started_at` cannot be parsed.
fn compact_ts_from_started_at(started_at: &str) -> String {
    match DateTime::parse_from_rfc3339(started_at) {
        // Project to UTC BEFORE formatting (see `date_parts_from_started_at`):
        // the ts string ends in 'Z' (asserting UTC), so it MUST encode the UTC
        // instant — otherwise dictionary order no longer equals chronological
        // order and the D2 watermark reverse-scan can mis-rank files.
        Ok(dt) => dt.with_timezone(&chrono::Utc).format("%Y%m%dT%H%M%SZ").to_string(),
        Err(_) => "19700101T000000Z".to_owned(),
    }
}

/// Scan the 3-level date-partitioned tree (`sessions/YYYY/MM/DD/`) for a rollout
/// file whose sanitized stem matches `sanitized`, returning the
/// lexicographically-largest (newest, since the timestamp prefix is fixed-width)
/// match. Returns `None` when no file matches or the tree is absent. A missing
/// branch (e.g. a non-date directory) is skipped, not fatal.
fn scan_date_dirs_for_thread(sessions_dir: &Path, sanitized: &str) -> Option<PathBuf> {
    let suffix = format!("-{sanitized}.jsonl");
    let mut newest: Option<PathBuf> = None;
    for year in list_dir_entries(sessions_dir) {
        if !is_all_digits(year.file_name(), 4) {
            continue;
        }
        for month in list_dir_entries(&year.path()) {
            if !is_all_digits(month.file_name(), 2) {
                continue;
            }
            for day in list_dir_entries(&month.path()) {
                if !is_all_digits(day.file_name(), 2) {
                    continue;
                }
                for file in list_dir_entries(&day.path()) {
                    let fname = file.file_name();
                    let fname = fname.to_string_lossy();
                    if fname.starts_with("rollout-") && fname.ends_with(&suffix) {
                        let p = file.path();
                        if newest.as_ref().is_none_or(|cur| p > *cur) {
                            newest = Some(p);
                        }
                    }
                }
            }
        }
    }
    newest
}

/// `read_dir(path)` flattened to a `Vec<DirEntry>` (errors dropped). Used by the
/// date-tree scan so a missing/unreadable branch is simply skipped.
fn list_dir_entries(path: &Path) -> Vec<std::fs::DirEntry> {
    std::fs::read_dir(path).map(|r| r.flatten().collect()).unwrap_or_default()
}

/// Walk the 3-level date-partitioned tree (`sessions/YYYY/MM/DD/`) and collect
/// every `rollout-*.jsonl` file. Used by [`RolloutFileStore::list_all_session_metas`]
/// (the DB-unavailable fallback surface) which folds the result by thread id,
/// keeping the newest path per thread (dict-order == chronological within the
/// tree). A missing/unreadable branch is skipped, not fatal. Legacy flat files
/// live at the top level and are handled separately by the caller.
fn scan_all_date_dirs(sessions_dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for year in list_dir_entries(sessions_dir) {
        if !is_all_digits(year.file_name(), 4) {
            continue;
        }
        for month in list_dir_entries(&year.path()) {
            if !is_all_digits(month.file_name(), 2) {
                continue;
            }
            for day in list_dir_entries(&month.path()) {
                if !is_all_digits(day.file_name(), 2) {
                    continue;
                }
                for file in list_dir_entries(&day.path()) {
                    let fname = file.file_name();
                    let fname = fname.to_string_lossy();
                    if fname.starts_with("rollout-") && fname.ends_with(".jsonl") {
                        out.push(file.path());
                    }
                }
            }
        }
    }
    out
}

/// Whether `name` is exactly `len` ASCII digits — the date-partition directory
/// filter (years are 4 digits, months/days are 2). Guards the scan against
/// descending into unrelated directories.
fn is_all_digits(name: std::ffi::OsString, len: usize) -> bool {
    let s = name.to_string_lossy();
    s.len() == len && s.bytes().all(|b| b.is_ascii_digit())
}

/// Read the first non-empty line of `path` and return its `SessionMeta` if it
/// parses as a `SessionMeta`-bearing rollout line. Used by the flat-to-date
/// migration to derive the new path from a legacy file's own header.
///
/// Reads at most the first 64 KiB (the `SessionMeta` header is always line 0 and
/// is far smaller) so a multi-gigabyte legacy rollout file does not get fully
/// buffered into memory just to parse its header — the migration runs once per
/// top-level flat file at startup, so this bounds the startup memory spike.
fn read_first_line_session_meta(path: &Path) -> Option<SessionMeta> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    const CAP: usize = 64 * 1024;
    let mut buf = Vec::with_capacity(CAP);
    file.take(CAP as u64).read_to_end(&mut buf).ok()?;
    for line_bytes in buf.split(|b| *b == b'\n') {
        if line_bytes.trim_ascii().is_empty() {
            continue;
        }
        let Ok(line): std::result::Result<RolloutLine, _> = serde_json::from_slice(line_bytes)
        else {
            continue;
        };
        if let RolloutItem::SessionMeta(meta) = line.item {
            return Some(meta);
        }
        // First non-empty line was not a SessionMeta header — not a recognizable
        // rollout file; stop (the header is always line 0).
        return None;
    }
    None
}

/// Atomically rewrite `path`, dropping every line at or beyond `from_turn`.
///
/// Reads the file as raw bytes, parses each line to decide keep/drop (preserving
/// the original bytes for kept lines), and replaces the file atomically via
/// [`replace_file_atomically`]. Unparseable lines are kept defensively.
///
/// The caller MUST ensure no write handle is open on `path` (the recorder drops
/// its handle before calling this) — otherwise the atomic replace fails on
/// Windows.
pub(crate) fn truncate_rollout_file(path: &std::path::Path, from_turn: u32) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read(path)?;
    let mut kept: Vec<Vec<u8>> = Vec::new();
    let mut running_turn: Option<u32> = None;
    for line_bytes in raw.split(|b| *b == b'\n') {
        if line_bytes.is_empty() {
            continue;
        }
        let parsed: std::result::Result<RolloutLine, _> = serde_json::from_slice(line_bytes);
        let keep = match parsed {
            Ok(line) => keep_line(&line.item, &mut running_turn, from_turn),
            Err(_) => true, // preserve unparseable lines (defensive).
        };
        if keep {
            let mut v = line_bytes.to_vec();
            v.push(b'\n');
            kept.push(v);
        }
    }
    replace_file_atomically(path, &kept)?;
    Ok(())
}

/// Atomically rewrite `path` with exactly these [`RolloutLine`]s, preserving
/// each line's original timestamp.
///
/// Each line is serialized to bytes (with a trailing newline) and the file is
/// replaced atomically via [`replace_file_atomically`]. Used by the backfill to
/// merge legacy SQL rows + post-migration rollout tail into one canonical file.
///
/// The caller MUST ensure no write handle is open on `path` (the recorder drops
/// its handle before calling this) — otherwise the atomic replace fails on
/// Windows.
pub(crate) fn rewrite_rollout_file(path: &std::path::Path, lines: &[RolloutLine]) -> Result<()> {
    let mut kept: Vec<Vec<u8>> = Vec::with_capacity(lines.len());
    for line in lines {
        let mut bytes = serde_json::to_vec(line)?;
        if !bytes.ends_with(b"\n") {
            bytes.push(b'\n');
        }
        kept.push(bytes);
    }
    replace_file_atomically(path, &kept)?;
    Ok(())
}

/// Decide whether a line should survive `truncate_from_turn(from_turn)`.
///
/// `running_turn` is updated in place as `TurnContext` / `Compacted` lines are
/// seen. `SessionMeta` is always kept (the session header survives any
/// rollback); every other variant is gated by its turn affiliation.
fn keep_line(item: &RolloutItem, running_turn: &mut Option<u32>, from_turn: u32) -> bool {
    match item {
        // The session header always survives a rollback.
        RolloutItem::SessionMeta(_) => true,
        // Compacted carries its own turn_index: keep it only if it belongs to a
        // turn strictly below the truncation point. A compaction at/after
        // from_turn is part of the rolled-back range and must be dropped —
        // otherwise read_messages would reset to a summary of deleted messages.
        RolloutItem::Compacted(payload) => {
            let t = payload.turn_index;
            *running_turn = Some(t);
            t < from_turn
        }
        RolloutItem::TurnContext(tc) => {
            *running_turn = Some(tc.turn_index());
            tc.turn_index() < from_turn
        }
        RolloutItem::TurnItem(_) | RolloutItem::EventMsg(_) => {
            // Use the most recently seen turn affiliation; when none is known yet
            // (items/events before any TurnContext), keep them defensively.
            match *running_turn {
                Some(t) => t < from_turn,
                None => true,
            }
        }
    }
}

#[async_trait]
impl RolloutStore for RolloutFileStore {
    async fn read_messages(&self, thread_id: &str) -> Vec<slab_types::ConversationMessage> {
        let _ = self.flush(thread_id).await;
        let lines = read_rollout_lines(&self.resolve_path(thread_id));
        let mut out: Vec<slab_types::ConversationMessage> = Vec::new();
        for line in lines {
            match line.item {
                // A `skipped` Compacted marker is a NO-OP: the started
                // compaction did not shrink the set, so the replayed baseline
                // must be preserved as-is (clearing it would orphan the
                // conversation). Any other status (`auto` / `compacted` /
                // `manual`) discards the prior conversation; then, if the marker
                // carries a non-empty baseline, that baseline becomes the new
                // starting point. An empty baseline means the summary is produced
                // asynchronously and the post-compaction baseline arrives in the
                // next TurnState.input_messages.
                RolloutItem::Compacted(payload) => {
                    if payload.status == "skipped" {
                        continue;
                    }
                    out.clear();
                    if !payload.compacted_messages.is_empty() {
                        out = payload.compacted_messages;
                    }
                }
                // A TurnState carries the FULL message list the model was sent this
                // turn, so it supersedes the prior history: REPLACE out with
                // input_messages when non-empty (replace-not-append is correct
                // because each TurnState rebuilds the whole list; only
                // MessageAppends after the last TurnState are true increments).
                RolloutItem::TurnContext(TurnContextPayload::TurnState {
                    input_messages, ..
                }) => {
                    if !input_messages.is_empty() {
                        out = input_messages;
                    }
                }
                RolloutItem::TurnContext(TurnContextPayload::MessageAppend { message, .. }) => {
                    out.push(message);
                }
                // SessionMeta / TurnItem / EventMsg do not contribute to the
                // LLM-visible message list. TurnItem is UI-grade; its assistant
                // text is already carried by MessageAppend/TurnState — projecting
                // it here would duplicate the assistant message.
                _ => {}
            }
        }
        out
    }

    async fn read_turn_items(&self, thread_id: &str) -> Vec<slab_agent::port::TurnItemRecord> {
        let _ = self.flush(thread_id).await;
        let lines = read_rollout_lines(&self.resolve_path(thread_id));
        let mut out = Vec::new();
        let mut current_turn: u32 = 0;
        // `seq` orders items within (thread_id, turn_index) — per-turn, matching
        // the SQL store contract. Reset to 0 whenever a TurnContext line advances
        // `current_turn` so seq restarts at 0 for each turn (F5).
        let mut seq = 0u32;
        for line in lines {
            match line.item {
                RolloutItem::TurnItem(ti) => {
                    let id = ti.id().to_owned();
                    let item_json = serde_json::to_string(&ti).unwrap_or_default();
                    out.push(slab_agent::port::TurnItemRecord {
                        id,
                        thread_id: thread_id.to_owned(),
                        turn_index: current_turn,
                        seq,
                        item_json,
                        created_at: line.timestamp,
                    });
                    seq += 1;
                }
                RolloutItem::TurnContext(tc) => {
                    let new_turn = tc.turn_index();
                    if new_turn != current_turn {
                        current_turn = new_turn;
                        seq = 0;
                    }
                }
                _ => {}
            }
        }
        out
    }

    async fn read_turn_timeline(&self, thread_id: &str) -> Vec<TurnTimelineEntry> {
        let _ = self.flush(thread_id).await;
        let lines = read_rollout_lines(&self.resolve_path(thread_id));
        let mut out: Vec<TurnTimelineEntry> = Vec::new();
        let mut current_turn: u32 = 0;
        // Item `seq` ordering within a turn (mirrors `read_turn_items`).
        let mut seq = 0u32;
        // Message record counter for synthetic ids (mirrors `replay_messages`).
        let mut msg_seq = 0u32;
        for line in lines {
            match line.item {
                RolloutItem::TurnItem(ti) => {
                    let id = ti.id().to_owned();
                    let item_json = serde_json::to_string(&ti).unwrap_or_default();
                    out.push(TurnTimelineEntry::Item(slab_agent::port::TurnItemRecord {
                        id,
                        thread_id: thread_id.to_owned(),
                        turn_index: current_turn,
                        seq,
                        item_json,
                        created_at: line.timestamp,
                    }));
                    seq += 1;
                }
                RolloutItem::TurnContext(TurnContextPayload::TurnState { turn_index, .. }) => {
                    // TurnStates contribute no timeline entries, but they carry
                    // the turn affiliation for subsequent TurnItem lines —
                    // advance the running turn exactly like `read_turn_items`.
                    if turn_index != current_turn {
                        current_turn = turn_index;
                        seq = 0;
                    }
                }
                RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                    turn_index,
                    message,
                    id,
                    created_at,
                }) => {
                    if turn_index != current_turn {
                        current_turn = turn_index;
                        seq = 0;
                    }
                    let stamped = created_at.unwrap_or_else(|| line.timestamp.clone());
                    let record_id = id.unwrap_or_else(|| format!("{thread_id}-r{msg_seq}"));
                    msg_seq += 1;
                    out.push(TurnTimelineEntry::Message(slab_agent::port::ThreadMessageRecord {
                        id: record_id,
                        thread_id: thread_id.to_owned(),
                        turn_index,
                        message,
                        created_at: stamped,
                    }));
                }
                RolloutItem::Compacted(payload) => {
                    // A skipped compaction changed nothing — no timeline
                    // entries and no running-turn advance (mirrors the
                    // `read_messages` no-op).
                    if payload.status == "skipped" {
                        continue;
                    }
                    // The compacted baseline becomes the conversation's new
                    // starting point AT this turn: advance the running turn so
                    // post-compaction items attribute to the compacted turn,
                    // and surface the baseline messages in timeline order.
                    if payload.turn_index != current_turn {
                        current_turn = payload.turn_index;
                        seq = 0;
                    }
                    for message in payload.compacted_messages {
                        let record_id = format!("{thread_id}-r{msg_seq}");
                        msg_seq += 1;
                        out.push(TurnTimelineEntry::Message(
                            slab_agent::port::ThreadMessageRecord {
                                id: record_id,
                                thread_id: thread_id.to_owned(),
                                turn_index: payload.turn_index,
                                message,
                                created_at: line.timestamp.clone(),
                            },
                        ));
                    }
                }
                // SessionMeta / EventMsg contribute no timeline entries.
                RolloutItem::SessionMeta(_) | RolloutItem::EventMsg(_) => {}
            }
        }
        out
    }

    async fn read_events(&self, thread_id: &str) -> Vec<slab_agent::protocol::EventMsg> {
        let _ = self.flush(thread_id).await;
        read_rollout_lines(&self.resolve_path(thread_id))
            .into_iter()
            .filter_map(|line| match line.item {
                RolloutItem::EventMsg(e) => Some(e),
                _ => None,
            })
            .collect()
    }

    async fn append(&self, thread_id: &str, item: RolloutItem) -> Result<()> {
        self.append_batch(thread_id, vec![item]).await
    }

    async fn append_batch(&self, thread_id: &str, items: Vec<RolloutItem>) -> Result<()> {
        // Prime the session from any SessionMeta in the batch BEFORE asking for
        // a recorder. create_session must run first so the recorder is seeded
        // with the real SessionMeta; calling recorder_for first would auto-create
        // a recorder with default_meta (session_id==thread_id), making the later
        // create_session a no-op (or_insert_with) and silently dropping the real
        // meta. (H1)
        let mut filtered = Vec::with_capacity(items.len());
        for item in items {
            match item {
                RolloutItem::SessionMeta(meta) => {
                    // The recorder writes the header itself, so SessionMeta is not
                    // emitted as a data line — it primes the session here.
                    self.create_session(meta);
                }
                other => filtered.push(other),
            }
        }
        let tx = self
            .recorder_for(thread_id)
            .ok_or_else(|| RolloutError::NoSessionMeta(thread_id.to_owned()))?;
        if filtered.is_empty() {
            return Ok(());
        }
        tx.send(RolloutCmd::AddItems(filtered)).map_err(|_| RolloutError::RecorderClosed)?;
        Ok(())
    }

    async fn flush(&self, thread_id: &str) -> Result<()> {
        let tx = match self.recorders.get(thread_id) {
            Some(handle) => handle.sender(),
            None => return Ok(()), // no recorder ⇒ nothing buffered.
        };
        self.flush_via_sender(&tx).await
    }

    async fn truncate_from_turn(&self, thread_id: &str, from_turn: u32) -> Result<()> {
        self.flush(thread_id).await?;
        // Route through the recorder actor: it owns the write handle and must
        // drop it before the on-disk atomic replace (Windows locks an open file
        // against rename). If no recorder exists, the file is untouched directly.
        let tx = match self.recorders.get(thread_id) {
            Some(handle) => handle.sender(),
            None => {
                // No live recorder — operate on the file directly.
                return truncate_rollout_file(&self.resolve_path(thread_id), from_turn);
            }
        };
        let (ack, rx) = oneshot::channel();
        tx.send(RolloutCmd::Truncate { from_turn, ack })
            .map_err(|_| RolloutError::RecorderClosed)?;
        rx.await.map_err(|_| RolloutError::RecorderClosed)?
    }

    async fn rewrite_session(&self, thread_id: &str, lines: Vec<RolloutLine>) -> Result<()> {
        self.flush(thread_id).await?;
        // Route through the recorder actor (mirrors truncate_from_turn): it owns
        // the write handle and must drop it before the on-disk atomic replace.
        // If no recorder exists, operate on the file directly.
        let tx = match self.recorders.get(thread_id) {
            Some(handle) => handle.sender(),
            None => {
                return rewrite_rollout_file(&self.resolve_path(thread_id), &lines);
            }
        };
        let (ack, rx) = oneshot::channel();
        tx.send(RolloutCmd::Rewrite { lines, ack }).map_err(|_| RolloutError::RecorderClosed)?;
        rx.await.map_err(|_| RolloutError::RecorderClosed)?
    }

    async fn file_exists(&self, thread_id: &str) -> bool {
        self.resolve_path(thread_id).exists()
    }

    async fn read_session_meta(&self, thread_id: &str) -> Option<SessionMeta> {
        let _ = self.flush(thread_id).await;
        read_rollout_lines(&self.resolve_path(thread_id)).into_iter().find_map(|line| {
            match line.item {
                RolloutItem::SessionMeta(meta) => Some(meta),
                _ => None,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_agent::protocol::{EventMsg, TurnItem};
    use slab_types::{ConversationMessage, ConversationMessageContent};

    fn store(dir: &tempfile::TempDir) -> RolloutFileStore {
        RolloutFileStore::new(dir.path().to_owned())
    }

    fn user_msg(text: &str) -> slab_types::ConversationMessage {
        ConversationMessage {
            role: "user".to_owned(),
            content: ConversationMessageContent::Text(text.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    #[tokio::test]
    async fn sanitize_applied_on_write_path_no_split_brain() {
        // Regression for L5: the recorder write path AND the store read path must
        // resolve to the SAME sanitized file. Pre-fix the recorder built its path
        // from the RAW thread_id ("a/b" -> sessions_dir/a/b.rollout.jsonl, a
        // nested subdir) while reads used the sanitized stem ("a_b") -> the read
        // returned 0 items (silent read/write split-brain), and a "../escape" id
        // traversed out of sessions_dir on write.
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(SessionMeta {
            thread_id: "a/b".to_owned(),
            session_id: "s".to_owned(),
            parent_id: None,
            started_at: "x".to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: 1,
            role_name: None,
            trace_path: None,
        });
        s.append(
            "a/b",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "m1".to_owned(),
                text: "hi".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.flush("a/b").await.unwrap();

        // Read uses resolve_path (sanitized "a_b"); the write must land in the
        // same file. lookup_path finds it via the session_index.jsonl entry
        // appended at create_session (or via the date-dir scan).
        let items = s.read_turn_items("a/b").await;
        assert_eq!(
            items.len(),
            1,
            "write/read must resolve to the same sanitized file (no split-brain)"
        );
        assert_eq!(items[0].id, "m1");

        // The on-disk file is under sessions_dir in the new date-partitioned
        // layout, with the SANITIZED stem "a_b" in the file name — NOT a nested
        // subdir created by the raw separator.
        let found = s.lookup_path("a/b").expect("file materialized under the new layout");
        let file_name = found.file_name().unwrap().to_string_lossy().into_owned();
        assert!(found.starts_with(dir.path()), "file stays under sessions_dir");
        assert!(file_name.contains("a_b"), "sanitized stem in {file_name}");
        assert!(!file_name.contains('/'), "no '/' in {file_name}");
        assert!(
            !dir.path().join("a").exists(),
            "raw separator must not create a top-level nested directory"
        );
        assert!(
            !dir.path().join("a_b.rollout.jsonl").exists(),
            "no top-level flat file (new layout is date-partitioned)"
        );
    }

    #[tokio::test]
    async fn create_then_append_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(SessionMeta {
            thread_id: "t".to_owned(),
            session_id: "s".to_owned(),
            parent_id: None,
            started_at: "x".to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: 1,
            role_name: None,
            trace_path: None,
        });

        // Turn 0: user message append + assistant turn item.
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 0,
                message: user_msg("hello"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "a1".to_owned(),
                text: "hi".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::EventMsg(EventMsg::TurnStarted(slab_agent::protocol::TurnStartedParams {
                thread_id: "t".to_owned(),
                turn: slab_agent::protocol::Turn::default(),
            })),
        )
        .await
        .unwrap();

        let messages = s.read_messages("t").await;
        // Only the MessageAppend contributes (assistant text is UI-grade).
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");

        let items = s.read_turn_items("t").await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "a1");
        assert_eq!(items[0].turn_index, 0);
        assert_eq!(items[0].seq, 0);

        let events = s.read_events("t").await;
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], EventMsg::TurnStarted(_)));

        assert!(s.read_session_meta("t").await.is_some());
        assert!(s.file_exists("t").await);
    }

    #[tokio::test]
    async fn truncate_from_turn_drops_later_turns() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t"));

        // Turn 0 + Turn 1 + Turn 2 — each a MessageAppend + a TurnItem.
        for turn in 0..3u32 {
            s.append(
                "t",
                RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                    turn_index: turn,
                    message: user_msg(&format!("t{turn}")),
                    id: None,
                    created_at: None,
                }),
            )
            .await
            .unwrap();
            s.append(
                "t",
                RolloutItem::TurnItem(TurnItem::AgentMessage {
                    id: format!("a{turn}"),
                    text: format!("r{turn}"),
                }),
            )
            .await
            .unwrap();
        }

        s.truncate_from_turn("t", 2).await.unwrap(); // drop turn 2+

        let items = s.read_turn_items("t").await;
        let ids: Vec<&str> = items.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["a0", "a1"], "turn 2 items must be dropped");

        let messages = s.read_messages("t").await;
        assert_eq!(messages.len(), 2, "turn 2 message must be dropped");

        // Session header survives.
        assert!(s.read_session_meta("t").await.is_some());
    }

    // The manual-compaction sequence HarnessService::compact_thread
    // now performs — truncate_from_turn(0) (keeps only SessionMeta) followed by a
    // single Compacted line carrying the compacted set with status="manual" —
    // must make read_messages return EXACTLY the compacted set, leave the file
    // holding only [SessionMeta, Compacted], and let the next append extend the
    // new baseline. Re-inserting the compacted messages as MessageAppend lines
    // would duplicate them on read; the Compacted line carries them.
    #[tokio::test]
    async fn manual_compact_sequence_truncate_then_compacted_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t"));
        // Several turns of history (messages + items + states).
        for turn in 0..3u32 {
            s.append(
                "t",
                RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                    turn_index: turn,
                    message: user_msg(&format!("old{turn}")),
                    id: None,
                    created_at: None,
                }),
            )
            .await
            .unwrap();
            s.append(
                "t",
                RolloutItem::TurnItem(TurnItem::AgentMessage {
                    id: format!("a{turn}"),
                    text: format!("r{turn}"),
                }),
            )
            .await
            .unwrap();
        }

        // Harness compact_thread step 1: truncate(0) keeps only SessionMeta.
        s.truncate_from_turn("t", 0).await.unwrap();
        // Step 2: the Compacted line carries the compacted baseline.
        s.append(
            "t",
            RolloutItem::Compacted(crate::item::CompactedPayload {
                thread_id: "t".to_owned(),
                compacted_messages: vec![user_msg("summary")],
                removed_messages: 6,
                output_tokens: 11,
                status: "manual".to_owned(),
                turn_index: 0,
            }),
        )
        .await
        .unwrap();

        // read_messages returns exactly the compacted set (not the old messages).
        let texts: Vec<String> = s
            .read_messages("t")
            .await
            .iter()
            .map(|m| match &m.content {
                ConversationMessageContent::Text(t) => t.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(texts, vec!["summary".to_owned()], "compacted baseline replaces history");

        // The file holds exactly [SessionMeta, Compacted] — no old items/messages.
        let lines = read_rollout_lines(&s.resolve_path("t"));
        assert_eq!(lines.len(), 2, "only the header + the Compacted line survive");
        assert!(matches!(lines[0].item, RolloutItem::SessionMeta(_)));
        assert!(matches!(lines[1].item, RolloutItem::Compacted(_)));

        // The next append (a new turn) extends the compacted baseline.
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 1,
                message: user_msg("after"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        let texts: Vec<String> = s
            .read_messages("t")
            .await
            .iter()
            .map(|m| match &m.content {
                ConversationMessageContent::Text(t) => t.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(
            texts,
            vec!["summary".to_owned(), "after".to_owned()],
            "new turn extends the compacted baseline"
        );
    }

    // Fork wholesale-copy mechanism. HarnessService::fork_thread
    // rebuilds the child rollout from the parent's lines in their original
    // interleaved order (preserving the child's SessionMeta), because
    // control.fork_thread's per-row adapter copy batches all TurnContext lines
    // before all TurnItem lines and breaks read_turn_items' running-turn
    // attribution. Verify the wholesale copy preserves attribution + messages.
    #[tokio::test]
    async fn fork_wholesale_copy_preserves_turn_attribution() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t-parent"));
        // Parent history in the production interleaved order: per turn, the
        // MessageAppend (user) lands before the TurnItem (assistant reply).
        for turn in 0..2u32 {
            s.append(
                "t-parent",
                RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                    turn_index: turn,
                    message: user_msg(&format!("u{turn}")),
                    id: None,
                    created_at: None,
                }),
            )
            .await
            .unwrap();
            s.append(
                "t-parent",
                RolloutItem::TurnItem(TurnItem::AgentMessage {
                    id: format!("a{turn}"),
                    text: format!("r{turn}"),
                }),
            )
            .await
            .unwrap();
        }

        // Simulate the harness wholesale copy: the child SessionMeta (carrying
        // parent_id) is preserved, then every parent data line is appended in
        // order. rewrite_session atomically replaces the child file. (In
        // production the child SessionMeta line is read from the file that
        // control.fork_thread's upsert_thread materialized; here it is built
        // directly since the test does not run control.fork_thread.)
        s.flush("t-parent").await.unwrap();
        let parent_lines = read_rollout_lines(&s.resolve_path("t-parent"));
        let child_meta = SessionMeta {
            thread_id: "t-child".to_owned(),
            session_id: "s".to_owned(),
            parent_id: Some("t-parent".to_owned()),
            started_at: "x".to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: None,
            trace_path: None,
        };
        let child_meta_line = RolloutLine::now(RolloutItem::SessionMeta(child_meta));
        let mut rebuilt = Vec::with_capacity(parent_lines.len() + 1);
        rebuilt.push(child_meta_line);
        rebuilt.extend(
            parent_lines.into_iter().filter(|l| !matches!(l.item, RolloutItem::SessionMeta(_))),
        );
        s.rewrite_session("t-child", rebuilt).await.unwrap();

        // The child replays the parent's items with CORRECT turn attribution
        // (the per-row batched copy would have misattributed a0 to turn 1).
        let child_items = s.read_turn_items("t-child").await;
        let child_item_ids: Vec<(u32, &str)> =
            child_items.iter().map(|i| (i.turn_index, i.id.as_str())).collect();
        assert_eq!(
            child_item_ids,
            vec![(0, "a0"), (1, "a1")],
            "wholesale copy preserves turn attribution"
        );

        // The child replays the same messages as the parent.
        let child_texts: Vec<String> = s
            .read_messages("t-child")
            .await
            .iter()
            .map(|m| match &m.content {
                ConversationMessageContent::Text(t) => t.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(
            child_texts,
            vec!["u0".to_owned(), "u1".to_owned()],
            "child replays parent messages"
        );

        // Fork provenance + parent independence.
        let child_meta_back = s.read_session_meta("t-child").await.expect("child meta");
        assert_eq!(child_meta_back.parent_id, Some("t-parent".to_owned()));
        assert_eq!(s.read_turn_items("t-parent").await.len(), 2, "parent untouched");
    }

    #[tokio::test]
    async fn compacted_resets_message_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t"));

        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 0,
                message: user_msg("old1"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 0,
                message: user_msg("old2"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        // Compaction replaces the baseline.
        s.append(
            "t",
            RolloutItem::Compacted(crate::item::CompactedPayload {
                thread_id: "t".to_owned(),
                compacted_messages: vec![user_msg("summary")],
                removed_messages: 2,
                output_tokens: 5,
                status: "auto".to_owned(),
                turn_index: 1,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 1,
                message: user_msg("after"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();

        let messages = s.read_messages("t").await;
        let texts: Vec<String> = messages
            .iter()
            .map(|m| match &m.content {
                ConversationMessageContent::Text(t) => t.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(texts, vec!["summary".to_owned(), "after".to_owned()]);
    }

    #[tokio::test]
    async fn dashmap_guards_single_recorder_per_thread() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t"));

        // Two concurrent appends share the recorder → only one header line.
        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        let h1 = {
            let sender = s.recorders.get("t").unwrap().sender();
            tokio::spawn(async move {
                sender
                    .send(RolloutCmd::AddItems(vec![RolloutItem::TurnItem(
                        TurnItem::AgentMessage { id: "x".to_owned(), text: "1".to_owned() },
                    )]))
                    .unwrap();
                let (ack, wait) = oneshot::channel();
                sender.send(RolloutCmd::Persist(Some(ack))).unwrap();
                wait.await.unwrap().unwrap();
                let _ = tx1.send(());
            })
        };
        let h2 = {
            let sender = s.recorders.get("t").unwrap().sender();
            tokio::spawn(async move {
                sender
                    .send(RolloutCmd::AddItems(vec![RolloutItem::TurnItem(
                        TurnItem::AgentMessage { id: "y".to_owned(), text: "2".to_owned() },
                    )]))
                    .unwrap();
                let (ack, wait) = oneshot::channel();
                sender.send(RolloutCmd::Persist(Some(ack))).unwrap();
                wait.await.unwrap().unwrap();
                let _ = tx2.send(());
            })
        };
        let _ = h1.await;
        let _ = h2.await;
        rx1.await.unwrap();
        rx2.await.unwrap();

        let lines = read_rollout_lines(&s.resolve_path("t"));
        let header_count =
            lines.iter().filter(|l| matches!(l.item, RolloutItem::SessionMeta(_))).count();
        assert_eq!(header_count, 1, "only one SessionMeta header expected");
        assert_eq!(lines.len(), 3, "header + two items");
    }

    #[tokio::test]
    async fn read_flushes_pending_first() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t"));
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "z".to_owned(),
                text: "y".to_owned(),
            }),
        )
        .await
        .unwrap();
        // No explicit flush — read_* must still see the just-appended item.
        let items = s.read_turn_items("t").await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "z");
    }

    #[tokio::test]
    async fn missing_thread_reads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        assert!(s.read_messages("ghost").await.is_empty());
        assert!(s.read_turn_items("ghost").await.is_empty());
        assert!(s.read_events("ghost").await.is_empty());
        assert!(s.read_session_meta("ghost").await.is_none());
        assert!(!s.file_exists("ghost").await);
    }

    // --- Regression tests (W1 review findings) ---

    // H1: append_batch must prime the session from the REAL SessionMeta, not a
    // default_meta auto-created by recorder_for.
    #[tokio::test]
    async fn append_batch_primes_real_session_meta() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        let real = SessionMeta {
            thread_id: "t".to_owned(),
            session_id: "REAL".to_owned(),
            parent_id: None,
            started_at: "x".to_owned(),
            config_json: serde_json::json!({"model": "m"}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: Some("rr".to_owned()),
            trace_path: None,
        };
        s.append_batch(
            "t",
            vec![
                RolloutItem::SessionMeta(real),
                RolloutItem::TurnItem(TurnItem::AgentMessage {
                    id: "a".to_owned(),
                    text: "hi".to_owned(),
                }),
            ],
        )
        .await
        .unwrap();
        s.flush("t").await.unwrap();

        let meta = s.read_session_meta("t").await.expect("meta present");
        assert_eq!(meta.session_id, "REAL", "real session_id, not default");
        assert_eq!(meta.role_name, Some("rr".to_owned()));
        assert_eq!(meta.config_json, serde_json::json!({"model": "m"}));
        assert_ne!(meta.session_id, "t", "must not be the default session_id");
    }

    // H2 (a): an empty-baseline Compacted (async summary) followed by a TurnState
    // carrying the post-compaction baseline must yield exactly the TurnState
    // input — the pre-compaction history must NOT replay.
    #[tokio::test]
    async fn empty_compacted_then_turn_state_replaces_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t"));

        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 0,
                message: user_msg("old"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        // Auto-compact with an EMPTY baseline (summary arrives async).
        s.append(
            "t",
            RolloutItem::Compacted(crate::item::CompactedPayload {
                thread_id: "t".to_owned(),
                compacted_messages: vec![],
                removed_messages: 1,
                output_tokens: 0,
                status: "auto".to_owned(),
                turn_index: 1,
            }),
        )
        .await
        .unwrap();
        // The post-compaction baseline rides in the next TurnState.
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::TurnState {
                turn_index: 1,
                status: "ok".to_owned(),
                input_messages: vec![user_msg("summary")],
                tool_specs_json: None,
                llm_response_json: None,
                error: None,
                completed_at: None,
                started_at: None,
                input_messages_raw: None,
            }),
        )
        .await
        .unwrap();

        let messages = s.read_messages("t").await;
        let texts: Vec<String> = messages
            .iter()
            .map(|m| match &m.content {
                ConversationMessageContent::Text(t) => t.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(texts, vec!["summary".to_owned()], "old must not reappear");
    }

    // H2 (b): a TurnState supersedes prior history — [TurnState([A,B]),
    // MessageAppend(C), TurnState([A,B,C,D])] replays as [A,B,C,D].
    #[tokio::test]
    async fn turn_state_replaces_then_appends() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t"));
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::TurnState {
                turn_index: 0,
                status: "ok".to_owned(),
                input_messages: vec![user_msg("A"), user_msg("B")],
                tool_specs_json: None,
                llm_response_json: None,
                error: None,
                completed_at: None,
                started_at: None,
                input_messages_raw: None,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 1,
                message: user_msg("C"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::TurnState {
                turn_index: 1,
                status: "ok".to_owned(),
                input_messages: vec![user_msg("A"), user_msg("B"), user_msg("C"), user_msg("D")],
                tool_specs_json: None,
                llm_response_json: None,
                error: None,
                completed_at: None,
                started_at: None,
                input_messages_raw: None,
            }),
        )
        .await
        .unwrap();
        let texts: Vec<String> = s
            .read_messages("t")
            .await
            .iter()
            .map(|m| match &m.content {
                ConversationMessageContent::Text(t) => t.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(texts, vec!["A".to_owned(), "B".to_owned(), "C".to_owned(), "D".to_owned()]);
    }

    // H3: rollback-after-compaction. A Compacted marker belonging to a rolled-back
    // turn must itself be dropped, otherwise read_messages resets to a summary of
    // deleted messages instead of returning the pre-compaction [A].
    #[tokio::test]
    async fn truncate_after_compaction_does_not_reset_to_summary() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t"));

        // turn 0: A, turn 1: B, compaction (turn 1, baseline=[sumAB]), turn 2: C
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 0,
                message: user_msg("A"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "a0".to_owned(),
                text: "ra".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 1,
                message: user_msg("B"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "a1".to_owned(),
                text: "rb".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::Compacted(crate::item::CompactedPayload {
                thread_id: "t".to_owned(),
                compacted_messages: vec![user_msg("sumAB")],
                removed_messages: 2,
                output_tokens: 5,
                status: "auto".to_owned(),
                turn_index: 1,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 2,
                message: user_msg("C"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "a2".to_owned(),
                text: "rc".to_owned(),
            }),
        )
        .await
        .unwrap();

        // Roll back to turn 0 (drop turn 1+).
        s.truncate_from_turn("t", 1).await.unwrap();

        let texts: Vec<String> = s
            .read_messages("t")
            .await
            .iter()
            .map(|m| match &m.content {
                ConversationMessageContent::Text(t) => t.clone(),
                _ => String::new(),
            })
            .collect();
        // Only A survives — the compaction (turn 1) is dropped so no reset to sumAB.
        assert_eq!(texts, vec!["A".to_owned()], "no reset to summary of deleted msgs");

        let items = s.read_turn_items("t").await;
        let ids: Vec<&str> = items.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["a0"], "only turn-0 item survives");
    }

    // F5: read_turn_items resets `seq` per turn — seq is ordering within
    // (thread_id, turn_index), not global. Two items in turn 0 are seq 0,1; the
    // first item in turn 1 is seq 0 again.
    #[tokio::test]
    async fn read_turn_items_resets_seq_per_turn() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t"));
        // Turn 0: stamp the turn via a MessageAppend, then two items.
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 0,
                message: user_msg("t0"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "a0a".to_owned(),
                text: "x".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "a0b".to_owned(),
                text: "y".to_owned(),
            }),
        )
        .await
        .unwrap();
        // Turn 1: stamp + one item.
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 1,
                message: user_msg("t1"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "a1a".to_owned(),
                text: "z".to_owned(),
            }),
        )
        .await
        .unwrap();

        let items = s.read_turn_items("t").await;
        let seqs: Vec<(u32, u32, &str)> =
            items.iter().map(|r| (r.turn_index, r.seq, r.id.as_str())).collect();
        assert_eq!(
            seqs,
            vec![
                (0, 0, "a0a"),
                (0, 1, "a0b"),
                (1, 0, "a1a"), // seq restarted at 0 for turn 1
            ],
            "seq must reset per turn"
        );
    }

    // F7: a `skipped` Compacted marker (a started compaction that did not shrink)
    // must NOT clear the replayed baseline — it is a no-op. Any other status
    // (`auto`/`compacted`/`manual`) clears + adopts the new baseline.
    #[tokio::test]
    async fn skipped_compacted_marker_is_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t"));

        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 0,
                message: user_msg("keep1"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 0,
                message: user_msg("keep2"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        // A SKIPPED compaction (no shrink) — must leave the baseline intact.
        s.append(
            "t",
            RolloutItem::Compacted(crate::item::CompactedPayload {
                thread_id: "t".to_owned(),
                compacted_messages: vec![],
                removed_messages: 0,
                output_tokens: 0,
                status: "skipped".to_owned(),
                turn_index: 1,
            }),
        )
        .await
        .unwrap();

        let messages = s.read_messages("t").await;
        let texts: Vec<String> = messages
            .iter()
            .map(|m| match &m.content {
                ConversationMessageContent::Text(t) => t.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(
            texts,
            vec!["keep1".to_owned(), "keep2".to_owned()],
            "skipped compaction must not clear the baseline"
        );
    }

    // M2: calling create_session a second time on a thread whose file already
    // exists must NOT write a second SessionMeta header.
    #[tokio::test]
    async fn create_session_twice_writes_single_header() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t"));
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "a".to_owned(),
                text: "x".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.flush("t").await.unwrap();
        // One header so far.
        let before = read_rollout_lines(&s.resolve_path("t"))
            .iter()
            .filter(|l| matches!(l.item, RolloutItem::SessionMeta(_)))
            .count();
        assert_eq!(before, 1);

        // Re-send meta for the existing thread (different role, must be ignored
        // for header purposes — the file already exists).
        s.create_session(SessionMeta {
            thread_id: "t".to_owned(),
            session_id: "REAL2".to_owned(),
            parent_id: None,
            started_at: "x".to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: Some("rr".to_owned()),
            trace_path: None,
        });
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "b".to_owned(),
                text: "y".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.flush("t").await.unwrap();

        let header_count = read_rollout_lines(&s.resolve_path("t"))
            .iter()
            .filter(|l| matches!(l.item, RolloutItem::SessionMeta(_)))
            .count();
        assert_eq!(header_count, 1, "exactly one SessionMeta header, not two");
    }

    // L5: a hostile thread id cannot traverse out of sessions_dir. Both the
    // write path (path_for_new) and the read path (resolve_path's flat fallback)
    // must sanitize the thread id to a single path-safe segment.
    #[test]
    fn path_for_new_sanitizes_traversal_thread_id() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        let started = "2026-08-03T00:00:00Z";
        let p = s.path_for_new("../escape", started);
        let parent = dir.path();
        assert!(
            p.starts_with(parent),
            "sanitized path must stay under sessions_dir: {}",
            p.display()
        );
        // The sanitized segment contains no path separators or dots.
        let file_name = p.file_name().unwrap().to_string_lossy().to_string();
        assert!(file_name.starts_with("rollout-"), "date-partitioned file name: {file_name}");
        assert!(!file_name.contains('/'), "no '/' in {file_name}");
        assert!(!file_name.contains('\\'), "no '\\' in {file_name}");
        assert!(
            file_name.ends_with("-___escape.jsonl"),
            "traversal chars collapsed to '_' (sanitized): {file_name}"
        );
        // The date partition is honored (2026/08/03 from the rfc3339 started_at).
        assert!(p.starts_with(parent.join("2026").join("08").join("03")), "date partition honored");

        // The legacy flat read fallback sanitizes too (and stays under sessions_dir).
        let flat = s.resolve_path("../escape");
        assert!(flat.starts_with(parent));
        let flat_name = flat.file_name().unwrap().to_string_lossy().to_string();
        assert!(!flat_name.contains('/'));
        assert!(!flat_name.contains('\\'));
        assert!(!flat_name.contains(".."));

        // A normal UUID-like id is a no-op on the stem.
        let normal = s.path_for_new("01234567-89ab-cdef", started);
        assert!(normal.starts_with(parent));
        assert_eq!(
            normal.file_name().unwrap().to_string_lossy(),
            "rollout-20260803T000000Z-01234567-89ab-cdef.jsonl"
        );
    }

    // G1.1: rewrite_session atomically replaces the rollout file with exactly
    // the given lines (preserving each line timestamp), and an append queued
    // AFTER the rewrite lands after the rewritten content (the next write
    // reopens the file lazily in append mode).
    #[tokio::test]
    async fn rewrite_session_replaces_lines_and_appends_after() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t"));
        // Initial content: a turn-0 user message + a turn item.
        s.append(
            "t",
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index: 0,
                message: user_msg("old"),
                id: None,
                created_at: None,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "a-old".to_owned(),
                text: "old-reply".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.flush("t").await.unwrap();

        // Rewrite with a DIFFERENT line set: keep the SessionMeta header, drop
        // the old lines, and write a new turn-2 message (timestamps preserved).
        let meta_line = read_rollout_lines(&s.resolve_path("t"))
            .into_iter()
            .find(|l| matches!(l.item, RolloutItem::SessionMeta(_)))
            .expect("header present");
        let new_lines = vec![
            meta_line,
            RolloutLine::with_timestamp(
                "2026-09-09T00:00:00Z",
                RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                    turn_index: 2,
                    message: user_msg("rewritten"),
                    id: Some("m-rw".to_owned()),
                    created_at: Some("2026-09-09T00:00:00Z".to_owned()),
                }),
            ),
        ];
        s.rewrite_session("t", new_lines).await.unwrap();

        // The file now contains EXACTLY the new set (header + one turn-2 line).
        let after = read_rollout_lines(&s.resolve_path("t"));
        assert_eq!(after.len(), 2, "rewrite replaced the whole file");
        assert!(matches!(after[0].item, RolloutItem::SessionMeta(_)));
        // The old content is gone.
        assert!(
            !after.iter().any(|l| matches!(
                &l.item,
                RolloutItem::TurnItem(TurnItem::AgentMessage { id, .. }) if id == "a-old"
            )),
            "old turn item dropped by rewrite"
        );
        // The new turn-2 message survived with its preserved timestamp.
        let has_new = after.iter().any(|l| {
            matches!(
                &l.item,
                RolloutItem::TurnContext(TurnContextPayload::MessageAppend { turn_index: 2, .. })
            )
        });
        assert!(has_new, "rewritten line present");

        // Concurrency: an append QUEUED AFTER the rewrite lands after the
        // rewritten content (the next write reopens the file in append mode).
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "a-after".to_owned(),
                text: "after".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.flush("t").await.unwrap();

        let final_lines = read_rollout_lines(&s.resolve_path("t"));
        assert_eq!(final_lines.len(), 3, "post-rewrite append landed");
        // Order: header, rewritten line, then the appended item.
        let ids: Vec<&str> = final_lines
            .iter()
            .filter_map(|l| match &l.item {
                RolloutItem::TurnItem(TurnItem::AgentMessage { id, .. }) => Some(id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec!["a-after"], "append landed after rewritten content");
    }

    // G1.1 (direct path): rewrite_session operates on the file directly when no
    // recorder exists. The file is seeded via direct fs writes (no recorder ever
    // holds a handle), then a fresh store with an empty recorder map rewrites
    // it directly.
    #[tokio::test]
    async fn rewrite_session_direct_path_without_recorder() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        // Seed a LEGACY FLAT file directly (no recorder ever holds a handle).
        // resolve_path's flat fallback finds it, so rewrite_session operates on
        // it directly.
        let path = dir.path().join("ghost.rollout.jsonl");
        let seed = RolloutLine::with_timestamp(
            "2026-08-02T00:00:00Z",
            RolloutItem::SessionMeta(default_meta("ghost")),
        );
        std::fs::write(&path, serde_json::to_vec(&seed).unwrap()).unwrap();
        assert_eq!(read_rollout_lines(&path).len(), 1);

        // rewrite_session with no recorder in the map → direct file path.
        let new_lines = vec![RolloutLine::with_timestamp(
            "2026-09-09T00:00:00Z",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "only".to_owned(),
                text: "rewritten".to_owned(),
            }),
        )];
        s.rewrite_session("ghost", new_lines).await.unwrap();
        let after = read_rollout_lines(&path);
        assert_eq!(after.len(), 1, "direct rewrite replaced the file");
        assert!(matches!(after[0].item, RolloutItem::TurnItem(_)));
    }

    // ── date-partitioned path layout + reverse lookup + migration ─
    //
    // These tests exercise the REAL file system (create_session writes a real
    // date-partitioned file → read reads it back; a synthesized legacy flat file
    // → migration moves it → read still resolves). No mocks: a false-green here
    // (e.g. resolve_path silently falling back to flat when the date file is the
    // truth) would hide a real production read/write split-brain.

    fn started(year: i32, month: u32, day: u32, sec: u32) -> String {
        use chrono::TimeZone;
        chrono::Utc.with_ymd_and_hms(year, month, day, 0, 0, sec).unwrap().to_rfc3339()
    }

    fn meta_with(thread_id: &str, started_at: &str) -> SessionMeta {
        SessionMeta {
            thread_id: thread_id.to_owned(),
            session_id: "s".to_owned(),
            parent_id: None,
            started_at: started_at.to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: None,
            trace_path: None,
        }
    }

    // create_session writes to the date-partitioned path derived from
    // SessionMeta.started_at: sessions/YYYY/MM/DD/rollout-<ts>-<tid>.jsonl.
    #[tokio::test]
    async fn create_session_writes_date_partitioned_path() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        let started_at = started(2026, 8, 3, 12);
        s.create_session(meta_with("t-partition", &started_at));
        s.append(
            "t-partition",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "m1".to_owned(),
                text: "hi".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.flush("t-partition").await.unwrap();

        // The file is at the expected date-partitioned path. The thread id
        // "t-partition" keeps its hyphen (sanitize preserves `-`).
        let expected = dir
            .path()
            .join("2026")
            .join("08")
            .join("03")
            .join("rollout-20260803T000012Z-t-partition.jsonl");
        assert!(expected.exists(), "file at date-partitioned path: {}", expected.display());

        // lookup_path / resolve_path find it.
        assert_eq!(s.lookup_path("t-partition").as_deref(), Some(expected.as_path()));
        assert_eq!(s.resolve_path("t-partition"), expected);

        // Round-trip read works.
        let items = s.read_turn_items("t-partition").await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "m1");

        // No top-level flat file was created.
        assert!(!dir.path().join("t-partition.rollout.jsonl").exists());
    }

    // resolve_path reverse-lookup chain — each link exercised in isolation:
    // (a) session_index.jsonl hit; (b) date-dir scan hit (no index); (c) legacy
    // flat fallback. A SECOND store over the SAME dir resolves the same file
    // (the index + scan survive across instances — important across restarts).
    #[tokio::test]
    async fn resolve_path_reverse_chain_index_scan_and_flat_fallback() {
        let dir = tempfile::tempdir().unwrap();

        // (a) Date-partitioned file created via create_session lands an
        // session_index.jsonl entry; a FRESH store instance resolves it through
        // the index (no in-memory recorder state shared).
        let s = store(&dir);
        let started_at = started(2026, 8, 3, 0);
        s.create_session(meta_with("t-index", &started_at));
        s.append(
            "t-index",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "ix".to_owned(),
                text: "x".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.flush("t-index").await.unwrap();
        let indexed_path = s.lookup_path("t-index").expect("indexed lookup");

        // (b) A date-partitioned file with NO index entry is found by the scan.
        // Drop a file directly into the date tree, bypassing create_session so
        // no index entry is appended.
        let scan_dir = dir.path().join("2026").join("08").join("04");
        std::fs::create_dir_all(&scan_dir).unwrap();
        let scan_path = scan_dir.join("rollout-20260804T000000Z-t-scan.jsonl");
        let scan_seed = RolloutLine::now(RolloutItem::SessionMeta(meta_with(
            "t-scan",
            &started(2026, 8, 4, 0),
        )));
        std::fs::write(&scan_path, serde_json::to_vec(&scan_seed).unwrap()).unwrap();

        // (c) A legacy flat file (pre-migration) is found by the flat fallback.
        let flat_path = dir.path().join("t-flat.rollout.jsonl");
        let flat_seed = RolloutLine::now(RolloutItem::SessionMeta(meta_with(
            "t-flat",
            &started(2026, 1, 1, 0),
        )));
        std::fs::write(&flat_path, serde_json::to_vec(&flat_seed).unwrap()).unwrap();

        // A FRESH store (no shared recorder map) must resolve all three purely
        // from the on-disk reverse chain.
        let fresh = store(&dir);
        assert_eq!(fresh.lookup_path("t-index").as_deref(), Some(indexed_path.as_path()));
        assert_eq!(fresh.lookup_path("t-scan").as_deref(), Some(scan_path.as_path()));
        assert_eq!(fresh.lookup_path("t-flat").as_deref(), Some(flat_path.as_path()));
        // A thread with no file anywhere resolves to the (non-existent) flat
        // fallback, so reads return empty rather than panicking.
        let ghost = fresh.resolve_path("ghost");
        assert!(!ghost.exists());
        assert!(fresh.read_turn_items("ghost").await.is_empty());
    }

    // The flat fallback is the LAST chain link — if a date-partitioned file AND
    // a flat file BOTH exist for the same thread (e.g. migration left a stale
    // flat duplicate), the date-partitioned (scan) result wins over flat. This
    // pins the chain order: index > scan > flat.
    #[tokio::test]
    async fn resolve_path_prefers_date_partitioned_over_flat() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);

        // Create the date-partitioned file (writes index entry too).
        s.create_session(meta_with("t-dup", &started(2026, 8, 3, 0)));
        s.append(
            "t-dup",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "new".to_owned(),
                text: "from-date".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.flush("t-dup").await.unwrap();
        let date_path = s.lookup_path("t-dup").expect("date file exists");

        // Synthesize a STALE flat duplicate at the top level.
        let flat_path = dir.path().join("t-dup.rollout.jsonl");
        let flat_seed =
            RolloutLine::now(RolloutItem::SessionMeta(meta_with("t-dup", &started(2020, 1, 1, 0))));
        std::fs::write(&flat_path, serde_json::to_vec(&flat_seed).unwrap()).unwrap();

        // resolve_path returns the DATE-partitioned file (not the flat dup).
        assert_eq!(s.resolve_path("t-dup"), date_path);
        // The read returns the date-file item ("new"), not the flat dup.
        let items = s.read_turn_items("t-dup").await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "new");
    }

    // Startup migration moves a top-level flat file into the date-partitioned
    // layout, appends a session_index.jsonl entry, and is IDEMPOTENT (a second
    // run finds no flat files and is a no-op). The reverse-lookup finds the
    // file at its NEW location after the move, and the read still returns the
    // original content.
    #[tokio::test]
    async fn migrate_flat_rollouts_moves_files_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);

        // Synthesize TWO legacy flat files (top-level *.rollout.jsonl). Each
        // line is written with a trailing newline (the real recorder always
        // appends `\n` per line), so read_first_line_session_meta can parse the
        // header in isolation.
        let flat_a = dir.path().join("t-legacy-a.rollout.jsonl");
        let meta_a = meta_with("t-legacy-a", &started(2026, 7, 15, 30));
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&flat_a).unwrap();
            let header = RolloutLine::now(RolloutItem::SessionMeta(meta_a.clone()));
            writeln!(f, "{}", serde_json::to_string(&header).unwrap()).unwrap();
            // Append a TurnItem so the file has real content to round-trip.
            let item = RolloutLine::now(RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "la".to_owned(),
                text: "legacy-a".to_owned(),
            }));
            writeln!(f, "{}", serde_json::to_string(&item).unwrap()).unwrap();
        }

        let flat_b = dir.path().join("t-legacy-b.rollout.jsonl");
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&flat_b).unwrap();
            let header = RolloutLine::now(RolloutItem::SessionMeta(meta_with(
                "t-legacy-b",
                &started(2026, 8, 1, 0),
            )));
            writeln!(f, "{}", serde_json::to_string(&header).unwrap()).unwrap();
        }

        assert!(flat_a.exists());
        assert!(flat_b.exists());

        // Run migration.
        let migrated = s.migrate_flat_rollouts();
        assert_eq!(migrated, 2, "both flat files migrated");

        // The flat files are gone; the date-partitioned files exist.
        assert!(!flat_a.exists(), "flat a moved away");
        assert!(!flat_b.exists(), "flat b moved away");
        let new_a = dir
            .path()
            .join("2026")
            .join("07")
            .join("15")
            .join("rollout-20260715T000030Z-t-legacy-a.jsonl");
        let new_b = dir
            .path()
            .join("2026")
            .join("08")
            .join("01")
            .join("rollout-20260801T000000Z-t-legacy-b.jsonl");
        assert!(new_a.exists(), "a moved to date partition: {}", new_a.display());
        assert!(new_b.exists(), "b moved to date partition: {}", new_b.display());

        // The reverse lookup finds them at the new locations.
        assert_eq!(s.lookup_path("t-legacy-a").as_deref(), Some(new_a.as_path()));
        assert_eq!(s.lookup_path("t-legacy-b").as_deref(), Some(new_b.as_path()));

        // The content round-trips through resolve_path.
        let items = s.read_turn_items("t-legacy-a").await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "la");

        // Idempotent: a second run finds no flat files and is a no-op.
        let migrated_again = s.migrate_flat_rollouts();
        assert_eq!(migrated_again, 0, "second migration is a no-op");
        assert!(new_a.exists());
        assert!(new_b.exists());

        // A FRESH store (simulating a restart) resolves the migrated files via
        // the session_index.jsonl entries appended by the migration.
        let fresh = store(&dir);
        assert_eq!(fresh.lookup_path("t-legacy-a").as_deref(), Some(new_a.as_path()));
    }

    // Crash-safety / partial-progress idempotency: if migration moved file A but
    // crashed before moving file B, a re-run moves only B (A is already gone
    // from the flat location). No file is moved twice; no content is lost.
    #[tokio::test]
    async fn migrate_flat_rollouts_partial_progress_resumes() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);

        let flat_a = dir.path().join("t-crash-a.rollout.jsonl");
        let flat_b = dir.path().join("t-crash-b.rollout.jsonl");
        std::fs::write(
            &flat_a,
            serde_json::to_vec(&RolloutLine::now(RolloutItem::SessionMeta(meta_with(
                "t-crash-a",
                &started(2026, 6, 1, 0),
            ))))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            &flat_b,
            serde_json::to_vec(&RolloutLine::now(RolloutItem::SessionMeta(meta_with(
                "t-crash-b",
                &started(2026, 6, 2, 0),
            ))))
            .unwrap(),
        )
        .unwrap();

        // Simulate "A already migrated, B still flat": move A manually (as if a
        // prior run did one then crashed).
        let new_a = dir
            .path()
            .join("2026")
            .join("06")
            .join("01")
            .join("rollout-20260601T000000Z-t-crash-a.jsonl");
        std::fs::create_dir_all(new_a.parent().unwrap()).unwrap();
        std::fs::rename(&flat_a, &new_a).unwrap();
        assert!(!flat_a.exists());
        assert!(flat_b.exists());

        // Re-run migration: only B is moved (A is not at the flat path anymore).
        let migrated = s.migrate_flat_rollouts();
        assert_eq!(migrated, 1, "only the remaining flat file moved");
        assert!(new_a.exists(), "A still at its migrated location");
        let new_b = dir
            .path()
            .join("2026")
            .join("06")
            .join("02")
            .join("rollout-20260602T000000Z-t-crash-b.jsonl");
        assert!(new_b.exists(), "B moved");
        assert!(!flat_b.exists());

        // Both resolve.
        assert!(s.lookup_path("t-crash-a").is_some());
        assert!(s.lookup_path("t-crash-b").is_some());
    }

    // Read compatibility for an UN-migrated flat file (migration skipped or not
    // yet run): resolve_path's flat fallback finds it and the read returns the
    // real content. This is the safety net that makes migration best-effort.
    #[tokio::test]
    async fn read_falls_back_to_legacy_flat_when_not_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);

        // A flat file the migration never touched.
        let flat = dir.path().join("t-unmigrated.rollout.jsonl");
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&flat).unwrap();
            let seed = RolloutLine::now(RolloutItem::SessionMeta(meta_with(
                "t-unmigrated",
                &started(2025, 12, 25, 0),
            )));
            writeln!(f, "{}", serde_json::to_string(&seed).unwrap()).unwrap();
            let item = RolloutLine::now(RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "um".to_owned(),
                text: "unmigrated-content".to_owned(),
            }));
            writeln!(f, "{}", serde_json::to_string(&item).unwrap()).unwrap();
        }

        // resolve_path's flat fallback finds it; the read returns the content.
        assert_eq!(s.resolve_path("t-unmigrated"), flat);
        assert!(s.file_exists("t-unmigrated").await);
        let items = s.read_turn_items("t-unmigrated").await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "um");
        assert!(s.read_session_meta("t-unmigrated").await.is_some());
    }

    // ts dictionary order = chronological order: files for different threads
    // created at different started_at values sort by path the same way they
    // sort by time. This is the invariant D2's watermark reverse-scan relies on.
    #[test]
    fn path_for_new_ts_dict_order_equals_time_order() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        let times = [
            ("a", started(2026, 8, 3, 1)),
            ("b", started(2026, 8, 3, 30)),
            ("c", started(2026, 8, 3, 59)),
            ("d", started(2026, 8, 4, 0)),
            ("e", started(2027, 1, 1, 0)),
        ];
        let paths: Vec<PathBuf> = times.iter().map(|(t, ts)| s.path_for_new(t, ts)).collect();
        // Paths sort the same as the input time order.
        let mut sorted_by_path = paths.clone();
        sorted_by_path.sort();
        assert_eq!(
            sorted_by_path, paths,
            "path dictionary order equals chronological (started_at) order"
        );
        // And each path's parent follows YYYY/MM/DD.
        for (path, (_t, ts)) in paths.iter().zip(times.iter()) {
            let parent = path.parent().unwrap();
            let date_dir = dir
                .path()
                .join(format!("{}", chrono::DateTime::parse_from_rfc3339(ts).unwrap().year()))
                .join(format!("{:02}", chrono::DateTime::parse_from_rfc3339(ts).unwrap().month()))
                .join(format!("{:02}", chrono::DateTime::parse_from_rfc3339(ts).unwrap().day()));
            assert_eq!(parent, date_dir);
        }
    }

    // An unparseable started_at ("x", as several existing tests use) still
    // produces a deterministic path: the epoch date partition + epoch ts. The
    // file round-trips through resolve_path.
    #[tokio::test]
    async fn unparseable_started_at_falls_back_to_epoch_partition() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(meta_with("t-epoch", "x"));
        s.append(
            "t-epoch",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "e1".to_owned(),
                text: "epoch".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.flush("t-epoch").await.unwrap();

        let epoch_path = dir
            .path()
            .join("1970")
            .join("01")
            .join("01")
            .join("rollout-19700101T000000Z-t-epoch.jsonl");
        assert!(epoch_path.exists(), "epoch fallback path: {}", epoch_path.display());
        assert_eq!(s.lookup_path("t-epoch").as_deref(), Some(epoch_path.as_path()));
        let items = s.read_turn_items("t-epoch").await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "e1");
    }

    // ── D1 fixup regressions ──

    // F1: session_index.jsonl entries must be keyed by the SANITIZED thread id
    // (the form used as the file-name stem AND as the lookup_path query), not the
    // raw thread id. For a thread id with a path separator ("a/b" -> "a_b"), the
    // bug wrote the RAW "a/b" key but lookup_path queried the SANITIZED "a_b" →
    // the index ALWAYS missed for special-char ids → silent fall-back to the full
    // date-tree scan (the index's O(1) reverse-lookup promise was lost). The
    // existing 't-index'/'t-scan' tests never caught this because their ids are
    // already sanitized (raw == sanitized). This test uses an id where raw !=
    // sanitized and asserts the index is hit DIRECTLY (find_latest_for_thread
    // with the sanitized id returns the entry), NOT via the scan fallback.
    #[tokio::test]
    async fn index_key_matches_sanitized_query_for_special_char_thread_id() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        // "a/b" sanitizes to "a_b" — raw != sanitized.
        s.create_session(meta_with("a/b", &started(2026, 8, 3, 0)));
        s.flush("a/b").await.unwrap();

        let sanitized = sanitize_thread_id("a/b");
        assert_eq!(sanitized, "a_b", "raw != sanitized for this id");

        // The index MUST have an entry keyed by the SANITIZED id ("a_b"), so a
        // direct reverse-lookup by "a_b" hits it WITHOUT needing the scan.
        // Mutation: revert the create_session write site back to
        // `thread_id: thread_id.clone()` (raw "a/b") → the index never matches
        // "a_b" → find_latest_for_thread returns None → this expect panics.
        let entry = crate::session_index::find_latest_for_thread(dir.path(), &sanitized)
            .expect("index keyed by sanitized id; raw-key mutation makes this None");
        // The indexed file_path must point at the real on-disk file (sanitized
        // stem "a_b"), proving the entry is genuine — not a scan-side ghost.
        let indexed = PathBuf::from(&entry.file_path);
        assert!(indexed.exists(), "indexed path exists on disk: {}", indexed.display());
        assert!(
            indexed.to_string_lossy().contains("a_b"),
            "sanitized stem in indexed path: {}",
            indexed.display(),
        );
        assert!(
            !indexed.to_string_lossy().contains("a/b"),
            "no raw separator in indexed path: {}",
            indexed.display(),
        );
    }

    // F2: compact_ts_from_started_at and date_parts_from_started_at must project
    // to UTC before formatting. A non-"Z" RFC-3339 offset (e.g. -05:00) must
    // produce a ts and date partition in UTC, not the offset's local wall-clock.
    // The bug formatted the DateTime<FixedOffset> directly: the ts string ended
    // in 'Z' (asserting UTC) but encoded the LOCAL offset time, so two distinct
    // UTC instants could produce the SAME ts — breaking the dict-order ==
    // chronological-order invariant D2's watermark reverse-scan relies on. The
    // existing dict-order test used only "Z" (UTC) inputs where FixedOffset ==
    // UTC, so it never caught this.
    #[test]
    fn path_for_new_uses_utc_for_non_z_offsets() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        // Same stem for both so the timestamp is the ONLY path differentiator.
        // 2026-08-03T23:00:00-05:00 == 2026-08-04T04:00:00Z (UTC).
        let minus_five = "2026-08-03T23:00:00-05:00";
        let path_off = s.path_for_new("t", minus_five);
        // ts must be the UTC instant 20260804T040000Z, not the local 20260803T230000Z.
        assert!(
            path_off.to_string_lossy().contains("rollout-20260804T040000Z-"),
            "non-Z offset must be projected to UTC: {}",
            path_off.display(),
        );
        // Date partition must be the UTC date 2026/08/04, not the local 08/03.
        assert!(
            path_off.starts_with(dir.path().join("2026").join("08").join("04")),
            "date partition in UTC: {}",
            path_off.display(),
        );

        // Control: the UTC instant 2026-08-03T23:00:00Z (earlier than the -05:00
        // instant in UTC).
        let zulu = "2026-08-03T23:00:00Z";
        let path_zulu = s.path_for_new("t", zulu);
        assert!(
            path_zulu.to_string_lossy().contains("rollout-20260803T230000Z-"),
            "Z input stays UTC: {}",
            path_zulu.display(),
        );
        assert!(
            path_zulu.starts_with(dir.path().join("2026").join("08").join("03")),
            "Z date partition: {}",
            path_zulu.display(),
        );

        // Dict-order == chronological-order: the -05:00 instant (UTC 04:00 on
        // 08-04) is LATER than the Z instant (UTC 23:00 on 08-03), so its path
        // must sort STRICTLY GREATER. Mutation: drop `with_timezone(&Utc)` from
        // compact_ts_from_started_at → the -05:00 ts becomes 20260803230000Z and
        // the date partition becomes 2026/08/03, so path_off == path_zulu and
        // BOTH the ts assertion above AND this strict-order assertion fail.
        assert_ne!(
            path_zulu, path_off,
            "the two UTC instants must produce distinct paths (bug collapsed them)",
        );
        let mut both = vec![path_zulu.clone(), path_off.clone()];
        both.sort();
        assert_eq!(
            both,
            vec![path_zulu, path_off],
            "dict order follows UTC instant order (zulu < minus-five after UTC projection)",
        );
    }

    // ── list_all_session_metas (DB-unavailable fallback surface) ──

    // list_all_session_metas walks BOTH the date tree AND the legacy flat files,
    // dedupes by thread id (newest path wins), and skips files whose first line
    // is not a SessionMeta header. The RolloutBackedAgentStore DB-unavailable
    // fallback depends on every discoverable thread surfacing here.
    //
    // Mutation guards:
    //  - Drop the flat-file branch from collect_all_rollout_files → t-flat
    //    disappears → its assertion fails.
    //  - Make the dedup keep the OLDEST path (>= instead of strict >) → the
    //    "t-dup newer started_at" assertion fails.
    //  - Make read_first_line_session_meta parse ANY first line → the corrupt
    //    file's "x" thread leaks in → the count assertion fails.
    #[tokio::test]
    async fn list_all_session_metas_dedupes_and_skips_unparseable() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);

        // Two real threads in the date tree (create_session + a flush each so the
        // file materializes with a SessionMeta header).
        s.create_session(meta_with("t-a", &started(2026, 8, 3, 0)));
        s.flush("t-a").await.unwrap();
        s.create_session(meta_with("t-b", &started(2026, 8, 4, 0)));
        s.flush("t-b").await.unwrap();

        // A legacy FLAT file (top-level) for a third thread.
        let flat = dir.path().join("t-flat.rollout.jsonl");
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&flat).unwrap();
            let header = RolloutLine::now(RolloutItem::SessionMeta(meta_with(
                "t-flat",
                &started(2026, 1, 1, 0),
            )));
            writeln!(f, "{}", serde_json::to_string(&header).unwrap()).unwrap();
        }

        // A stale flat DUPLICATE of t-a (older started_at than the date file).
        // list_all_session_metas must keep ONLY the date-tree t-a (newest path).
        let dup_flat = dir.path().join("t-a.rollout.jsonl");
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&dup_flat).unwrap();
            let header = RolloutLine::now(RolloutItem::SessionMeta(meta_with(
                "t-a",
                &started(2019, 1, 1, 0),
            )));
            writeln!(f, "{}", serde_json::to_string(&header).unwrap()).unwrap();
        }

        // A corrupt file whose first line is NOT a SessionMeta — must be skipped.
        let corrupt = dir
            .path()
            .join("2026")
            .join("08")
            .join("05")
            .join("rollout-20260805T000000Z-t-corrupt.jsonl");
        std::fs::create_dir_all(corrupt.parent().unwrap()).unwrap();
        std::fs::write(
            &corrupt,
            serde_json::to_vec(&RolloutLine::now(RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "nope".to_owned(),
                text: "no-header".to_owned(),
            })))
            .unwrap(),
        )
        .unwrap();

        let metas = s.list_all_session_metas();
        // Exactly three threads: t-a, t-b, t-flat (t-corrupt skipped; t-a deduped).
        assert_eq!(metas.len(), 3, "corrupt skipped + dup deduped: {metas:?}");
        let by_id: std::collections::HashMap<&str, &SessionMeta> =
            metas.iter().map(|m| (m.thread_id.as_str(), m)).collect();
        assert!(by_id.contains_key("t-a"), "date-tree t-a present");
        assert!(by_id.contains_key("t-b"), "date-tree t-b present");
        assert!(by_id.contains_key("t-flat"), "flat t-flat present");

        // Dedup: t-a's kept entry is the NEWER date-tree file, not the stale flat
        // dup. The date-tree file's started_at is 2026-08-03; the flat dup's is
        // 2019-01-01.
        let a = by_id["t-a"];
        assert!(
            a.started_at.starts_with("2026-08-03"),
            "dup dedup keeps the newest (date-tree) t-a: got started_at={}",
            a.started_at,
        );
    }

    // ── read_turn_timeline ──────────────────────────────────────────────────
    //
    // Regression for the restore-ordering bug: the fixture mirrors a REAL
    // production rollout file (sessions/2026/08/16/rollout-...-3a5d231e) —
    // per-turn TurnState full snapshots, re-appended duplicate messages from
    // the historical emit-anchor drift, and an auto-compaction marker. The
    // timeline must surface entries in FILE order with per-turn attribution
    // that matches the live event sequence (not the TurnState restamp that
    // collapses all messages onto the last turn).

    fn dev_msg(text: &str) -> slab_types::ConversationMessage {
        ConversationMessage {
            role: "developer".to_owned(),
            content: ConversationMessageContent::Text(text.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    fn turn_state(turn: u32, input: Vec<slab_types::ConversationMessage>) -> RolloutItem {
        RolloutItem::TurnContext(TurnContextPayload::TurnState {
            turn_index: turn,
            status: "completed".to_owned(),
            input_messages: input,
            tool_specs_json: None,
            llm_response_json: None,
            error: None,
            completed_at: None,
            started_at: None,
            input_messages_raw: None,
        })
    }

    fn append(turn: u32, message: slab_types::ConversationMessage) -> RolloutItem {
        RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
            turn_index: turn,
            message,
            id: None,
            created_at: None,
        })
    }

    #[tokio::test]
    async fn read_turn_timeline_preserves_file_order_and_append_attribution() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        s.create_session(default_meta("t"));

        // Turn 0: init batch (system + dev) + user, then assistant item+append,
        // closed by a TurnState carrying the FULL input snapshot (this is the
        // line whose restamp collapsed every message onto the last turn in the
        // old bucket-merge restore path).
        s.append(
            "t",
            append(
                0,
                ConversationMessage {
                    role: "system".to_owned(),
                    content: ConversationMessageContent::Text("persona".to_owned()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ),
        )
        .await
        .unwrap();
        s.append("t", append(0, dev_msg("<environment_context>"))).await.unwrap();
        s.append("t", append(0, user_msg("你是谁"))).await.unwrap();
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "a0".to_owned(),
                text: "我是 Slab".to_owned(),
            }),
        )
        .await
        .unwrap();
        s.append("t", append(0, user_msg_reply("我是 Slab"))).await.unwrap();
        s.append("t", turn_state(0, vec![user_msg("你是谁")])).await.unwrap();

        // Turn 1: the historical emit-drift re-append (old dev + prior
        // user/assistant duplicated at the new turn's index) + the new input.
        s.append("t", append(1, dev_msg("<environment_context>"))).await.unwrap();
        s.append("t", append(1, user_msg("你是谁"))).await.unwrap();
        s.append("t", append(1, user_msg_reply("我是 Slab"))).await.unwrap();
        s.append("t", append(1, user_msg("权限测试"))).await.unwrap();
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::CommandExecution {
                id: "c1".to_owned(),
                command: "echo hi".to_owned(),
                cwd: "/".to_owned(),
                process_id: None,
                status: "completed".to_owned(),
                aggregated_output: None,
                exit_code: Some(0),
                duration_ms: None,
            }),
        )
        .await
        .unwrap();
        s.append(
            "t",
            append(
                1,
                ConversationMessage {
                    role: "tool".to_owned(),
                    content: ConversationMessageContent::Text("hi".to_owned()),
                    name: None,
                    tool_call_id: Some("c1".to_owned()),
                    tool_calls: vec![],
                },
            ),
        )
        .await
        .unwrap();

        // Auto-compaction with a non-empty baseline at turn 1.
        s.append(
            "t",
            RolloutItem::Compacted(crate::item::CompactedPayload {
                thread_id: "t".to_owned(),
                compacted_messages: vec![user_msg("summary")],
                removed_messages: 4,
                output_tokens: 10,
                status: "auto".to_owned(),
                turn_index: 1,
            }),
        )
        .await
        .unwrap();
        // Post-compaction turn 2.
        s.append("t", append(2, user_msg("你能做什么"))).await.unwrap();
        s.append(
            "t",
            RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "a2".to_owned(),
                text: "很多".to_owned(),
            }),
        )
        .await
        .unwrap();

        let timeline = s.read_turn_timeline("t").await;
        // Render the timeline as a compact (kind, turn, key) list for assertion.
        let rendered: Vec<(u8, u32, String)> = timeline
            .iter()
            .map(|e| match e {
                TurnTimelineEntry::Item(r) => (0, r.turn_index, format!("item:{}", r.id)),
                TurnTimelineEntry::Message(r) => (
                    1,
                    r.turn_index,
                    format!("msg:{}:{}", r.message.role, r.message.content.rendered_text()),
                ),
            })
            .collect();
        assert_eq!(
            rendered,
            vec![
                (1, 0, "msg:system:persona".to_owned()),
                (1, 0, "msg:developer:<environment_context>".to_owned()),
                (1, 0, "msg:user:你是谁".to_owned()),
                (0, 0, "item:a0".to_owned()),
                (1, 0, "msg:assistant:我是 Slab".to_owned()),
                // TurnState contributes nothing.
                (1, 1, "msg:developer:<environment_context>".to_owned()),
                (1, 1, "msg:user:你是谁".to_owned()),
                (1, 1, "msg:assistant:我是 Slab".to_owned()),
                (1, 1, "msg:user:权限测试".to_owned()),
                (0, 1, "item:c1".to_owned()),
                (1, 1, "msg:tool:hi".to_owned()),
                // Compacted baseline surfaces at its turn.
                (1, 1, "msg:user:summary".to_owned()),
                (1, 2, "msg:user:你能做什么".to_owned()),
                (0, 2, "item:a2".to_owned()),
            ],
            "timeline must preserve file order with per-append turn attribution"
        );

        // The item seq resets per turn (mirrors read_turn_items).
        let seqs: Vec<(u32, u32, &str)> = timeline
            .iter()
            .filter_map(|e| match e {
                TurnTimelineEntry::Item(r) => Some((r.turn_index, r.seq, r.id.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(seqs, vec![(0, 0, "a0"), (1, 0, "c1"), (2, 0, "a2")]);

        // Message records carry stable synthetic ids and the append's own
        // turn index — NOT the last TurnState's restamp.
        let user_turns: Vec<u32> = timeline
            .iter()
            .filter_map(|e| match e {
                TurnTimelineEntry::Message(r) if r.message.role == "user" => Some(r.turn_index),
                _ => None,
            })
            .collect();
        assert_eq!(
            user_turns,
            vec![0, 1, 1, 1, 2],
            "user messages keep their append-turn attribution (incl. compacted summary)"
        );
    }

    fn user_msg_reply(text: &str) -> slab_types::ConversationMessage {
        ConversationMessage {
            role: "assistant".to_owned(),
            content: ConversationMessageContent::Text(text.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }
    }
}

/// Unit tests for the pure truncate-predicate helper.
#[cfg(test)]
mod keep_line_tests {
    use super::*;
    use slab_agent::protocol::TurnItem;

    #[test]
    fn keeps_meta_unconditionally_gates_compacted_by_turn() {
        // SessionMeta is still kept unconditionally (the session header survives
        // any rollback), but Compacted is now turn-gated (see H3): a compaction
        // belonging to a turn that is itself being rolled back must be dropped,
        // otherwise read_messages would reset to a summary of deleted messages.
        let mut turn = None;
        let meta = RolloutItem::SessionMeta(default_meta("t"));
        let compacted_old = RolloutItem::Compacted(crate::item::CompactedPayload {
            thread_id: "t".to_owned(),
            compacted_messages: vec![],
            removed_messages: 0,
            output_tokens: 0,
            status: "auto".to_owned(),
            turn_index: 0, // turn 0 < from_turn 1 → keep
        });
        let compacted_dropped = RolloutItem::Compacted(crate::item::CompactedPayload {
            thread_id: "t".to_owned(),
            compacted_messages: vec![],
            removed_messages: 0,
            output_tokens: 0,
            status: "auto".to_owned(),
            turn_index: 1, // turn 1 >= from_turn 1 → drop
        });
        assert!(keep_line(&meta, &mut turn, 1), "SessionMeta always kept");
        assert!(keep_line(&compacted_old, &mut turn, 1), "compaction below from_turn kept");
        assert!(
            !keep_line(&compacted_dropped, &mut turn, 1),
            "compaction at/above from_turn dropped"
        );
    }

    #[test]
    fn turn_context_updates_running_turn() {
        let mut turn = None;
        let tc0 = RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
            turn_index: 0,
            message: slab_types::ConversationMessage {
                role: "user".to_owned(),
                content: slab_types::ConversationMessageContent::Text("a".to_owned()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            id: None,
            created_at: None,
        });
        let tc2 = RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
            turn_index: 2,
            message: slab_types::ConversationMessage {
                role: "user".to_owned(),
                content: slab_types::ConversationMessageContent::Text("b".to_owned()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            id: None,
            created_at: None,
        });
        assert!(keep_line(&tc0, &mut turn, 2)); // turn 0 < 2
        assert_eq!(turn, Some(0));
        assert!(!keep_line(&tc2, &mut turn, 2)); // turn 2 not < 2
        assert_eq!(turn, Some(2));
    }

    #[test]
    fn turn_item_uses_running_turn() {
        let mut turn = Some(1);
        let item = RolloutItem::TurnItem(TurnItem::AgentMessage {
            id: "a".to_owned(),
            text: "x".to_owned(),
        });
        assert!(keep_line(&item, &mut turn, 2)); // turn 1 < 2
        assert!(!keep_line(&item, &mut turn, 1)); // turn 1 not < 1
    }

    #[test]
    fn turn_item_before_any_turn_context_is_kept() {
        let mut turn = None;
        let item = RolloutItem::TurnItem(TurnItem::AgentMessage {
            id: "a".to_owned(),
            text: "x".to_owned(),
        });
        // Defensive: keep items we can't attribute to a turn yet.
        assert!(keep_line(&item, &mut turn, 1));
    }
}
