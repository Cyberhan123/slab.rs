use std::path::{Path, PathBuf};
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
    #[serde(default)]
    output_format: Option<String>,
    #[serde(default)]
    workspace_scope: Option<String>,
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
                },
                "output_format": {
                    "type": "string",
                    "description": "Optional requested output format for the child result."
                },
                "workspace_scope": {
                    "type": "string",
                    "description": "Optional workspace-relative path that bounds the delegated work."
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
        let output_format =
            args.output_format.as_deref().map(str::trim).filter(|value| !value.is_empty());
        let workspace_scope = resolve_workspace_scope(
            ctx.workspace.as_ref().map(|workspace| workspace.root.as_path()),
            args.workspace_scope.as_deref(),
        )?;

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
            content: ConversationMessageContent::Text(render_child_task(
                args.task.trim(),
                output_format,
                workspace_scope.as_ref(),
            )),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }];
        let child_thread_id =
            self.control.spawn_child_for_parent(&ctx.thread_id, child_config, messages).await?;
        let snapshot = self.control.wait_for_terminal_snapshot(&child_thread_id).await?;
        let artifact_refs = write_subagent_artifact(
            ctx.workspace.as_ref().map(|workspace| workspace.root.as_path()),
            &snapshot.id,
            &snapshot.completion_text,
        )
        .await?;
        let completion_text =
            if artifact_refs.is_empty() { snapshot.completion_text } else { None };

        Ok(ToolOutput {
            content: serde_json::json!({
                "child_thread_id": snapshot.id,
                "status": snapshot.status,
                "completion_text": completion_text,
                "artifact_refs": artifact_refs,
            })
            .to_string(),
            metadata: None,
        })
    }
}

fn default_system_prompt() -> String {
    "You are a focused subagent. Work only on the delegated task, use the allowed tools, and return a concise result for the parent agent.".to_owned()
}

fn render_child_task(
    task: &str,
    output_format: Option<&str>,
    workspace_scope: Option<&WorkspaceScope>,
) -> String {
    let mut prompt =
        format!("Objective:\n{task}\n\nConstraints:\n- Work only on this delegated task.");
    if let Some(scope) = workspace_scope {
        prompt.push_str("\n- Limit workspace file operations to this workspace-relative scope: ");
        prompt.push_str(&scope.relative);
    }
    if let Some(output_format) = output_format {
        prompt.push_str("\n\nRequired output format:\n");
        prompt.push_str(output_format);
    }
    prompt
}

#[derive(Debug, Clone)]
struct WorkspaceScope {
    relative: String,
}

fn resolve_workspace_scope(
    workspace_root: Option<&Path>,
    workspace_scope: Option<&str>,
) -> Result<Option<WorkspaceScope>, AgentError> {
    let Some(scope) = workspace_scope.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let Some(workspace_root) = workspace_root else {
        return Err(AgentError::ToolExecution(
            "workspace_scope requires a workspace context".to_owned(),
        ));
    };
    let scope_path = Path::new(scope);
    if scope_path.components().any(|component| {
        matches!(
            component,
            std::path::Component::Prefix(_)
                | std::path::Component::RootDir
                | std::path::Component::ParentDir
        )
    }) {
        return Err(AgentError::ToolExecution(
            "workspace_scope must stay inside the workspace".to_owned(),
        ));
    }
    let root = normalize_path(workspace_root);
    let resolved = normalize_path(root.join(scope_path));
    if !resolved.starts_with(&root) {
        return Err(AgentError::ToolExecution(
            "workspace_scope must stay inside the workspace".to_owned(),
        ));
    }
    Ok(Some(WorkspaceScope { relative: normalize_relative_scope(scope_path) }))
}

fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.as_ref().components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn normalize_relative_scope(path: &Path) -> String {
    let normalized = normalize_path(path);
    normalized.to_string_lossy().replace('\\', "/")
}

async fn write_subagent_artifact(
    workspace_root: Option<&Path>,
    child_thread_id: &str,
    completion_text: &Option<String>,
) -> Result<Vec<String>, AgentError> {
    let Some(workspace_root) = workspace_root else {
        return Ok(Vec::new());
    };
    let artifact_ref = format!(".slab/artifacts/{child_thread_id}/result.json");
    let artifact_path = workspace_root.join(&artifact_ref);
    if let Some(parent) = artifact_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
    }
    let content = serde_json::json!({
        "child_thread_id": child_thread_id,
        "completion_text": completion_text,
    });
    let bytes = serde_json::to_vec_pretty(&content)
        .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
    tokio::fs::write(&artifact_path, bytes)
        .await
        .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
    Ok(vec![artifact_ref])
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use slab_agent::port::{
        AgentNotifyPort, AgentStorePort, ApprovalDecision, ApprovalPort, LlmPort, LlmResponse,
        ThreadMessageRecord, ThreadSnapshot, ThreadStatus, ToolCallRecord, ToolSpec,
    };
    use slab_agent::{AgentControlLimits, ToolContext, ToolRouter, WorkspaceRef};
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
                usage: None,
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
                &ToolContext::for_thread("parent").build(),
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
    async fn delegate_subagent_writes_workspace_artifact_and_returns_reference() {
        let temp_dir =
            std::env::temp_dir().join(format!("slab-agent-tools-subagent-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        tokio::fs::create_dir_all(&temp_dir).await.expect("temp workspace");

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
                &ToolContext::for_thread("parent")
                    .workspace(WorkspaceRef { root: temp_dir.clone(), session_id: None })
                    .build(),
                &serde_json::json!({
                    "task": "summarize",
                    "workspace_scope": "src",
                    "output_format": "Return JSON with a summary field.",
                    "max_turns": 1
                }),
            )
            .await
            .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let artifact_ref = value["artifact_refs"][0].as_str().expect("artifact ref");

        assert_eq!(value["completion_text"], serde_json::Value::Null);
        assert!(artifact_ref.starts_with(".slab/artifacts/"));
        assert!(artifact_ref.ends_with("/result.json"));

        let artifact_path = temp_dir.join(artifact_ref);
        let artifact = tokio::fs::read_to_string(&artifact_path).await.expect("artifact content");
        let artifact: serde_json::Value = serde_json::from_str(&artifact).expect("artifact json");
        assert_eq!(artifact["completion_text"], "child result");

        let child_id = value["child_thread_id"].as_str().expect("child id");
        let messages = store.messages.lock().unwrap();
        let child_prompt = messages
            .iter()
            .find(|record| record.thread_id == child_id && record.message.role == "user")
            .expect("child prompt")
            .message
            .rendered_text();
        assert!(child_prompt.contains("Objective:\nsummarize"));
        assert!(child_prompt.contains("workspace-relative scope: src"));
        assert!(child_prompt.contains("Required output format:"));
        drop(messages);

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn delegate_subagent_rejects_workspace_scope_escape() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
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
                &ToolContext::for_thread("parent")
                    .workspace(WorkspaceRef {
                        root: PathBuf::from("C:/workspace/demo"),
                        session_id: None,
                    })
                    .build(),
                &serde_json::json!({"task": "summarize", "workspace_scope": "../outside"}),
            )
            .await;

        let error = result.expect_err("scope escape rejected").to_string();
        assert!(error.contains("workspace_scope must stay inside the workspace"));
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
                &ToolContext::for_thread("parent").build(),
                &serde_json::json!({"task": "summarize"}),
            )
            .await;

        assert!(matches!(result, Err(AgentError::DepthLimitExceeded { current: 1, max: 0 })));
    }
}
