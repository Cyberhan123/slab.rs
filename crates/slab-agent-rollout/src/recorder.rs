//! Single-writer actor that owns the write handle for one rollout file.
//!
//! Each agent thread gets one [`RolloutRecorderHandle`] backed by a tokio task
//! draining an mpsc channel of [`RolloutCmd`]s. Materialization is **lazy**:
//! the file is not opened (and, for [`RolloutRecorderParams::Create`], the
//! `SessionMeta` header is not written) until the first `AddItems` or `Persist`.
//!
//! Writes are durable per item: each appended line is serialized to bytes ONCE,
//! written via `append_raw`, then the buffer is flushed so the bytes reach the
//! OS file BEFORE the per-line durable count advances. An `fsync` failure after
//! that point is a **durability gap** (the bytes are already in the OS file), so
//! it is logged and the write continues rather than triggering a retry that
//! would duplicate the line. `RolloutWriterState::write_pending_with_recovery`
//! performs a **two-phase retry** — on the first error the writer is dropped
//! (recovery mode) but the pending items are retained and replayed once after
//! reopening, so middle events are never lost and never duplicated.

use std::path::PathBuf;

#[cfg(test)]
use std::cell::Cell;

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::error::{Result, RolloutError};
use crate::item::{RolloutItem, RolloutLine, SessionMeta};
use crate::writer::JsonlWriter;

/// Commands sent to the recorder actor.
pub enum RolloutCmd {
    /// Enqueue items to be written on the next persist.
    AddItems(Vec<RolloutItem>),
    /// Flush pending items to disk. The optional oneshot signals completion
    /// (carrying the result) for callers that need to wait — `flush_and_wait`.
    Persist(Option<oneshot::Sender<Result<()>>>),
    /// Atomically drop every line belonging to turn `from_turn` and later. The
    /// actor drops its write handle first so the on-disk replace succeeds even
    /// on Windows (where an open handle locks the file against rename).
    Truncate { from_turn: u32, ack: oneshot::Sender<Result<()>> },
    /// Atomically replace the thread rollout file with exactly these lines
    /// (preserving each line timestamp). The actor flushes pending items, drops
    /// its write handle, then performs the on-disk atomic replace (Windows-safe,
    /// like [`RolloutCmd::Truncate`]). The next write reopens the file lazily
    /// (append mode) so it lands AFTER the rewritten content. The caller's
    /// `lines` are the authoritative full file contents.
    Rewrite { lines: Vec<RolloutLine>, ack: oneshot::Sender<Result<()>> },
    /// Flush any remaining items then stop the actor.
    Shutdown { ack: oneshot::Sender<()> },
}

/// How a recorder is created — fresh (`Create`) or appending to an existing file
/// (`Resume`).
pub enum RolloutRecorderParams {
    /// Start a new file. The `SessionMeta` header is written lazily on first
    /// materialization, NOT at spawn time.
    Create { meta: SessionMeta },
    /// Append to an existing file; never write a new header.
    Resume { thread_id: String },
}

impl RolloutRecorderParams {
    /// The thread id this recorder targets.
    pub fn thread_id(&self) -> &str {
        match self {
            Self::Create { meta } => &meta.thread_id,
            Self::Resume { thread_id } => thread_id,
        }
    }
}

/// Handle to a running recorder actor.
pub struct RolloutRecorderHandle {
    tx: mpsc::UnboundedSender<RolloutCmd>,
    join: Option<JoinHandle<()>>,
}

impl RolloutRecorderHandle {
    /// Spawn a recorder actor. `file_path` is the FULL path to the rollout
    /// file this recorder owns. The store computes the path once (sanitizing
    /// the thread id + date-partitioning via
    /// [`RolloutFileStore::path_for_new`](crate::store::RolloutFileStore::path_for_new))
    /// and passes it here verbatim, so the write path and the store's read path
    /// resolve to the SAME file (no read/write split-brain, L5).
    pub fn spawn(params: RolloutRecorderParams, file_path: PathBuf) -> Self {
        let thread_id = params.thread_id().to_owned();
        let path = file_path;
        let meta_to_write = match params {
            RolloutRecorderParams::Create { meta } => Some(meta),
            RolloutRecorderParams::Resume { .. } => None,
        };
        let state = RolloutWriterState {
            thread_id,
            path,
            writer: None,
            pending_items: Vec::new(),
            pending_written_count: 0,
            seq: 0,
            meta_to_write,
            #[cfg(test)]
            fail_opens: Cell::new(0),
        };
        let (tx, rx) = mpsc::unbounded_channel();
        let join = tokio::spawn(run(state, rx));
        Self { tx, join: Some(join) }
    }

    /// Enqueue a single item (best-effort; the channel is unbounded so this only
    /// fails if the actor has stopped).
    pub fn add_item(&self, item: RolloutItem) {
        self.add_items(vec![item]);
    }

    /// Enqueue several items at once.
    pub fn add_items(&self, items: Vec<RolloutItem>) {
        if let Err(e) = self.tx.send(RolloutCmd::AddItems(items)) {
            tracing::error!(error = ?e, "rollout recorder stopped; item dropped");
        }
    }

    /// Fire-and-forget flush of pending items.
    pub fn persist(&self) {
        let _ = self.tx.send(RolloutCmd::Persist(None));
    }

    /// Flush pending items and wait until the write attempt completes.
    pub async fn flush_and_wait(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(RolloutCmd::Persist(Some(tx))).map_err(|_| RolloutError::RecorderClosed)?;
        rx.await.map_err(|_| RolloutError::RecorderClosed)?
    }

    /// Flush remaining items and stop the actor.
    pub async fn shutdown(mut self) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(RolloutCmd::Shutdown { ack: tx }).is_ok() {
            let _ = rx.await;
        }
        if let Some(join) = self.join.take() {
            let _ = join.await;
        }
    }

    /// The thread id this recorder writes for.
    pub fn thread_id(&self) -> Option<&str> {
        // Not stored separately on the handle to keep it cheap; callers track it.
        None
    }

    /// Clone of the command sender — lets callers (e.g. [`crate::store`]) issue a
    /// command without holding the [`DashMap`](dashmap::DashMap) guard across an
    /// `.await`.
    pub(crate) fn sender(&self) -> mpsc::UnboundedSender<RolloutCmd> {
        self.tx.clone()
    }
}

/// Internal actor state.
struct RolloutWriterState {
    thread_id: String,
    path: PathBuf,
    writer: Option<JsonlWriter>,
    pending_items: Vec<RolloutItem>,
    /// Number of pending items already durably written (drained from the front).
    pending_written_count: usize,
    /// Monotonic per-recorder counter, bumped for every durable line.
    seq: u64,
    /// For `Create`: the header to write on first open. Cleared once durable.
    meta_to_write: Option<SessionMeta>,
    /// Test seam: the next N `ensure_writer_open` calls fail with an injected
    /// error, exercising the two-phase recovery path.
    #[cfg(test)]
    fail_opens: Cell<u32>,
}

impl RolloutWriterState {
    /// The recorder's monotonic seq counter (number of durable lines so far).
    #[allow(dead_code)]
    pub(crate) fn seq(&self) -> u64 {
        self.seq
    }

    fn ensure_writer_open(&mut self) -> Result<()> {
        #[cfg(test)]
        if self.fail_opens.get() > 0 {
            self.fail_opens.set(self.fail_opens.get() - 1);
            return Err(RolloutError::Io(std::io::Error::other("injected open failure")));
        }

        if self.writer.is_none() {
            let mut writer = JsonlWriter::open(&self.path)?;
            // For Create: write the SessionMeta header on first open. Only clear
            // it once the header is flushed (durable), so a failure here retries
            // the header on the next open rather than losing it.
            if let Some(meta) = self.meta_to_write.take() {
                self.write_meta_header(&mut writer, meta)?;
            }
            self.writer = Some(writer);
        }
        Ok(())
    }

    /// Write the SessionMeta header; on failure restore `meta_to_write`.
    fn write_meta_header(&mut self, writer: &mut JsonlWriter, meta: SessionMeta) -> Result<()> {
        let line = RolloutLine::now(RolloutItem::SessionMeta(meta.clone()));
        match writer.append_line(&line).and_then(|_| writer.flush()) {
            Ok(()) => {
                self.meta_to_write = None;
                self.seq += 1;
                Ok(())
            }
            Err(e) => {
                // Restore so the next ensure_writer_open retries the header.
                self.meta_to_write = Some(meta);
                Err(e)
            }
        }
    }

    fn enter_recovery_mode(&mut self) {
        if let Some(mut w) = self.writer.take() {
            // Best-effort final flush of whatever is buffered.
            let _ = w.flush();
        }
    }

    /// Write every pending item durably. On the first error, drop the writer
    /// (recovery mode, **keeping** the pending items) and retry once after
    /// reopening. Pending items are drained from the front as they become
    /// durable, so a mid-batch failure never re-writes or drops items.
    fn write_pending_with_recovery(&mut self) -> Result<()> {
        // Phase 1: open (and, for Create, write the header) then write pending.
        let phase1 = (|| -> Result<()> {
            self.ensure_writer_open()?;
            self.try_write_pending()
        })();
        if phase1.is_ok() {
            return Ok(());
        }
        let e1 = phase1.unwrap_err();
        tracing::warn!(thread_id = %self.thread_id, error = %e1, "rollout write failed; entering recovery");
        self.enter_recovery_mode();
        // Phase 2: reopen + retry once.
        self.ensure_writer_open()?;
        match self.try_write_pending() {
            Ok(()) => {
                tracing::info!(thread_id = %self.thread_id, "rollout write recovered after retry");
                Ok(())
            }
            Err(e2) => Err(e2),
        }
    }

    fn try_write_pending(&mut self) -> Result<()> {
        while self.pending_written_count < self.pending_items.len() {
            // Serialize the line to bytes ONCE. We then write the raw bytes,
            // flush the buffer so they reach the OS file, and ONLY THEN advance
            // the durable count. An fsync failure after that point is a
            // durability gap (the bytes are in the OS file) — we log and
            // continue rather than returning Err, which would trigger a two-phase
            // retry that re-writes the SAME line (a duplication). (M3)
            let idx = self.pending_written_count;
            let line = RolloutLine::now(self.pending_items[idx].clone());
            let mut bytes = serde_json::to_vec(&line)?;
            if !bytes.ends_with(b"\n") {
                bytes.push(b'\n');
            }

            let writer = self.writer.as_mut().ok_or_else(|| {
                RolloutError::Io(std::io::Error::other("writer not open before write_pending"))
            })?;
            writer.append_raw(&bytes)?;
            writer.flush_buffered()?;
            // Bytes are now in the OS file — the line is durable-enough for the
            // no-duplication invariant. Advance the count before fsync.
            self.pending_written_count += 1;
            self.seq += 1;
            // fsync is best-effort durability; on failure we warn and continue.
            if let Err(e) = writer.sync() {
                tracing::warn!(
                    thread_id = %self.thread_id,
                    error = %e,
                    "rollout fsync failed; durability gap, no duplication"
                );
            }
        }
        // Everything durable — drain.
        self.pending_items.clear();
        self.pending_written_count = 0;
        Ok(())
    }
}

async fn run(mut state: RolloutWriterState, mut rx: mpsc::UnboundedReceiver<RolloutCmd>) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            RolloutCmd::AddItems(items) => state.pending_items.extend(items),
            RolloutCmd::Persist(ack) => {
                if let Err(e) = state.write_pending_with_recovery() {
                    tracing::error!(
                        thread_id = %state.thread_id,
                        error = %e,
                        "rollout persist failed; pending items retained for next attempt"
                    );
                    if let Some(tx) = ack {
                        let _ = tx.send(Err(e));
                    }
                } else if let Some(tx) = ack {
                    let _ = tx.send(Ok(()));
                }
            }
            RolloutCmd::Shutdown { ack } => {
                if let Err(e) = state.write_pending_with_recovery() {
                    tracing::error!(
                        thread_id = %state.thread_id,
                        error = %e,
                        "rollout final flush on shutdown failed"
                    );
                }
                let _ = ack.send(());
                break;
            }
            RolloutCmd::Truncate { from_turn, ack } => {
                // Drop pending items BEFORE the truncate. The store flushes
                // (Persist + ack) then sends Truncate as a SEPARATE command, so a
                // racing AddItems that lands between the flush-ack and the
                // Truncate survives in pending_items and would be written by the
                // next Persist, RESURRECTING the turns just rolled back. Anything
                // still pending at truncate time belongs to a turn >= from_turn
                // (the flush made everything older durable) and must be discarded.
                // (B1)
                state.pending_items.clear();
                state.pending_written_count = 0;
                // Drop the write handle so the on-disk atomic replace can succeed
                // (Windows locks an open file against rename). The next write
                // reopens the file lazily.
                state.enter_recovery_mode();
                let result = crate::store::truncate_rollout_file(&state.path, from_turn);
                if let Err(e) = &result {
                    tracing::error!(
                        thread_id = %state.thread_id,
                        from_turn,
                        error = %e,
                        "rollout truncate failed"
                    );
                }
                let _ = ack.send(result);
            }
            RolloutCmd::Rewrite { lines, ack } => {
                // Flush pending first so any in-flight observer writes are
                // durable and the recorder's pending buffer is drained before
                // the rewrite (the caller's `lines` are the authoritative full
                // file contents). Then drop the write handle so the on-disk
                // atomic replace can succeed (Windows locks an open file against
                // rename), and write the full file. The next write reopens the
                // file lazily in append mode, landing after the rewritten
                // content. Do NOT reopen eagerly.
                let result = state.write_pending_with_recovery().and_then(|_| {
                    state.enter_recovery_mode();
                    let result = crate::store::rewrite_rollout_file(&state.path, &lines);
                    if result.is_ok() {
                        // The rewrite established the file (including its
                        // SessionMeta header when the caller provided one).
                        // Clear any pending Create header so the next append
                        // does not duplicate it.
                        state.meta_to_write = None;
                    }
                    result
                });
                if let Err(e) = &result {
                    tracing::error!(
                        thread_id = %state.thread_id,
                        error = %e,
                        "rollout rewrite failed"
                    );
                }
                let _ = ack.send(result);
            }
        }
    }
    // Channel closed (all senders dropped, e.g. the handle went out of scope or
    // the process is tearing down) without an explicit Shutdown. Best-effort
    // final flush so buffered items do not vanish. The explicit Shutdown path
    // also lands here after its break, having already flushed — the second flush
    // is a no-op (pending already drained). (H4)
    if let Err(e) = state.write_pending_with_recovery() {
        tracing::error!(
            thread_id = %state.thread_id,
            error = %e,
            "rollout final flush on channel close failed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::read_rollout_lines;
    use slab_agent::protocol::TurnItem;

    fn sessions_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn meta(thread_id: &str) -> SessionMeta {
        SessionMeta {
            thread_id: thread_id.to_owned(),
            session_id: "s1".to_owned(),
            parent_id: None,
            started_at: "2026-08-02T00:00:00Z".to_owned(),
            config_json: serde_json::json!({"model": "x"}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: None,
            trace_path: None,
        }
    }

    #[tokio::test]
    async fn create_does_not_materialize_until_first_write() {
        let dir = sessions_dir();
        let expected = dir.path().join("t1.rollout.jsonl");
        let handle = RolloutRecorderHandle::spawn(
            RolloutRecorderParams::Create { meta: meta("t1") },
            expected.clone(),
        );
        // Spawned with no items — file must not exist yet.
        assert!(!expected.exists(), "file created before any write");

        handle.add_item(RolloutItem::TurnItem(TurnItem::AgentMessage {
            id: "a1".to_owned(),
            text: "hi".to_owned(),
        }));
        handle.flush_and_wait().await.unwrap();
        assert!(expected.exists());

        let lines = read_rollout_lines(&expected);
        // Line 0 = SessionMeta header (written on first open), line 1 = the item.
        assert_eq!(lines.len(), 2);
        assert!(matches!(lines[0].item, RolloutItem::SessionMeta(_)));
        assert!(matches!(lines[1].item, RolloutItem::TurnItem(_)));
        handle.shutdown().await;
    }

    #[tokio::test]
    async fn append_flush_read_consistency() {
        let dir = sessions_dir();
        let path = dir.path().join("t2.rollout.jsonl");
        let handle = RolloutRecorderHandle::spawn(
            RolloutRecorderParams::Create { meta: meta("t2") },
            path.clone(),
        );
        for i in 0..5 {
            handle.add_item(RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: format!("a{i}"),
                text: format!("msg {i}"),
            }));
        }
        handle.flush_and_wait().await.unwrap();

        let lines = read_rollout_lines(&path);
        // 1 header + 5 items, in order.
        assert_eq!(lines.len(), 6);
        for (i, line) in lines.iter().skip(1).enumerate() {
            let RolloutItem::TurnItem(TurnItem::AgentMessage { id, .. }) = &line.item else {
                panic!("expected agent message at {i}");
            };
            assert_eq!(id, &format!("a{i}"));
        }
        handle.shutdown().await;
    }

    #[tokio::test]
    async fn resume_does_not_write_header() {
        let dir = sessions_dir();
        let path = dir.path().join("t3.rollout.jsonl");
        // Pre-seed a file with a SessionMeta header.
        {
            let h = RolloutRecorderHandle::spawn(
                RolloutRecorderParams::Create { meta: meta("t3") },
                path.clone(),
            );
            h.flush_and_wait().await.unwrap();
            h.shutdown().await;
        }
        assert_eq!(read_rollout_lines(&path).len(), 1);

        // Resume and append — header must not be duplicated.
        let h = RolloutRecorderHandle::spawn(
            RolloutRecorderParams::Resume { thread_id: "t3".to_owned() },
            path.clone(),
        );
        h.add_item(RolloutItem::TurnItem(TurnItem::AgentMessage {
            id: "x".to_owned(),
            text: "y".to_owned(),
        }));
        h.flush_and_wait().await.unwrap();
        h.shutdown().await;

        let lines = read_rollout_lines(&path);
        assert_eq!(lines.len(), 2, "resume must not duplicate the header");
        assert!(matches!(lines[0].item, RolloutItem::SessionMeta(_)));
    }

    #[tokio::test]
    async fn two_phase_recovery_never_loses_items() {
        let dir = sessions_dir();
        let path = dir.path().join("t4.rollout.jsonl");

        // Build state directly so we can arm the test seam.
        let mut state = RolloutWriterState {
            thread_id: "t4".to_owned(),
            path: path.clone(),
            writer: None,
            pending_items: vec![
                RolloutItem::TurnItem(TurnItem::AgentMessage {
                    id: "a".to_owned(),
                    text: "one".to_owned(),
                }),
                RolloutItem::TurnItem(TurnItem::AgentMessage {
                    id: "b".to_owned(),
                    text: "two".to_owned(),
                }),
                RolloutItem::TurnItem(TurnItem::AgentMessage {
                    id: "c".to_owned(),
                    text: "three".to_owned(),
                }),
            ],
            pending_written_count: 0,
            seq: 0,
            meta_to_write: Some(meta("t4")),
            fail_opens: Cell::new(1), // first open fails, retry succeeds
        };

        // First attempt: phase-1 ensure_writer_open fails (injected), recovery
        // drops writer, reopens (succeeds), writes everything.
        let result = state.write_pending_with_recovery();
        assert!(result.is_ok(), "recovery should succeed: {result:?}");

        // The actor is normally responsible for the header; here state wrote it
        // itself via ensure_writer_open on Create.
        let lines = read_rollout_lines(&path);
        // header + 3 items, none lost.
        assert_eq!(lines.len(), 4);
        assert!(matches!(lines[0].item, RolloutItem::SessionMeta(_)));
        assert_eq!(state.pending_items.len(), 0, "pending drained after success");
    }

    #[tokio::test]
    async fn two_phase_recovery_retains_pending_when_retry_also_fails() {
        let dir = sessions_dir();
        let path = dir.path().join("t5.rollout.jsonl");
        let mut state = RolloutWriterState {
            thread_id: "t5".to_owned(),
            path: path.clone(),
            writer: None,
            pending_items: vec![RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "a".to_owned(),
                text: "one".to_owned(),
            })],
            pending_written_count: 0,
            seq: 0,
            meta_to_write: Some(meta("t5")),
            fail_opens: Cell::new(2), // both attempts fail
        };
        let result = state.write_pending_with_recovery();
        assert!(result.is_err(), "both phases failed — should error");
        // Pending items retained so the next Persist can retry (no loss).
        assert_eq!(state.pending_items.len(), 1);
        assert_eq!(state.pending_written_count, 0);
        // And the meta header is still queued for retry.
        assert!(state.meta_to_write.is_some());
    }

    #[tokio::test]
    async fn shutdown_acks_after_final_flush() {
        let dir = sessions_dir();
        let path = dir.path().join("t6.rollout.jsonl");
        let handle = RolloutRecorderHandle::spawn(
            RolloutRecorderParams::Create { meta: meta("t6") },
            path.clone(),
        );
        handle.add_item(RolloutItem::TurnItem(TurnItem::AgentMessage {
            id: "z".to_owned(),
            text: "final".to_owned(),
        }));
        // shutdown flushes first.
        handle.shutdown().await;
        let lines = read_rollout_lines(&path);
        assert_eq!(lines.len(), 2); // header + the one item
    }

    // --- Regression tests (W1 review findings) ---

    // B1: a racing AddItems that lands in pending between the pre-truncate flush
    // and the Truncate command must NOT survive — the Truncate handler clears
    // pending, so the next Persist cannot resurrect the rolled-back turn.
    #[tokio::test]
    async fn truncate_clears_pending_no_resurrection() {
        let dir = sessions_dir();
        let path = dir.path().join("tb1.rollout.jsonl");
        let handle = RolloutRecorderHandle::spawn(
            RolloutRecorderParams::Create { meta: meta("tb1") },
            path.clone(),
        );
        // Materialize a durable turn-1 line so the file exists with known state.
        handle.add_item(RolloutItem::TurnContext(crate::item::TurnContextPayload::MessageAppend {
            turn_index: 1,
            message: slab_types::ConversationMessage {
                role: "user".to_owned(),
                content: slab_types::ConversationMessageContent::Text("durable".to_owned()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            id: None,
            created_at: None,
        }));
        handle.flush_and_wait().await.unwrap();

        // Append a turn-2 item WITHOUT flushing — it sits in pending. The channel
        // is FIFO, so AddItems is processed (into pending) before the Truncate.
        let sender = handle.sender();
        sender
            .send(RolloutCmd::AddItems(vec![RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "resurrect".to_owned(),
                text: "should-not-survive".to_owned(),
            })]))
            .unwrap();

        // Truncate from turn 2 — must clear pending (B1).
        let (ack, rx) = oneshot::channel();
        sender.send(RolloutCmd::Truncate { from_turn: 2, ack }).unwrap();
        rx.await.unwrap().unwrap();

        // A subsequent persist must NOT resurrect the cleared pending item.
        handle.flush_and_wait().await.unwrap();

        let lines = read_rollout_lines(&path);
        let has_resurrect = lines.iter().any(|l| {
            matches!(
                &l.item,
                RolloutItem::TurnItem(TurnItem::AgentMessage { id, .. }) if id == "resurrect"
            )
        });
        assert!(!has_resurrect, "pending item was cleared by truncate, not resurrected (B1)");
        handle.shutdown().await;
    }

    // H4: dropping the handle WITHOUT an explicit shutdown closes the channel;
    // run()'s final flush on channel close must make the buffered item durable.
    #[tokio::test]
    async fn drop_without_shutdown_flushes_pending() {
        let dir = sessions_dir();
        let path = dir.path().join("th4.rollout.jsonl");
        {
            let handle = RolloutRecorderHandle::spawn(
                RolloutRecorderParams::Create { meta: meta("th4") },
                path.clone(),
            );
            handle.add_item(RolloutItem::TurnItem(TurnItem::AgentMessage {
                id: "pending".to_owned(),
                text: "must-be-durable".to_owned(),
            }));
            // Drop WITHOUT shutdown — channel closes; run()'s tail flush runs (H4).
            drop(handle);
        }
        // The actor task is detached; poll the file until the item lands (or time
        // out) instead of a fixed sleep, for robustness across schedulers.
        let mut durable = false;
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let lines = read_rollout_lines(&path);
            if lines.iter().any(|l| {
                matches!(
                    &l.item,
                    RolloutItem::TurnItem(TurnItem::AgentMessage { id, .. }) if id == "pending"
                )
            }) {
                durable = true;
                break;
            }
        }
        assert!(durable, "pending item must be flushed on channel close (H4)");
    }
}
