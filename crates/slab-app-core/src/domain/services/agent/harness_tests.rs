//! Integration tests for `HarnessService` fork / compact / rollback (W3 H1/H3/H4).
//!
//! These drive the REAL `slab_agent::AgentControl::fork_thread` +
//! `HarnessService::fork_thread` path against a real
//! [`RolloutBackedAgentStore`] adapter over a real [`RolloutFileStore`], backed
//! by an in-memory SQL-delegate + `RolloutIndex` mock. The earlier store-level
//! fork unit test built the child `SessionMeta` by hand and bypassed
//! `control.fork_thread` — that is why it stayed green while the production
//! path lost the child header on every fork (H1).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use slab_agent::compact::{CompactContext, CompactOutcome, CompactPort};
use slab_agent::config::AgentConfig;
use slab_agent::port::{
    AgentNotifyPort, AgentStorePort, ApprovalDecision, ApprovalPort, LlmPort, LlmResponse,
    ThreadListFilter, ThreadMessageRecord, ThreadSnapshot, ThreadStatus, TurnItemRecord,
    TurnStateRecord,
};
use slab_agent::{
    AgentControl, AgentError, AgentRuntime, OperationDescriptor, ToolRiskAssessment, ToolRouter,
};
use slab_agent_rollout::{RolloutItem, RolloutStore, read_rollout_lines};
use slab_types::{ConversationMessage, ConversationMessageContent};

use super::AgentCore;
use super::RolloutConversationReader;
use super::harness::HarnessService;
use crate::infra::agent::event_hub::AgentEventHub;
use crate::infra::agent::rollout_store::RolloutBackedAgentStore;
use crate::infra::db::repository::rollout_index::RolloutIndex;

// ── in-memory SQL delegate + RolloutIndex mock ──────────────────────────────

/// Minimal in-memory mock that implements `AgentStorePort` + `RolloutIndex` +
/// `RolloutConversationReader`. Stores thread metadata, messages, items, and
/// states. Slice E dropped the legacy/backfill surface (`thread_has_legacy_data`
/// and the backfill/lease methods): rollout is the only source, so the mock no
/// longer tracks a per-thread legacy flag.
struct HarnessMockStore {
    threads: Mutex<HashMap<String, ThreadSnapshot>>,
    messages: Mutex<Vec<ThreadMessageRecord>>,
    items: Mutex<Vec<TurnItemRecord>>,
    states: Mutex<Vec<TurnStateRecord>>,
    backfill: Mutex<HashMap<String, String>>,
}

impl HarnessMockStore {
    fn new() -> Self {
        Self {
            threads: Mutex::new(HashMap::new()),
            messages: Mutex::new(Vec::new()),
            items: Mutex::new(Vec::new()),
            states: Mutex::new(Vec::new()),
            backfill: Mutex::new(HashMap::new()),
        }
    }

    /// Seed a thread WITHOUT creating a rollout file — used by the H4 tests to
    /// drive the `!file_exists` guard (compact/rollback refuse a thread whose
    /// rollout file has not materialized).
    fn seed_legacy_thread(&self, snapshot: &ThreadSnapshot, messages: Vec<ThreadMessageRecord>) {
        self.threads.lock().unwrap().insert(snapshot.id.clone(), snapshot.clone());
        self.messages.lock().unwrap().extend(messages);
    }
}

#[async_trait]
impl RolloutIndex for HarnessMockStore {
    async fn rollout_backfill_status(&self, thread_id: &str) -> sqlx::Result<Option<String>> {
        Ok(self.backfill.lock().unwrap().get(thread_id).cloned())
    }

    async fn rollout_backfill_progress_for(
        &self,
        thread_ids: &[String],
    ) -> sqlx::Result<std::collections::HashMap<String, (Option<String>, i64)>> {
        // This mock tracks only `backfill_status` (no line_count); report
        // line_count = 0 for every seeded thread. The harness tests this mock
        // backs do not exercise list-path ghost exclusion.
        let map = self.backfill.lock().unwrap();
        Ok(thread_ids
            .iter()
            .filter_map(|id| map.get(id).map(|s| (id.clone(), (Some(s.clone()), 0))))
            .collect())
    }

    async fn mark_rollout_session(
        &self,
        thread_id: &str,
        _session_id: &str,
        _file_path: &str,
        _last_turn_index: u32,
        _last_item_id: Option<&str>,
        _line_count: u32,
        backfill_status: &str,
    ) -> sqlx::Result<()> {
        self.backfill.lock().unwrap().insert(thread_id.to_owned(), backfill_status.to_owned());
        Ok(())
    }
}

#[async_trait]
impl AgentStorePort for HarnessMockStore {
    async fn upsert_thread(&self, snapshot: &ThreadSnapshot) -> Result<(), AgentError> {
        self.threads.lock().unwrap().insert(snapshot.id.clone(), snapshot.clone());
        Ok(())
    }
    async fn get_thread(&self, id: &str) -> Result<Option<ThreadSnapshot>, AgentError> {
        Ok(self.threads.lock().unwrap().get(id).cloned())
    }
    async fn list_session_threads(
        &self,
        session_id: &str,
    ) -> Result<Vec<ThreadSnapshot>, AgentError> {
        Ok(self
            .threads
            .lock()
            .unwrap()
            .values()
            .filter(|t| t.session_id == session_id)
            .cloned()
            .collect())
    }
    async fn list_session_threads_filtered(
        &self,
        session_id: &str,
        _filter: &ThreadListFilter,
    ) -> Result<Vec<ThreadSnapshot>, AgentError> {
        self.list_session_threads(session_id).await
    }
    async fn update_thread_status(
        &self,
        id: &str,
        status: ThreadStatus,
        completion_text: Option<&str>,
    ) -> Result<(), AgentError> {
        let mut threads = self.threads.lock().unwrap();
        if let Some(t) = threads.get_mut(id) {
            t.status = status;
            t.completion_text = completion_text.map(str::to_owned);
        }
        Ok(())
    }
    async fn insert_thread_message(&self, record: &ThreadMessageRecord) -> Result<(), AgentError> {
        self.messages.lock().unwrap().push(record.clone());
        Ok(())
    }
    async fn list_thread_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AgentError> {
        Ok(self
            .messages
            .lock()
            .unwrap()
            .iter()
            .filter(|m| m.thread_id == thread_id)
            .cloned()
            .collect())
    }
    async fn upsert_turn_state(&self, record: &TurnStateRecord) -> Result<(), AgentError> {
        let mut states = self.states.lock().unwrap();
        if let Some(existing) = states
            .iter_mut()
            .find(|s| s.thread_id == record.thread_id && s.turn_index == record.turn_index)
        {
            *existing = record.clone();
        } else {
            states.push(record.clone());
        }
        Ok(())
    }
    async fn archive_thread(&self, id: &str, archived_at: Option<&str>) -> Result<(), AgentError> {
        let mut threads = self.threads.lock().unwrap();
        if let Some(t) = threads.get_mut(id) {
            t.archived_at = archived_at.map(str::to_owned);
        }
        Ok(())
    }
}

// Slice E: list_turn_states / list_turn_items moved off `AgentStorePort` onto
// the app-core-internal `RolloutConversationReader` trait.
#[async_trait]
impl RolloutConversationReader for HarnessMockStore {
    async fn list_turn_states(&self, thread_id: &str) -> Result<Vec<TurnStateRecord>, AgentError> {
        Ok(self
            .states
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.thread_id == thread_id)
            .cloned()
            .collect())
    }
    async fn list_turn_items(&self, thread_id: &str) -> Result<Vec<TurnItemRecord>, AgentError> {
        Ok(self
            .items
            .lock()
            .unwrap()
            .iter()
            .filter(|i| i.thread_id == thread_id)
            .cloned()
            .collect())
    }
}

// ── stub ports (fork/compact/rollback never invoke the LLM or approval) ─────

struct StubLlm;

#[async_trait]
impl LlmPort for StubLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[slab_agent::port::ToolSpec],
        _config: &AgentConfig,
        _trace_context: &slab_agent_tracing::AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        Err(AgentError::Internal("StubLlm is not invoked by fork/compact".to_owned()))
    }
}

struct NoopNotify;

#[async_trait]
impl AgentNotifyPort for NoopNotify {
    async fn on_status_change(&self, _thread_id: &str, _status: ThreadStatus) {}
}

#[async_trait]
impl ApprovalPort for NoopNotify {
    async fn request_approval(
        &self,
        _thread_id: &str,
        _call_id: &str,
        _tool_name: &str,
        _descriptor: &OperationDescriptor,
        _risk: Option<ToolRiskAssessment>,
    ) -> ApprovalDecision {
        ApprovalDecision::Approved(slab_agent::ApprovalScope::RunOnce)
    }
}

/// Scripted compaction policy: always returns a `Replaced` outcome carrying a
/// fixed summary, so `compact_thread` reaches the rollout rewrite (H3) without a
/// real LLM. The `output_tokens`/`replaced_messages` are reported back verbatim.
struct ScriptedCompact {
    summary: ConversationMessage,
}

#[async_trait]
impl CompactPort for ScriptedCompact {
    fn threshold_tokens(&self) -> usize {
        1
    }
    fn estimate_tokens(&self, _messages: &[ConversationMessage]) -> usize {
        999
    }
    async fn compact(
        &self,
        messages: &[ConversationMessage],
        _ctx: &CompactContext<'_>,
    ) -> Result<CompactOutcome, AgentError> {
        Ok(CompactOutcome::Replaced {
            messages: vec![self.summary.clone()],
            output_tokens: 7,
            replaced_messages: messages.len(),
        })
    }
}

/// Compaction policy that always skips — used by the fork/H4 tests where the
/// CompactPort is never actually invoked (fork never compacts; H4 refuse tests
/// error before compaction runs).
struct SkipCompact;

#[async_trait]
impl CompactPort for SkipCompact {
    fn threshold_tokens(&self) -> usize {
        usize::MAX
    }
    fn estimate_tokens(&self, _messages: &[ConversationMessage]) -> usize {
        0
    }
    async fn compact(
        &self,
        _messages: &[ConversationMessage],
        _ctx: &CompactContext<'_>,
    ) -> Result<CompactOutcome, AgentError> {
        Ok(CompactOutcome::Skipped { reason: "test skip".to_owned() })
    }
}

// ── harness builder ─────────────────────────────────────────────────────────

struct TestHarness {
    harness: HarnessService,
    store: Arc<dyn AgentStorePort>,
    rollout: Arc<slab_agent_rollout::RolloutFileStore>,
    mock: Arc<HarnessMockStore>,
    _dir: tempfile::TempDir,
}

impl TestHarness {
    /// Build a harness with the given compaction policy over a rollout-native
    /// adapter. The returned `store` handle is the SAME adapter wired into the
    /// runtime, so a test can seed rollout-native history through it.
    async fn build(compact: Arc<dyn CompactPort>) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let rollout = Arc::new(slab_agent_rollout::RolloutFileStore::new(dir.path().to_owned()));
        let mock = Arc::new(HarnessMockStore::new());
        let store_adapter: Arc<RolloutBackedAgentStore> = Arc::new(RolloutBackedAgentStore::new(
            Arc::clone(&mock) as Arc<dyn AgentStorePort>,
            Arc::clone(&mock) as Arc<dyn RolloutIndex>,
            Arc::clone(&rollout),
            None,
        ));
        let event_hub = Arc::new(AgentEventHub::new());
        let notify: Arc<dyn AgentNotifyPort> = Arc::new(NoopNotify);
        let approval: Arc<dyn ApprovalPort> = Arc::new(NoopNotify);
        let control = Arc::new(AgentControl::new(
            Arc::new(StubLlm) as Arc<dyn LlmPort>,
            Arc::clone(&store_adapter) as Arc<dyn AgentStorePort>,
            notify,
            approval,
            Arc::new(ToolRouter::new()),
            8,
            8,
        ));
        let runtime = AgentRuntime::new(control);
        let core = AgentCore::new(
            runtime,
            Arc::clone(&store_adapter) as Arc<dyn AgentStorePort>,
            event_hub,
            compact,
            rollout.clone(),
            Arc::clone(&store_adapter) as Arc<dyn RolloutConversationReader>,
            None,
        );
        let harness = HarnessService::new(core);
        Self { harness, store: store_adapter as Arc<dyn AgentStorePort>, rollout, mock, _dir: dir }
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn user_msg(text: &str) -> ConversationMessage {
    ConversationMessage {
        role: "user".to_owned(),
        content: ConversationMessageContent::Text(text.to_owned()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }
}

fn agent_config() -> AgentConfig {
    AgentConfig { model: "test-model".to_owned(), max_depth: 8, ..Default::default() }
}

fn snapshot(id: &str, session: &str) -> ThreadSnapshot {
    let config_json = serde_json::to_string(&agent_config()).unwrap();
    ThreadSnapshot {
        id: id.to_owned(),
        session_id: session.to_owned(),
        parent_id: None,
        depth: 0,
        status: ThreadStatus::Pending,
        role_name: None,
        config_json,
        completion_text: None,
        created_at: "2026-08-02T00:00:00Z".to_owned(),
        updated_at: "2026-08-02T00:00:00Z".to_owned(),
        archived_at: None,
    }
}

/// Seed a rollout-native parent thread (via the adapter so the rollout file is
/// created + the read gate flips) with interleaved per-turn history: per turn a
/// user MessageAppend then an assistant TurnItem.
async fn seed_rollout_native_parent(
    store: &Arc<dyn AgentStorePort>,
    rollout: &slab_agent_rollout::RolloutFileStore,
    parent_id: &str,
    session: &str,
    turns: u32,
) {
    store.upsert_thread(&snapshot(parent_id, session)).await.expect("upsert parent");
    for turn in 0..turns {
        store
            .insert_thread_message(&ThreadMessageRecord {
                id: format!("m{parent_id}-{turn}"),
                thread_id: parent_id.to_owned(),
                turn_index: turn,
                message: user_msg(&format!("u{turn}")),
                created_at: "2026-08-02T00:00:00Z".to_owned(),
            })
            .await
            .expect("insert message");
        let item = slab_agent::protocol::TurnItem::AgentMessage {
            id: format!("a{parent_id}-{turn}"),
            text: format!("r{turn}"),
        };
        // Slice E: TurnItems are written via the rollout directly (the adapter's
        // `insert_turn_item` was removed; production writes via the rollout
        // persistence observer).
        rollout.append(parent_id, RolloutItem::TurnItem(item)).await.expect("append turn item");
    }
    // Materialize the rollout file so file_exists flips true (H4 guard).
    rollout.flush(parent_id).await.expect("flush parent");
}

// ── H1 (headline): fork_thread preserves the child SessionMeta ─────────────

// The pre-fix path read the child rollout file to recover the child SessionMeta,
// but control.fork_thread had only buffered AddItems into the child recorder
// (NOT flushed) — the file was absent, child_meta_line was None, and the
// wholesale rewrite produced a header-less child. This drives the REAL
// control.fork_thread + harness.fork_thread and asserts the child header
// survives with parent_id provenance.
#[tokio::test]
async fn h1_fork_preserves_child_session_meta_header() {
    let compact: Arc<dyn CompactPort> = Arc::new(SkipCompact);
    let th = TestHarness::build(compact).await;
    let parent_id = "parent-h1";
    seed_rollout_native_parent(&th.store, &th.rollout, parent_id, "sess-h1", 2).await;

    let child = th.harness.fork_thread(parent_id, None).await.expect("fork succeeds");

    // H1: the child rollout file starts with a SessionMeta carrying parent_id +
    // the child session id + the child thread id (none of these were recovered
    // pre-fix because the child file was never materialized before the read).
    let child_meta = th
        .rollout
        .read_session_meta(&child.id)
        .await
        .expect("H1: child SessionMeta header must be present after fork");
    assert_eq!(
        child_meta.parent_id,
        Some(parent_id.to_owned()),
        "H1: parent_id provenance preserved on the child header"
    );
    assert_eq!(child_meta.thread_id, child.id, "header thread_id matches the child");
    assert_eq!(child_meta.session_id, "sess-h1", "header session_id preserved");

    // The child file's first physical line is the SessionMeta header.
    let lines = read_rollout_lines(&th.rollout.resolve_path(&child.id));
    assert!(!lines.is_empty(), "child rollout file is non-empty");
    assert!(
        matches!(lines[0].item, RolloutItem::SessionMeta(_)),
        "H1: first line is the SessionMeta header"
    );

    // H2 corollary: the wholesale rewrite also fixes turn attribution that the
    // batched adapter copy would have broken (a0 must attribute to turn 0, not
    // the last turn). The child replays the parent's attribution-correct items.
    let child_items = th.rollout.read_turn_items(&child.id).await;
    let attribution: Vec<(u32, &str)> =
        child_items.iter().map(|i| (i.turn_index, i.id.as_str())).collect();
    assert_eq!(
        attribution,
        vec![(0, "aparent-h1-0"), (1, "aparent-h1-1")],
        "H2: wholesale copy preserves interleaved turn attribution"
    );
}

// ── H3: compact_thread is durable as a unit (atomic rewrite) ───────────────

// After compact_thread returns, the rollout file must hold exactly
// [SessionMeta, Compacted] (durable — no truncate-then-pending-append window).
#[tokio::test]
async fn h3_compact_thread_leaves_atomic_session_meta_plus_compacted() {
    let compact: Arc<dyn CompactPort> = Arc::new(ScriptedCompact {
        summary: ConversationMessage {
            role: "system".to_owned(),
            content: ConversationMessageContent::Text("compact-summary".to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        },
    });
    let th = TestHarness::build(compact).await;
    let thread_id = "thread-h3";
    seed_rollout_native_parent(&th.store, &th.rollout, thread_id, "sess-h3", 3).await;

    let (snapshot, removed, tokens) =
        th.harness.compact_thread(thread_id, None).await.expect("compact succeeds");
    assert_eq!(snapshot.id, thread_id);
    assert!(removed > 0, "compaction removed messages");
    assert_eq!(tokens, 7);

    // H3: the file holds EXACTLY [SessionMeta, Compacted] right after return
    // (no intermediate truncated state, no pending Compacted).
    let lines = read_rollout_lines(&th.rollout.resolve_path(thread_id));
    assert_eq!(lines.len(), 2, "H3: exactly [SessionMeta, Compacted] on disk");
    assert!(matches!(lines[0].item, RolloutItem::SessionMeta(_)));
    match &lines[1].item {
        RolloutItem::Compacted(p) => {
            assert_eq!(p.status, "manual");
            assert_eq!(p.turn_index, 0);
            assert_eq!(p.compacted_messages.len(), 1);
        }
        other => panic!("H3: expected Compacted line, got {other:?}"),
    }

    // read_session_meta still resolves (the header survived the rewrite).
    let meta = th.rollout.read_session_meta(thread_id).await.expect("header present");
    assert_eq!(meta.thread_id, thread_id);

    // read_messages returns the compacted baseline (the old turns are gone).
    let msgs = th.rollout.read_messages(thread_id).await;
    assert_eq!(msgs.len(), 1, "compacted baseline replaces history");
}

// ── H4: compact_thread / rollback_thread refuse a thread with no rollout file ─

#[tokio::test]
async fn h4_compact_thread_refuses_legacy_thread() {
    let compact: Arc<dyn CompactPort> = Arc::new(SkipCompact);
    let th = TestHarness::build(compact).await;
    let thread_id = "legacy-compact";
    let snap = snapshot(thread_id, "sess-h4c");
    th.mock.seed_legacy_thread(
        &snap,
        vec![ThreadMessageRecord {
            id: "lm".to_owned(),
            thread_id: thread_id.to_owned(),
            turn_index: 0,
            message: user_msg("legacy"),
            created_at: "2026-08-02T00:00:00Z".to_owned(),
        }],
    );
    // No rollout file → not rollout-native.
    assert!(!th.rollout.file_exists(thread_id).await);

    let err = th.harness.compact_thread(thread_id, None).await;
    assert!(
        err.is_err(),
        "H4: compact_thread must refuse a thread with no rollout file, not silently no-op"
    );
    let msg = format!("{}", err.unwrap_err());
    assert!(
        msg.contains("no rollout file"),
        "H4: refuse message explains the missing rollout file: {msg}"
    );
}

#[tokio::test]
async fn h4_rollback_thread_refuses_legacy_thread() {
    let compact: Arc<dyn CompactPort> = Arc::new(SkipCompact);
    let th = TestHarness::build(compact).await;
    let thread_id = "legacy-rollback";
    let snap = snapshot(thread_id, "sess-h4r");
    th.mock.seed_legacy_thread(
        &snap,
        vec![ThreadMessageRecord {
            id: "lm".to_owned(),
            thread_id: thread_id.to_owned(),
            turn_index: 0,
            message: user_msg("legacy"),
            created_at: "2026-08-02T00:00:00Z".to_owned(),
        }],
    );
    assert!(!th.rollout.file_exists(thread_id).await);

    let err = th.harness.rollback_thread(thread_id, 0).await;
    assert!(
        err.is_err(),
        "H4: rollback_thread must refuse a thread with no rollout file, not silently no-op"
    );
    let msg = format!("{}", err.unwrap_err());
    assert!(
        msg.contains("no rollout file"),
        "H4: refuse message explains the missing rollout file: {msg}"
    );
}

// ── H4: rollback on a rollout-native thread still works (happy path) ────────

#[tokio::test]
async fn h4_rollback_rollout_native_thread_truncates() {
    let compact: Arc<dyn CompactPort> = Arc::new(SkipCompact);
    let th = TestHarness::build(compact).await;
    let thread_id = "native-rollback";
    seed_rollout_native_parent(&th.store, &th.rollout, thread_id, "sess-rb", 3).await;

    // Roll back to turn 0 (drop turn 1+).
    let snap = th.harness.rollback_thread(thread_id, 0).await.expect("rollback succeeds");
    assert_eq!(snap.id, thread_id);

    let items = th.rollout.read_turn_items(thread_id).await;
    let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, vec!["anative-rollback-0"], "H4 happy path: turns 1+ dropped");
    // Header survived.
    assert!(th.rollout.read_session_meta(thread_id).await.is_some());
}

// ── HarnessService::list_turn_states / list_turn_items reader delegation ────

// `HarnessService::list_turn_states` / `list_turn_items` are one-line delegations
// to `core.reader().list_turn_states/items(...)`. The underlying
// `RolloutBackedAgentStore` reader impl has its own unit coverage, but WITHOUT
// this test the wrapper delegation is unguarded — a future refactor that drops
// the `self.0.reader()` delegation (e.g. returns an empty Vec, or routes through
// a different surface) would silently break `thread/resume` while every other
// harness test stays green. Seeds a rollout-native parent with one TurnState +
// one TurnItem, then asserts the wrapper returns both (mutation guard below).
#[tokio::test]
async fn harness_list_turn_states_and_items_delegate_to_reader() {
    let compact: Arc<dyn CompactPort> = Arc::new(SkipCompact);
    let th = TestHarness::build(compact).await;
    let parent_id = "reader-delegate";
    seed_rollout_native_parent(&th.store, &th.rollout, parent_id, "sess-rd", 1).await;
    // seed_rollout_native_parent writes a MessageAppend + a TurnItem per turn but
    // no TurnState — write one through the adapter so list_turn_states has a row
    // to find via the reader delegation.
    th.store
        .upsert_turn_state(&TurnStateRecord {
            thread_id: parent_id.to_owned(),
            turn_index: 0,
            status: "completed".to_owned(),
            input_messages_json: None,
            tool_specs_json: None,
            llm_response_json: None,
            error: None,
            started_at: "2026-08-02T00:00:00Z".to_owned(),
            completed_at: Some("2026-08-02T00:00:05Z".to_owned()),
        })
        .await
        .expect("upsert turn state");

    // The wrapper delegation reads through `core.reader()` → the adapter → the
    // rollout replay. Both must return the seeded row.
    let states = th.harness.list_turn_states(parent_id).await.expect("list turn states");
    assert_eq!(
        states.len(),
        1,
        "list_turn_states delegated to the reader and observed the seeded TurnState",
    );
    assert_eq!(states[0].turn_index, 0);

    let items = th.harness.list_turn_items(parent_id).await.expect("list turn items");
    assert_eq!(
        items.len(),
        1,
        "list_turn_items delegated to the reader and observed the seeded TurnItem",
    );
    assert_eq!(items[0].id, "areader-delegate-0");
}
