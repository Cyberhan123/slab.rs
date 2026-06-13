use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use slab_agent::{AgentConfig, AgentControl, AgentError, ToolContext, ToolHandler, ToolOutput};
use slab_types::{ConversationMessage, ConversationMessageContent};

const DEFAULT_SUBAGENT_TURNS: u32 = 8;

pub struct DelegateSubagentTool {
    control: Arc<AgentControl>,
}

impl DelegateSubagentTool {
    pub fn new(control: Arc<AgentControl>) -> Self {
        Self { control }
    }
}

#[derive(Debug, Deserialize)]
struct DelegateSubagentArgs {
    task: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    system_prompt: Option<String>,
    #[serde(default)]
    allowed_tools: Option<Vec<String>>,
    #[serde(default)]
    max_turns: Option<u32>,
}

#[async_trait]
impl ToolHandler for DelegateSubagentTool {
    fn name(&self) -> &str {
        "delegate_subagent"
    }

    fn description(&self) -> &str {
        "Delegate a focused task to an isolated child agent and wait for its result."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The focused task for the child agent."
                },
                "model": {
                    "type": "string",
                    "description": "Optional model override for the child agent."
                },
                "system_prompt": {
                    "type": "string",
                    "description": "Optional child-agent system prompt."
                },
                "allowed_tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tool allow-list for the child agent."
                },
                "max_turns": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Optional child-agent turn limit."
                }
            },
            "required": ["task"]
        })
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args: DelegateSubagentArgs =
            serde_json::from_value(arguments.clone()).map_err(|error| {
                AgentError::ToolExecution(format!("invalid subagent args: {error}"))
            })?;
        if args.task.trim().is_empty() {
            return Err(AgentError::ToolExecution("subagent task must not be blank".to_owned()));
        }

        let parent = self
            .control
            .thread_snapshot(&ctx.thread_id)
            .await?
            .ok_or_else(|| AgentError::ThreadNotFound(ctx.thread_id.clone()))?;
        let mut child_config =
            serde_json::from_str::<AgentConfig>(&parent.config_json).map_err(|error| {
                AgentError::ToolExecution(format!("invalid parent agent config: {error}"))
            })?;
        if let Some(model) = args.model.filter(|value| !value.trim().is_empty()) {
            child_config.model = model;
        }
        child_config.system_prompt = Some(args.system_prompt.unwrap_or_else(default_system_prompt));
        if let Some(allowed_tools) = args.allowed_tools {
            child_config.allowed_tools =
                allowed_tools.into_iter().filter(|tool| !tool.trim().is_empty()).collect();
        }
        child_config.max_turns = args.max_turns.unwrap_or(DEFAULT_SUBAGENT_TURNS).max(1);
        child_config.transient = true;

        let messages = vec![ConversationMessage {
            role: "user".to_owned(),
            content: ConversationMessageContent::Text(args.task),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }];
        let child_thread_id =
            self.control.spawn_child_for_parent(&ctx.thread_id, child_config, messages).await?;
        let snapshot = self.control.wait_for_terminal_snapshot(&child_thread_id).await?;

        Ok(ToolOutput {
            content: serde_json::json!({
                "child_thread_id": snapshot.id,
                "status": snapshot.status,
                "completion_text": snapshot.completion_text,
            })
            .to_string(),
            metadata: None,
        })
    }
}

fn default_system_prompt() -> String {
    "You are a focused subagent. Work only on the delegated task, use the allowed tools, and return a concise result for the parent agent.".to_owned()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use slab_agent::port::{
        AgentNotifyPort, AgentStorePort, ApprovalDecision, ApprovalPort, LlmPort, LlmResponse,
        ThreadMessageRecord, ThreadSnapshot, ThreadStatus, ToolCallRecord, ToolSpec,
    };
    use slab_agent::{AgentControlLimits, ToolRouter};
    use slab_agent_tracing::AgentTraceContext;
    use slab_types::ConversationMessage;

    use super::*;

    struct FinalLlm;

    #[async_trait]
    impl LlmPort for FinalLlm {
        async fn chat_completion(
            &self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _config: &AgentConfig,
            _trace_context: &AgentTraceContext,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                content: Some("child result".to_owned()),
                content_already_streamed: false,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".to_owned()),
            })
        }
    }

    #[derive(Default)]
    struct MemoryStore {
        threads: Mutex<HashMap<String, ThreadSnapshot>>,
        messages: Mutex<Vec<ThreadMessageRecord>>,
    }

    impl MemoryStore {
        fn insert_parent(&self, max_depth: u32) {
            let config = AgentConfig { model: "mock".into(), max_depth, ..AgentConfig::default() };
            let now = "2026-01-01T00:00:00Z".to_owned();
            self.threads.lock().unwrap().insert(
                "parent".to_owned(),
                ThreadSnapshot {
                    id: "parent".to_owned(),
                    session_id: "session".to_owned(),
                    parent_id: None,
                    depth: 0,
                    status: ThreadStatus::Completed,
                    role_name: None,
                    config_json: serde_json::to_string(&config).expect("config"),
                    completion_text: Some("parent".to_owned()),
                    created_at: now.clone(),
                    updated_at: now,
                },
            );
        }
    }

    #[async_trait]
    impl AgentStorePort for MemoryStore {
        async fn upsert_thread(&self, snapshot: &ThreadSnapshot) -> Result<(), AgentError> {
            self.threads.lock().unwrap().insert(snapshot.id.clone(), snapshot.clone());
            Ok(())
        }

        async fn get_thread(&self, id: &str) -> Result<Option<ThreadSnapshot>, AgentError> {
            Ok(self.threads.lock().unwrap().get(id).cloned())
        }

        async fn list_session_threads(
            &self,
            _session_id: &str,
        ) -> Result<Vec<ThreadSnapshot>, AgentError> {
            Ok(Vec::new())
        }

        async fn update_thread_status(
            &self,
            id: &str,
            status: ThreadStatus,
            completion_text: Option<&str>,
        ) -> Result<(), AgentError> {
            let mut threads = self.threads.lock().unwrap();
            let snapshot =
                threads.get_mut(id).ok_or_else(|| AgentError::ThreadNotFound(id.to_owned()))?;
            snapshot.status = status;
            snapshot.completion_text = completion_text.map(str::to_owned);
            Ok(())
        }

        async fn insert_tool_call(&self, _record: &ToolCallRecord) -> Result<(), AgentError> {
            Ok(())
        }

        async fn update_tool_call_status(
            &self,
            _id: &str,
            _status: slab_types::agent::ToolCallStatus,
        ) -> Result<(), AgentError> {
            Ok(())
        }

        async fn update_tool_call(
            &self,
            _id: &str,
            _output: Option<&str>,
            _status: slab_types::agent::ToolCallStatus,
            _completed_at: &str,
        ) -> Result<(), AgentError> {
            Ok(())
        }

        async fn insert_thread_message(
            &self,
            record: &ThreadMessageRecord,
        ) -> Result<(), AgentError> {
            self.messages.lock().unwrap().push(record.clone());
            Ok(())
        }

        async fn list_thread_messages(
            &self,
            _thread_id: &str,
        ) -> Result<Vec<ThreadMessageRecord>, AgentError> {
            Ok(Vec::new())
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
            _command: &str,
            _risk: Option<slab_agent::ToolRiskAssessment>,
        ) -> ApprovalDecision {
            ApprovalDecision::Approved
        }
    }

    #[tokio::test]
    async fn delegate_subagent_spawns_transient_child_and_returns_result() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store.clone(),
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = DelegateSubagentTool::new(control);

        let output = tool
            .execute(
                &ToolContext { thread_id: "parent".into(), turn_index: 0, depth: 0 },
                &serde_json::json!({
                    "task": "summarize",
                    "allowed_tools": ["read_file"],
                    "max_turns": 1
                }),
            )
            .await
            .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let child_id = value["child_thread_id"].as_str().expect("child id");
        assert_eq!(value["status"], "completed");
        assert_eq!(value["completion_text"], "child result");

        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        assert_eq!(child.parent_id.as_deref(), Some("parent"));
        assert_eq!(child.depth, 1);
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        assert!(child_config.transient);
        assert_eq!(child_config.allowed_tools, vec!["read_file"]);
        assert_eq!(child_config.max_turns, 1);
    }

    #[tokio::test]
    async fn delegate_subagent_respects_parent_depth_limit() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(0);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store,
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = DelegateSubagentTool::new(control);

        let result = tool
            .execute(
                &ToolContext { thread_id: "parent".into(), turn_index: 0, depth: 0 },
                &serde_json::json!({"task": "summarize"}),
            )
            .await;

        assert!(matches!(result, Err(AgentError::DepthLimitExceeded { current: 1, max: 0 })));
    }
}
