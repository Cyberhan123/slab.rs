//! Stable runtime facade for host-facing agent operations.
//!
//! [`AgentRuntime`] keeps host crates from reaching into [`AgentControl`] for
//! common session operations. The control object remains the execution kernel;
//! this facade is the public API surface hosts should prefer.

use std::sync::Arc;

use slab_types::{ConversationMessage, agent::AgentThreadStatus};
use tokio::sync::watch;

use crate::{
    config::AgentConfig, control::AgentControl, error::AgentError, hook::AgentHook,
    tool::ToolRouter,
};

/// Host-facing agent runtime API.
///
/// Implementations own or wrap the control-plane kernel and expose stable,
/// task-oriented operations for app and transport layers.
#[derive(Clone)]
pub struct AgentRuntime {
    control: Arc<AgentControl>,
}

impl AgentRuntime {
    /// Create a runtime facade around an initialized control-plane kernel.
    pub fn new(control: Arc<AgentControl>) -> Self {
        Self { control }
    }

    /// Return the wrapped control object for low-level integrations that have
    /// not yet migrated to this facade.
    pub fn control(&self) -> Arc<AgentControl> {
        Arc::clone(&self.control)
    }

    /// Spawn a root agent thread.
    pub async fn create_response(
        &self,
        session_id: String,
        config: AgentConfig,
        messages: Vec<ConversationMessage>,
    ) -> Result<String, AgentError> {
        self.control.spawn(session_id, config, messages).await
    }

    /// Resume a persisted thread with a pre-built message history and run the
    /// next turn. The conversation read + user-content append was hoisted into
    /// the app-core caller; slab-agent receives the full message
    /// vec + the `emit_new` anchor (how many TRAILING messages are new and
    /// must be emitted — a count, so the init-batch merge shifting positions
    /// cannot drift the anchor).
    pub async fn resume_thread(
        &self,
        thread_id: &str,
        messages: Vec<ConversationMessage>,
        starting_turn_index: u32,
        emit_new: Option<usize>,
    ) -> Result<(), AgentError> {
        self.control.resume_thread(thread_id, messages, starting_turn_index, emit_new).await
    }

    /// Interrupt the current turn while keeping the thread resumable.
    pub async fn interrupt(&self, thread_id: &str) -> Result<(), AgentError> {
        self.control.interrupt(thread_id).await
    }

    /// Shut down a running thread.
    pub async fn shutdown(&self, thread_id: &str) -> Result<(), AgentError> {
        self.control.shutdown(thread_id).await
    }

    /// Subscribe to a live thread status stream.
    pub async fn subscribe(
        &self,
        thread_id: &str,
    ) -> Result<watch::Receiver<AgentThreadStatus>, AgentError> {
        self.control.subscribe(thread_id).await
    }

    /// Return the number of currently active threads.
    pub async fn active_thread_count(&self) -> usize {
        self.control.active_thread_count().await
    }

    /// Interrupt all active threads and return the targeted ids.
    pub async fn interrupt_all(&self) -> Vec<String> {
        self.control.interrupt_all().await
    }

    /// Replace hooks used by active and future threads.
    pub fn replace_hooks(&self, hooks: Vec<Arc<dyn AgentHook>>) {
        self.control.replace_hooks(hooks);
    }

    /// Return the shared tool router for host-owned dynamic registrations.
    pub fn tool_router(&self) -> Arc<ToolRouter> {
        self.control.tool_router()
    }
}
