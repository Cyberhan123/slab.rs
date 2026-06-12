//! Agent hook system.
//!
//! Hooks let host code observe the agent lifecycle, inject local context, and
//! apply policy to tool calls without making `slab-agent` depend on host
//! infrastructure.

use std::sync::{Arc, RwLock};

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
        session_id: String,
        turn_index: u32,
        messages: Vec<ConversationMessage>,
        tools: Vec<ToolSpec>,
    },
    OnLlmEnd {
        thread_id: String,
        session_id: String,
        turn_index: u32,
        messages: Vec<ConversationMessage>,
        response: LlmResponse,
    },
    OnToolStart {
        thread_id: String,
        session_id: String,
        turn_index: u32,
        messages: Vec<ConversationMessage>,
        call_id: String,
        tool_name: String,
        arguments: Value,
    },
    OnToolEnd {
        thread_id: String,
        session_id: String,
        turn_index: u32,
        messages: Vec<ConversationMessage>,
        call_id: String,
        tool_name: String,
        arguments: Value,
        output: String,
        status: ToolCallStatus,
    },
    OnAgentEnd {
        thread_id: String,
        session_id: String,
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

/// Mutable registry of hooks shared by active and future agent threads.
///
/// Dispatchers snapshot the registered hooks before awaiting hook execution, so
/// runtime updates can replace the registry without holding a lock across user
/// hook code.
#[derive(Clone, Default)]
pub struct AgentHookRegistry {
    hooks: Arc<RwLock<Vec<Arc<dyn AgentHook>>>>,
}

impl AgentHookRegistry {
    pub fn new(hooks: Vec<Arc<dyn AgentHook>>) -> Self {
        Self { hooks: Arc::new(RwLock::new(hooks)) }
    }

    pub fn replace(&self, hooks: Vec<Arc<dyn AgentHook>>) {
        *self.hooks.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = hooks;
    }

    pub fn snapshot(&self) -> Vec<Arc<dyn AgentHook>> {
        self.hooks.read().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }
}

/// Run `event` through all hooks and merge their effects.
pub async fn dispatch_hooks(hooks: &[Arc<dyn AgentHook>], event: &HookEvent) -> HookEffects {
    let mut effects = HookEffects::default();
    for hook in hooks {
        effects.merge_outcome(hook.on_event(event).await);
    }
    effects
}

/// Snapshot the current registry and run `event` through all registered hooks.
pub async fn dispatch_registered_hooks(
    registry: &AgentHookRegistry,
    event: &HookEvent,
) -> HookEffects {
    let hooks = registry.snapshot();
    dispatch_hooks(&hooks, event).await
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::*;
    use crate::config::AgentConfig;

    struct RecordingHook {
        label: &'static str,
        events: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl AgentHook for RecordingHook {
        async fn on_event(&self, _event: &HookEvent) -> HookOutcome {
            self.events.lock().expect("events lock").push(self.label.to_owned());
            HookOutcome::Continue
        }
    }

    #[tokio::test]
    async fn hook_registry_replace_affects_next_dispatch() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let registry = AgentHookRegistry::new(vec![Arc::new(RecordingHook {
            label: "first",
            events: Arc::clone(&events),
        })]);
        dispatch_registered_hooks(&registry, &agent_start_event()).await;

        registry.replace(vec![Arc::new(RecordingHook {
            label: "second",
            events: Arc::clone(&events),
        })]);
        dispatch_registered_hooks(&registry, &agent_start_event()).await;

        let events = events.lock().expect("events lock").clone();
        assert_eq!(events, vec!["first", "second"]);
    }

    fn agent_start_event() -> HookEvent {
        HookEvent::OnAgentStart {
            thread_id: "thread".to_owned(),
            session_id: "session".to_owned(),
            parent_id: None,
            depth: 0,
            config: AgentConfig::default(),
        }
    }
}
