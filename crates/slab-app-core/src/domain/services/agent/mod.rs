//! Agent application services.
//!
//! Split along slab-agent's two distinct external interfaces, inspired by the
//! Codex Model/Engine layering (but shaped by slab-agent's own callers, not a
//! verbatim copy of Codex):
//! - [`HarnessService`] drives the turn loop for `/v1/agent/harness` (WS).
//! - [`ResponseService`] produces the OpenAI Responses wire for `/responses`.
//!
//! Both services hold a cheap clone of the shared `AgentCore`; they do not
//! wrap each other.

pub mod compact;
pub mod harness;
#[cfg(test)]
mod harness_tests;
pub mod response;

pub use compact::{SummarizingCompactPort, maybe_compact_messages};
pub use harness::HarnessService;
pub use response::ResponseService;
/// Re-exported so harness consumers (`slab-server`) can name the timeline
/// entry type without a direct `slab-agent-rollout` dependency.
pub use slab_agent_rollout::TurnTimelineEntry;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashSet;
use slab_agent::AgentRuntime;
use slab_agent::CompactPort;
use slab_agent::config::AgentConfig;
use slab_agent::error::AgentError;
use slab_agent::port::{
    AgentStorePort, ThreadMessageRecord, ThreadSnapshot, TurnItemRecord, TurnStateRecord,
};
use slab_types::{ConversationMessage, ConversationMessageContent};

use slab_agent_rollout::RolloutStore;

use crate::error::AppCoreError;
use crate::infra::agent::event_hub::{AgentEventHub, AgentEventMsgSubscription};
use crate::infra::agent::rollout_persistence;

/// app-core-internal read+write handle over the rollout-native conversation +
/// turn-state / turn-item streams. The turn-state / turn-item
/// reads were moved OFF the slab-agent `AgentStorePort` trait (slab-agent never calls them
/// in production) and onto this trait so `HarnessService::list_turn_states` /
/// `list_turn_items` (the `thread/resume` path) keep a typed, mockable handle
/// without polluting the slab-agent surface. This is the SOLE
/// conversation read/write path for app-core-internal callers that do NOT flow
/// through the slab-agent event stream (notably `single_shot`, which has no
/// turn loop and emits no `EventMsg` — its out-of-band writes go directly
/// through `append_message` / `append_turn_state`). slab-agent itself emits
/// conversation data via `EventMsg` (observed by the rollout persistence
/// observer); it never touches this trait. Implemented by
/// `RolloutBackedAgentStore` (rollout replay is the only source); mocked in
/// harness tests.
#[async_trait]
pub(crate) trait RolloutConversationStore: Send + Sync {
    /// Persisted full-fidelity `TurnItem` snapshots for a thread, ordered by
    /// `(turn_index, seq)` for deterministic replay.
    async fn list_turn_items(&self, thread_id: &str) -> Result<Vec<TurnItemRecord>, AgentError>;

    /// Persisted interleaved `TurnItem` + `MessageAppend` timeline for a
    /// thread, in rollout write (file) order with per-turn attribution — the
    /// single ordered source for the `thread/resume` history projection (the
    /// bucket-merge over separate reads lost ordering when a `TurnState`
    /// restamp collapsed every message onto the last turn).
    async fn list_turn_timeline(
        &self,
        thread_id: &str,
    ) -> Result<Vec<slab_agent_rollout::TurnTimelineEntry>, AgentError>;

    /// Persisted turn-state records for a thread ordered by `turn_index`.
    async fn list_turn_states(&self, thread_id: &str) -> Result<Vec<TurnStateRecord>, AgentError>;

    /// Persisted conversation messages for a thread in replay order (read).
    async fn list_thread_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AgentError>;

    /// Append a conversation message out-of-band (single_shot write path; NOT
    /// the slab-agent turn loop, which emits `MessageAppended` via `EventMsg`).
    async fn append_message(&self, record: &ThreadMessageRecord) -> Result<(), AgentError>;

    /// Append a turn-state snapshot out-of-band (single_shot write path; NOT
    /// the slab-agent turn loop, which emits `TurnStateChanged` via `EventMsg`).
    /// Currently exercised by the harness mock + the round-trip test; kept on
    /// the trait so a future single_shot turn-state write has a home without
    /// re-polluting the slab-agent surface.
    #[allow(dead_code)]
    async fn append_turn_state(&self, record: &TurnStateRecord) -> Result<(), AgentError>;
}
/// Shared core held by both the harness and response services.
///
/// INVARIANT: every field must be `Arc`-backed — cloning yields another handle
/// to the *same* state. The two services therefore share one runtime, one store,
/// one event hub, one rollout true source, and one `rollout_observers` guard
/// set. Adding any owned/non-`Arc` field would silently make them diverge
/// (especially the `rollout_observers` idempotency relied on by [`Self::spawn`]
/// / [`Self::send_input`]).
#[derive(Clone)]
pub(crate) struct AgentCore {
    runtime: AgentRuntime,
    store: Arc<dyn AgentStorePort>,
    events: Arc<AgentEventHub>,
    /// Shared compaction policy used by the harness turn loop (via
    /// `AgentControl`), the manual `thread/compact/start` op, and the HTTP
    /// chat/responses paths. Same `Arc` instance as the one wired into
    /// `AgentControl` so all paths compact identically.
    compact: Arc<dyn CompactPort>,
    /// Append-only rollout event-source true source. Shared with the
    /// harness so `compact_thread` / `fork_thread` / `rollback_thread` can
    /// access the rollout directly.
    rollout: Arc<slab_agent_rollout::RolloutFileStore>,
    /// app-core-internal reader for turn states + turn items (the
    /// `thread/resume` path). These reads left the slab-agent `AgentStorePort`
    /// trait; `HarnessService` reaches them through this handle. Backed by the
    /// same `RolloutBackedAgentStore` Arc wired as the runtime store.
    /// Renamed to `RolloutConversationStore` (read+write); the
    /// accessor stays `reader()` to avoid touching callers.
    reader: Arc<dyn RolloutConversationStore>,
    /// The trace directory configured from `agent.debug`, threaded
    /// in so the harness can apply the SAME root-vs-child `trace_path` rule as
    /// `RolloutBackedAgentStore::upsert_thread` when it reconstructs a
    /// `SessionMeta` (J4: fork / compact fallback). `None` when agent debugging
    /// is off — then even a root thread carries no `trace_path`.
    trace_dir: Option<PathBuf>,
    /// Thread ids that already have a rollout persistence observer running.
    /// Guards `spawn_rollout_persistence` to one observer per thread.
    rollout_observers: Arc<DashSet<String>>,
}

/// Persisted session state restored by the unified agent responses route.
pub struct RestoredAgentSession {
    pub thread: Option<ThreadSnapshot>,
    pub messages: Vec<ThreadMessageRecord>,
    /// Complete OpenAI-Responses-canonical `Response` JSON objects, one per
    /// agent run, oldest first. Empty for pre-migration history.
    pub responses: Vec<serde_json::Value>,
}

impl AgentCore {
    pub(crate) fn new(
        runtime: AgentRuntime,
        store: Arc<dyn AgentStorePort>,
        events: Arc<AgentEventHub>,
        compact: Arc<dyn CompactPort>,
        rollout: Arc<slab_agent_rollout::RolloutFileStore>,
        reader: Arc<dyn RolloutConversationStore>,
        trace_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            runtime,
            store,
            events,
            compact,
            rollout,
            reader,
            trace_dir,
            rollout_observers: Arc::new(DashSet::new()),
        }
    }

    pub(crate) fn runtime(&self) -> AgentRuntime {
        self.runtime.clone()
    }

    pub(crate) fn compact(&self) -> &Arc<dyn CompactPort> {
        &self.compact
    }

    pub(crate) fn store(&self) -> &Arc<dyn AgentStorePort> {
        &self.store
    }

    pub(crate) fn reader(&self) -> &Arc<dyn RolloutConversationStore> {
        &self.reader
    }

    pub(crate) fn events(&self) -> &Arc<AgentEventHub> {
        &self.events
    }

    /// Rollout true source accessor. Consumed by the harness `compact_thread`
    /// (truncate + `Compacted` append) and `rollback_thread` (single atomic
    /// `truncate_from_turn`) paths so those operations act on the rollout file
    /// directly instead of going through the store adapter's per-table deletes.
    pub(crate) fn rollout(&self) -> &Arc<slab_agent_rollout::RolloutFileStore> {
        &self.rollout
    }

    /// The configured trace directory, so the harness can apply the
    /// canonical root-vs-child `trace_path` rule when reconstructing a
    /// `SessionMeta` (J4). `None` when agent debugging is off.
    pub(crate) fn trace_dir(&self) -> Option<&Path> {
        self.trace_dir.as_deref()
    }

    /// Spawn a root agent thread. Returns the new thread ID.
    pub(crate) async fn spawn(
        &self,
        session_id: String,
        config: AgentConfig,
        messages: Vec<ConversationMessage>,
    ) -> Result<String, AppCoreError> {
        let thread_id = self
            .runtime
            .create_response(session_id, config, messages)
            .await
            .map_err(AppCoreError::from)?;
        self.ensure_rollout_persistence(&thread_id);
        Ok(thread_id)
    }

    /// Ensure exactly one rollout persistence observer is running for the
    /// thread. The first call for a given thread spawns it; subsequent calls
    /// (e.g. `send_input` resuming a thread) are no-ops. The observer runs for
    /// the process lifetime, capturing every finalized `TurnItem`, compaction
    /// marker, and allowed lifecycle event across all of the thread's runs.
    fn ensure_rollout_persistence(&self, real_thread_id: &str) {
        if self.rollout_observers.insert(real_thread_id.to_owned()) {
            rollout_persistence::spawn_rollout_persistence(
                Arc::clone(&self.rollout),
                Arc::clone(&self.events),
                real_thread_id.to_owned(),
                slab_agent_rollout::EventPersistenceMode::Limited,
            );
        }
    }

    /// Append a structured user message to an existing agent thread and run the
    /// next turn.
    ///
    /// The conversation read + sort + max-turn + user-content append
    /// is HOISTED here (out of slab-agent). slab-agent's `resume_thread` receives
    /// the pre-built message vec + the `emit_from` anchor (index of the first
    /// new message — the M5 within-turn attribution anchor that slab-agent emits
    /// as `MessageAppended` before the turn loop).
    ///
    /// Unlike [`send_input`], this carries the verbatim message content — so a
    /// `ConversationMessageContent::Parts` (e.g. text + image parts for VLM turns)
    /// reaches the chat pipeline with structure intact, where `extract_image_parts`
    /// can decode the images for the mtmd projector.
    pub(crate) async fn send_input_message(
        &self,
        thread_id: &str,
        message: ConversationMessage,
    ) -> Result<bool, AppCoreError> {
        // Steering: a RUNNING thread queues the input for the next iteration
        // boundary instead of hard-failing with ThreadBusy. The run loop
        // drains it after the current LLM call / tool batch (needs_follow_up =
        // model wants more OR queue non-empty).
        match self.runtime().control().queue_input(thread_id, message.clone()).await {
            Ok(slab_agent::SendOutcome::Queued { position }) => {
                tracing::debug!(thread_id, position, "steering input queued for the running turn");
                return Ok(true);
            }
            Ok(slab_agent::SendOutcome::NeedsResume) => {}
            Err(error) => return Err(AppCoreError::from(error)),
        }

        let mut records = self.reader().list_thread_messages(thread_id).await?;
        records.sort_by(|left, right| {
            left.turn_index
                .cmp(&right.turn_index)
                .then_with(|| left.created_at.cmp(&right.created_at))
                .then_with(|| left.id.cmp(&right.id))
        });
        let starting_turn_index =
            records.iter().map(|record| record.turn_index).max().map_or(0, |index| index + 1);
        let mut messages: Vec<ConversationMessage> =
            records.into_iter().map(|record| record.message).collect();
        messages.push(message.clone());
        // Exactly ONE message is new (the user input above). A trailing COUNT,
        // not an absolute index — the OnAgentStart init-batch merge inside
        // `run()` shifts history positions, and an absolute anchor would drift
        // and re-emit a tail of old messages as `MessageAppended` (duplicating
        // them in the rollout).
        match self
            .runtime
            .resume_thread(thread_id, messages, starting_turn_index, Some(1))
            .await
            .map_err(AppCoreError::from)
        {
            Ok(()) => {}
            // Lost the idle-window race: another starter made the thread live
            // between the queue probe and the resume — the input now belongs
            // in that run's steering queue.
            Err(AppCoreError::TooManyRequests(detail)) if detail.contains("already running") => {
                return match self.runtime().control().queue_input(thread_id, message).await {
                    Ok(slab_agent::SendOutcome::Queued { .. }) => Ok(true),
                    // Went idle again in the meantime — surface the original
                    // busy error; a retry from the caller will take the
                    // resume path cleanly.
                    Ok(slab_agent::SendOutcome::NeedsResume) => {
                        Err(AppCoreError::TooManyRequests(detail))
                    }
                    Err(error) => Err(AppCoreError::from(error)),
                };
            }
            Err(error) => return Err(error),
        }
        self.ensure_rollout_persistence(thread_id);
        Ok(false)
    }

    /// Append plain-text user input to an existing agent thread and run the next
    /// turn. Thin structured wrapper over [`send_input_message`] preserving the
    /// historical text-only call sites byte-for-byte.
    pub(crate) async fn send_input(
        &self,
        thread_id: &str,
        content: String,
    ) -> Result<(), AppCoreError> {
        self.send_input_message(
            thread_id,
            ConversationMessage {
                role: "user".to_owned(),
                content: ConversationMessageContent::Text(content),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
        )
        .await
        .map(|_| ())
    }

    /// Restore the latest root thread for a chat session and its persisted messages.
    pub(crate) async fn restore_session(
        &self,
        session_id: &str,
    ) -> Result<RestoredAgentSession, AppCoreError> {
        let thread = self.list_session_threads(session_id).await?.into_iter().next();
        let messages = match thread.as_ref() {
            Some(thread) => {
                // Cross-turn barrier — a thread that was active
                // shortly before restore may still have its observer draining.
                // Wait for quiescence so the replayed messages reflect the
                // latest emitted conversation data.
                self.await_durable(&thread.id).await;
                self.list_thread_messages(&thread.id).await?
            }
            None => Vec::new(),
        };
        // response_json persistence was removed — only complete messages and
        // turn state are stored now. The field is retained for interface
        // stability (`/v1/sessions/{id}/agent-history` still returns it, empty).
        let responses = Vec::new();
        Ok(RestoredAgentSession { thread, messages, responses })
    }

    /// List persisted root agent threads for a chat session, newest first.
    async fn list_session_threads(
        &self,
        session_id: &str,
    ) -> Result<Vec<ThreadSnapshot>, AppCoreError> {
        self.store
            .list_session_threads(session_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))
    }

    /// List persisted messages for a thread in replay order.
    ///
    /// Reads now flow through the `RolloutConversationStore` reader
    /// (rollout is the sole conversation source), not the slab-agent store trait.
    pub(crate) async fn list_thread_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AppCoreError> {
        if self
            .store
            .get_thread(thread_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?
            .is_none()
        {
            return Err(AppCoreError::NotFound(format!("agent thread not found: {thread_id}")));
        }

        self.reader()
            .list_thread_messages(thread_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))
    }

    /// Fetch a thread snapshot or fail with `NotFound`.
    pub(crate) async fn get_thread_snapshot(
        &self,
        thread_id: &str,
    ) -> Result<ThreadSnapshot, AppCoreError> {
        self.store
            .get_thread(thread_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?
            .ok_or_else(|| AppCoreError::NotFound(format!("agent thread not found: {thread_id}")))
    }

    /// Get the current status of an agent thread.
    ///
    /// First checks the in-memory registry (for live threads), then falls back
    /// to the persisted snapshot so callers polling after completion still get
    /// an accurate status rather than a 404.
    #[allow(dead_code)]
    pub(crate) async fn get_status(
        &self,
        thread_id: &str,
    ) -> Result<slab_types::agent::AgentThreadStatus, AppCoreError> {
        // Try the live in-memory registry first.
        match self.runtime.subscribe(thread_id).await {
            Ok(rx) => {
                return Ok(*rx.borrow());
            }
            Err(AgentError::ThreadNotFound(_)) => {
                // Thread has already finished and was removed from the registry.
                // Fall through to the DB lookup below.
            }
            Err(e) => return Err(AppCoreError::from(e)),
        }

        // Fallback: look up the persisted snapshot.
        match self.store.get_thread(thread_id).await {
            Ok(Some(snapshot)) => Ok(snapshot.status),
            Ok(None) => Err(AppCoreError::NotFound(format!("agent thread not found: {thread_id}"))),
            Err(e) => Err(AppCoreError::Internal(e.to_string())),
        }
    }

    /// Subscribe to the harness-protocol (`EventMsg`) stream for a thread.
    ///
    /// Returns a replay+live subscription carrying slab-agent's harness protocol
    /// (turn lifecycle / text / reasoning / tool items). Consumed by the harness
    /// WS fan-out and turn-item persistence.
    pub(crate) fn subscribe_event_msgs(&self, thread_id: &str) -> AgentEventMsgSubscription {
        self.events.subscribe_event_msgs(thread_id)
    }

    /// Cross-turn durability barrier. Enqueue a FIFO sentinel on
    /// the persistence channel and await the observer's reply — which (FIFO
    /// ordering) means EVERY persistence event emitted for `thread_id` so far
    /// has been appended + flushed to the rollout. Call this BEFORE any
    /// cross-turn re-read that must reflect already-emitted events
    /// (`fork_thread` / `compact_thread` / `rollback_thread` / `restore_session`).
    ///
    /// Within a turn, FIFO ordering of the DEDICATED UNBOUNDED persistence mpsc
    /// is the guarantee (no await needed). ACROSS turns, this barrier closes the
    /// timing window deterministically: the sentinel fences exactly the events
    /// already emitted, with no quiescence/timing heuristic (which a slow
    /// observer would defeat). If no observer is running for the thread (e.g. a
    /// session restored from disk whose observer never started), there is nothing
    /// to wait for — just flush the recorder and return. A bounded timeout keeps
    /// a still-running thread (fork of a live parent) from blocking forever.
    pub(crate) async fn await_durable(&self, thread_id: &str) {
        if let Some(rx) = self.events.persistence_barrier(thread_id) {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(10), rx).await;
        }
        // Belt-and-suspenders flush of the recorder (the barrier flush already
        // ran inside the observer; this catches any tail append a concurrent
        // writer made after the barrier reply).
        let _ = self.rollout.flush(thread_id).await;
    }
}
