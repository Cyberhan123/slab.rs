//! Top-level agent controller — manages all active agent threads.

use std::{collections::HashMap, sync::Arc};

use tokio::{
    sync::{RwLock, watch},
    time::{Duration, sleep},
};
use tokio_util::sync::CancellationToken;
use tracing::warn;

use slab_agent_tracing::{AgentTraceSink, NoopAgentTraceSink};
use slab_types::{ConversationMessage, ConversationMessageContent};

use crate::{
    compact::{CompactPort, SlidingWindowCompactPort},
    concurrency_gate::ConcurrencyGate,
    config::AgentConfig,
    error::AgentError,
    event::{AgentEventKind, AgentResponseRef},
    hook::{AgentHook, AgentHookRegistry},
    port::{AgentNotifyPort, AgentStorePort, ApprovalPort, LlmPort, ThreadStatus},
    risk::{BasicToolRiskAnalyzer, ToolRiskAnalyzer},
    state::ThreadStateMachine,
    thread::{AgentThread, AgentThreadRuntime},
    tool::{AgentThreadContext, ToolRouter},
};

// ── Internal handle stored per active thread ─────────────────────────────────

struct ThreadEntry {
    status_rx: watch::Receiver<ThreadStatus>,
    state: Arc<ThreadStateMachine>,
    abort: tokio::task::AbortHandle,
    cancellation: CancellationToken,
}

struct SpawnRequest {
    session_id: String,
    parent_id: Option<String>,
    depth: u32,
    config: AgentConfig,
    messages: Vec<ConversationMessage>,
    starting_turn_index: u32,
    persist_messages_from: Option<usize>,
}

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
            persist_messages_from: Some(0),
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
            persist_messages_from: Some(0),
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

    /// Append user input to a persisted thread and run another agent turn.
    pub async fn send_input(&self, thread_id: &str, content: String) -> Result<(), AgentError> {
        if self.threads.read().await.contains_key(thread_id) {
            return Err(AgentError::ThreadBusy(thread_id.to_owned()));
        }

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
        let mut records = self.store.list_thread_messages(thread_id).await?;
        records.sort_by(|left, right| {
            left.turn_index
                .cmp(&right.turn_index)
                .then_with(|| left.created_at.cmp(&right.created_at))
                .then_with(|| left.id.cmp(&right.id))
        });
        let starting_turn_index =
            records.iter().map(|record| record.turn_index).max().map_or(0, |index| index + 1);
        let mut messages = records.into_iter().map(|record| record.message).collect::<Vec<_>>();
        let persist_from = messages.len();
        messages.push(ConversationMessage {
            role: "user".to_owned(),
            content: ConversationMessageContent::Text(content),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        });

        let (thread, status_rx) = AgentThread::new_with_id(
            snapshot.id.clone(),
            snapshot.session_id,
            snapshot.parent_id,
            snapshot.depth,
            config,
        );
        self.start_thread(thread, status_rx, messages, starting_turn_index, Some(persist_from))
            .await?;
        Ok(())
    }

    /// Get a [`watch::Receiver`] that emits the latest status for the given thread.
    pub async fn subscribe(
        &self,
        thread_id: &str,
    ) -> Result<watch::Receiver<ThreadStatus>, AgentError> {
        self.threads
            .read()
            .await
            .get(thread_id)
            .map(|e| e.status_rx.clone())
            .ok_or_else(|| AgentError::ThreadNotFound(thread_id.to_owned()))
    }

    /// Abort a running thread, broadcast the `Shutdown` status, persist it,
    /// and remove the entry from the registry.
    pub async fn shutdown(&self, thread_id: &str) -> Result<(), AgentError> {
        let entry = self
            .threads
            .write()
            .await
            .remove(thread_id)
            .ok_or_else(|| AgentError::ThreadNotFound(thread_id.to_owned()))?;

        // Signal the terminal status before aborting so all watch subscribers
        // see `Shutdown` rather than the last intermediate status.
        entry.state.transition(ThreadStatus::Shutdown)?;
        entry.abort.abort();

        // Persist and fan-out the Shutdown transition.
        self.notify.on_status_change(thread_id, ThreadStatus::Shutdown).await;
        self.store
            .update_thread_status(thread_id, ThreadStatus::Shutdown, Some("shutdown"))
            .await
            .ok();

        Ok(())
    }

    /// Cancel the current turn while keeping the thread available for later input.
    pub async fn interrupt(&self, thread_id: &str) -> Result<(), AgentError> {
        let guard = self.threads.read().await;
        let entry =
            guard.get(thread_id).ok_or_else(|| AgentError::ThreadNotFound(thread_id.to_owned()))?;
        let state = Arc::clone(&entry.state);
        let cancellation = entry.cancellation.clone();
        drop(guard);

        state.transition(ThreadStatus::Interrupting)?;
        cancellation.cancel();
        self.notify.on_status_change(thread_id, ThreadStatus::Interrupting).await;
        self.notify
            .on_turn_event(
                thread_id,
                &crate::port::TurnEvent::Response {
                    turn_index: None,
                    event: AgentEventKind::ResponseCancelled {
                        response: AgentResponseRef {
                            id: thread_id.to_owned(),
                            status: ThreadStatus::Interrupting,
                        },
                        reason: "interrupt requested".to_owned(),
                    },
                },
            )
            .await;
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

    // ── private helpers ──────────────────────────────────────────────────────

    async fn spawn_inner(&self, request: SpawnRequest) -> Result<String, AgentError> {
        let SpawnRequest {
            session_id,
            parent_id,
            depth,
            config,
            messages,
            starting_turn_index,
            persist_messages_from,
        } = request;

        let (thread, status_rx) = AgentThread::new(session_id, parent_id, depth, config);
        self.start_thread(thread, status_rx, messages, starting_turn_index, persist_messages_from)
            .await
    }

    async fn start_thread(
        &self,
        thread: AgentThread,
        status_rx: watch::Receiver<ThreadStatus>,
        messages: Vec<ConversationMessage>,
        starting_turn_index: u32,
        persist_messages_from: Option<usize>,
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
        let tools = Arc::clone(&self.tool_router);
        let hooks = self.hooks.clone();
        let compact = Arc::clone(&self.compact);
        let risk = Arc::clone(&self.risk);
        let trace = Arc::clone(&self.trace);
        let trace_dir = self.trace_dir.clone();
        let thread_context = self.thread_context.clone();
        let cancellation = CancellationToken::new();
        let threads_cleanup = Arc::clone(&self.threads);
        let id_cleanup = thread_id.clone();
        let runtime = AgentThreadRuntime {
            llm,
            store,
            notify,
            approval,
            tools,
            hooks,
            compact,
            risk,
            trace,
            trace_dir,
            thread_context,
            cancellation: cancellation.clone(),
        };

        // Spawn the thread task first to obtain the AbortHandle.
        // The task removes itself from the registry when it finishes so that
        // `active_thread_count` stays accurate. The admission permit is moved
        // into the task and dropped on completion or abort.
        let join_handle = tokio::spawn(async move {
            let _permit = permit;
            let result =
                thread.run(messages, runtime, starting_turn_index, persist_messages_from).await;
            if let Err(ref e) = result {
                warn!(thread_id = %id_cleanup, error = %e, "agent thread finished with error");
            }
            threads_cleanup.write().await.remove(&id_cleanup);
            result
        });

        let abort = join_handle.abort_handle();

        let mut guard = self.threads.write().await;
        guard.insert(thread_id.clone(), ThreadEntry { status_rx, state, abort, cancellation });
        drop(guard);

        Ok(thread_id)
    }

    async fn wait_for_persisted_terminal_snapshot(
        &self,
        thread_id: &str,
    ) -> Result<crate::port::ThreadSnapshot, AgentError> {
        loop {
            let snapshot = self
                .store
                .get_thread(thread_id)
                .await?
                .ok_or_else(|| AgentError::ThreadNotFound(thread_id.to_owned()))?;
            if is_terminal_status(snapshot.status) {
                return Ok(snapshot);
            }
            sleep(Duration::from_millis(100)).await;
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
