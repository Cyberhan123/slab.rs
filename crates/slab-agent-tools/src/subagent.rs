use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use slab_agent::{
    AgentConfig, AgentControl, AgentError, ModelPolicy, ToolContext, ToolOutput, TypedTool,
};
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DelegateSubagentArgs {
    /// The focused task for the child agent.
    task: String,
    /// Optional built-in agent type (e.g. "plan"). Resolves a tool constraint and system prompt from the agent registry; the call fails if the type is unknown.
    agent_type: Option<String>,
    /// Optional model override for the child agent.
    model: Option<String>,
    /// Optional child-agent system prompt.
    system_prompt: Option<String>,
    /// Optional tool allow-list for the child agent.
    allowed_tools: Option<Vec<String>>,
    /// Optional child-agent turn limit.
    #[schemars(range(min = 1))]
    max_turns: Option<u32>,
    /// Optional requested output format for the child result.
    output_format: Option<String>,
    /// Optional workspace-relative path that bounds the delegated work.
    workspace_scope: Option<String>,
}

#[async_trait]
impl TypedTool for DelegateSubagentTool {
    type Input = DelegateSubagentArgs;
    fn name(&self) -> &str {
        "delegate_subagent"
    }

    fn description(&self) -> &str {
        "Delegate a focused task to an isolated child agent and wait for its result."
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        args: DelegateSubagentArgs,
    ) -> Result<ToolOutput, AgentError> {
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
        // Slice 4: resolve a built-in agent_type (if any) BEFORE applying caller
        // overrides so an explicit caller value still wins. A named type that is
        // absent from the registry is a hard error — the model asked for an agent
        // that does not exist.
        let definition =
            match args.agent_type.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
                Some(agent_type) => {
                    let registry = self.control.agent_registry();
                    Some(registry.get(agent_type).ok_or_else(|| {
                        AgentError::ToolExecution(format!("unknown agent_type: {agent_type}"))
                    })?)
                }
                None => None,
            };
        if let Some(definition) = &definition {
            child_config.agent_type = Some(definition.agent_type.clone());
        }
        if let Some(model) = args.model.filter(|value| !value.trim().is_empty()) {
            child_config.model = model;
        } else if let Some(definition) = &definition
            && let ModelPolicy::Fixed(model) = &definition.model
        {
            child_config.model = model.clone();
        }
        child_config.system_prompt = Some(match args.system_prompt {
            Some(prompt) => prompt,
            None => definition
                .as_ref()
                .map(|definition| definition.system_prompt.clone())
                .unwrap_or_else(default_system_prompt),
        });
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
        // The snapshot's completion_text is LLM-grade (reasoning embedded as
        // `<think>` blocks for the next chat-template round); the parent
        // conversation and the persisted artifact only want the final answer.
        // Same strip the UI-delta and history-preview paths already apply —
        // this is the third exit that used to leak it.
        let completion_text =
            snapshot.completion_text.as_deref().map(slab_agent::strip_think_blocks);
        let artifact_refs = write_subagent_artifact(
            ctx.workspace.as_ref().map(|workspace| workspace.root.as_path()),
            &snapshot.id,
            &completion_text,
        )
        .await?;
        let completion_text = if artifact_refs.is_empty() { completion_text } else { None };

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
    let content = serde_json::json!({
        "child_thread_id": child_thread_id,
        "completion_text": completion_text,
    });
    let bytes = serde_json::to_vec_pretty(&content)
        .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
    // Thin wrapper over the shared spill helper (same result.json layout as
    // before the generalization).
    match crate::artifact::write_tool_artifact(
        Some(workspace_root),
        child_thread_id,
        "result.json",
        &bytes,
    )
    .await
    {
        Some(artifact_ref) => Ok(vec![artifact_ref]),
        None => Err(AgentError::ToolExecution("failed to write subagent artifact".to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use slab_agent::ToolHandler;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use slab_agent::port::{
        AgentNotifyPort, AgentStorePort, ApprovalDecision, ApprovalPort, LlmPort, LlmResponse,
        ThreadMessageRecord, ThreadSnapshot, ThreadStatus, ToolSpec,
    };
    use slab_agent::{
        AgentControlLimits, AgentDefinition, AgentRegistry, ToolConstraint, ToolContext,
        ToolRouter, WorkspaceRef,
    };
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
        // The slab-agent `insert_thread_message` trait method is gone;
        // retained for direct-push seeding in tests. Unread — tests verify
        // emission via `RecordingNotify`.
        #[allow(dead_code)]
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
                    archived_at: None,
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
    }

    struct NoopNotify;

    #[async_trait]
    impl AgentNotifyPort for NoopNotify {
        async fn on_status_change(&self, _thread_id: &str, _status: ThreadStatus) {}
    }

    /// A notify port that records emitted `EventMsg`s so tests can
    /// verify emission (slab-agent no longer writes conversation data to the
    /// store — it emits `MessageAppended` / `TurnStateChanged` events).
    #[derive(Default)]
    struct RecordingNotify {
        events: std::sync::Mutex<Vec<slab_agent::protocol::EventMsg>>,
    }

    #[async_trait]
    impl AgentNotifyPort for RecordingNotify {
        async fn on_status_change(&self, _thread_id: &str, _status: ThreadStatus) {}

        async fn on_event_msg(&self, _thread_id: &str, msg: &slab_agent::protocol::EventMsg) {
            self.events.lock().unwrap().push(msg.clone());
        }
    }

    #[async_trait]
    impl ApprovalPort for RecordingNotify {
        async fn request_approval(
            &self,
            _thread_id: &str,
            _call_id: &str,
            _tool_name: &str,
            _descriptor: &slab_agent::OperationDescriptor,
            _risk: Option<slab_agent::ToolRiskAssessment>,
        ) -> ApprovalDecision {
            ApprovalDecision::Approved(slab_agent::ApprovalScope::RunOnce)
        }
    }

    impl RecordingNotify {
        /// Emitted `MessageAppended` conversation messages for a thread.
        fn emitted_messages(&self, thread_id: &str) -> Vec<slab_types::ConversationMessage> {
            use slab_agent::protocol::EventMsg;
            self.events
                .lock()
                .unwrap()
                .iter()
                .filter_map(|event| match event {
                    EventMsg::MessageAppended(p) if p.thread_id == thread_id => {
                        Some(p.message.clone())
                    }
                    _ => None,
                })
                .collect()
        }
    }

    #[async_trait]
    impl ApprovalPort for NoopNotify {
        async fn request_approval(
            &self,
            _thread_id: &str,
            _call_id: &str,
            _tool_name: &str,
            _descriptor: &slab_agent::OperationDescriptor,
            _risk: Option<slab_agent::ToolRiskAssessment>,
        ) -> ApprovalDecision {
            ApprovalDecision::Approved(slab_agent::ApprovalScope::RunOnce)
        }
    }

    /// LLM double whose final answer embeds a `<think>` reasoning block
    /// (LLM-grade text, as the chat-template round actually produces).
    struct ThinkingLlm;

    #[async_trait]
    impl LlmPort for ThinkingLlm {
        async fn chat_completion(
            &self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _config: &AgentConfig,
            _trace_context: &AgentTraceContext,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                content: Some(
                    "<think status=\"done\">plan the summary privately</think>child result"
                        .to_owned(),
                ),
                content_already_streamed: false,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".to_owned()),
                usage: None,
            })
        }
    }

    #[tokio::test]
    async fn delegate_subagent_strips_think_blocks_from_completion() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(ThinkingLlm),
            store.clone(),
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = DelegateSubagentTool::new(control);

        // No workspace: the stripped completion flows into the parent tool
        // output verbatim.
        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "summarize", "max_turns": 1 }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        assert_eq!(value["status"], "completed");
        assert_eq!(
            value["completion_text"], "child result",
            "think blocks must not leak into the parent conversation"
        );

        // Workspace variant: the persisted artifact carries the stripped text.
        let temp_dir = std::env::temp_dir()
            .join(format!("slab-agent-tools-subagent-think-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        tokio::fs::create_dir_all(&temp_dir).await.expect("temp workspace");
        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent")
                .workspace(WorkspaceRef { root: temp_dir.clone(), session_id: None })
                .build(),
            &serde_json::json!({ "task": "summarize", "max_turns": 1 }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let artifact_ref = value["artifact_refs"][0].as_str().expect("artifact ref");
        let artifact =
            tokio::fs::read_to_string(temp_dir.join(artifact_ref)).await.expect("artifact content");
        let artifact: serde_json::Value = serde_json::from_str(&artifact).expect("artifact json");
        assert_eq!(artifact["completion_text"], "child result");

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
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

        let output = ToolHandler::execute(
            &tool,
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
        let notify = Arc::new(RecordingNotify::default());
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store.clone(),
            notify.clone(),
            notify.clone(),
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = DelegateSubagentTool::new(control);

        let output = ToolHandler::execute(
            &tool,
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
        // slab-agent emits `MessageAppended` (no store writes); read
        // the emitted child-prompt message from the recording notify.
        let child_prompt = notify
            .emitted_messages(child_id)
            .iter()
            .find(|message| message.role == "user")
            .expect("emitted child prompt")
            .rendered_text();
        assert!(child_prompt.contains("Objective:\nsummarize"));
        assert!(child_prompt.contains("workspace-relative scope: src"));
        assert!(child_prompt.contains("Required output format:"));

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

        let result = ToolHandler::execute(
            &tool,
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

        let result = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({"task": "summarize"}),
        )
        .await;

        assert!(matches!(result, Err(AgentError::DepthLimitExceeded { current: 1, max: 0 })));
    }

    // ---- Slice 4: agent_type integration ----

    const PLAN_PROMPT: &str = "You are a read-only planning agent.";

    /// LLM that records the tool list presented each call, so tests can verify
    /// the agent tool constraint reached the model-facing projection.
    #[derive(Default)]
    struct RecordingLlm {
        captured_tools: Mutex<Vec<Vec<ToolSpec>>>,
    }

    #[async_trait]
    impl LlmPort for RecordingLlm {
        async fn chat_completion(
            &self,
            _model: &str,
            _messages: &[ConversationMessage],
            tools: &[ToolSpec],
            _config: &AgentConfig,
            _trace_context: &AgentTraceContext,
        ) -> Result<LlmResponse, AgentError> {
            self.captured_tools.lock().unwrap().push(tools.to_vec());
            Ok(LlmResponse {
                content: Some("child result".to_owned()),
                content_already_streamed: false,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".to_owned()),
                usage: None,
            })
        }
    }

    /// Minimal named tool handler used only to populate a router with specs.
    struct StubTool {
        tool_name: String,
    }

    impl StubTool {
        fn new(name: &str) -> Self {
            Self { tool_name: name.to_owned() }
        }
    }

    #[async_trait]
    impl TypedTool for StubTool {
        type Input = serde_json::Value;
        fn name(&self) -> &str {
            &self.tool_name
        }

        fn description(&self) -> &str {
            "stub"
        }

        async fn execute(
            &self,
            _ctx: &ToolContext,
            _arguments: serde_json::Value,
        ) -> Result<ToolOutput, AgentError> {
            Ok(ToolOutput { content: "stub".to_owned(), metadata: None })
        }
    }

    /// HashMap-backed agent registry for tests.
    #[derive(Default)]
    struct MockRegistry {
        agents: Vec<AgentDefinition>,
    }

    impl MockRegistry {
        /// A "plan" agent that denies `shell` with a fixed system prompt.
        fn plan() -> Self {
            Self {
                agents: vec![AgentDefinition {
                    agent_type: "plan".to_owned(),
                    description: "test plan agent".to_owned(),
                    tools: ToolConstraint::Denylist(vec!["shell".to_owned()]),
                    system_prompt: PLAN_PROMPT.to_owned(),
                    model: ModelPolicy::Inherit,
                }],
            }
        }

        /// A "plan" agent that also pins a model (for caller-overrides tests).
        fn plan_with_fixed_model(model: &str) -> Self {
            let mut registry = Self::plan();
            if let Some(def) = registry.agents.get_mut(0) {
                def.model = ModelPolicy::Fixed(model.to_owned());
            }
            registry
        }
    }

    impl AgentRegistry for MockRegistry {
        fn get(&self, agent_type: &str) -> Option<AgentDefinition> {
            self.agents.iter().find(|def| def.agent_type == agent_type).cloned()
        }
        fn list(&self) -> Vec<AgentDefinition> {
            self.agents.clone()
        }
    }

    fn build_control(
        llm: Arc<dyn LlmPort>,
        store: Arc<MemoryStore>,
        router: Arc<ToolRouter>,
        registry: Arc<dyn AgentRegistry>,
    ) -> Arc<AgentControl> {
        let notify = Arc::new(NoopNotify);
        Arc::new(
            slab_agent::AgentControl::new_with_hooks(
                llm,
                store,
                notify.clone(),
                notify,
                router,
                AgentControlLimits { max_threads: 4, max_depth: 4 },
                Vec::new(),
            )
            .with_agent_registry(registry),
        )
    }

    #[tokio::test]
    async fn delegate_subagent_with_agent_type_sets_config_and_system_prompt() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let control = build_control(
            Arc::new(FinalLlm),
            store.clone(),
            Arc::new(ToolRouter::new()),
            Arc::new(MockRegistry::plan()),
        );
        let tool = DelegateSubagentTool::new(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "plan it", "agent_type": "plan" }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let child_id = value["child_thread_id"].as_str().expect("child id");

        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        assert_eq!(child_config.agent_type.as_deref(), Some("plan"));
        assert_eq!(child_config.system_prompt.as_deref(), Some(PLAN_PROMPT));
        assert!(child_config.transient);
    }

    #[tokio::test]
    async fn delegate_subagent_agent_type_enforces_tool_constraint_end_to_end() {
        let router = ToolRouter::new();
        router.register(Box::new(StubTool::new("shell")));
        router.register(Box::new(StubTool::new("read_file")));
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let llm = Arc::new(RecordingLlm::default());
        let control = build_control(
            Arc::clone(&llm) as Arc<dyn LlmPort>,
            store.clone(),
            Arc::new(router),
            Arc::new(MockRegistry::plan()),
        );
        let tool = DelegateSubagentTool::new(control);

        ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "plan it", "agent_type": "plan", "max_turns": 1 }),
        )
        .await
        .expect("delegate");

        let captured = llm.captured_tools.lock().unwrap().clone();
        let names: Vec<String> = captured.iter().flatten().map(|spec| spec.name.clone()).collect();
        assert!(
            !names.contains(&"shell".to_owned()),
            "plan agent must not see the denied `shell` tool: {names:?}"
        );
        assert!(
            names.contains(&"read_file".to_owned()),
            "plan agent should still see `read_file`: {names:?}"
        );
    }

    #[tokio::test]
    async fn delegate_subagent_agent_type_not_in_registry_errors() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        // Default control carries a NoopAgentRegistry — "missing" is unknown.
        let control = build_control(
            Arc::new(FinalLlm),
            store.clone(),
            Arc::new(ToolRouter::new()),
            Arc::new(MockRegistry::default()),
        );
        let tool = DelegateSubagentTool::new(control);

        let result = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "plan it", "agent_type": "missing" }),
        )
        .await;

        let error = result.expect_err("unknown agent_type rejected").to_string();
        assert!(error.contains("unknown agent_type: missing"), "{error}");
    }

    #[tokio::test]
    async fn delegate_subagent_explicit_system_prompt_overrides_definition() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let control = build_control(
            Arc::new(FinalLlm),
            store.clone(),
            Arc::new(ToolRouter::new()),
            Arc::new(MockRegistry::plan()),
        );
        let tool = DelegateSubagentTool::new(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({
                "task": "plan it",
                "agent_type": "plan",
                "system_prompt": "custom prompt"
            }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let child_id = value["child_thread_id"].as_str().expect("child id");

        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        assert_eq!(child_config.system_prompt.as_deref(), Some("custom prompt"));
        assert_eq!(child_config.agent_type.as_deref(), Some("plan"));
    }

    #[tokio::test]
    async fn delegate_subagent_explicit_model_overrides_definition() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let control = build_control(
            Arc::new(FinalLlm),
            store.clone(),
            Arc::new(ToolRouter::new()),
            Arc::new(MockRegistry::plan_with_fixed_model("plan-model")),
        );
        let tool = DelegateSubagentTool::new(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({
                "task": "plan it",
                "agent_type": "plan",
                "model": "caller-model"
            }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let child_id = value["child_thread_id"].as_str().expect("child id");

        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        // Caller wins over ModelPolicy::Fixed.
        assert_eq!(child_config.model, "caller-model");
        assert_eq!(child_config.agent_type.as_deref(), Some("plan"));
    }

    #[tokio::test]
    async fn delegate_subagent_definition_model_applies_when_caller_omits() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let control = build_control(
            Arc::new(FinalLlm),
            store.clone(),
            Arc::new(ToolRouter::new()),
            Arc::new(MockRegistry::plan_with_fixed_model("plan-model")),
        );
        let tool = DelegateSubagentTool::new(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "plan it", "agent_type": "plan" }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let child_id = value["child_thread_id"].as_str().expect("child id");

        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        // No caller model → definition's Fixed policy applies.
        assert_eq!(child_config.model, "plan-model");
    }
}
