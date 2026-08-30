//! Top-level agent controller — manages all active agent threads.

use std::{collections::HashMap, collections::VecDeque, sync::Arc};

use tokio::{
    sync::{RwLock, watch},
    time::{Duration, sleep},
};
use tokio_util::sync::CancellationToken;
use tracing::warn;

use chrono::Utc;
use slab_agent_tracing::{AgentTraceSink, NoopAgentTraceSink};
use slab_types::ConversationMessage;
use uuid::Uuid;

use crate::{
    compact::{CompactPort, SlidingWindowCompactPort},
    concurrency_gate::ConcurrencyGate,
    config::AgentConfig,
    error::AgentError,
    hook::{AgentHook, AgentHookRegistry},
    port::{AgentNotifyPort, AgentStorePort, ApprovalPort, LlmPort, ThreadSnapshot, ThreadStatus},
    protocol::{EventMsg, ThreadStatusChangedParams},
    risk::{BasicToolRiskAnalyzer, ToolRiskAnalyzer},
    state::ThreadStateMachine,
    thread::{AgentThread, AgentThreadRuntime},
    tool::{AgentThreadContext, ToolDiscoveryState, ToolRouter},
};

// ── Internal handle stored per active thread ─────────────────────────────────

/// Registry entry for a thread known to the controller.
///
/// `Reserved` closes the resume TOCTOU: the busy check and the registry
/// insert happen atomically under one write lock, BEFORE the async spawn work
/// (store fetch, gate admission, runtime assembly). Previously the check ran
/// against a read lock and the entry only appeared after the task spawned —
/// two concurrent `turn/start` calls could both pass and double-run the
/// thread. Interrupt/shutdown on a reserved slot act on the state machine
/// alone; the spawned run observes the resulting status at startup and ends
/// before its first turn.
enum ThreadEntry {
    Reserved {
        state: Arc<ThreadStateMachine>,
    },
    Live {
        status_rx: watch::Receiver<ThreadStatus>,
        state: Arc<ThreadStateMachine>,
        join: Option<tokio::task::JoinHandle<Result<String, AgentError>>>,
        abort: tokio::task::AbortHandle,
        cancellation: CancellationToken,
        /// Steering input queued while the run is in flight; drained by the
        /// run loop at the next iteration boundary (see
        /// [`AgentControl::queue_input`]).
        pending_input: Arc<std::sync::Mutex<VecDeque<ConversationMessage>>>,
    },
}

impl ThreadEntry {
    fn state(&self) -> &Arc<ThreadStateMachine> {
        match self {
            Self::Reserved { state } | Self::Live { state, .. } => state,
        }
    }
}

/// How long a graceful shutdown waits for the run tail (terminal-status
/// persist + `OnAgentEnd` hooks, incl. host memory extraction) before
/// falling back to a hard task abort.
const SHUTDOWN_GRACE: std::time::Duration = std::time::Duration::from_secs(5);

/// Upper bound for [`AgentControl::wait_for_persisted_terminal_snapshot`]'s
/// poll loop — previously unbounded (100 ms forever) when a thread never
/// reached a terminal persisted status.
const TERMINAL_SNAPSHOT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Upper bound on queued steering input. When full the caller gets an
/// `AgentError::Internal("input queue full")` — an unbounded queue would let
/// a runaway client balloon a single run.
const MAX_QUEUED_INPUT: usize = 32;

/// Outcome of [`AgentControl::queue_input`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendOutcome {
    /// Thread idle — the caller owns the resume flow (the rollout read and
    /// `starting_turn_index` live in app-core) and must run it.
    NeedsResume,
    /// Thread busy — the message was queued and will be injected at the next
    /// iteration boundary, after the current LLM call / tool batch finishes.
    Queued { position: usize },
}

struct SpawnRequest {
    session_id: String,
    parent_id: Option<String>,
    depth: u32,
    config: AgentConfig,
    messages: Vec<ConversationMessage>,
    starting_turn_index: u32,
    emit_new: Option<usize>,
}

/// `emit_new` marker for "the whole (post-injection) context is new" — the
/// spawn paths persist the fresh thread's ENTIRE message vec (init batch
/// included) as `MessageAppended`. `run()`'s
/// `messages.len().saturating_sub(EMIT_ALL)` collapses to `0`, i.e. skip
/// nothing.
const EMIT_ALL: usize = usize::MAX;

/// Defensive upper bound on parent-chain walks (e.g. [`crate::thread::resolve_root_thread_id`]).
/// Well beyond any realistic `max_depth` (default 4, configurable per config) —
/// the only purpose is to terminate deterministically if a persisted parent
/// chain were ever cyclic or malformed. Normal walks stop at the root in
/// `depth` hops.
pub(crate) const MAX_SPAWN_DEPTH_GUARD: u32 = 64;

// ── AgentControl ─────────────────────────────────────────────────────────────

/// Top-level controller that owns and coordinates all active agent threads.
///
/// Inject the port adapters at construction time; the controller owns them for
/// its lifetime and shares them (via [`Arc`]) with every thread it spawns.
#[derive(Clone, Copy, Debug)]
pub struct AgentControlLimits {
    /// Hard cap on concurrently active threads across all nesting levels.
    pub max_threads: usize,
    /// Maximum allowed child nesting depth (inclusive, root threads are depth 0).
    pub max_depth: u32,
}

pub struct AgentControl {
    threads: Arc<RwLock<HashMap<String, ThreadEntry>>>,
    llm: Arc<dyn LlmPort>,
    store: Arc<dyn AgentStorePort>,
    notify: Arc<dyn AgentNotifyPort>,
    approval: Arc<dyn ApprovalPort>,
    exec_policy: Arc<dyn crate::port::ExecPolicyPort>,
    agent_registry: Arc<dyn crate::agent::AgentRegistry>,
    plan_store: Arc<dyn crate::port::PlanStorePort>,
    tool_router: Arc<ToolRouter>,
    hooks: AgentHookRegistry,
    compact: Arc<dyn CompactPort>,
    risk: Arc<dyn ToolRiskAnalyzer>,
    trace: Arc<dyn AgentTraceSink>,
    trace_dir: Option<std::path::PathBuf>,
    thread_context: AgentThreadContext,
    max_threads: usize,
    max_depth: u32,
    gate: Arc<ConcurrencyGate>,
    memory_pressure: Arc<dyn crate::port::MemoryPressurePort>,
}

impl AgentControl {
    /// Create a new controller with no hooks.
    ///
    /// - `max_threads`: hard cap on concurrently active threads (across all depths).
    /// - `max_depth`: maximum allowed child nesting depth (inclusive, 0-based; root
    ///   agents are depth 0).
    pub fn new(
        llm: Arc<dyn LlmPort>,
        store: Arc<dyn AgentStorePort>,
        notify: Arc<dyn AgentNotifyPort>,
        approval: Arc<dyn ApprovalPort>,
        tool_router: Arc<ToolRouter>,
        max_threads: usize,
        max_depth: u32,
    ) -> Self {
        Self::new_with_hooks(
            llm,
            store,
            notify,
            approval,
            tool_router,
            AgentControlLimits { max_threads, max_depth },
            vec![],
        )
    }

    /// Create a new controller with a pre-registered set of hooks.
    pub fn new_with_hooks(
        llm: Arc<dyn LlmPort>,
        store: Arc<dyn AgentStorePort>,
        notify: Arc<dyn AgentNotifyPort>,
        approval: Arc<dyn ApprovalPort>,
        tool_router: Arc<ToolRouter>,
        limits: AgentControlLimits,
        hooks: Vec<Arc<dyn AgentHook>>,
    ) -> Self {
        Self::new_with_hooks_and_tracing(
            llm,
            store,
            notify,
            approval,
            tool_router,
            limits,
            hooks,
            Arc::new(NoopAgentTraceSink),
            None,
        )
    }

    /// Create a new controller with hooks and an explicit trace sink.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_hooks_and_tracing(
        llm: Arc<dyn LlmPort>,
        store: Arc<dyn AgentStorePort>,
        notify: Arc<dyn AgentNotifyPort>,
        approval: Arc<dyn ApprovalPort>,
        tool_router: Arc<ToolRouter>,
        limits: AgentControlLimits,
        hooks: Vec<Arc<dyn AgentHook>>,
        trace: Arc<dyn AgentTraceSink>,
        trace_dir: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            threads: Arc::new(RwLock::new(HashMap::new())),
            llm,
            store,
            notify,
            approval,
            exec_policy: Arc::new(slab_exec_policy::AllowAllExecPolicy),
            plan_store: Arc::new(crate::port::NoopPlanStore),
            agent_registry: Arc::new(crate::agent::NoopAgentRegistry),
            tool_router,
            hooks: AgentHookRegistry::new(hooks),
            compact: Arc::new(SlidingWindowCompactPort::default()),
            risk: Arc::new(BasicToolRiskAnalyzer::default()),
            trace,
            trace_dir,
            thread_context: AgentThreadContext::default(),
            max_threads: limits.max_threads,
            max_depth: limits.max_depth,
            gate: Arc::new(ConcurrencyGate::new(limits.max_threads, 0)),
            memory_pressure: Arc::new(crate::port::NoopMemoryPressurePort),
        }
    }

    /// Create a new controller with explicit compact and risk-analysis ports.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_ports(
        llm: Arc<dyn LlmPort>,
        store: Arc<dyn AgentStorePort>,
        notify: Arc<dyn AgentNotifyPort>,
        approval: Arc<dyn ApprovalPort>,
        tool_router: Arc<ToolRouter>,
        limits: AgentControlLimits,
        compact: Arc<dyn CompactPort>,
        risk: Arc<dyn ToolRiskAnalyzer>,
    ) -> Self {
        Self {
            threads: Arc::new(RwLock::new(HashMap::new())),
            llm,
            store,
            notify,
            approval,
            exec_policy: Arc::new(slab_exec_policy::AllowAllExecPolicy),
            plan_store: Arc::new(crate::port::NoopPlanStore),
            agent_registry: Arc::new(crate::agent::NoopAgentRegistry),
            tool_router,
            hooks: AgentHookRegistry::default(),
            compact,
            risk,
            trace: Arc::new(NoopAgentTraceSink),
            trace_dir: None,
            thread_context: AgentThreadContext::default(),
            max_threads: limits.max_threads,
            max_depth: limits.max_depth,
            gate: Arc::new(ConcurrencyGate::new(limits.max_threads, 0)),
            memory_pressure: Arc::new(crate::port::NoopMemoryPressurePort),
        }
    }

    /// Attach host-provided thread context used when building tool contexts.
    pub fn with_thread_context(mut self, thread_context: AgentThreadContext) -> Self {
        self.thread_context = thread_context;
        self
    }

    /// Set the FIFO wait-queue capacity (INFRA-05). `0` (default) keeps the
    /// legacy behavior of rejecting spawns as soon as `max_threads` is reached;
    /// `> 0` lets that many excess spawns wait in arrival order before
    /// rejection. Rebuilds the admission gate with the current `max_threads`.
    pub fn with_queue_capacity(mut self, queue_capacity: usize) -> Self {
        self.gate = Arc::new(ConcurrencyGate::new(self.max_threads, queue_capacity));
        self
    }

    /// Number of spawns currently waiting for an admission slot (FIFO backlog).
    pub fn queued_thread_count(&self) -> usize {
        self.gate.waiting_count()
    }

    /// Attach a host-owned memory circuit breaker that gates spawns when
    /// process RSS exceeds the configured threshold (INFRA-05). When the breaker
    /// is tripped, [`spawn`](Self::spawn) / [`spawn_child`](Self::spawn_child)
    /// return [`AgentError::MemoryPressureExceeded`] until it clears.
    pub fn with_memory_pressure(mut self, port: Arc<dyn crate::port::MemoryPressurePort>) -> Self {
        self.memory_pressure = port;
        self
    }

    /// Attach the unified permission decision engine. When unset, a permissive
    /// [`slab_exec_policy::AllowAllExecPolicy`] stub is used (every operation
    /// allowed, nothing persisted) — suitable for tests but not production.
    pub fn with_exec_policy(mut self, port: Arc<dyn crate::port::ExecPolicyPort>) -> Self {
        self.exec_policy = port;
        self
    }

    /// Attach the context-compaction policy. When unset, a pure trailing-window
    /// [`SlidingWindowCompactPort`] is used. Hosts that want LLM-summarizing
    /// compaction inject their own [`CompactPort`] here.
    pub fn with_compact(mut self, compact: Arc<dyn CompactPort>) -> Self {
        self.compact = compact;
        self
    }

    /// Attach the per-thread plan store backing Plan interaction mode. When
    /// unset, a [`crate::port::NoopPlanStore`] stub is used (plans are not
    /// persisted) — suitable for tests but not production. The host (app-core)
    /// injects an in-memory per-thread store here.
    pub fn with_plan_store(mut self, plan_store: Arc<dyn crate::port::PlanStorePort>) -> Self {
        self.plan_store = plan_store;
        self
    }

    /// Attach the built-in agent registry. When unset, a
    /// [`crate::agent::NoopAgentRegistry`] stub is used — suitable for tests.
    /// The host (app-core) injects a populated registry here.
    pub fn with_agent_registry(mut self, registry: Arc<dyn crate::agent::AgentRegistry>) -> Self {
        self.agent_registry = registry;
        self
    }

    /// Access the agent registry (used by `delegate_subagent` to resolve
    /// `agent_type` into a definition).
    pub fn agent_registry(&self) -> Arc<dyn crate::agent::AgentRegistry> {
        Arc::clone(&self.agent_registry)
    }

    /// Spawn a root agent thread (depth 0).
    ///
    /// Returns the new thread's unique ID.
    pub async fn spawn(
        &self,
        session_id: String,
        config: AgentConfig,
        messages: Vec<ConversationMessage>,
    ) -> Result<String, AgentError> {
        self.spawn_inner(SpawnRequest {
            session_id,
            parent_id: None,
            depth: 0,
            config,
            messages,
            starting_turn_index: 0,
            emit_new: Some(EMIT_ALL),
        })
        .await
    }

    /// Spawn a child agent thread with an explicit parent and depth.
    ///
    /// Returns an error if `depth` exceeds `max_depth`.  `max_depth` is
    /// inclusive: a `max_depth` of 3 allows depths 0 through 3.
    pub async fn spawn_child(
        &self,
        session_id: String,
        parent_id: String,
        depth: u32,
        config: AgentConfig,
        messages: Vec<ConversationMessage>,
    ) -> Result<String, AgentError> {
        if depth > self.max_depth {
            return Err(AgentError::DepthLimitExceeded { current: depth, max: self.max_depth });
        }
        self.spawn_inner(SpawnRequest {
            session_id,
            parent_id: Some(parent_id),
            depth,
            config,
            messages,
            starting_turn_index: 0,
            emit_new: Some(EMIT_ALL),
        })
        .await
    }

    /// Spawn a child agent using an existing thread as its parent.
    pub async fn spawn_child_for_parent(
        &self,
        parent_thread_id: &str,
        config: AgentConfig,
        messages: Vec<ConversationMessage>,
    ) -> Result<String, AgentError> {
        let parent = self
            .store
            .get_thread(parent_thread_id)
            .await?
            .ok_or_else(|| AgentError::ThreadNotFound(parent_thread_id.to_owned()))?;
        let parent_config =
            serde_json::from_str::<AgentConfig>(&parent.config_json).map_err(|error| {
                AgentError::Internal(format!("failed to deserialize parent agent config: {error}"))
            })?;
        let depth = parent.depth + 1;
        if depth > parent_config.max_depth {
            return Err(AgentError::DepthLimitExceeded {
                current: depth,
                max: parent_config.max_depth,
            });
        }
        self.spawn_child(parent.session_id, parent.id, depth, config, messages).await
    }

    /// Fork a thread: create a child at `parent.depth + 1` whose persisted
    /// history (messages + turn states) is cloned from the parent, without
    /// running a new turn.
    ///
    /// The child is persisted with [`ThreadStatus::Pending`] and is not
    /// registered as a live thread — a later `send_input` / first turn
    /// materializes it, mirroring the lazy-spawn path. An optional
    /// `model_override` replaces the parent's model on the cloned config.
    /// Returns the new child thread id.
    pub async fn fork_thread(
        &self,
        parent_thread_id: &str,
        model_override: Option<String>,
    ) -> Result<String, AgentError> {
        let parent = self
            .store
            .get_thread(parent_thread_id)
            .await?
            .ok_or_else(|| AgentError::ThreadNotFound(parent_thread_id.to_owned()))?;
        let mut config = serde_json::from_str::<AgentConfig>(&parent.config_json).map_err(|e| {
            AgentError::Internal(format!("failed to deserialize parent agent config: {e}"))
        })?;
        if let Some(model) = model_override {
            config.model = model;
        }

        let depth = parent.depth + 1;
        if depth > config.max_depth {
            return Err(AgentError::DepthLimitExceeded { current: depth, max: config.max_depth });
        }

        let now = Utc::now().to_rfc3339();
        let child_id = Uuid::new_v4().to_string();
        let child = ThreadSnapshot {
            id: child_id.clone(),
            session_id: parent.session_id.clone(),
            parent_id: Some(parent.id.clone()),
            depth,
            status: ThreadStatus::Pending,
            role_name: parent.role_name.clone(),
            config_json: serde_json::to_string(&config).map_err(|e| {
                AgentError::Internal(format!("failed to serialize fork config: {e}"))
            })?,
            completion_text: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            archived_at: None,
        };
        self.store.upsert_thread(&child).await?;

        // NOTE: the parent's persisted history is NOT copied per-record here.
        // Fork production goes through the harness wholesale `rewrite_session`
        // path (see `HarnessService::fork_thread`), which snapshots the parent
        // rollout file into the child in correct interleaved order. The previous
        // per-record copy (messages → turn states → turn items through the store
        // adapter) was dead: it batched every TurnContext before every TurnItem,
        // breaking replay attribution, and was unconditionally overwritten by
        // the wholesale rewrite. The now-unused store methods it depended on
        // were removed (`list_turn_states` / `list_turn_items` / `insert_turn_item`).

        Ok(child_id)
    }

    /// Apply (or clear) a built-in agent override for the next turn on a thread.
    ///
    /// Resolved from `TurnStartParams.agent_type`. When `def` is `Some`, the
    /// thread's persisted [`AgentConfig`] is rewritten to carry the agent's
    /// `agent_type` + `system_prompt` so the next `resume_thread` runs as that
    /// agent (tool denylist via `allowed_tool_specs`, system prompt at turn 0).
    /// When `def` is `None`, both are cleared so the thread runs as the default
    /// agent. Re-applied every `turn_start` — this is the single chokepoint that
    /// keeps a thread from sticking as the plan agent after a plan is approved.
    pub async fn apply_agent_override(
        &self,
        thread_id: &str,
        def: Option<&crate::agent::AgentDefinition>,
    ) -> Result<(), AgentError> {
        let snapshot = self
            .store
            .get_thread(thread_id)
            .await?
            .ok_or_else(|| AgentError::ThreadNotFound(thread_id.to_owned()))?;
        let mut config =
            serde_json::from_str::<AgentConfig>(&snapshot.config_json).map_err(|e| {
                AgentError::Internal(format!("failed to deserialize agent config: {e}"))
            })?;
        match def {
            Some(def) => {
                config.agent_type = Some(def.agent_type.clone());
                config.system_prompt = Some(def.system_prompt.clone());
            }
            None => {
                config.agent_type = None;
                config.system_prompt = None;
            }
        }
        let config_json = serde_json::to_string(&config)
            .map_err(|e| AgentError::Internal(format!("failed to serialize agent config: {e}")))?;
        let updated = ThreadSnapshot { config_json, ..snapshot };
        self.store.upsert_thread(&updated).await?;
        Ok(())
    }

    /// Apply (or clear) the reasoning-effort override for the next turn on a
    /// thread (flows from the harness `turn/start` `effort` param). Same
    /// read-modify-write as [`Self::apply_agent_override`]: `resume_thread`
    /// re-reads the persisted `config_json` every turn, so this is the
    /// chokepoint that carries the per-turn effort into the LLM request.
    pub async fn set_thread_reasoning_effort(
        &self,
        thread_id: &str,
        effort: Option<slab_types::chat::ChatReasoningEffort>,
    ) -> Result<(), AgentError> {
        let snapshot = self
            .store
            .get_thread(thread_id)
            .await?
            .ok_or_else(|| AgentError::ThreadNotFound(thread_id.to_owned()))?;
        let mut config =
            serde_json::from_str::<AgentConfig>(&snapshot.config_json).map_err(|e| {
                AgentError::Internal(format!("failed to deserialize agent config: {e}"))
            })?;
        config.reasoning_effort = effort;
        let config_json = serde_json::to_string(&config)
            .map_err(|e| AgentError::Internal(format!("failed to serialize agent config: {e}")))?;
        let updated = ThreadSnapshot { config_json, ..snapshot };
        self.store.upsert_thread(&updated).await?;
        Ok(())
    }

    /// Overwrite the persisted run-iteration budget for a thread (flows from the
    /// `agent.runtime.limits.max_turns` setting on every harness `turn/start`).
    /// Same read-modify-write as [`Self::set_thread_reasoning_effort`]:
    /// `resume_thread` re-reads the persisted `config_json` every turn, so this
    /// is also what retroactively upgrades legacy threads that still carry the
    /// old default (`10`) in their stored config.
    pub async fn set_thread_max_turns(
        &self,
        thread_id: &str,
        max_turns: u32,
    ) -> Result<(), AgentError> {
        let snapshot = self
            .store
            .get_thread(thread_id)
            .await?
            .ok_or_else(|| AgentError::ThreadNotFound(thread_id.to_owned()))?;
        let mut config =
            serde_json::from_str::<AgentConfig>(&snapshot.config_json).map_err(|e| {
                AgentError::Internal(format!("failed to deserialize agent config: {e}"))
            })?;
        config.max_turns = max_turns;
        let config_json = serde_json::to_string(&config)
            .map_err(|e| AgentError::Internal(format!("failed to serialize agent config: {e}")))?;
        let updated = ThreadSnapshot { config_json, ..snapshot };
        self.store.upsert_thread(&updated).await?;
        Ok(())
    }

    /// Return a persisted thread snapshot.
    pub async fn thread_snapshot(
        &self,
        thread_id: &str,
    ) -> Result<Option<crate::port::ThreadSnapshot>, AgentError> {
        self.store.get_thread(thread_id).await
    }

    /// Wait for a thread to reach a terminal status and return its latest snapshot.
    pub async fn wait_for_terminal_snapshot(
        &self,
        thread_id: &str,
    ) -> Result<crate::port::ThreadSnapshot, AgentError> {
        match self.subscribe(thread_id).await {
            Ok(mut rx) => loop {
                let status = *rx.borrow();
                if is_terminal_status(status) {
                    break;
                }
                if rx.changed().await.is_err() {
                    break;
                }
            },
            Err(AgentError::ThreadNotFound(_)) => {
                return self.wait_for_persisted_terminal_snapshot(thread_id).await;
            }
            Err(error) => return Err(error),
        }

        self.wait_for_persisted_terminal_snapshot(thread_id).await
    }

    /// Resume a persisted thread with a pre-built message history and run the
    /// next turn.
    ///
    /// The conversation read + sort + max-turn + user-content append was hoisted
    /// into the app-core caller (`AgentCore::send_input`); slab-agent
    /// no longer reads conversation data (it leaves via the `EventMsg` protocol
    /// only). This entry point receives the FULL message vec (history + the new
    /// user message already appended), the `starting_turn_index`, and
    /// `emit_new` — HOW MANY trailing messages are new and must be emitted as
    /// `MessageAppended` before the turn loop (the M5 within-turn attribution
    /// anchor). A TRAILING COUNT, not an absolute index: the OnAgentStart
    /// init-batch merge shifts message positions between the caller's read and
    /// the emit, so an absolute anchor drifts and re-emits a tail of old
    /// history.
    pub async fn resume_thread(
        &self,
        thread_id: &str,
        messages: Vec<ConversationMessage>,
        starting_turn_index: u32,
        emit_new: Option<usize>,
    ) -> Result<(), AgentError> {
        // Phase A — async prep with NO lock held: snapshot fetch + config
        // parse + thread construction.
        let snapshot = self
            .store
            .get_thread(thread_id)
            .await?
            .ok_or_else(|| AgentError::ThreadNotFound(thread_id.to_owned()))?;
        if snapshot.status == ThreadStatus::Shutdown {
            return Err(AgentError::ThreadNotResumable {
                id: thread_id.to_owned(),
                status: snapshot.status,
            });
        }
        let config = serde_json::from_str::<AgentConfig>(&snapshot.config_json).map_err(|e| {
            AgentError::Internal(format!("failed to deserialize agent config: {e}"))
        })?;
        let (thread, status_rx) = AgentThread::new_with_id(
            snapshot.id.clone(),
            snapshot.session_id,
            snapshot.parent_id,
            snapshot.depth,
            config,
        );

        // Phase B — reserve the registry slot ATOMICALLY with the busy check
        // (short write-lock critical section, no awaits). This closes the
        // TOCTOU where two concurrent resumes both passed a read-lock check
        // before either spawned task registered.
        let reserved_state = Arc::clone(&thread.state);
        let mut previous_join = None;
        {
            let mut guard = self.threads.write().await;
            if let Some(entry) = guard.get_mut(thread_id) {
                if !is_terminal_status(entry.state().status()) {
                    // A genuinely live/reserved run owns the slot.
                    return Err(AgentError::ThreadBusy(thread_id.to_owned()));
                }
                // A terminal Live entry is a dead husk waiting for task-exit
                // cleanup: `run()` reached its epilogue and the per-thread
                // exec-mode/plan teardown already happened before the state
                // went terminal, so replacing it cannot race a stale clear.
                // The old task's identity-guarded remove then no-ops on this
                // new entry.
                if let ThreadEntry::Live { join, .. } = entry {
                    previous_join = join.take();
                }
            }
            guard.insert(
                thread_id.to_owned(),
                ThreadEntry::Reserved { state: Arc::clone(&reserved_state) },
            );
        }
        // Join the replaced run's task (bounded): a resume that read the
        // rollout while the old epilogue was still emitting its final events
        // would miss the last turn and mis-attribute the new input's turn
        // index. Waiting for task exit closes that window; a stuck epilogue
        // must not block resume forever, hence the grace.
        if let Some(mut join) = previous_join
            && tokio::time::timeout(SHUTDOWN_GRACE, &mut join).await.is_err()
        {
            warn!(
                thread_id,
                grace_ms = SHUTDOWN_GRACE.as_millis() as u64,
                "resume: timed out joining the previous run's epilogue; continuing"
            );
        }

        // Phase C — spawn. Admission (memory pressure / gate) can still
        // reject; roll back OUR reservation (never a Live entry a concurrent
        // path installed).
        if let Err(error) =
            self.start_thread(thread, status_rx, messages, starting_turn_index, emit_new).await
        {
            let mut guard = self.threads.write().await;
            if let Some(ThreadEntry::Reserved { state }) = guard.get(thread_id)
                && Arc::ptr_eq(state, &reserved_state)
            {
                guard.remove(thread_id);
            }
            drop(guard);
            return Err(error);
        }
        Ok(())
    }

    /// Steering: deliver user input to a RUNNING thread. Busy threads used to
    /// hard-fail with [`AgentError::ThreadBusy`]; now the message queues and
    /// the run loop injects it at the next iteration boundary (needs_follow_up
    /// = model wants more OR queue non-empty). Returns
    /// [`SendOutcome::NeedsResume`] for an idle thread — the caller (app-core)
    /// owns the resume flow.
    pub async fn queue_input(
        &self,
        thread_id: &str,
        message: ConversationMessage,
    ) -> Result<SendOutcome, AgentError> {
        let guard = self.threads.read().await;
        let Some(entry) = guard.get(thread_id) else {
            return Ok(SendOutcome::NeedsResume);
        };
        match entry {
            ThreadEntry::Live { pending_input, state, .. } => {
                // A run whose state machine already reached a terminal status
                // is in its epilogue (the task removes the registry entry only
                // after `run()` returns). Nothing will ever drain a queue
                // pushed here — the leftover-input drain already ran — so the
                // honest answer is NeedsResume: the caller re-runs the resume
                // flow, which replaces the dead entry. Without this the
                // message was silently lost in the run-return → remove window.
                if is_terminal_status(state.status()) {
                    return Ok(SendOutcome::NeedsResume);
                }
                let mut queue =
                    pending_input.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                if queue.len() >= MAX_QUEUED_INPUT {
                    return Err(AgentError::Internal(
                        "input queue full; interrupt or wait for the run to finish".to_owned(),
                    ));
                }
                queue.push_back(message);
                Ok(SendOutcome::Queued { position: queue.len() })
            }
            // A resume is being assembled (TOCTOU reservation) — nothing is
            // runnable to drain a queue yet; the honest answer is busy.
            ThreadEntry::Reserved { .. } => Err(AgentError::ThreadBusy(thread_id.to_owned())),
        }
    }

    /// Get a [`watch::Receiver`] that emits the latest status for the given thread.
    pub async fn subscribe(
        &self,
        thread_id: &str,
    ) -> Result<watch::Receiver<ThreadStatus>, AgentError> {
        let guard = self.threads.read().await;
        let entry =
            guard.get(thread_id).ok_or_else(|| AgentError::ThreadNotFound(thread_id.to_owned()))?;
        match entry {
            ThreadEntry::Live { status_rx, .. } => Ok(status_rx.clone()),
            // A reserved slot has no spawned task yet, but its state machine
            // already broadcasts — subscribe straight from it.
            ThreadEntry::Reserved { state } => Ok(state.subscribe()),
        }
    }

    /// Shut a thread down: broadcast and persist `Shutdown`, cancel the run,
    /// and wait (gracefully) for the run tail — the terminal-status persist
    /// and the `OnAgentEnd` hooks (host memory extraction, sleep-inhibitor
    /// release) — before falling back to a hard task abort.
    pub async fn shutdown(&self, thread_id: &str) -> Result<(), AgentError> {
        let entry = self
            .threads
            .write()
            .await
            .remove(thread_id)
            .ok_or_else(|| AgentError::ThreadNotFound(thread_id.to_owned()))?;

        // Signal the terminal status before cancelling so all watch
        // subscribers see `Shutdown` rather than the last intermediate status.
        entry.state().transition(ThreadStatus::Shutdown)?;
        let live_parts = match entry {
            ThreadEntry::Reserved { .. } => None,
            ThreadEntry::Live { cancellation, join, abort, .. } => {
                Some((cancellation, join, abort))
            }
        };

        // Persist and fan-out the Shutdown transition.
        self.notify.on_status_change(thread_id, ThreadStatus::Shutdown).await;
        let status_msg = EventMsg::ThreadStatusChanged(ThreadStatusChangedParams {
            thread_id: thread_id.to_owned(),
            status: ThreadStatus::Shutdown.to_string(),
            reason: Some("shutdown".to_owned()),
        });
        self.notify.on_event_msg(thread_id, &status_msg).await;
        self.store
            .update_thread_status(thread_id, ThreadStatus::Shutdown, Some("shutdown"))
            .await
            .ok();

        if let Some((cancellation, mut join, abort)) = live_parts {
            // Cancel FIRST so the run's teardown (status persist + OnAgentEnd
            // hooks) executes; the hard abort is the last resort after the
            // grace window. Previously the abort fired immediately and the
            // run tail never ran.
            cancellation.cancel();
            let graceful = match join.as_mut() {
                Some(join) => tokio::time::timeout(SHUTDOWN_GRACE, join).await.is_ok(),
                None => true,
            };
            if !graceful {
                warn!(
                    thread_id,
                    grace_ms = SHUTDOWN_GRACE.as_millis() as u64,
                    "shutdown grace expired; aborting thread task"
                );
                abort.abort();
            }
        }
        // The entry is gone from the registry and the task (if any) was joined
        // or aborted: drop its per-thread mode + durable plan here too — the
        // spawn-path cleanup never runs for an aborted task.
        self.clear_thread_mode(thread_id).await;
        Ok(())
    }

    /// Cancel the current turn while keeping the thread available for later input.
    pub async fn interrupt(&self, thread_id: &str) -> Result<(), AgentError> {
        let guard = self.threads.read().await;
        let entry =
            guard.get(thread_id).ok_or_else(|| AgentError::ThreadNotFound(thread_id.to_owned()))?;
        let state = Arc::clone(entry.state());
        let cancellation = match entry {
            ThreadEntry::Live { cancellation, .. } => Some(cancellation.clone()),
            // Reserved: no task to cancel — the state transition alone ends
            // the run at its startup check.
            ThreadEntry::Reserved { .. } => None,
        };
        drop(guard);

        state.transition(ThreadStatus::Interrupting)?;
        if let Some(cancellation) = cancellation {
            cancellation.cancel();
        }
        self.notify.on_status_change(thread_id, ThreadStatus::Interrupting).await;
        // Instant UI feedback for the interrupt: the authoritative turn-level
        // terminal event is emitted by the turn loop's teardown (it knows the
        // active turn index; this layer does not). The eager placeholder
        // `TurnAborted { turn: "current" }` this used to emit was removed —
        // clients saw a wrong-id terminal before the real one landed.
        let status_msg = EventMsg::ThreadStatusChanged(ThreadStatusChangedParams {
            thread_id: thread_id.to_owned(),
            status: ThreadStatus::Interrupting.to_string(),
            reason: None,
        });
        self.notify.on_event_msg(thread_id, &status_msg).await;
        self.store
            .update_thread_status(thread_id, ThreadStatus::Interrupting, Some("interrupting"))
            .await
            .ok();
        Ok(())
    }

    /// Return the number of currently active (not yet completed) threads.
    pub async fn active_thread_count(&self) -> usize {
        self.threads.read().await.len()
    }

    /// IDs of all currently active (not yet completed) threads.
    pub async fn active_thread_ids(&self) -> Vec<String> {
        self.threads.read().await.keys().cloned().collect()
    }

    /// Interrupt every active thread (best-effort, graceful) and return the IDs
    /// that were targeted. Used by workspace migration (B-8 / INFRA-01) to shed
    /// agent work before a switch so no "ghost" threads carry into the new
    /// workspace. A thread that terminates between enumeration and interrupt is
    /// silently skipped.
    pub async fn interrupt_all(&self) -> Vec<String> {
        let ids = self.active_thread_ids().await;
        for id in &ids {
            let _ = self.interrupt(id).await;
        }
        ids
    }

    /// Replace hooks used by active threads at their next hook dispatch.
    pub fn replace_hooks(&self, hooks: Vec<Arc<dyn AgentHook>>) {
        self.hooks.replace(hooks);
    }

    /// Return the shared tool router used by active and future threads.
    pub fn tool_router(&self) -> Arc<ToolRouter> {
        Arc::clone(&self.tool_router)
    }

    /// Set the per-session permission mode for a thread (flows from
    /// `ThreadStartParams`/`TurnStartParams`). The engine keys mode by
    /// `thread_id` so concurrent sessions on the singleton control don't race.
    pub async fn set_thread_mode(&self, thread_id: &str, mode: slab_exec_policy::PermissionMode) {
        self.exec_policy.set_thread_mode(thread_id, mode).await;
    }

    /// Drop per-thread state when the thread ends: the exec-policy permission
    /// mode and the durable plan (plan store). Both are keyed by `thread_id`;
    /// clearing here prevents cross-thread leakage on the process-wide singleton.
    pub async fn clear_thread_mode(&self, thread_id: &str) {
        self.exec_policy.clear_thread(thread_id).await;
        self.plan_store.clear(thread_id).await;
    }

    /// Test seam: install a terminal Live registry entry that mimics the
    /// post-run → pre-remove window (the task removes the entry only after
    /// `run()` returns), so crate tests can exercise the terminal-entry
    /// branches of `queue_input` / `resume_thread` deterministically.
    #[cfg(test)]
    pub(crate) async fn install_dead_entry_for_test(&self, thread_id: &str) {
        let (state, status_rx) = ThreadStateMachine::new();
        let _ = state.transition(ThreadStatus::Running);
        let _ = state.transition(ThreadStatus::Completed);
        self.threads.write().await.insert(
            thread_id.to_owned(),
            ThreadEntry::Live {
                status_rx,
                state,
                join: None,
                abort: tokio::spawn(async {}).abort_handle(),
                cancellation: CancellationToken::new(),
                pending_input: Arc::new(std::sync::Mutex::new(VecDeque::new())),
            },
        );
    }

    /// The shared exec-policy handle. Hosts use it to snapshot permission state
    /// for context rendering (e.g. the `<permissions_instructions>` fragment and
    /// progressive tool exposure) without re-deriving the engine.
    pub fn exec_policy(&self) -> Arc<dyn crate::port::ExecPolicyPort> {
        Arc::clone(&self.exec_policy)
    }

    // ── private helpers ──────────────────────────────────────────────────────

    async fn spawn_inner(&self, request: SpawnRequest) -> Result<String, AgentError> {
        let SpawnRequest {
            session_id,
            parent_id,
            depth,
            config,
            messages,
            starting_turn_index,
            emit_new,
        } = request;

        let (thread, status_rx) = AgentThread::new(session_id, parent_id, depth, config);
        self.start_thread(thread, status_rx, messages, starting_turn_index, emit_new).await
    }

    async fn start_thread(
        &self,
        thread: AgentThread,
        status_rx: watch::Receiver<ThreadStatus>,
        messages: Vec<ConversationMessage>,
        starting_turn_index: u32,
        emit_new: Option<usize>,
    ) -> Result<String, AgentError> {
        // Memory circuit breaker (INFRA-05): pause spawns while the host reports
        // process RSS above the configured threshold.
        if let crate::port::MemoryPressure::Tripped { current_mb, threshold_mb } =
            self.memory_pressure.check()
        {
            return Err(AgentError::MemoryPressureExceeded { current_mb, threshold_mb });
        }

        // Bounded FIFO admission (INFRA-05). The permit is held for the thread's
        // lifetime and dropped when the task finishes or is aborted, releasing
        // the slot to the next waiter in arrival order.
        let permit = self.gate.acquire().await?;

        let thread_id = thread.id.clone();
        let state = Arc::clone(&thread.state);

        let llm = Arc::clone(&self.llm);
        let store = Arc::clone(&self.store);
        let notify = Arc::clone(&self.notify);
        let approval = Arc::clone(&self.approval);
        let exec_policy = Arc::clone(&self.exec_policy);
        let plan_store = Arc::clone(&self.plan_store);
        let agent_registry = Arc::clone(&self.agent_registry);
        let tools = Arc::clone(&self.tool_router);
        let hooks = self.hooks.clone();
        let compact = Arc::clone(&self.compact);
        let risk = Arc::clone(&self.risk);
        let trace = Arc::clone(&self.trace);
        let trace_dir = self.trace_dir.clone();
        let thread_context = self.thread_context.clone();
        let cancellation = CancellationToken::new();
        let pending_input: Arc<std::sync::Mutex<VecDeque<ConversationMessage>>> =
            Arc::new(std::sync::Mutex::new(VecDeque::new()));
        let threads_cleanup = Arc::clone(&self.threads);
        let id_cleanup = thread_id.clone();
        let runtime = AgentThreadRuntime {
            llm,
            store,
            notify,
            approval,
            exec_policy,
            agent_registry,
            plan_store,
            tools,
            tool_discovery: ToolDiscoveryState::new(),
            hooks,
            compact,
            risk,
            trace,
            trace_dir,
            thread_context,
            cancellation: cancellation.clone(),
            pending_input: Arc::clone(&pending_input),
        };

        // Spawn the thread task first to obtain the AbortHandle.
        // The task removes itself from the registry when it finishes so that
        // `active_thread_count` stays accurate. The admission permit is moved
        // into the task and dropped on completion or abort.
        let run_state = Arc::clone(&state);
        let join_handle = tokio::spawn(async move {
            let _permit = permit;
            let result = thread.run(messages, runtime, starting_turn_index, emit_new).await;
            if let Err(ref e) = result {
                warn!(thread_id = %id_cleanup, error = %e, "agent thread finished with error");
            }
            // Remove OUR entry only: a resume may legitimately replace a
            // terminal Live entry while this task is still in its epilogue,
            // and an unconditional remove could drop the fresh one.
            {
                let mut guard = threads_cleanup.write().await;
                if let Some(entry) = guard.get(&id_cleanup)
                    && Arc::ptr_eq(entry.state(), &run_state)
                {
                    guard.remove(&id_cleanup);
                }
            }
            result
        });

        let abort = join_handle.abort_handle();

        // Replaces the resume-path reservation (same state machine instance)
        // or inserts fresh (spawn path, brand-new id).
        let mut guard = self.threads.write().await;
        guard.insert(
            thread_id.clone(),
            ThreadEntry::Live {
                status_rx,
                state,
                join: Some(join_handle),
                abort,
                cancellation,
                pending_input,
            },
        );
        drop(guard);

        Ok(thread_id)
    }

    async fn wait_for_persisted_terminal_snapshot(
        &self,
        thread_id: &str,
    ) -> Result<crate::port::ThreadSnapshot, AgentError> {
        // Bounded: previously an unbounded 100 ms poll that hung forever when
        // a thread never reached a terminal persisted status. On expiry,
        // return the latest snapshot even if non-terminal — callers prefer a
        // stale answer over hanging.
        let mut last_snapshot: Option<crate::port::ThreadSnapshot> = None;
        match tokio::time::timeout(TERMINAL_SNAPSHOT_TIMEOUT, async {
            loop {
                let snapshot = self
                    .store
                    .get_thread(thread_id)
                    .await?
                    .ok_or_else(|| AgentError::ThreadNotFound(thread_id.to_owned()))?;
                if is_terminal_status(snapshot.status) {
                    return Ok(snapshot);
                }
                last_snapshot = Some(snapshot);
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        {
            Ok(result) => result,
            Err(_elapsed) => {
                warn!(
                    thread_id,
                    timeout_ms = TERMINAL_SNAPSHOT_TIMEOUT.as_millis() as u64,
                    "terminal snapshot wait timed out; returning latest non-terminal snapshot"
                );
                last_snapshot.ok_or_else(|| AgentError::ThreadNotFound(thread_id.to_owned()))
            }
        }
    }
}

fn is_terminal_status(status: ThreadStatus) -> bool {
    matches!(
        status,
        ThreadStatus::Completed
            | ThreadStatus::Errored
            | ThreadStatus::Interrupted
            | ThreadStatus::Shutdown
    )
}
