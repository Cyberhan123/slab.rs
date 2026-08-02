//! [`RolloutStore`] trait + the file-backed implementation.
//!
//! [`RolloutFileStore`] owns one [`RolloutRecorderHandle`] per thread (guarded
//! by a [`DashMap`]). Writes go through the recorder actor; reads open a fresh
//! read-only handle (flushing the writer first so pending items are durable).

use std::path::PathBuf;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::{mpsc, oneshot};

use crate::error::{Result, RolloutError};
use crate::item::{RolloutItem, RolloutLine, SessionMeta, TurnContextPayload};
use crate::reader::read_rollout_lines;
use crate::recorder::{RolloutCmd, RolloutRecorderHandle, RolloutRecorderParams};
use crate::writer::replace_file_atomically;

/// Abstract read/write surface over the rollout true source.
#[async_trait]
pub trait RolloutStore: Send + Sync {
    /// Replay the LLM-visible conversation (from `TurnContext::MessageAppend`,
    /// reset at each `Compacted` baseline).
    async fn read_messages(&self, thread_id: &str) -> Vec<slab_types::ConversationMessage>;
    /// Replay finalized items (from `RolloutItem::TurnItem`), attaching the
    /// currently-tracked turn index and a monotonic `seq`.
    async fn read_turn_items(&self, thread_id: &str) -> Vec<slab_agent::port::TurnItemRecord>;
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

    /// File path for `thread_id`.
    ///
    /// The thread id is sanitized to a single path-safe segment: any char that
    /// is not ascii-alphanumeric, `-` or `_` becomes `_`. This prevents path
    /// traversal (`..`, `/`, `\`) from a hostile or malformed thread id, and is a
    /// no-op for normal UUID-like ids. (L5)
    pub fn path_for(&self, thread_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{}.rollout.jsonl", sanitize_thread_id(thread_id)))
    }

    /// Explicitly create a session (writes the `SessionMeta` header lazily on the
    /// first write). Returns without writing.
    ///
    /// If a rollout file for `meta.thread_id` already exists (e.g. on process
    /// restart, or when the adapter re-sends meta for an existing thread), a
    /// fresh `Create` recorder would append a SECOND `SessionMeta` header into
    /// the existing file. To avoid that we mirror `recorder_for`: spawn with
    /// `Resume` when the file is present, `Create` otherwise. (M2)
    pub fn create_session(&self, meta: SessionMeta) {
        let thread_id = meta.thread_id.clone();
        self.recorders.entry(thread_id).or_insert_with(|| {
            let params = if self.path_for(&meta.thread_id).exists() {
                RolloutRecorderParams::Resume { thread_id: meta.thread_id.clone() }
            } else {
                RolloutRecorderParams::Create { meta }
            };
            RolloutRecorderHandle::spawn(params, self.sessions_dir.clone())
        });
    }

    /// Get or spawn the recorder for `thread_id`. Auto-creates with a default
    /// `SessionMeta` when neither a recorder nor a file exists yet.
    fn recorder_for(&self, thread_id: &str) -> Option<mpsc::UnboundedSender<RolloutCmd>> {
        // Fast path: existing recorder.
        if let Some(handle) = self.recorders.get(thread_id) {
            return Some(handle.sender());
        }
        // Slow path: insert a new recorder (Create vs Resume by file existence).
        let path = self.path_for(thread_id);
        let params = if path.exists() {
            RolloutRecorderParams::Resume { thread_id: thread_id.to_owned() }
        } else {
            RolloutRecorderParams::Create { meta: default_meta(thread_id) }
        };
        let entry = self
            .recorders
            .entry(thread_id.to_owned())
            .or_insert_with(|| RolloutRecorderHandle::spawn(params, self.sessions_dir.clone()));
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
/// a no-op for normal UUID-like ids. Applied both inside
/// [`RolloutFileStore::path_for`] (every store path lookup: recorder_for /
/// create_session / read / truncate) AND inside [`RolloutRecorderHandle::spawn`]
/// (the on-disk write path) so a hostile thread id cannot traverse out of
/// `sessions_dir` and — critically — the write path and the store's read path
/// resolve to the SAME file (no read/write split-brain). (L5)
pub(crate) fn sanitize_thread_id(thread_id: &str) -> String {
    thread_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
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
        let lines = read_rollout_lines(&self.path_for(thread_id));
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
        let lines = read_rollout_lines(&self.path_for(thread_id));
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

    async fn read_events(&self, thread_id: &str) -> Vec<slab_agent::protocol::EventMsg> {
        let _ = self.flush(thread_id).await;
        read_rollout_lines(&self.path_for(thread_id))
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
                return truncate_rollout_file(&self.path_for(thread_id), from_turn);
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
                return rewrite_rollout_file(&self.path_for(thread_id), &lines);
            }
        };
        let (ack, rx) = oneshot::channel();
        tx.send(RolloutCmd::Rewrite { lines, ack }).map_err(|_| RolloutError::RecorderClosed)?;
        rx.await.map_err(|_| RolloutError::RecorderClosed)?
    }

    async fn file_exists(&self, thread_id: &str) -> bool {
        self.path_for(thread_id).exists()
    }

    async fn read_session_meta(&self, thread_id: &str) -> Option<SessionMeta> {
        let _ = self.flush(thread_id).await;
        read_rollout_lines(&self.path_for(thread_id)).into_iter().find_map(|line| match line.item {
            RolloutItem::SessionMeta(meta) => Some(meta),
            _ => None,
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

        // Read uses path_for (sanitized "a_b"); the write must land in the same file.
        let items = s.read_turn_items("a/b").await;
        assert_eq!(
            items.len(),
            1,
            "write/read must resolve to the same sanitized file (no split-brain)"
        );
        assert_eq!(items[0].id, "m1");

        // The on-disk file is the sanitized stem, NOT a nested subdir.
        assert!(dir.path().join("a_b.rollout.jsonl").exists(), "file must be the sanitized stem");
        assert!(!dir.path().join("a").exists(), "raw separator must not create a nested directory");
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
            RolloutItem::EventMsg(EventMsg::ThreadStarted(
                slab_agent::protocol::ThreadStartedParams {
                    thread: slab_agent::protocol::Thread::default(),
                },
            )),
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
        assert!(matches!(events[0], EventMsg::ThreadStarted(_)));

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

    // Slice 6 (2a): the manual-compaction sequence HarnessService::compact_thread
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
        let lines = read_rollout_lines(&s.path_for("t"));
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

    // Slice 6 (2c) — fork wholesale-copy mechanism. HarnessService::fork_thread
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
        let parent_lines = read_rollout_lines(&s.path_for("t-parent"));
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

        let lines = read_rollout_lines(&s.path_for("t"));
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
        let before = read_rollout_lines(&s.path_for("t"))
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

        let header_count = read_rollout_lines(&s.path_for("t"))
            .iter()
            .filter(|l| matches!(l.item, RolloutItem::SessionMeta(_)))
            .count();
        assert_eq!(header_count, 1, "exactly one SessionMeta header, not two");
    }

    // L5: a hostile thread id cannot traverse out of sessions_dir.
    #[test]
    fn path_for_sanitizes_traversal_thread_id() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        let p = s.path_for("../escape");
        let parent = dir.path();
        assert!(
            p.starts_with(parent),
            "sanitized path must stay under sessions_dir: {}",
            p.display()
        );
        // The sanitized segment contains no path separators or dots.
        let file_name = p.file_name().unwrap().to_string_lossy().to_string();
        assert!(!file_name.contains('/'), "no '/' in {file_name}");
        assert!(!file_name.contains('\\'), "no '\\' in {file_name}");
        assert!(!file_name.contains(".."), "no '..' in {file_name}");
        // A normal UUID-like id is a no-op.
        let normal = s.path_for("01234567-89ab-cdef");
        assert!(normal.starts_with(parent));
        assert_eq!(
            normal.file_name().unwrap().to_string_lossy(),
            "01234567-89ab-cdef.rollout.jsonl"
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
        let meta_line = read_rollout_lines(&s.path_for("t"))
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
        let after = read_rollout_lines(&s.path_for("t"));
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

        let final_lines = read_rollout_lines(&s.path_for("t"));
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
        let path = s.path_for("ghost");
        // Seed the file directly (no recorder), so no handle is open.
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
