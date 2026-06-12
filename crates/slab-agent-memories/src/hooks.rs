use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use slab_agent::{AgentHook, HookEvent, HookOutcome, HookToolAction};
use slab_types::{ConversationMessage, ConversationMessageContent, PluginPermissionsManifest};

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
    #[serde(rename = "javascript")]
    JavaScript,
    Python,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HookScript {
    pub name: String,
    pub plugin_id: Option<String>,
    pub language: HookScriptLanguage,
    pub root_dir: String,
    pub entry: String,
    pub bundle: Option<String>,
    pub export_name: String,
    pub events: Vec<String>,
    pub permissions: PluginPermissionsManifest,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct HookScriptOutput {
    #[serde(default)]
    pub injected_messages: Vec<HookInjectedMessage>,
    #[serde(default)]
    pub observations: Vec<String>,
    #[serde(default)]
    pub tool_action: Option<HookScriptToolAction>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct HookInjectedMessage {
    #[serde(default)]
    pub role: Option<String>,
    pub content: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookScriptToolAction {
    Continue,
    Block { reason: Option<String> },
    ModifyArgs { arguments: serde_json::Value },
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
        let mut tool_action = HookToolAction::Continue;
        for script in self.scripts.iter().filter(|script| {
            script.events.is_empty() || script.events.iter().any(|event| event == event_name)
        }) {
            match self.runner.run(script, event_name, payload.clone()).await {
                Ok(output) => {
                    for message in output.injected_messages {
                        match hook_message(script, message) {
                            Ok(message) => injected_messages.push(message),
                            Err(error) => observations.push(error),
                        }
                    }
                    observations.extend(output.observations);
                    merge_script_tool_action(
                        event_name,
                        script,
                        output.tool_action,
                        &mut tool_action,
                        &mut observations,
                    );
                }
                Err(error) => {
                    observations.push(format!("hook script `{}` failed: {error}", script.name))
                }
            }
        }

        if injected_messages.is_empty()
            && observations.is_empty()
            && matches!(tool_action, HookToolAction::Continue)
        {
            HookOutcome::Continue
        } else {
            HookOutcome::Effects { tool_action, injected_messages, observations }
        }
    }
}

fn hook_message(
    script: &HookScript,
    message: HookInjectedMessage,
) -> std::result::Result<ConversationMessage, String> {
    let role = message.role.as_deref().unwrap_or("developer").trim();
    if role != "developer" && role != "user" {
        return Err(format!(
            "hook script `{}` attempted to inject unsupported `{role}` message",
            script.name
        ));
    }
    if message.content.trim().is_empty() {
        return Err(format!("hook script `{}` returned an empty injected message", script.name));
    }
    Ok(ConversationMessage {
        role: role.to_owned(),
        content: ConversationMessageContent::Text(message.content),
        name: message.name.or_else(|| Some(script.name.clone())),
        tool_call_id: None,
        tool_calls: Vec::new(),
    })
}

fn merge_script_tool_action(
    event_name: &str,
    script: &HookScript,
    action: Option<HookScriptToolAction>,
    tool_action: &mut HookToolAction,
    observations: &mut Vec<String>,
) {
    let Some(action) = action else {
        return;
    };
    if event_name != "on_tool_start" {
        observations.push(format!(
            "hook script `{}` returned a tool action outside on_tool_start",
            script.name
        ));
        return;
    }
    if !matches!(tool_action, HookToolAction::Continue) {
        observations.push(format!(
            "hook script `{}` returned a tool action after one was already selected",
            script.name
        ));
        return;
    }
    match action {
        HookScriptToolAction::Continue => {}
        HookScriptToolAction::Block { reason } => {
            let reason = reason.unwrap_or_else(|| "blocked by hook script".to_owned());
            if reason.trim().is_empty() {
                observations
                    .push(format!("hook script `{}` returned a blank block reason", script.name));
                return;
            }
            *tool_action = HookToolAction::Block { reason };
        }
        HookScriptToolAction::ModifyArgs { arguments } => {
            if !arguments.is_object() {
                observations.push(format!(
                    "hook script `{}` returned non-object tool arguments",
                    script.name
                ));
                return;
            }
            *tool_action = HookToolAction::ModifyArgs { arguments };
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
        HookEvent::OnLlmStart { thread_id, session_id, turn_index, messages, tools } => (
            "on_llm_start",
            serde_json::json!({
                "thread_id": thread_id,
                "session_id": session_id,
                "turn_index": turn_index,
                "messages": messages,
                "tools": tools,
            }),
        ),
        HookEvent::OnLlmEnd { thread_id, session_id, turn_index, messages, response } => (
            "on_llm_end",
            serde_json::json!({
                "thread_id": thread_id,
                "session_id": session_id,
                "turn_index": turn_index,
                "messages": messages,
                "response": response,
            }),
        ),
        HookEvent::OnToolStart {
            thread_id,
            session_id,
            turn_index,
            messages,
            call_id,
            tool_name,
            arguments,
        } => (
            "on_tool_start",
            serde_json::json!({
                "thread_id": thread_id,
                "session_id": session_id,
                "turn_index": turn_index,
                "messages": messages,
                "call_id": call_id,
                "tool_name": tool_name,
                "arguments": arguments,
            }),
        ),
        HookEvent::OnToolEnd {
            thread_id,
            session_id,
            turn_index,
            messages,
            call_id,
            tool_name,
            arguments,
            output,
            status,
        } => (
            "on_tool_end",
            serde_json::json!({
                "thread_id": thread_id,
                "session_id": session_id,
                "turn_index": turn_index,
                "messages": messages,
                "call_id": call_id,
                "tool_name": tool_name,
                "arguments": arguments,
                "output": output,
                "status": status,
            }),
        ),
        HookEvent::OnAgentEnd { thread_id, session_id, status, error } => (
            "on_agent_end",
            serde_json::json!({
                "thread_id": thread_id,
                "session_id": session_id,
                "status": status,
                "error": error,
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use slab_agent::{AgentConfig, HookToolAction};

    use super::*;

    #[derive(Clone)]
    struct StaticRunner {
        output: HookScriptOutput,
    }

    #[async_trait]
    impl HookScriptRunner for StaticRunner {
        async fn run(
            &self,
            _script: &HookScript,
            _event_name: &str,
            _payload: serde_json::Value,
        ) -> std::result::Result<HookScriptOutput, String> {
            Ok(self.output.clone())
        }
    }

    #[tokio::test]
    async fn script_output_injects_safe_messages_and_rejects_unsafe_roles() {
        let hook = RegisteredScriptHook::new(
            vec![script(vec!["on_agent_start"])],
            Arc::new(StaticRunner {
                output: HookScriptOutput {
                    injected_messages: vec![
                        HookInjectedMessage {
                            role: None,
                            content: "developer note".to_owned(),
                            name: None,
                        },
                        HookInjectedMessage {
                            role: Some("user".to_owned()),
                            content: "user note".to_owned(),
                            name: Some("custom".to_owned()),
                        },
                        HookInjectedMessage {
                            role: Some("assistant".to_owned()),
                            content: "unsafe".to_owned(),
                            name: None,
                        },
                    ],
                    observations: Vec::new(),
                    tool_action: None,
                },
            }),
        );

        let outcome = hook
            .on_event(&HookEvent::OnAgentStart {
                thread_id: "thread".into(),
                session_id: "session".into(),
                parent_id: None,
                depth: 0,
                config: AgentConfig::default(),
            })
            .await;

        let HookOutcome::Effects { injected_messages, observations, .. } = outcome else {
            panic!("expected hook effects");
        };
        assert_eq!(injected_messages.len(), 2);
        assert_eq!(injected_messages[0].role, "developer");
        assert_eq!(injected_messages[0].name.as_deref(), Some("test_hook"));
        assert_eq!(injected_messages[1].role, "user");
        assert_eq!(injected_messages[1].name.as_deref(), Some("custom"));
        assert!(observations.iter().any(|value| value.contains("unsupported `assistant`")));
    }

    #[tokio::test]
    async fn script_tool_action_only_applies_on_tool_start() {
        let hook = RegisteredScriptHook::new(
            vec![script(Vec::new())],
            Arc::new(StaticRunner {
                output: HookScriptOutput {
                    injected_messages: Vec::new(),
                    observations: Vec::new(),
                    tool_action: Some(HookScriptToolAction::ModifyArgs {
                        arguments: serde_json::json!({"path": "changed"}),
                    }),
                },
            }),
        );

        let outcome = hook
            .on_event(&HookEvent::OnLlmStart {
                thread_id: "thread".into(),
                session_id: "session".into(),
                turn_index: 0,
                messages: Vec::new(),
                tools: Vec::new(),
            })
            .await;
        let HookOutcome::Effects { tool_action, observations, .. } = outcome else {
            panic!("expected hook effects");
        };
        assert!(matches!(tool_action, HookToolAction::Continue));
        assert!(observations.iter().any(|value| value.contains("outside on_tool_start")));

        let outcome = hook
            .on_event(&HookEvent::OnToolStart {
                thread_id: "thread".into(),
                session_id: "session".into(),
                turn_index: 0,
                messages: Vec::new(),
                call_id: "call".into(),
                tool_name: "read_file".into(),
                arguments: serde_json::json!({"path": "old"}),
            })
            .await;
        let HookOutcome::Effects { tool_action, observations, .. } = outcome else {
            panic!("expected hook effects");
        };
        assert!(observations.is_empty());
        assert!(
            matches!(tool_action, HookToolAction::ModifyArgs { arguments } if arguments["path"] == "changed")
        );
    }

    #[tokio::test]
    async fn script_rejects_non_object_modify_args() {
        let hook = RegisteredScriptHook::new(
            vec![script(Vec::new())],
            Arc::new(StaticRunner {
                output: HookScriptOutput {
                    injected_messages: Vec::new(),
                    observations: Vec::new(),
                    tool_action: Some(HookScriptToolAction::ModifyArgs {
                        arguments: serde_json::json!("not-object"),
                    }),
                },
            }),
        );

        let outcome = hook
            .on_event(&HookEvent::OnToolStart {
                thread_id: "thread".into(),
                session_id: "session".into(),
                turn_index: 0,
                messages: Vec::new(),
                call_id: "call".into(),
                tool_name: "read_file".into(),
                arguments: serde_json::json!({"path": "old"}),
            })
            .await;

        let HookOutcome::Effects { tool_action, observations, .. } = outcome else {
            panic!("expected hook effects");
        };
        assert!(matches!(tool_action, HookToolAction::Continue));
        assert!(observations.iter().any(|value| value.contains("non-object")));
    }

    fn script(events: Vec<&str>) -> HookScript {
        HookScript {
            name: "test_hook".to_owned(),
            plugin_id: None,
            language: HookScriptLanguage::JavaScript,
            root_dir: ".".to_owned(),
            entry: "hook.mjs".to_owned(),
            bundle: None,
            export_name: "run".to_owned(),
            events: events.into_iter().map(str::to_owned).collect(),
            permissions: PluginPermissionsManifest::default(),
        }
    }
}
