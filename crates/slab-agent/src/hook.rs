//! Agent hook system.
//!
//! Hooks let host code observe the agent lifecycle, inject local context, and
//! apply policy to tool calls without making `slab-agent` depend on host
//! infrastructure.

use async_trait::async_trait;
use serde_json::Value;
use slab_types::{ConversationMessage, agent::ToolCallStatus};

use crate::{
    config::AgentConfig,
    port::{LlmResponse, ThreadStatus, ToolSpec},
};

/// An event dispatched to registered [`AgentHook`] implementations.
#[derive(Debug, Clone)]
pub enum HookEvent {
    OnAgentStart {
        thread_id: String,
        session_id: String,
        parent_id: Option<String>,
        depth: u32,
        config: AgentConfig,
    },
    OnLlmStart {
        thread_id: String,
        turn_index: u32,
        messages: Vec<ConversationMessage>,
        tools: Vec<ToolSpec>,
    },
    OnLlmEnd {
        thread_id: String,
        turn_index: u32,
        response: LlmResponse,
    },
    OnToolStart {
        thread_id: String,
        turn_index: u32,
        call_id: String,
        tool_name: String,
        arguments: Value,
    },
    OnToolEnd {
        thread_id: String,
        turn_index: u32,
        call_id: String,
        tool_name: String,
        arguments: Value,
        output: String,
        status: ToolCallStatus,
    },
    OnAgentEnd {
        thread_id: String,
        status: ThreadStatus,
        error: Option<String>,
    },
}

/// Tool-specific action requested by a hook.
#[derive(Debug, Clone)]
pub enum HookToolAction {
    Continue,
    Block { reason: String },
    ModifyArgs { arguments: Value },
}

impl Default for HookToolAction {
    fn default() -> Self {
        Self::Continue
    }
}

/// The decision returned by a hook handler.
#[derive(Debug, Clone)]
pub enum HookOutcome {
    Continue,
    Block {
        reason: String,
    },
    ModifyArgs {
        arguments: Value,
    },
    InjectMessages {
        messages: Vec<ConversationMessage>,
    },
    AppendObservation {
        observation: String,
    },
    Effects {
        tool_action: HookToolAction,
        injected_messages: Vec<ConversationMessage>,
        observations: Vec<String>,
    },
}

impl HookOutcome {
    pub fn inject_message(message: ConversationMessage) -> Self {
        Self::InjectMessages { messages: vec![message] }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HookEffects {
    pub tool_action: HookToolAction,
    pub injected_messages: Vec<ConversationMessage>,
    pub observations: Vec<String>,
}

impl HookEffects {
    pub fn is_continue(&self) -> bool {
        matches!(self.tool_action, HookToolAction::Continue)
            && self.injected_messages.is_empty()
            && self.observations.is_empty()
    }

    fn merge_outcome(&mut self, outcome: HookOutcome) {
        match outcome {
            HookOutcome::Continue => {}
            HookOutcome::Block { reason } => {
                self.set_first_tool_action(HookToolAction::Block { reason });
            }
            HookOutcome::ModifyArgs { arguments } => {
                self.set_first_tool_action(HookToolAction::ModifyArgs { arguments });
            }
            HookOutcome::InjectMessages { messages } => {
                self.injected_messages.extend(messages);
            }
            HookOutcome::AppendObservation { observation } => {
                self.observations.push(observation);
            }
            HookOutcome::Effects { tool_action, injected_messages, observations } => {
                self.set_first_tool_action(tool_action);
                self.injected_messages.extend(injected_messages);
                self.observations.extend(observations);
            }
        }
    }

    fn set_first_tool_action(&mut self, action: HookToolAction) {
        if matches!(self.tool_action, HookToolAction::Continue) {
            self.tool_action = action;
        }
    }
}

/// A hook that can intercept agent lifecycle events.
///
/// Implementations should be deterministic and scoped to host-owned concerns:
/// prompt injection, local policy checks, script results, and telemetry.
#[async_trait]
pub trait AgentHook: Send + Sync {
    /// Handle an agent event and return a [`HookOutcome`].
    async fn on_event(&self, event: &HookEvent) -> HookOutcome;
}

/// Run `event` through all hooks and merge their effects.
pub async fn dispatch_hooks(
    hooks: &[std::sync::Arc<dyn AgentHook>],
    event: &HookEvent,
) -> HookEffects {
    let mut effects = HookEffects::default();
    for hook in hooks {
        effects.merge_outcome(hook.on_event(event).await);
    }
    effects
}
