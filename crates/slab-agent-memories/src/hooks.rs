use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use slab_agent::{AgentHook, HookEvent, HookOutcome};
use slab_types::{ConversationMessage, ConversationMessageContent};

use crate::read::{MemoryReadConfig, render_read_developer_turn};

#[derive(Debug, Clone)]
pub struct MemoryInstructionHook {
    enabled: bool,
    memory_root: PathBuf,
}

impl MemoryInstructionHook {
    pub fn new(enabled: bool, memory_root: PathBuf) -> Self {
        Self { enabled, memory_root }
    }
}

#[async_trait]
impl AgentHook for MemoryInstructionHook {
    async fn on_event(&self, event: &HookEvent) -> HookOutcome {
        let HookEvent::OnAgentStart { config, .. } = event else {
            return HookOutcome::Continue;
        };
        if !self.enabled || config.transient {
            return HookOutcome::Continue;
        }
        let config = MemoryReadConfig {
            memory_root: self.memory_root.clone(),
            inject_hook_instructions: true,
        };
        match render_read_developer_turn(&config) {
            Ok(Some(message)) => HookOutcome::inject_message(message),
            Ok(None) => HookOutcome::Continue,
            Err(error) => HookOutcome::AppendObservation {
                observation: format!("memory instruction injection skipped: {error}"),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HookScriptLanguage {
    JavaScript,
    Python,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct HookScript {
    pub name: String,
    pub language: HookScriptLanguage,
    pub source: String,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct HookScriptOutput {
    #[serde(default)]
    pub injected_messages: Vec<String>,
    #[serde(default)]
    pub observations: Vec<String>,
}

/// Host-owned executor for registered hook scripts.
///
/// Implementations are expected to call supervised JS/Python runtimes and
/// return only local, deterministic hook effects.
#[async_trait]
pub trait HookScriptRunner: Send + Sync {
    async fn run(
        &self,
        script: &HookScript,
        event_name: &str,
        payload: serde_json::Value,
    ) -> std::result::Result<HookScriptOutput, String>;
}

pub struct RegisteredScriptHook {
    scripts: Vec<HookScript>,
    runner: Arc<dyn HookScriptRunner>,
}

impl RegisteredScriptHook {
    pub fn new(scripts: Vec<HookScript>, runner: Arc<dyn HookScriptRunner>) -> Self {
        Self { scripts, runner }
    }
}

#[async_trait]
impl AgentHook for RegisteredScriptHook {
    async fn on_event(&self, event: &HookEvent) -> HookOutcome {
        let (event_name, payload) = hook_event_payload(event);
        let mut injected_messages = Vec::new();
        let mut observations = Vec::new();
        for script in self.scripts.iter().filter(|script| {
            script.events.is_empty() || script.events.iter().any(|event| event == event_name)
        }) {
            match self.runner.run(script, event_name, payload.clone()).await {
                Ok(output) => {
                    injected_messages.extend(output.injected_messages.into_iter().map(|content| {
                        ConversationMessage {
                            role: "developer".to_owned(),
                            content: ConversationMessageContent::Text(content),
                            name: Some(script.name.clone()),
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                        }
                    }));
                    observations.extend(output.observations);
                }
                Err(error) => {
                    observations.push(format!("hook script `{}` failed: {error}", script.name))
                }
            }
        }

        if injected_messages.is_empty() && observations.is_empty() {
            HookOutcome::Continue
        } else {
            HookOutcome::Effects {
                tool_action: slab_agent::HookToolAction::Continue,
                injected_messages,
                observations,
            }
        }
    }
}

fn hook_event_payload(event: &HookEvent) -> (&'static str, serde_json::Value) {
    match event {
        HookEvent::OnAgentStart { thread_id, session_id, parent_id, depth, config } => (
            "on_agent_start",
            serde_json::json!({
                "thread_id": thread_id,
                "session_id": session_id,
                "parent_id": parent_id,
                "depth": depth,
                "config": config,
            }),
        ),
        HookEvent::OnLlmStart { thread_id, turn_index, messages, tools } => (
            "on_llm_start",
            serde_json::json!({
                "thread_id": thread_id,
                "turn_index": turn_index,
                "messages": messages,
                "tools": tools,
            }),
        ),
        HookEvent::OnLlmEnd { thread_id, turn_index, response } => (
            "on_llm_end",
            serde_json::json!({
                "thread_id": thread_id,
                "turn_index": turn_index,
                "response": response,
            }),
        ),
        HookEvent::OnToolStart { thread_id, turn_index, call_id, tool_name, arguments } => (
            "on_tool_start",
            serde_json::json!({
                "thread_id": thread_id,
                "turn_index": turn_index,
                "call_id": call_id,
                "tool_name": tool_name,
                "arguments": arguments,
            }),
        ),
        HookEvent::OnToolEnd {
            thread_id,
            turn_index,
            call_id,
            tool_name,
            arguments,
            output,
            status,
        } => (
            "on_tool_end",
            serde_json::json!({
                "thread_id": thread_id,
                "turn_index": turn_index,
                "call_id": call_id,
                "tool_name": tool_name,
                "arguments": arguments,
                "output": output,
                "status": status,
            }),
        ),
        HookEvent::OnAgentEnd { thread_id, status, error } => (
            "on_agent_end",
            serde_json::json!({
                "thread_id": thread_id,
                "status": status,
                "error": error,
            }),
        ),
    }
}
