//! Top-level agent controller — manages all active agent threads.

use std::{collections::HashMap, sync::Arc};

use tokio::sync::{watch, RwLock};
use tracing::warn;

use slab_types::ConversationMessage;

use crate::{
    config::AgentConfig,
    error::AgentError,
    port::{AgentNotifyPort, AgentStorePort, LlmPort, ThreadStatus},
    thread::AgentThread,
    tool::ToolRouter,
};

// ── Internal handle stored per active thread ─────────────────────────────────

struct ThreadEntry {
    status_rx: watch::Receiver<ThreadStatus>,
    /// Shared sender — lets the controller push the `Shutdown` status even
    /// after the spawned task has been aborted.
    status_tx: Arc<watch::Sender<ThreadStatus>>,
    abort: tokio::task::AbortHandle,
}

// ── AgentControl ─────────────────────────────────────────────────────────────

/// Top-level controller that owns and coordinates all active agent threads.
///
/// Inject the port adapters at construction time; the controller owns them for
/// its lifetime and shares them (via [`Arc`]) with every thread it spawns.
pub struct AgentControl {
    threads: Arc<RwLock<HashMap<String, ThreadEntry>>>,
    llm: Arc<dyn LlmPort>,
    store: Arc<dyn AgentStorePort>,
    notify: Arc<dyn AgentNotifyPort>,
    tool_router: Arc<ToolRouter>,
    max_threads: usize,
    max_depth: u32,
}

impl AgentControl {
    /// Create a new controller.
    ///
    /// - `max_threads`: hard cap on concurrently active threads (across all depths).
    /// - `max_depth`: maximum allowed child nesting depth (inclusive, 0-based; root
    ///   agents are depth 0).
    pub fn new(
        llm: Arc<dyn LlmPort>,
        store: Arc<dyn AgentStorePort>,
        notify: Arc<dyn AgentNotifyPort>,
        tool_router: Arc<ToolRouter>,
        max_threads: usize,
        max_depth: u32,
    ) -> Self {
        Self {
            threads: Arc::new(RwLock::new(HashMap::new())),
            llm,
            store,
            notify,
            tool_router,
            max_threads,
            max_depth,
        }
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
        self.spawn_inner(session_id, None, 0, config, messages).await
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
        self.spawn_inner(session_id, Some(parent_id), depth, config, messages).await
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
        if let Err(err) = entry.status_tx.send(ThreadStatus::Shutdown) {
            warn!(?err, thread_id, "failed to send Shutdown status before aborting thread");
        }
        entry.abort.abort();

        // Persist and fan-out the Shutdown transition.
        self.notify.on_status_change(thread_id, ThreadStatus::Shutdown).await;
        self.store
            .update_thread_status(thread_id, ThreadStatus::Shutdown, Some("shutdown"))
            .await
            .ok();

        Ok(())
    }

    /// Return the number of currently active (not yet completed) threads.
    pub async fn active_thread_count(&self) -> usize {
        self.threads.read().await.len()
    }

    // ── private helpers ──────────────────────────────────────────────────────

    async fn spawn_inner(
        &self,
        session_id: String,
        parent_id: Option<String>,
        depth: u32,
        config: AgentConfig,
        messages: Vec<ConversationMessage>,
    ) -> Result<String, AgentError> {
        let (thread, status_rx) = AgentThread::new(session_id, parent_id, depth, config);
        let thread_id = thread.id.clone();
        let status_tx = Arc::clone(&thread.status_tx);

        let llm = Arc::clone(&self.llm);
        let store = Arc::clone(&self.store);
        let notify = Arc::clone(&self.notify);
        let tools = Arc::clone(&self.tool_router);
        let threads_cleanup = Arc::clone(&self.threads);
        let id_cleanup = thread_id.clone();

        // Spawn the thread task first to obtain the AbortHandle.
        // The task removes itself from the registry when it finishes so that
        // `active_thread_count` stays accurate.
        let join_handle = tokio::spawn(async move {
            let result = thread.run(messages, llm, store, notify, tools).await;
            if let Err(ref e) = result {
                warn!(thread_id = %id_cleanup, error = %e, "agent thread finished with error");
            }
            threads_cleanup.write().await.remove(&id_cleanup);
            result
        });

        let abort = join_handle.abort_handle();

        // Atomically check the concurrency limit and insert the entry under the
        // same write guard to prevent TOCTOU races between concurrent spawns.
        // If the limit is already reached, abort the just-spawned task and bail.
        let mut guard = self.threads.write().await;
        if guard.len() >= self.max_threads {
            abort.abort();
            return Err(AgentError::ThreadLimitExceeded {
                current: guard.len(),
                max: self.max_threads,
            });
        }
        guard.insert(thread_id.clone(), ThreadEntry { status_rx, status_tx, abort });
        drop(guard);

        Ok(thread_id)
    }
}
