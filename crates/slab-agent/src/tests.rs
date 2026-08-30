//! P6 smoke test: agent runs the echo tool and completes.
//!
//! This test exercises the entire agent loop in isolation, using a mock
//! [`LlmPort`] instead of a real model.  The mock:
//! 1. First call → returns a tool call to `echo` with `message = "hello"`.
//! 2. Second call (after the tool result is appended) → returns a plain-text
//!    final answer so the loop terminates.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{
    AgentControl, AgentControlLimits, AgentDefinition, AgentError, AgentHook, AgentThreadContext,
    HookEvent, HookOutcome, ModelPolicy, PlanRef, ToolCallRender, ToolConstraint, ToolContext,
    ToolHandler, ToolOutput, ToolRouter, ToolVisibility, WorkspaceRef,
    compact::{CompactContext, CompactOutcome, CompactPort, SlidingWindowCompactPort},
    config::{AgentConfig, AgentToolChoice},
    port::{
        AgentNotifyPort, AgentStorePort, ApprovalDecision, ApprovalPort, LlmPort, LlmResponse,
        LlmUsage, ParsedToolCall, ThreadMessageRecord, ThreadSnapshot, ThreadStatus, ToolSpec,
        TurnStateRecord,
    },
    protocol::{EventMsg, TurnItem},
    risk::ToolRiskAnalyzer,
};
use async_trait::async_trait;
use slab_agent_tracing::{AgentTraceContext, AgentTraceEvent, AgentTraceSink};
use slab_types::{
    ConversationMessage, ConversationMessageContent, ConversationToolCall, ConversationToolFunction,
};

struct TestEchoTool;

#[async_trait]
impl ToolHandler for TestEchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echo the provided message back verbatim."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The text to echo back."
                }
            },
            "required": ["message"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        let message = arguments.get("message").and_then(serde_json::Value::as_str).unwrap_or("");
        Ok(ToolOutput { content: message.to_owned(), metadata: None })
    }
}

struct CountingEchoTool {
    executions: Arc<Mutex<u32>>,
}

#[async_trait]
impl ToolHandler for CountingEchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echo and count executions."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" }
            },
            "required": ["message"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        *self.executions.lock().unwrap() += 1;
        let message = arguments.get("message").and_then(serde_json::Value::as_str).unwrap_or("");
        Ok(ToolOutput { content: message.to_owned(), metadata: None })
    }
}

struct CapturingContextTool {
    workspaces: Arc<Mutex<Vec<Option<WorkspaceRef>>>>,
    plans: Arc<Mutex<Vec<Option<PlanRef>>>>,
}

#[async_trait]
impl ToolHandler for CapturingContextTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Capture tool context and echo the provided message."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" }
            },
            "required": ["message"]
        })
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        self.workspaces.lock().unwrap().push(ctx.workspace.clone());
        self.plans.lock().unwrap().push(ctx.plan.clone());
        let message = arguments.get("message").and_then(serde_json::Value::as_str).unwrap_or("");
        Ok(ToolOutput { content: message.to_owned(), metadata: None })
    }
}

struct ApprovalEchoTool;

#[async_trait]
impl ToolHandler for ApprovalEchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echo with approval."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" }
            },
            "required": ["message"]
        })
    }

    fn describe_operation(
        &self,
        arguments: &serde_json::Value,
    ) -> Option<crate::OperationDescriptor> {
        let message = arguments.get("message").and_then(serde_json::Value::as_str).unwrap_or("");
        Some(crate::OperationDescriptor::shell(message))
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        let message = arguments.get("message").and_then(serde_json::Value::as_str).unwrap_or("");
        Ok(ToolOutput { content: format!("approved: {message}"), metadata: None })
    }
}

// ── Mock LLM ─────────────────────────────────────────────────────────────────

struct SecretTool {
    executions: Arc<Mutex<u32>>,
}

#[async_trait]
impl ToolHandler for SecretTool {
    fn name(&self) -> &str {
        "secret"
    }

    fn description(&self) -> &str {
        "A tool that must not run unless explicitly allowed."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object" })
    }

    fn describe_operation(
        &self,
        _arguments: &serde_json::Value,
    ) -> Option<crate::OperationDescriptor> {
        // Classify as a shell-like operation so the exec-policy engine gates it
        // (a tool with no category would default to read-only and be allowed).
        Some(crate::OperationDescriptor::shell("secret"))
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        _arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        *self.executions.lock().unwrap() += 1;
        Ok(ToolOutput { content: "secret executed".to_owned(), metadata: None })
    }
}

struct DelayEchoTool;

#[async_trait]
impl ToolHandler for DelayEchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echo after an optional delay."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" },
                "delay_ms": { "type": "integer" }
            },
            "required": ["message"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        let delay_ms = arguments.get("delay_ms").and_then(serde_json::Value::as_u64).unwrap_or(0);
        if delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
        let message = arguments.get("message").and_then(serde_json::Value::as_str).unwrap_or("");
        Ok(ToolOutput { content: message.to_owned(), metadata: None })
    }
}

struct MockLlm {
    call_count: Mutex<u32>,
}

impl MockLlm {
    fn new() -> Self {
        Self { call_count: Mutex::new(0) }
    }
}

#[async_trait]
impl LlmPort for MockLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;

        if *count == 1 {
            // First turn: request an echo tool call.
            Ok(LlmResponse {
                content: None,
                content_already_streamed: false,
                tool_calls: vec![ParsedToolCall {
                    id: "call-1".into(),
                    name: "echo".into(),
                    arguments: r#"{"message":"hello from agent"}"#.into(),
                }],
                finish_reason: Some("tool_calls".into()),
                usage: None,
            })
        } else {
            // Second turn: final text answer after receiving the tool result.
            Ok(LlmResponse {
                content: Some("echo completed: hello from agent".into()),
                content_already_streamed: false,
                tool_calls: vec![],
                finish_reason: Some("stop".into()),
                usage: None,
            })
        }
    }
}

struct InvalidToolArgsLlm {
    call_count: Mutex<u32>,
}

impl InvalidToolArgsLlm {
    fn new() -> Self {
        Self { call_count: Mutex::new(0) }
    }
}

#[async_trait]
impl LlmPort for InvalidToolArgsLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;

        if *count == 1 {
            Ok(LlmResponse {
                content: None,
                content_already_streamed: false,
                tool_calls: vec![ParsedToolCall {
                    id: "call-invalid".into(),
                    name: "echo".into(),
                    arguments: "{not json".into(),
                }],
                finish_reason: Some("tool_calls".into()),
                usage: None,
            })
        } else {
            Ok(LlmResponse {
                content: Some("handled invalid tool args".into()),
                content_already_streamed: false,
                tool_calls: vec![],
                finish_reason: Some("stop".into()),
                usage: None,
            })
        }
    }
}

// ── Mock store ────────────────────────────────────────────────────────────────

struct SecretToolCallLlm {
    call_count: Mutex<u32>,
}

impl SecretToolCallLlm {
    fn new() -> Self {
        Self { call_count: Mutex::new(0) }
    }
}

#[async_trait]
impl LlmPort for SecretToolCallLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        if *count == 1 {
            Ok(LlmResponse {
                content: None,
                content_already_streamed: false,
                tool_calls: vec![ParsedToolCall {
                    id: "call-secret".into(),
                    name: "secret".into(),
                    arguments: "{}".into(),
                }],
                finish_reason: Some("tool_calls".into()),
                usage: None,
            })
        } else {
            Ok(LlmResponse {
                content: Some("secret was blocked".into()),
                content_already_streamed: false,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".into()),
                usage: None,
            })
        }
    }
}

struct RepeatingInvalidToolLlm;

#[async_trait]
impl LlmPort for RepeatingInvalidToolLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            content: None,
            content_already_streamed: false,
            tool_calls: vec![ParsedToolCall {
                id: "call-missing".into(),
                name: "missing".into(),
                arguments: "{}".into(),
            }],
            finish_reason: Some("tool_calls".into()),
            usage: None,
        })
    }
}

struct RepeatingToolCallLlm {
    tool_name: &'static str,
    arguments: &'static str,
    final_after_calls: Option<u32>,
    call_count: Mutex<u32>,
}

impl RepeatingToolCallLlm {
    fn new(tool_name: &'static str, arguments: &'static str) -> Self {
        Self { tool_name, arguments, final_after_calls: None, call_count: Mutex::new(0) }
    }
}

#[async_trait]
impl LlmPort for RepeatingToolCallLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let call_index = {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            *count
        };
        if self.final_after_calls.is_some_and(|final_after| call_index > final_after) {
            return Ok(LlmResponse {
                content: Some("continued after soft stop".into()),
                content_already_streamed: false,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".into()),
                usage: None,
            });
        }

        Ok(LlmResponse {
            content: None,
            content_already_streamed: false,
            tool_calls: vec![ParsedToolCall {
                id: format!("call-{call_index}"),
                name: self.tool_name.to_owned(),
                arguments: self.arguments.to_owned(),
            }],
            finish_reason: Some("tool_calls".into()),
            usage: None,
        })
    }
}

struct BudgetedToolCallLlm;

#[async_trait]
impl LlmPort for BudgetedToolCallLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            content: None,
            content_already_streamed: false,
            tool_calls: vec![ParsedToolCall {
                id: "call-budgeted".into(),
                name: "echo".into(),
                arguments: r#"{"message":"budget"}"#.into(),
            }],
            finish_reason: Some("tool_calls".into()),
            usage: Some(LlmUsage {
                prompt_tokens: 3,
                completion_tokens: 4,
                total_tokens: 7,
                estimated: false,
            }),
        })
    }
}

struct JsonNoopTool {
    name: &'static str,
}

#[async_trait]
impl ToolHandler for JsonNoopTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        "No-op JSON tool for agent loop tests."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object" })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        _arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        Ok(ToolOutput { content: "{}".to_owned(), metadata: None })
    }
}

struct CapturingToolsLlm {
    calls: Arc<Mutex<Vec<Vec<String>>>>,
}

#[async_trait]
impl LlmPort for CapturingToolsLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let call_index = {
            let mut calls = self.calls.lock().unwrap();
            calls.push(tools.iter().map(|tool| tool.name.clone()).collect());
            calls.len()
        };
        if call_index == 1 {
            Ok(LlmResponse {
                content: None,
                content_already_streamed: false,
                tool_calls: vec![ParsedToolCall {
                    id: "call-echo".into(),
                    name: "echo".into(),
                    arguments: r#"{"message":"done"}"#.into(),
                }],
                finish_reason: Some("tool_calls".into()),
                usage: None,
            })
        } else {
            Ok(LlmResponse {
                content: Some("done".into()),
                content_already_streamed: false,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".into()),
                usage: None,
            })
        }
    }
}

// A Deferred read-only tool the model can only reach via `tool_search`.
struct DeferredSearchableTool;

#[async_trait]
impl ToolHandler for DeferredSearchableTool {
    fn name(&self) -> &str {
        "deferred_read_tool"
    }
    fn description(&self) -> &str {
        "A deferred read-only tool used for tool_search tests."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "properties": {}})
    }
    fn visibility(&self) -> ToolVisibility {
        ToolVisibility::Deferred
    }
    async fn execute(
        &self,
        _ctx: &ToolContext,
        _arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        Ok(ToolOutput { content: "deferred ok".to_owned(), metadata: None })
    }
}

// A `tool_search` placeholder registered so the model can see/call it; its
// execution is intercepted by the dispatch layer, so `execute` is never reached.
struct ToolSearchStubTool;

#[async_trait]
impl ToolHandler for ToolSearchStubTool {
    fn name(&self) -> &str {
        "tool_search"
    }
    fn description(&self) -> &str {
        "Discover deferred tools."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]})
    }
    async fn execute(
        &self,
        _ctx: &ToolContext,
        _arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        Ok(ToolOutput { content: "{}".to_owned(), metadata: None })
    }
}

// LLM that records the visible tool names per call. Call 1 emits a `tool_search`
// tool call with the given query; call 2 returns a plain final answer.
struct ToolSearchLlm {
    query: String,
    calls: Arc<Mutex<Vec<Vec<String>>>>,
}

#[async_trait]
impl LlmPort for ToolSearchLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let mut calls = self.calls.lock().unwrap();
        calls.push(tools.iter().map(|t| t.name.clone()).collect());
        let call_index = calls.len();
        drop(calls);
        if call_index == 1 {
            Ok(LlmResponse {
                content: None,
                content_already_streamed: false,
                tool_calls: vec![ParsedToolCall {
                    id: "search-1".into(),
                    name: "tool_search".into(),
                    arguments: format!(r#"{{"query":"{}"}}"#, self.query),
                }],
                finish_reason: Some("tool_calls".into()),
                usage: None,
            })
        } else {
            Ok(LlmResponse {
                content: Some("done".into()),
                content_already_streamed: false,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".into()),
                usage: None,
            })
        }
    }
}

struct TwoToolCallsLlm {
    call_count: Mutex<u32>,
}
impl TwoToolCallsLlm {
    fn new() -> Self {
        Self { call_count: Mutex::new(0) }
    }
}

#[async_trait]
impl LlmPort for TwoToolCallsLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        if *count == 1 {
            Ok(LlmResponse {
                content: None,
                content_already_streamed: false,
                tool_calls: vec![
                    ParsedToolCall {
                        id: "call-slow".into(),
                        name: "echo".into(),
                        arguments: r#"{"message":"slow","delay_ms":50}"#.into(),
                    },
                    ParsedToolCall {
                        id: "call-fast".into(),
                        name: "echo".into(),
                        arguments: r#"{"message":"fast","delay_ms":0}"#.into(),
                    },
                ],
                finish_reason: Some("tool_calls".into()),
                usage: None,
            })
        } else {
            Ok(LlmResponse {
                content: Some("done".into()),
                content_already_streamed: false,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".into()),
                usage: None,
            })
        }
    }
}

struct CapturingMessagesLlm {
    calls: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    first_call_uses_tool: bool,
}

#[async_trait]
impl LlmPort for CapturingMessagesLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let call_index = {
            let mut calls = self.calls.lock().unwrap();
            calls.push(messages.to_vec());
            calls.len()
        };
        if self.first_call_uses_tool && call_index == 1 {
            return Ok(LlmResponse {
                content: None,
                content_already_streamed: false,
                tool_calls: vec![ParsedToolCall {
                    id: "call-1".into(),
                    name: "echo".into(),
                    arguments: r#"{"message":"ok"}"#.into(),
                }],
                finish_reason: Some("tool_calls".into()),
                usage: None,
            });
        }
        Ok(LlmResponse {
            content: Some("done".into()),
            content_already_streamed: false,
            tool_calls: Vec::new(),
            finish_reason: Some("stop".into()),
            usage: None,
        })
    }
}

struct NoopStore;

#[async_trait]
impl AgentStorePort for NoopStore {
    async fn upsert_thread(&self, _snapshot: &ThreadSnapshot) -> Result<(), AgentError> {
        Ok(())
    }

    async fn get_thread(&self, _id: &str) -> Result<Option<ThreadSnapshot>, AgentError> {
        Ok(None)
    }

    async fn list_session_threads(
        &self,
        _session_id: &str,
    ) -> Result<Vec<ThreadSnapshot>, AgentError> {
        Ok(Vec::new())
    }

    async fn update_thread_status(
        &self,
        _id: &str,
        _status: ThreadStatus,
        _completion_text: Option<&str>,
    ) -> Result<(), AgentError> {
        Ok(())
    }
}

#[derive(Default)]
struct RecordingStore {}

#[async_trait]
impl AgentStorePort for RecordingStore {
    async fn upsert_thread(&self, _snapshot: &ThreadSnapshot) -> Result<(), AgentError> {
        Ok(())
    }

    async fn get_thread(&self, _id: &str) -> Result<Option<ThreadSnapshot>, AgentError> {
        Ok(None)
    }

    async fn list_session_threads(
        &self,
        _session_id: &str,
    ) -> Result<Vec<ThreadSnapshot>, AgentError> {
        Ok(Vec::new())
    }

    async fn update_thread_status(
        &self,
        _id: &str,
        _status: ThreadStatus,
        _completion_text: Option<&str>,
    ) -> Result<(), AgentError> {
        Ok(())
    }
}

#[derive(Default)]
struct RecordingPersistingStore {
    snapshots: Mutex<HashMap<String, ThreadSnapshot>>,
    // The slab-agent `insert_thread_message` trait method is gone;
    // retained for direct-push seeding in tests. Unread — tests verify emission
    // via `RecordingNotify`.
    #[allow(dead_code)]
    messages: Mutex<Vec<ThreadMessageRecord>>,
}

#[async_trait]
impl AgentStorePort for RecordingPersistingStore {
    async fn upsert_thread(&self, snapshot: &ThreadSnapshot) -> Result<(), AgentError> {
        self.snapshots.lock().unwrap().insert(snapshot.id.clone(), snapshot.clone());
        Ok(())
    }

    async fn get_thread(&self, id: &str) -> Result<Option<ThreadSnapshot>, AgentError> {
        Ok(self.snapshots.lock().unwrap().get(id).cloned())
    }

    async fn list_session_threads(
        &self,
        session_id: &str,
    ) -> Result<Vec<ThreadSnapshot>, AgentError> {
        Ok(self
            .snapshots
            .lock()
            .unwrap()
            .values()
            .filter(|snapshot| snapshot.session_id == session_id && snapshot.parent_id.is_none())
            .cloned()
            .collect())
    }

    async fn update_thread_status(
        &self,
        id: &str,
        status: ThreadStatus,
        completion_text: Option<&str>,
    ) -> Result<(), AgentError> {
        if let Some(snapshot) = self.snapshots.lock().unwrap().get_mut(id) {
            snapshot.status = status;
            snapshot.completion_text = completion_text.map(str::to_owned);
        }
        Ok(())
    }
}

#[derive(Default)]
struct PersistingStore {
    snapshots: Mutex<HashMap<String, ThreadSnapshot>>,
    messages: Mutex<Vec<ThreadMessageRecord>>,
    turn_states: Mutex<Vec<TurnStateRecord>>,
}

#[async_trait]
impl AgentStorePort for PersistingStore {
    async fn upsert_thread(&self, snapshot: &ThreadSnapshot) -> Result<(), AgentError> {
        self.snapshots.lock().unwrap().insert(snapshot.id.clone(), snapshot.clone());
        Ok(())
    }

    async fn get_thread(&self, id: &str) -> Result<Option<ThreadSnapshot>, AgentError> {
        Ok(self.snapshots.lock().unwrap().get(id).cloned())
    }

    async fn list_session_threads(
        &self,
        session_id: &str,
    ) -> Result<Vec<ThreadSnapshot>, AgentError> {
        Ok(self
            .snapshots
            .lock()
            .unwrap()
            .values()
            .filter(|snapshot| snapshot.session_id == session_id && snapshot.parent_id.is_none())
            .cloned()
            .collect())
    }

    async fn update_thread_status(
        &self,
        id: &str,
        status: ThreadStatus,
        completion_text: Option<&str>,
    ) -> Result<(), AgentError> {
        if let Some(snapshot) = self.snapshots.lock().unwrap().get_mut(id) {
            snapshot.status = status;
            snapshot.completion_text = completion_text.map(str::to_owned);
        }
        Ok(())
    }
}

// ── Mock notify ───────────────────────────────────────────────────────────────

struct NoopNotify;

#[async_trait]
impl AgentNotifyPort for NoopNotify {
    async fn on_status_change(&self, _thread_id: &str, _status: ThreadStatus) {}
}

/// A notify port that records every `EventMsg` slab-agent emits, so
/// tests can verify emission (slab-agent no longer writes conversation data to
/// the store — it emits `MessageAppended` / `TurnStateChanged` events).
/// Implements `ApprovalPort` (rejecting, matching `NoopNotify`) so it can stand
/// in for `NoopNotify` in tests that wire both ports from one Arc.
#[derive(Default)]
struct RecordingNotify {
    events: Mutex<Vec<EventMsg>>,
}

#[async_trait]
impl AgentNotifyPort for RecordingNotify {
    async fn on_status_change(&self, _thread_id: &str, _status: ThreadStatus) {}

    async fn on_event_msg(&self, _thread_id: &str, msg: &EventMsg) {
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
        _descriptor: &crate::OperationDescriptor,
        _risk: Option<crate::ToolRiskAssessment>,
    ) -> ApprovalDecision {
        ApprovalDecision::Rejected
    }
}

impl RecordingNotify {
    /// Collect emitted `MessageAppended` conversation messages for a thread, in
    /// emission order. Replaces the old `store.messages` read for tests that
    /// verified slab-agent persisted messages to the store.
    fn emitted_messages(&self, thread_id: &str) -> Vec<ConversationMessage> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match event {
                EventMsg::MessageAppended(p) if p.thread_id == thread_id => Some(p.message.clone()),
                _ => None,
            })
            .collect()
    }
}

/// Poll for an emitted `MessageAppended` whose rendered text contains
/// `text` (replaces `wait_for_persisted_message`, which polled the store).
async fn wait_for_emitted_message(notify: &RecordingNotify, thread_id: &str, text: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let found = notify
                .emitted_messages(thread_id)
                .iter()
                .any(|message| message.rendered_text().contains(text));
            if found {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("emitted message did not appear");
}

#[async_trait]
impl ApprovalPort for NoopNotify {
    async fn request_approval(
        &self,
        _thread_id: &str,
        _call_id: &str,
        _tool_name: &str,
        _descriptor: &crate::OperationDescriptor,
        _risk: Option<crate::ToolRiskAssessment>,
    ) -> ApprovalDecision {
        ApprovalDecision::Approved(crate::ApprovalScope::RunOnce)
    }
}

struct RejectingApproval;

#[async_trait]
impl ApprovalPort for RejectingApproval {
    async fn request_approval(
        &self,
        _thread_id: &str,
        _call_id: &str,
        _tool_name: &str,
        _descriptor: &crate::OperationDescriptor,
        _risk: Option<crate::ToolRiskAssessment>,
    ) -> ApprovalDecision {
        ApprovalDecision::Rejected
    }
}

/// Test exec-policy that requires approval for every non-read-only operation,
/// mirroring the legacy `ApprovalEchoTool` behavior so approval-flow tests
/// still observe a `Pending` → `Running`/`Failed` transition.
struct AskAllExecPolicy;

#[async_trait]
impl crate::ExecPolicyPort for AskAllExecPolicy {
    async fn evaluate(
        &self,
        _thread_id: &str,
        descriptor: &crate::OperationDescriptor,
    ) -> crate::ExecDecision {
        match descriptor.category {
            crate::OperationCategory::ReadOnly => crate::ExecDecision::Allow,
            _ => crate::ExecDecision::RequireApproval,
        }
    }
    async fn remember(
        &self,
        _thread_id: &str,
        _descriptor: &crate::OperationDescriptor,
        _scope: crate::ApprovalScope,
    ) {
    }
    async fn set_thread_mode(&self, _thread_id: &str, _mode: crate::PermissionMode) {}
    async fn clear_thread(&self, _thread_id: &str) {}
    fn permission_state_for(&self, _thread_id: &str) -> crate::PermissionStateSnapshot {
        // Full exposure keeps the tool list unfiltered in approval-flow tests.
        crate::PermissionStateSnapshot {
            mode: crate::PermissionMode::FullControl,
            baseline: crate::PermissionBaseline::FullAccess,
            exposure: crate::ToolExposure::all(),
        }
    }
}

/// Test exec-policy that refuses every non-read-only operation. Proves the
/// kernel returns "blocked by policy" WITHOUT requesting approval (the
/// approve-then-block bug).
struct DenyAllExecPolicy;

#[async_trait]
impl crate::ExecPolicyPort for DenyAllExecPolicy {
    async fn evaluate(
        &self,
        _thread_id: &str,
        descriptor: &crate::OperationDescriptor,
    ) -> crate::ExecDecision {
        match descriptor.category {
            crate::OperationCategory::ReadOnly => crate::ExecDecision::Allow,
            _ => crate::ExecDecision::Deny,
        }
    }
    async fn remember(
        &self,
        _thread_id: &str,
        _descriptor: &crate::OperationDescriptor,
        _scope: crate::ApprovalScope,
    ) {
    }
    async fn set_thread_mode(&self, _thread_id: &str, _mode: crate::PermissionMode) {}
    async fn clear_thread(&self, _thread_id: &str) {}
    fn permission_state_for(&self, _thread_id: &str) -> crate::PermissionStateSnapshot {
        // Full exposure keeps the tool list unfiltered in denial-flow tests.
        crate::PermissionStateSnapshot {
            mode: crate::PermissionMode::FullControl,
            baseline: crate::PermissionBaseline::FullAccess,
            exposure: crate::ToolExposure::all(),
        }
    }
}

/// ApprovalPort that counts how many times `request_approval` was called, so a
/// test can assert the kernel did NOT prompt for a hard-denied operation.
struct CountingApproval {
    calls: Arc<std::sync::atomic::AtomicU32>,
}

impl CountingApproval {
    fn new() -> Self {
        Self { calls: Arc::new(std::sync::atomic::AtomicU32::new(0)) }
    }
    fn calls(&self) -> u32 {
        self.calls.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[async_trait]
impl ApprovalPort for CountingApproval {
    async fn request_approval(
        &self,
        _thread_id: &str,
        _call_id: &str,
        _tool_name: &str,
        _descriptor: &crate::OperationDescriptor,
        _risk: Option<crate::ToolRiskAssessment>,
    ) -> ApprovalDecision {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        ApprovalDecision::Approved(crate::ApprovalScope::RunOnce)
    }
}

struct BlockingHook;

#[async_trait]
impl AgentHook for BlockingHook {
    async fn on_event(&self, event: &HookEvent) -> HookOutcome {
        match event {
            HookEvent::OnToolStart { .. } => {
                HookOutcome::Block { reason: "blocked by test hook".into() }
            }
            HookEvent::OnAgentStart { .. }
            | HookEvent::OnLlmStart { .. }
            | HookEvent::OnLlmEnd { .. }
            | HookEvent::OnToolEnd { .. }
            | HookEvent::OnAgentEnd { .. } => HookOutcome::Continue,
        }
    }
}

struct LifecycleInjectionHook;

#[async_trait]
impl AgentHook for LifecycleInjectionHook {
    async fn on_event(&self, event: &HookEvent) -> HookOutcome {
        match event {
            HookEvent::OnAgentStart { .. } => {
                HookOutcome::AppendObservation { observation: "agent started".into() }
            }
            HookEvent::OnLlmStart { .. } => HookOutcome::inject_message(ConversationMessage {
                role: "developer".into(),
                content: ConversationMessageContent::Text("llm start".into()),
                name: Some("test_hook".into()),
                tool_call_id: None,
                tool_calls: Vec::new(),
            }),
            HookEvent::OnLlmEnd { .. } => HookOutcome::inject_message(ConversationMessage {
                role: "developer".into(),
                content: ConversationMessageContent::Text("llm end".into()),
                name: Some("test_hook".into()),
                tool_call_id: None,
                tool_calls: Vec::new(),
            }),
            HookEvent::OnToolStart { .. }
            | HookEvent::OnToolEnd { .. }
            | HookEvent::OnAgentEnd { .. } => HookOutcome::Continue,
        }
    }
}

struct ToolObservationHook;

#[async_trait]
impl AgentHook for ToolObservationHook {
    async fn on_event(&self, event: &HookEvent) -> HookOutcome {
        match event {
            HookEvent::OnToolStart { .. } => {
                HookOutcome::AppendObservation { observation: "tool args checked".into() }
            }
            HookEvent::OnAgentStart { .. }
            | HookEvent::OnLlmStart { .. }
            | HookEvent::OnLlmEnd { .. }
            | HookEvent::OnToolEnd { .. }
            | HookEvent::OnAgentEnd { .. } => HookOutcome::Continue,
        }
    }
}

struct HighRiskToolAnalyzer;

#[async_trait]
impl ToolRiskAnalyzer for HighRiskToolAnalyzer {
    async fn analyze(
        &self,
        tool_name: &str,
        _arguments: &serde_json::Value,
    ) -> crate::ToolRiskAssessment {
        crate::ToolRiskAssessment {
            level: crate::ToolRiskLevel::High,
            labels: vec![tool_name.to_owned()],
            reason: Some("test high risk".to_owned()),
        }
    }
}

#[derive(Default)]
struct RecordingTraceSink {
    events: Mutex<Vec<(AgentTraceContext, AgentTraceEvent)>>,
}

impl AgentTraceSink for RecordingTraceSink {
    fn record(&self, context: &AgentTraceContext, event: AgentTraceEvent) {
        self.events.lock().unwrap().push((context.clone(), event));
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

async fn wait_for_persisted_status(
    store: &PersistingStore,
    thread_id: &str,
    expected: ThreadStatus,
) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let status =
                store.snapshots.lock().unwrap().get(thread_id).map(|snapshot| snapshot.status);
            if status == Some(expected) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("persisted status did not reach expected value");
}

/// Test helper: resume a thread with a new user message, mirroring the
/// hoisted `AgentCore::send_input` flow (read history from the mock + sort +
/// max-turn + append user + `resume_thread`). slab-agent no longer reads
/// conversation data itself (the trait method is gone), so tests that previously
/// called `control.send_input(thread_id, content)` build the resume payload here
/// from the `PersistingStore` mock's in-memory messages.
async fn resume_with_input(
    store: &PersistingStore,
    control: &AgentControl,
    thread_id: &str,
    content: String,
) -> Result<(), AgentError> {
    let mut records: Vec<ThreadMessageRecord> = store
        .messages
        .lock()
        .unwrap()
        .iter()
        .filter(|m| m.thread_id == thread_id)
        .cloned()
        .collect();
    records.sort_by(|a, b| {
        a.turn_index
            .cmp(&b.turn_index)
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    let starting_turn_index =
        records.iter().map(|r| r.turn_index).max().map_or(0, |index| index + 1);
    let mut messages: Vec<ConversationMessage> =
        records.into_iter().map(|record| record.message).collect();
    messages.push(ConversationMessage {
        role: "user".into(),
        content: ConversationMessageContent::Text(content),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    });
    // One trailing message is new (the user input) — a count, not an index.
    control.resume_thread(thread_id, messages, starting_turn_index, Some(1)).await
}

#[tokio::test]
async fn wait_for_terminal_snapshot_polls_persisted_status_when_thread_is_not_active() {
    let store = Arc::new(PersistingStore::default());
    let now = "2026-01-01T00:00:00Z".to_owned();
    store.snapshots.lock().unwrap().insert(
        "orphan-running".to_owned(),
        ThreadSnapshot {
            id: "orphan-running".to_owned(),
            session_id: "session".to_owned(),
            parent_id: None,
            depth: 0,
            status: ThreadStatus::Running,
            role_name: None,
            config_json: serde_json::to_string(&AgentConfig::default()).expect("config"),
            completion_text: None,
            created_at: now.clone(),
            updated_at: now,
            archived_at: None,
        },
    );

    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let control = AgentControl::new(
        Arc::new(MockLlm::new()),
        store_port,
        notify.clone(),
        notify,
        Arc::new(ToolRouter::new()),
        8,
        4,
    );
    let store_for_update = Arc::clone(&store);
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        store_for_update
            .update_thread_status("orphan-running", ThreadStatus::Completed, Some("done"))
            .await
            .expect("update thread");
    });

    let snapshot = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        control.wait_for_terminal_snapshot("orphan-running"),
    )
    .await
    .expect("terminal snapshot timeout")
    .expect("terminal snapshot");

    assert_eq!(snapshot.status, ThreadStatus::Completed);
    assert_eq!(snapshot.completion_text.as_deref(), Some("done"));
}

#[tokio::test]
async fn smoke_echo_tool_agent_completes() {
    // Wire up the agent control with the echo tool registered.
    let llm = Arc::new(MockLlm::new());
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let notify = Arc::new(NoopNotify);

    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new(llm, store, notify, approval, Arc::new(router), 8, 4));

    // Spawn a root agent with a single user message.
    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text(
            "Please echo 'hello from agent'".into(),
        ),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];

    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };

    let thread_id =
        control.spawn("session-1".into(), config, messages).await.expect("spawn should succeed");

    // Subscribe to status before the thread finishes.
    let mut status_rx = control.subscribe(&thread_id).await.expect("subscribe should succeed");

    // Wait for the thread to reach a terminal state.
    let final_status = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            status_rx.changed().await.expect("status channel closed");
            let status = *status_rx.borrow();
            if matches!(
                status,
                ThreadStatus::Completed
                    | ThreadStatus::Errored
                    | ThreadStatus::Shutdown
                    | ThreadStatus::Interrupted
            ) {
                return status;
            }
        }
    })
    .await
    .expect("agent did not complete within timeout");

    assert_eq!(
        final_status,
        ThreadStatus::Completed,
        "agent should complete successfully, got {final_status:?}"
    );

    // By now the thread has been removed from the registry; verify the count.
    assert_eq!(control.active_thread_count().await, 0);
}

/// Spawn an agent whose router has a Deferred tool + a `tool_search` stub, drive
/// it with a [`ToolSearchLlm`] that calls `tool_search` with `query` on turn 1,
/// and return the per-turn captured visible-tool-name lists.
async fn run_tool_search_agent(query: &str) -> Vec<Vec<String>> {
    let calls: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let llm = Arc::new(ToolSearchLlm { query: query.to_owned(), calls: Arc::clone(&calls) });
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(DeferredSearchableTool));
    router.register(Box::new(ToolSearchStubTool));
    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new(llm, store, notify, approval, Arc::new(router), 8, 4));

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: ConversationMessageContent::Text("search for tools".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };
    let thread_id = control.spawn("session-search".into(), config, messages).await.expect("spawn");
    let mut status_rx = control.subscribe(&thread_id).await.expect("subscribe");
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            status_rx.changed().await.expect("status channel closed");
            if matches!(
                *status_rx.borrow(),
                ThreadStatus::Completed
                    | ThreadStatus::Errored
                    | ThreadStatus::Shutdown
                    | ThreadStatus::Interrupted
            ) {
                return;
            }
        }
    })
    .await;

    calls.lock().unwrap().clone()
}

#[tokio::test]
async fn tool_search_discovers_and_injects_deferred_tool() {
    let calls = run_tool_search_agent("deferred").await;
    assert!(calls.len() >= 2, "expected at least 2 LLM calls, got {calls:?}");
    // Turn 1 (before search): the Deferred tool is hidden; tool_search is visible.
    assert!(
        !calls[0].iter().any(|n| n == "deferred_read_tool"),
        "deferred tool should be hidden before tool_search, got {:?}",
        calls[0]
    );
    assert!(calls[0].iter().any(|n| n == "tool_search"), "tool_search should be visible");
    // Turn 2 (after search): the Deferred tool has been injected and is visible.
    assert!(
        calls[1].iter().any(|n| n == "deferred_read_tool"),
        "deferred tool should be visible after tool_search injected it, got {:?}",
        calls[1]
    );
}

#[tokio::test]
async fn tool_search_no_match_returns_empty_and_does_not_inject() {
    let calls = run_tool_search_agent("zzz_no_match").await;
    assert!(calls.len() >= 2, "expected at least 2 LLM calls, got {calls:?}");
    // A non-matching query injects nothing: the Deferred tool stays hidden.
    assert!(
        !calls[1].iter().any(|n| n == "deferred_read_tool"),
        "deferred tool should stay hidden after a non-matching search, got {:?}",
        calls[1]
    );
}

// ── present_plan approval gate (Plan → Default mode flip) ────────────────────

/// In-memory plan store for tests (mirrors the app-core impl).
#[derive(Default)]
struct InMemoryPlanStore {
    plan: Mutex<Option<crate::Plan>>,
}

#[async_trait]
impl crate::PlanStorePort for InMemoryPlanStore {
    async fn replace_plan(&self, _thread_id: &str, plan: crate::Plan) -> Result<(), AgentError> {
        *self.plan.lock().unwrap() = Some(plan);
        Ok(())
    }
    async fn current_plan(&self, _thread_id: &str) -> Option<crate::Plan> {
        self.plan.lock().unwrap().clone()
    }
    async fn clear(&self, _thread_id: &str) {
        *self.plan.lock().unwrap() = None;
    }
}

/// Full-exposure exec-policy stub: allows everything and reports full tool
/// exposure. Read-only enforcement in plan mode now comes from the plan agent's
/// tool denylist (`filter_tools_for_agent` via `agent_type`), not from the
/// exec-policy exposure, so the present_plan approval tests no longer need a
/// mode-tracking policy.
struct FullExposureExecPolicy;

#[async_trait]
impl crate::ExecPolicyPort for FullExposureExecPolicy {
    async fn evaluate(&self, _: &str, _: &crate::OperationDescriptor) -> crate::ExecDecision {
        crate::ExecDecision::Allow
    }
    async fn remember(&self, _: &str, _: &crate::OperationDescriptor, _: crate::ApprovalScope) {}
    async fn set_thread_mode(&self, _: &str, _: crate::PermissionMode) {}
    async fn clear_thread(&self, _: &str) {}
    fn permission_state_for(&self, _: &str) -> crate::PermissionStateSnapshot {
        crate::PermissionStateSnapshot {
            mode: crate::PermissionMode::FullControl,
            baseline: crate::PermissionBaseline::FullAccess,
            exposure: crate::ToolExposure::all(),
        }
    }
}

/// Test registry exposing a single `plan` agent whose denylist hides the test
/// mutation tool — mirroring how the built-in plan agent's denylist hides
/// mutation tools when a turn runs with `agent_type = "plan"`.
struct PlanAgentRegistry;

impl crate::AgentRegistry for PlanAgentRegistry {
    fn get(&self, agent_type: &str) -> Option<AgentDefinition> {
        (agent_type == "plan").then_some(AgentDefinition {
            agent_type: "plan".into(),
            description: "test plan agent".into(),
            tools: ToolConstraint::Denylist(vec!["mutate_tool".into()]),
            system_prompt: "test plan prompt".into(),
            model: ModelPolicy::Inherit,
        })
    }
    fn list(&self) -> Vec<AgentDefinition> {
        vec![self.get("plan").expect("plan agent registered")]
    }
}

struct ApprovingApproval;
#[async_trait]
impl ApprovalPort for ApprovingApproval {
    async fn request_approval(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &crate::OperationDescriptor,
        _: Option<crate::ToolRiskAssessment>,
    ) -> ApprovalDecision {
        ApprovalDecision::Approved(crate::ApprovalScope::RunOnce)
    }
}

/// A mutation (FileEdit) tool used only to assert progressive exposure.
struct MutatingTestTool;
#[async_trait]
impl ToolHandler for MutatingTestTool {
    fn name(&self) -> &str {
        "mutate_tool"
    }
    fn description(&self) -> &str {
        "A mutating tool used to assert visibility."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{}})
    }
    fn category(&self) -> crate::OperationCategory {
        crate::OperationCategory::FileEdit
    }
    async fn execute(
        &self,
        _: &ToolContext,
        _: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        Ok(ToolOutput { content: "mutated".into(), metadata: None })
    }
}

/// Test-only `plan` tool (slab-agent tests cannot depend on slab-agent-tools).
struct PlanStubTool;
#[async_trait]
impl ToolHandler for PlanStubTool {
    fn name(&self) -> &str {
        "plan"
    }
    fn description(&self) -> &str {
        "Create a plan (test stub)."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{"items":{"type":"array"}}})
    }
    async fn execute(
        &self,
        ctx: &ToolContext,
        _: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        let plan = crate::Plan {
            plan_id: "plan-test".into(),
            summary: Some("test plan".into()),
            items: vec![crate::PlanItem {
                step: "do thing".into(),
                status: crate::PlanStatus::InProgress,
                depends_on: None,
                result_ref: None,
            }],
            counts: crate::PlanCounts { pending: 0, in_progress: 1, completed: 0, blocked: 0 },
            current_step: Some(0),
        };
        ctx.plan_store
            .replace_plan(&ctx.thread_id, plan.clone())
            .await
            .map_err(|e| AgentError::ToolExecution(e.to_string()))?;
        Ok(ToolOutput { content: format!("plan created: {}", plan.summary_line()), metadata: None })
    }
}

/// Test-only `present_plan` tool. The turn loop detects its name and drives the
/// approval gate; this stub just surfaces the stored plan as content.
struct PresentPlanStubTool;
#[async_trait]
impl ToolHandler for PresentPlanStubTool {
    fn name(&self) -> &str {
        "present_plan"
    }
    fn description(&self) -> &str {
        "Present the plan (test stub)."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{}})
    }
    async fn execute(
        &self,
        ctx: &ToolContext,
        _: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        let plan = ctx
            .plan_store
            .current_plan(&ctx.thread_id)
            .await
            .ok_or_else(|| AgentError::ToolExecution("no plan".into()))?;
        Ok(ToolOutput {
            content: format!("presenting plan for approval: {}", plan.summary_line()),
            metadata: None,
        })
    }
}

struct PlanPresentLlm {
    calls: Arc<Mutex<Vec<Vec<String>>>>,
}

#[async_trait]
impl LlmPort for PlanPresentLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let mut calls = self.calls.lock().unwrap();
        calls.push(tools.iter().map(|t| t.name.clone()).collect());
        let idx = calls.len();
        drop(calls);
        let tc = |id: &str, name: &str, args: &str| ParsedToolCall {
            id: id.into(),
            name: name.into(),
            arguments: args.into(),
        };
        Ok(match idx {
            1 => LlmResponse {
                content: None,
                content_already_streamed: false,
                tool_calls: vec![tc(
                    "p1",
                    "plan",
                    r#"{"items":[{"step":"do","status":"in_progress"}]}"#,
                )],
                finish_reason: Some("tool_calls".into()),
                usage: None,
            },
            2 => LlmResponse {
                content: None,
                content_already_streamed: false,
                tool_calls: vec![tc("pp", "present_plan", "{}")],
                finish_reason: Some("tool_calls".into()),
                usage: None,
            },
            _ => LlmResponse {
                content: Some("done".into()),
                content_already_streamed: false,
                tool_calls: vec![],
                finish_reason: Some("stop".into()),
                usage: None,
            },
        })
    }
}

/// Drive a plan-agent turn through `plan` → `present_plan` → final, capturing
/// the per-turn visible tool lists. Runs with `agent_type = "plan"` against a
/// test registry whose plan denylist hides `mutate_tool`, so the capture shows
/// progressive tool exposure driven by the plan agent constraint.
async fn run_present_plan_agent(approval: Arc<dyn ApprovalPort>) -> Arc<Mutex<Vec<Vec<String>>>> {
    let tool_calls_capture: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let llm = Arc::new(PlanPresentLlm { calls: Arc::clone(&tool_calls_capture) });
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(MutatingTestTool));
    router.register(Box::new(PlanStubTool));
    router.register(Box::new(PresentPlanStubTool));
    let exec_policy: Arc<FullExposureExecPolicy> = Arc::new(FullExposureExecPolicy);
    let plan_store: Arc<dyn crate::PlanStorePort> = Arc::new(InMemoryPlanStore::default());
    let control = Arc::new(
        AgentControl::new(llm, store, notify, approval, Arc::new(router), 8, 4)
            .with_exec_policy(exec_policy)
            .with_plan_store(plan_store)
            .with_agent_registry(Arc::new(PlanAgentRegistry)),
    );
    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: ConversationMessageContent::Text("plan then present".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig {
        model: "mock".into(),
        max_turns: 6,
        agent_type: Some("plan".into()),
        ..AgentConfig::default()
    };
    let thread_id = control.spawn("session-plan".into(), config, messages).await.expect("spawn");
    let mut status_rx = control.subscribe(&thread_id).await.expect("subscribe");
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            status_rx.changed().await.expect("status channel closed");
            if matches!(
                *status_rx.borrow(),
                ThreadStatus::Completed
                    | ThreadStatus::Errored
                    | ThreadStatus::Shutdown
                    | ThreadStatus::Interrupted
            ) {
                return;
            }
        }
    })
    .await;
    tool_calls_capture
}

#[tokio::test]
async fn present_plan_approval_runs_as_plan_agent_and_hides_mutation() {
    let capture = run_present_plan_agent(Arc::new(ApprovingApproval)).await;
    let calls = capture.lock().unwrap().clone();
    assert!(calls.len() >= 2, "expected >=2 LLM turns, got {:?}", calls);

    // The plan agent's denylist hides the mutation tool for the whole turn;
    // plan + present_plan remain visible so the agent can author and submit.
    assert!(
        !calls[0].iter().any(|n| n == "mutate_tool"),
        "mutation tool should be hidden under the plan agent, got {:?}",
        calls[0]
    );
    assert!(calls[0].iter().any(|n| n == "plan"), "plan tool visible");
    assert!(calls[0].iter().any(|n| n == "present_plan"), "present_plan tool visible");
}

#[tokio::test]
async fn present_plan_rejected_keeps_mutation_hidden() {
    let capture = run_present_plan_agent(Arc::new(RejectingApproval)).await;
    let calls = capture.lock().unwrap().clone();
    // Rejection does not change the agent_type, so the plan agent denylist keeps
    // the mutation tool hidden on every turn.
    for (i, turn) in calls.iter().enumerate() {
        assert!(
            !turn.iter().any(|n| n == "mutate_tool"),
            "turn {i}: mutation tool should stay hidden under the plan agent, got {:?}",
            turn
        );
    }
}

#[tokio::test]
async fn trace_sink_records_prompt_llm_tool_and_turn_events() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));
    let trace = Arc::new(RecordingTraceSink::default());
    let trace_sink: Arc<dyn AgentTraceSink> = trace.clone();

    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new_with_hooks_and_tracing(
        llm,
        store_port,
        notify,
        approval,
        Arc::new(router),
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        Vec::new(),
        trace_sink,
        None,
    ));

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("Please echo".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig {
        model: "mock".into(),
        max_turns: 5,
        system_prompt: Some("trace system prompt".into()),
        ..AgentConfig::default()
    };

    let thread_id = control.spawn("trace-session".into(), config, messages).await.expect("spawn");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    let events = trace.events.lock().unwrap().clone();
    assert!(
        events.iter().all(|(context, _event)| context.session_id == "trace-session"),
        "{events:#?}"
    );
    assert_trace_event(&events, "system_prompt_injected");
    assert_trace_event(&events, "agent_llm_request");
    assert_trace_event(&events, "llm_response_normalized");
    assert_trace_event(&events, "tool_call_detected");
    assert_trace_event(&events, "tool_call_arguments_parsed");
    assert_trace_event(&events, "tool_call_output");
    assert_trace_event(&events, "turn_completed");
    assert_trace_event(&events, "thread_completed");
    assert!(events.iter().any(|(context, _event)| context.turn_index == Some(0)));

    let system_prompt = events
        .iter()
        .find(|(_context, event)| event.event == "system_prompt_injected")
        .expect("system prompt event");
    assert_eq!(system_prompt.1.payload["system_prompt"], "trace system prompt");

    let tool_output = events
        .iter()
        .find(|(_context, event)| event.event == "tool_call_output")
        .expect("tool output event");
    assert_eq!(tool_output.1.payload["tool_name"], "echo");
    assert_eq!(tool_output.1.payload["output"], "hello from agent");
}

fn assert_trace_event(events: &[(AgentTraceContext, AgentTraceEvent)], event_name: &str) {
    assert!(
        events.iter().any(|(_context, event)| event.event == event_name),
        "missing trace event {event_name}; events: {events:#?}"
    );
}

// ── F3: real-path integration — slab-agent → BundleAgentTraceSink ───────────
//
// Every bundle_sink unit test hand-builds an AgentTraceContext and stuffs
// `root_thread_id` into it. If `thread.rs` ever stopped stamping
// `root_thread_id` on the trace context, ALL of those sink tests would stay
// green while production stopped writing bundles (classic false-green). This
// test drives the REAL production path: AgentControl → AgentThread::run →
// record_json → BundleAgentTraceSink → bundle on disk. It pins the wiring.

#[tokio::test]
async fn bundle_sink_receives_events_from_real_agent_control_path() {
    use slab_agent_tracing::{
        AGENT_TRACE_DIR_NAME, BundleAgentTraceSink, MANIFEST_FILE, TRACE_FILE,
        bundle_dir_for_root_thread,
    };

    let trace_root = tempfile::tempdir().expect("trace temp dir");
    let trace_dir = trace_root.path().to_path_buf();

    // Real bundle sink + real trace_dir flowing into AgentControl (mirrors
    // slab-app-core bootstrap when agent.debug is on).
    let trace_sink: Arc<dyn AgentTraceSink> = BundleAgentTraceSink::shared(trace_dir.clone());

    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    // Register echo so MockLlm's first-call tool request resolves (build before
    // wrapping in Arc — ToolRouter is registered through &self before the run).
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));
    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new_with_hooks_and_tracing(
        llm,
        store_port,
        notify,
        approval,
        Arc::new(router),
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        Vec::new(),
        trace_sink,
        Some(trace_dir.clone()),
    ));

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: ConversationMessageContent::Text("Please echo".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };

    let thread_id = control.spawn("bundle-session".into(), config, messages).await.expect("spawn");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    // The bundle the sink MUST have written into (deterministic path).
    let bundle_dir = bundle_dir_for_root_thread(&trace_dir, &thread_id);
    assert!(bundle_dir.is_dir(), "bundle materialized: {}", bundle_dir.display());
    assert!(bundle_dir.join(MANIFEST_FILE).is_file(), "manifest.json written");
    assert!(bundle_dir.join(TRACE_FILE).is_file(), "trace.jsonl written");

    // Parse trace.jsonl; the manifest points at trace_id == root_thread_id.
    let manifest: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(bundle_dir.join(MANIFEST_FILE)).expect("read manifest").trim(),
    )
    .expect("manifest json");
    assert_eq!(manifest["root_thread_id"], thread_id, "manifest stamped with root thread id");

    let trace_text =
        std::fs::read_to_string(bundle_dir.join(TRACE_FILE)).expect("read trace.jsonl");
    let lines: Vec<&str> = trace_text.trim().lines().collect();
    assert!(!lines.is_empty(), "trace.jsonl has events");

    // The full turn lifecycle landed: turn_started → agent_llm_request →
    // llm_response_normalized → tool lifecycle → turn_completed. Pins the
    // slab-agent→sink path against a future bypass.
    assert!(trace_text.contains("\"kind\":\"turn_started\""), "turn_started: {trace_text}");
    assert!(
        trace_text.contains("\"kind\":\"inference_started\""),
        "agent_llm_request bridged to InferenceStarted: {trace_text}",
    );
    assert!(
        trace_text.contains("\"kind\":\"inference_completed\""),
        "llm_response_normalized bridged to InferenceCompleted: {trace_text}",
    );
    assert!(trace_text.contains("\"kind\":\"turn_completed\""), "turn_completed: {trace_text}",);

    // Every event stamped with the root thread id (root thread owns this bundle).
    for line in &lines {
        let event: serde_json::Value = serde_json::from_str(line).expect("parse trace line");
        assert_eq!(event["thread_id"], thread_id, "event stamped with root thread id: {line}");
    }

    // The bundle lives under <trace_dir>/agent_trace/ (the deterministic root).
    assert!(bundle_dir.starts_with(trace_dir.join(AGENT_TRACE_DIR_NAME)));
}

// ── F2: depth>=2 spawn chain — true root propagates to grandchild bundle ─────
//
// A naive "child root = parent_id" stamp groups a depth-2 grandchild under its
// depth-1 parent, producing an orphan bundle unreachable from the rollout
// (build_session_meta stamps trace_path ONLY on the true root). This test
// spawns a 3-layer chain (root → child → grandchild) through the REAL
// AgentControl spawn path and asserts all three thread_ids land in the SAME
// (root) bundle.

/// LLM that immediately returns a final text answer (no tool calls) so each
/// spawned thread completes in a single turn.
struct FinalAnswerLlm;

#[async_trait]
impl LlmPort for FinalAnswerLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            content: Some("done".into()),
            content_already_streamed: false,
            tool_calls: vec![],
            finish_reason: Some("stop".into()),
            usage: None,
        })
    }
}

#[tokio::test]
async fn depth_two_grandchild_groups_under_root_bundle() {
    use slab_agent_tracing::{BundleAgentTraceSink, TRACE_FILE, bundle_dir_for_root_thread};

    let trace_root = tempfile::tempdir().expect("trace temp dir");
    let trace_dir = trace_root.path().to_path_buf();
    let trace_sink: Arc<dyn AgentTraceSink> = BundleAgentTraceSink::shared(trace_dir.clone());

    let llm = Arc::new(FinalAnswerLlm);
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new_with_hooks_and_tracing(
        llm,
        store_port,
        notify,
        approval,
        Arc::new(ToolRouter::new()),
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        Vec::new(),
        trace_sink,
        Some(trace_dir.clone()),
    ));

    let mk = || ConversationMessage {
        role: "user".into(),
        content: ConversationMessageContent::Text("go".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    };
    let config = AgentConfig { model: "mock".into(), max_turns: 3, ..AgentConfig::default() };

    // depth 0 — root.
    let root_id = control.spawn("depth-session".into(), config.clone(), vec![mk()]).await.unwrap();
    wait_for_persisted_status(&store, &root_id, ThreadStatus::Completed).await;

    // depth 1 — child whose root is root_id.
    let child_id = control
        .spawn_child("depth-session".into(), root_id.clone(), 1, config.clone(), vec![mk()])
        .await
        .unwrap();
    wait_for_persisted_status(&store, &child_id, ThreadStatus::Completed).await;

    // depth 2 — grandchild whose nearest parent is child_id but whose ROOT is root_id.
    let grandchild_id = control
        .spawn_child("depth-session".into(), child_id.clone(), 2, config.clone(), vec![mk()])
        .await
        .unwrap();
    wait_for_persisted_status(&store, &grandchild_id, ThreadStatus::Completed).await;

    // All three thread_ids must appear in the SAME root bundle. If the
    // nearest-ancestor bug regressed, the grandchild would write into a
    // trace-<child>-<child> orphan dir instead.
    let root_bundle = bundle_dir_for_root_thread(&trace_dir, &root_id);
    let trace_text = std::fs::read_to_string(root_bundle.join(TRACE_FILE))
        .expect("read root bundle trace.jsonl");
    assert!(trace_text.contains(&format!("\"thread_id\":\"{root_id}\"")), "root in root bundle");
    assert!(trace_text.contains(&format!("\"thread_id\":\"{child_id}\"")), "child in root bundle");
    assert!(
        trace_text.contains(&format!("\"thread_id\":\"{grandchild_id}\"")),
        "grandchild in ROOT bundle (not orphaned under child): {trace_text}",
    );

    // No orphan bundle directory exists for the child or grandchild as a root.
    let child_as_root = bundle_dir_for_root_thread(&trace_dir, &child_id);
    assert!(
        !child_as_root.join(TRACE_FILE).exists(),
        "no orphan child bundle: a depth-1 child is NOT its own root",
    );
    let grandchild_as_root = bundle_dir_for_root_thread(&trace_dir, &grandchild_id);
    assert!(
        !grandchild_as_root.join(TRACE_FILE).exists(),
        "no orphan grandchild bundle: depth-2 groups under the true root",
    );
}

#[tokio::test]
async fn resolve_root_thread_id_walks_three_layer_parent_chain() {
    // Focused unit test for the chain walker (the heart of the F2 fix), with a
    // hand-built 3-layer parent chain in the in-memory store.
    use crate::thread::resolve_root_thread_id;

    let store = Arc::new(PersistingStore::default());
    let now = chrono::Utc::now().to_rfc3339();
    let snap = |id: &str, parent: Option<&str>, depth: u32| ThreadSnapshot {
        id: id.into(),
        session_id: "s".into(),
        parent_id: parent.map(str::to_owned),
        depth,
        status: ThreadStatus::Completed,
        role_name: None,
        config_json: "{}".into(),
        completion_text: None,
        created_at: now.clone(),
        updated_at: now.clone(),
        archived_at: None,
    };
    store.upsert_thread(&snap("root", None, 0)).await.unwrap();
    store.upsert_thread(&snap("child", Some("root"), 1)).await.unwrap();
    store.upsert_thread(&snap("grandchild", Some("child"), 2)).await.unwrap();

    // Walking from the grandchild's parent (child) reaches root in 2 hops.
    let resolved = resolve_root_thread_id(&*store, "child").await;
    assert_eq!(resolved.as_deref(), Some("root"), "grandchild resolves to the true root");

    // depth-1 child's parent is the root → one hop.
    let resolved = resolve_root_thread_id(&*store, "root").await;
    assert_eq!(resolved.as_deref(), Some("root"), "child resolves to root");

    // A missing parent snapshot falls back to None (caller uses nearest ancestor).
    let resolved = resolve_root_thread_id(&*store, "nonexistent").await;
    assert_eq!(resolved, None, "missing parent chain → None fallback");
}

#[tokio::test]
async fn lifecycle_hooks_inject_start_observations_and_llm_messages_in_order() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let llm = Arc::new(CapturingMessagesLlm { calls: calls.clone(), first_call_uses_tool: true });
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new_with_hooks(
        llm,
        store_port,
        notify,
        approval,
        Arc::new(router),
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        vec![Arc::new(LifecycleInjectionHook)],
    ));
    let thread_id = control
        .spawn(
            "session-hooks".into(),
            AgentConfig { model: "mock".into(), max_turns: 3, ..AgentConfig::default() },
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("use tool".into()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
        )
        .await
        .expect("spawn");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    let first = message_texts(&calls[0]);
    let start_observation = first
        .iter()
        .position(|text| text.contains("Local hook observation:\nagent started"))
        .expect("start observation");
    let llm_start = first.iter().position(|text| text == "llm start").expect("llm start");
    let user = first.iter().position(|text| text == "use tool").expect("user");
    assert!(start_observation < llm_start);
    assert!(llm_start < user);

    let second = message_texts(&calls[1]);
    let llm_end = second.iter().position(|text| text == "llm end").expect("llm end");
    let user = second.iter().position(|text| text == "use tool").expect("user");
    assert!(llm_end < user);
}

#[tokio::test]
async fn tool_start_observations_are_appended_to_tool_output() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&Arc::new(NoopNotify));
    let control = Arc::new(AgentControl::new_with_hooks(
        llm,
        store_port,
        notify.clone(),
        approval,
        Arc::new(router),
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        vec![Arc::new(ToolObservationHook)],
    ));
    let thread_id = control
        .spawn(
            "session-tool-hook".into(),
            AgentConfig { model: "mock".into(), max_turns: 3, ..AgentConfig::default() },
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("use tool".into()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
        )
        .await
        .expect("spawn");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    // slab-agent emits `MessageAppended` (no store writes).
    let tool_output = notify
        .emitted_messages(&thread_id)
        .iter()
        .find(|message| message.role == "tool")
        .expect("tool message")
        .rendered_text();
    assert!(tool_output.contains("hello from agent"));
    assert!(tool_output.contains("Hook observations:"));
    assert!(tool_output.contains("tool args checked"));
}

#[tokio::test]
async fn turn_state_records_running_llm_tool_and_completed_statuses() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&Arc::new(NoopNotify));
    let control = Arc::new(AgentControl::new(
        llm,
        store_port,
        notify.clone(),
        approval,
        Arc::new(router),
        8,
        4,
    ));
    let thread_id = control
        .spawn(
            "session-turn-state".into(),
            AgentConfig { model: "mock".into(), max_turns: 3, ..AgentConfig::default() },
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("use tool".into()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
        )
        .await
        .expect("spawn");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    // slab-agent emits `TurnStateChanged` events (no longer writes
    // turn-state records to the store). Assert the emitted events carry the
    // typed phase progression; the app-core observer lands them in rollout.
    let statuses = notify
        .events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            EventMsg::TurnStateChanged(p) => Some(p.status.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    // Entry anchor + tool phase + terminal for the tool-calling iteration,
    // then the entry + terminal of the final iteration. The intermediate
    // `llm_completed` / `tool_calls_completed` full snapshots are gone
    // (replay only needs the terminal line per iteration).
    assert!(statuses.contains(&"sampling".to_owned()));
    assert!(statuses.contains(&"executing_tools".to_owned()));
    assert!(statuses.contains(&"completed".to_owned()));
    assert!(!statuses.contains(&"running".to_owned()));
    assert!(!statuses.contains(&"llm_completed".to_owned()));
    assert!(!statuses.contains(&"tool_calls_completed".to_owned()));
    // The terminal event must reference the LAST iteration, not the run's
    // starting index (regression for the wrong-turn-id defect).
    let final_completed = notify
        .events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            EventMsg::TurnCompleted(p) => Some((p.turn.id.clone(), p.reason.clone())),
            _ => None,
        })
        .next_back();
    assert_eq!(final_completed, Some(("1".to_owned(), Some("completed".to_owned()))));
}

fn message_texts(messages: &[ConversationMessage]) -> Vec<String> {
    messages.iter().map(ConversationMessage::rendered_text).collect()
}

async fn wait_for_control_terminal_status(control: &AgentControl, thread_id: &str) -> ThreadStatus {
    let mut status_rx = control.subscribe(thread_id).await.expect("subscribe");
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            status_rx.changed().await.expect("status channel closed");
            let status = *status_rx.borrow();
            if matches!(
                status,
                ThreadStatus::Completed
                    | ThreadStatus::Errored
                    | ThreadStatus::Interrupted
                    | ThreadStatus::Shutdown
            ) {
                break status;
            }
        }
    })
    .await
    .expect("thread should finish")
}

#[tokio::test]
async fn tool_choice_specific_filters_tools_sent_to_llm() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let llm = Arc::new(CapturingToolsLlm { calls: calls.clone() });
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));
    router.register(Box::new(SecretTool { executions: Arc::new(Mutex::new(0)) }));

    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4));
    let config = AgentConfig {
        model: "mock".into(),
        max_turns: 1,
        tool_choice: AgentToolChoice::Tool { name: "echo".into() },
        ..AgentConfig::default()
    };
    let thread_id = control
        .spawn(
            "session-tool-choice".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("finish".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Interrupted).await;
    let calls = calls.lock().unwrap().clone();
    assert!(!calls.is_empty());
    assert!(calls.iter().all(|tools| tools == &vec!["echo".to_owned()]), "{calls:#?}");
}

#[tokio::test]
async fn offline_mode_drops_external_tools_from_llm_tool_list() {
    // INFRA-07: with thread_context.offline = true, the tool list sent to the
    // LLM must drop external tools (web_search / mcp_call / mcp__*) while
    // keeping local tools (echo). This is the offline-degradation key node.
    let calls = Arc::new(Mutex::new(Vec::new()));
    let llm = Arc::new(CapturingToolsLlm { calls: calls.clone() });
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));
    router.register(Box::new(JsonNoopTool { name: "web_search" }));
    router.register(Box::new(JsonNoopTool { name: "mcp_call" }));
    router.register(Box::new(JsonNoopTool { name: "mcp__weather__forecast" }));

    let approval = Arc::clone(&notify);
    let control = Arc::new(
        AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4)
            .with_thread_context(AgentThreadContext::new().with_offline(true)),
    );
    let config = AgentConfig { model: "mock".into(), max_turns: 1, ..AgentConfig::default() };
    let thread_id = control
        .spawn(
            "session-offline".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("finish".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Interrupted).await;
    let calls = calls.lock().unwrap().clone();
    assert!(!calls.is_empty(), "LLM should have been called at least once");
    for tools in &calls {
        assert!(tools.contains(&"echo".to_owned()), "local tool `echo` must remain: {tools:?}");
        assert!(!tools.contains(&"web_search".to_owned()), "web_search must be dropped: {tools:?}");
        assert!(!tools.contains(&"mcp_call".to_owned()), "mcp_call must be dropped: {tools:?}");
        assert!(
            !tools.iter().any(|name| name.starts_with("mcp__")),
            "mcp__* must be dropped: {tools:?}"
        );
    }
}

/// Test tool with a configurable exposure category (defaults to `ReadOnly`).
struct CategorizedTool {
    name: &'static str,
    category: crate::OperationCategory,
}

#[async_trait]
impl ToolHandler for CategorizedTool {
    fn name(&self) -> &str {
        self.name
    }
    fn description(&self) -> &str {
        "Categorized no-op tool for progressive-exposure tests."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object" })
    }
    fn category(&self) -> crate::OperationCategory {
        self.category
    }
    async fn execute(
        &self,
        _ctx: &ToolContext,
        _arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        Ok(ToolOutput { content: "{}".to_owned(), metadata: None })
    }
}

/// Test exec-policy that reports a read-only exposure (mirrors a read-only
/// permission mode) so the tool-list filter hides mutating categories.
struct ReadOnlyExposurePolicy;

#[async_trait]
impl crate::ExecPolicyPort for ReadOnlyExposurePolicy {
    async fn evaluate(
        &self,
        _thread_id: &str,
        descriptor: &crate::OperationDescriptor,
    ) -> crate::ExecDecision {
        match descriptor.category {
            crate::OperationCategory::ReadOnly => crate::ExecDecision::Allow,
            _ => crate::ExecDecision::Deny,
        }
    }
    async fn remember(
        &self,
        _thread_id: &str,
        _descriptor: &crate::OperationDescriptor,
        _scope: crate::ApprovalScope,
    ) {
    }
    async fn set_thread_mode(&self, _thread_id: &str, _mode: crate::PermissionMode) {}
    async fn clear_thread(&self, _thread_id: &str) {}
    fn permission_state_for(&self, _thread_id: &str) -> crate::PermissionStateSnapshot {
        crate::PermissionStateSnapshot {
            mode: crate::PermissionMode::Custom,
            baseline: crate::PermissionBaseline::ReadOnly,
            exposure: crate::ToolExposure::read_only(),
        }
    }
}

#[tokio::test]
async fn read_only_permission_mode_drops_mutating_tools_from_llm_tool_list() {
    // Progressive tool exposure: a read-only permission state hides tools whose
    // category isn't ReadOnly (shell / file_edit / network), while read-only
    // tools (echo) stay visible. Composes with the offline filter the same way.
    let calls = Arc::new(Mutex::new(Vec::new()));
    let llm = Arc::new(CapturingToolsLlm { calls: calls.clone() });
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));
    router.register(Box::new(CategorizedTool {
        name: "shell",
        category: crate::OperationCategory::Shell,
    }));
    router.register(Box::new(CategorizedTool {
        name: "write_file",
        category: crate::OperationCategory::FileEdit,
    }));
    router.register(Box::new(CategorizedTool {
        name: "web_search",
        category: crate::OperationCategory::Network,
    }));

    let approval = Arc::clone(&notify);
    let control = Arc::new(
        AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4)
            .with_exec_policy(Arc::new(ReadOnlyExposurePolicy)),
    );
    let config = AgentConfig { model: "mock".into(), max_turns: 1, ..AgentConfig::default() };
    let thread_id = control
        .spawn(
            "session-readonly".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("finish".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Interrupted).await;
    let calls = calls.lock().unwrap().clone();
    assert!(!calls.is_empty(), "LLM should have been called at least once");
    for tools in &calls {
        assert!(tools.contains(&"echo".to_owned()), "read-only tool `echo` must remain: {tools:?}");
        assert!(
            !tools.contains(&"shell".to_owned()),
            "shell must be hidden in read-only mode: {tools:?}"
        );
        assert!(
            !tools.contains(&"write_file".to_owned()),
            "write_file must be hidden in read-only mode: {tools:?}"
        );
        assert!(
            !tools.contains(&"web_search".to_owned()),
            "web_search must be hidden in read-only mode: {tools:?}"
        );
    }
}

#[tokio::test]
async fn disallowed_registered_tool_is_not_executed_and_feedback_is_persisted() {
    let executions = Arc::new(Mutex::new(0));
    let llm = Arc::new(SecretToolCallLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));
    router.register(Box::new(SecretTool { executions: executions.clone() }));

    let approval = Arc::clone(&Arc::new(NoopNotify));
    let control = Arc::new(AgentControl::new(
        llm,
        store_port,
        notify.clone(),
        approval,
        Arc::new(router),
        8,
        4,
    ));
    let config = AgentConfig {
        model: "mock".into(),
        max_turns: 5,
        allowed_tools: vec!["echo".into()],
        ..AgentConfig::default()
    };
    let thread_id = control
        .spawn(
            "session-disallowed-tool".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("try secret".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;
    assert_eq!(*executions.lock().unwrap(), 0);
    wait_for_emitted_message(&notify, &thread_id, "invalid tool call: tool not allowed: secret")
        .await;
}

#[tokio::test]
async fn invalid_tool_call_retry_budget_errors_thread() {
    let llm = Arc::new(RepeatingInvalidToolLlm);
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4));
    let config = AgentConfig {
        model: "mock".into(),
        max_turns: 5,
        invalid_tool_call_retries: 0,
        ..AgentConfig::default()
    };
    let thread_id = control
        .spawn(
            "session-invalid-budget".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("call missing".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Errored).await;
    let snapshot = store.snapshots.lock().unwrap().get(&thread_id).cloned().expect("snapshot");
    assert!(
        snapshot
            .completion_text
            .as_deref()
            .is_some_and(|text| text.contains("invalid tool call retry budget exceeded")),
        "{snapshot:#?}"
    );
}

#[tokio::test]
async fn concurrent_tool_calls_preserve_persisted_message_order() {
    let llm = Arc::new(TwoToolCallsLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let router = ToolRouter::new();
    router.register(Box::new(DelayEchoTool));

    let approval = Arc::clone(&Arc::new(NoopNotify));
    let control = Arc::new(AgentControl::new(
        llm,
        store_port,
        notify.clone(),
        approval,
        Arc::new(router),
        8,
        4,
    ));
    let config = AgentConfig {
        model: "mock".into(),
        max_turns: 5,
        tool_concurrency: 2,
        ..AgentConfig::default()
    };
    let thread_id = control
        .spawn(
            "session-tool-concurrency".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("call twice".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;
    // slab-agent emits `MessageAppended` (no store writes); read the
    // emitted tool messages and assert FIFO order is preserved.
    let tool_outputs = notify
        .emitted_messages(&thread_id)
        .iter()
        .filter(|message| message.role == "tool")
        .map(|message| match &message.content {
            ConversationMessageContent::Text(text) => text.clone(),
            ConversationMessageContent::Parts(_) => message.rendered_text(),
        })
        .collect::<Vec<_>>();
    assert_eq!(tool_outputs, vec!["slow".to_owned(), "fast".to_owned()]);
}

#[tokio::test]
async fn sliding_window_compaction_drops_leading_orphan_tool_result() {
    let compact = SlidingWindowCompactPort::new(10, 8);
    let messages = vec![
        ConversationMessage {
            role: "system".into(),
            content: ConversationMessageContent::Text("system".into()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        },
        ConversationMessage {
            role: "assistant".into(),
            content: ConversationMessageContent::Text("x".repeat(200)),
            name: None,
            tool_call_id: None,
            tool_calls: vec![ConversationToolCall {
                id: Some("call-old".into()),
                r#type: "function".into(),
                function: ConversationToolFunction { name: "echo".into(), arguments: "{}".into() },
            }],
        },
        ConversationMessage {
            role: "tool".into(),
            content: ConversationMessageContent::Text("old tool result".into()),
            name: None,
            tool_call_id: Some("call-old".into()),
            tool_calls: vec![],
        },
        ConversationMessage {
            role: "user".into(),
            content: ConversationMessageContent::Text("next".into()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        },
    ];

    let outcome = compact
        .compact(
            &messages,
            &CompactContext {
                model_id: "test",
                summary_instructions: None,
                force: false,
                progress: None,
                memory_pressure_hint: None,
            },
        )
        .await
        .expect("compact");
    let crate::CompactOutcome::Replaced { messages, .. } = outcome else {
        panic!("expected replaced outcome");
    };
    assert_eq!(messages.first().map(|message| message.role.as_str()), Some("system"));
    assert!(messages.get(1).is_some_and(|message| message.role != "tool"));
    assert!(messages.iter().any(|message| message.rendered_text() == "next"));
}

#[test]
fn tool_router_supports_runtime_unregister() {
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));
    assert!(router.get("echo").is_some());
    assert!(router.unregister("echo").is_some());
    assert!(router.get("echo").is_none());
}

#[tokio::test]
async fn denied_tool_does_not_request_approval_and_is_blocked() {
    // Reproduces the approve-then-block bug's fix: when the exec-policy denies
    // an operation, the kernel must NOT request approval and must return a
    // blocked result. Previously a shell tool under `Block` policy would still
    // prompt (risk fallback) then fail with "blocked by policy".
    let llm = Arc::new(MockLlm::new());
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let approval = Arc::new(CountingApproval::new());
    let router = ToolRouter::new();
    router.register(Box::new(ApprovalEchoTool));

    let approval_port: Arc<dyn ApprovalPort> = approval.clone();
    let control = Arc::new(
        AgentControl::new(llm, store, Arc::new(NoopNotify), approval_port, Arc::new(router), 8, 4)
            .with_exec_policy(Arc::new(DenyAllExecPolicy)),
    );

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: ConversationMessageContent::Text("Please echo".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };
    let thread_id = control.spawn("session-deny".into(), config, messages).await.expect("spawn");
    let final_status = wait_for_control_terminal_status(&control, &thread_id).await;

    assert_eq!(final_status, ThreadStatus::Completed);
    // The operation was denied — approval must NOT have been requested.
    assert_eq!(approval.calls(), 0, "denied tool must not prompt for approval");
}

#[tokio::test]
async fn approved_tool_runs_after_prompting_exec_policy() {
    // Counterpart: when the exec-policy requires approval and the host approves
    // (RunOnce), the tool executes and the turn completes.
    let llm = Arc::new(MockLlm::new());
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let approval = Arc::new(CountingApproval::new());
    let router = ToolRouter::new();
    router.register(Box::new(ApprovalEchoTool));

    let approval_port: Arc<dyn ApprovalPort> = approval.clone();
    let control = Arc::new(
        AgentControl::new(llm, store, Arc::new(NoopNotify), approval_port, Arc::new(router), 8, 4)
            .with_exec_policy(Arc::new(AskAllExecPolicy)),
    );

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: ConversationMessageContent::Text("Please echo".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };
    let thread_id = control.spawn("session-approve".into(), config, messages).await.expect("spawn");
    let final_status = wait_for_control_terminal_status(&control, &thread_id).await;

    assert_eq!(final_status, ThreadStatus::Completed);
    assert_eq!(approval.calls(), 1, "approved tool must prompt exactly once");
}

#[tokio::test]
async fn approval_required_tool_is_recorded_pending_then_completed() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(RecordingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);

    let router = ToolRouter::new();
    router.register(Box::new(ApprovalEchoTool));

    let approval = Arc::clone(&notify);
    let control = Arc::new(
        AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4)
            .with_exec_policy(Arc::new(AskAllExecPolicy)),
    );

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("Please echo".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };

    let thread_id =
        control.spawn("session-approval".into(), config, messages).await.expect("spawn");
    let mut status_rx = control.subscribe(&thread_id).await.expect("subscribe");
    let final_status = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            status_rx.changed().await.expect("status channel closed");
            let status = *status_rx.borrow();
            if matches!(status, ThreadStatus::Completed | ThreadStatus::Errored) {
                break status;
            }
        }
    })
    .await
    .expect("thread should finish");

    assert_eq!(final_status, ThreadStatus::Completed);
}

/// Records every `EventMsg` AND auto-approves approval requests, for tests
/// that assert on the full approval → execution → completion event sequence.
#[derive(Default)]
struct ApprovingRecordingNotify {
    events: Mutex<Vec<EventMsg>>,
}

#[async_trait]
impl AgentNotifyPort for ApprovingRecordingNotify {
    async fn on_status_change(&self, _thread_id: &str, _status: ThreadStatus) {}

    async fn on_event_msg(&self, _thread_id: &str, msg: &EventMsg) {
        self.events.lock().unwrap().push(msg.clone());
    }
}

#[async_trait]
impl ApprovalPort for ApprovingRecordingNotify {
    async fn request_approval(
        &self,
        _thread_id: &str,
        _call_id: &str,
        _tool_name: &str,
        _descriptor: &crate::OperationDescriptor,
        _risk: Option<crate::ToolRiskAssessment>,
    ) -> ApprovalDecision {
        ApprovalDecision::Approved(crate::ApprovalScope::RunOnce)
    }
}

/// One tool call on the first LLM turn, a plain final answer on the second —
/// the `MockLlm` shape but with a configurable tool name/arguments.
struct OneShotToolLlm {
    call_count: Mutex<u32>,
    tool_name: String,
    arguments: String,
}

impl OneShotToolLlm {
    fn new(tool_name: &str, arguments: &str) -> Self {
        Self {
            call_count: Mutex::new(0),
            tool_name: tool_name.to_owned(),
            arguments: arguments.to_owned(),
        }
    }
}

#[async_trait]
impl LlmPort for OneShotToolLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        if *count == 1 {
            Ok(LlmResponse {
                content: None,
                content_already_streamed: false,
                tool_calls: vec![ParsedToolCall {
                    id: "call-1".into(),
                    name: self.tool_name.clone(),
                    arguments: self.arguments.clone(),
                }],
                finish_reason: Some("tool_calls".into()),
                usage: None,
            })
        } else {
            Ok(LlmResponse {
                content: Some("done".into()),
                content_already_streamed: false,
                tool_calls: vec![],
                finish_reason: Some("stop".into()),
                usage: None,
            })
        }
    }
}

/// The approval notification must carry the SAME id as the item lifecycle
/// events for the call (the provider `tool_call.id`), so the client can merge
/// the approval banner onto the in-stream tool card and route its resolve
/// back. Regression lock for the live "Running + Completed split card" bug.
#[tokio::test]
async fn approval_notification_shares_item_id_with_lifecycle_events() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(RecordingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(ApprovingRecordingNotify::default());

    let router = ToolRouter::new();
    router.register(Box::new(ApprovalEchoTool));

    let approval = Arc::clone(&notify) as Arc<dyn ApprovalPort>;
    let control = Arc::new(
        AgentControl::new(llm, store_port, notify.clone(), approval, Arc::new(router), 8, 4)
            .with_exec_policy(Arc::new(AskAllExecPolicy)),
    );

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: ConversationMessageContent::Text("Please echo".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };
    let thread_id =
        control.spawn("session-approval-id".into(), config, messages).await.expect("spawn");
    let final_status = wait_for_control_terminal_status(&control, &thread_id).await;
    assert_eq!(final_status, ThreadStatus::Completed);

    let events = notify.events.lock().unwrap().clone();
    let started_ids: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            EventMsg::ItemStarted(p) if p.thread_id == thread_id => match &p.item {
                TurnItem::ToolCall { id, .. } => Some(id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(
        started_ids,
        vec!["call-1".to_owned()],
        "ItemStarted uses the provider tool_call.id"
    );

    let approval_ids: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            EventMsg::CommandExecutionRequestApproval(p) if p.thread_id == thread_id => {
                Some(p.item_id.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        approval_ids,
        vec!["call-1".to_owned()],
        "approval notification must correlate on the same id as the item events"
    );

    let completed_ids: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            EventMsg::ItemCompleted(p) if p.thread_id == thread_id => match &p.item {
                TurnItem::ToolCall { id, .. } => Some(id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(completed_ids, vec!["call-1".to_owned()]);
}

/// A write-like tool whose render is a `TurnItem::FileChange` must request
/// approval through the dedicated `FileChangeRequestApproval` notification
/// (with the per-file change list), not the command-style channel — and with
/// the same correlation id as its lifecycle events.
#[tokio::test]
async fn file_change_tool_approval_uses_file_change_notification() {
    struct FileEditWriteTool;
    #[async_trait]
    impl ToolHandler for FileEditWriteTool {
        fn name(&self) -> &str {
            "write_file"
        }
        fn description(&self) -> &str {
            "Write a file (approval routing test)."
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" }, "content": { "type": "string" } },
                "required": ["path", "content"]
            })
        }
        fn describe_operation(
            &self,
            args: &serde_json::Value,
        ) -> Option<crate::OperationDescriptor> {
            Some(crate::OperationDescriptor::file_edit(
                args.get("path").and_then(serde_json::Value::as_str).unwrap_or(""),
            ))
        }
        fn render_turn_item(&self, render: &ToolCallRender<'_>) -> TurnItem {
            TurnItem::FileChange {
                id: render.call.id.clone(),
                changes: vec![serde_json::json!({
                    "path": render.args.get("path").and_then(serde_json::Value::as_str).unwrap_or(""),
                    "type": "edit",
                    "diff": "+hello",
                })],
                status: render.status.to_owned(),
            }
        }
        async fn execute(
            &self,
            _ctx: &ToolContext,
            _args: &serde_json::Value,
        ) -> Result<ToolOutput, AgentError> {
            Ok(ToolOutput { content: "written".to_owned(), metadata: None })
        }
    }

    let llm = Arc::new(OneShotToolLlm::new("write_file", r#"{"path":"a.txt","content":"hello"}"#));
    let store = Arc::new(RecordingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(ApprovingRecordingNotify::default());

    let router = ToolRouter::new();
    router.register(Box::new(FileEditWriteTool));

    let approval = Arc::clone(&notify) as Arc<dyn ApprovalPort>;
    let control = Arc::new(
        AgentControl::new(llm, store_port, notify.clone(), approval, Arc::new(router), 8, 4)
            .with_exec_policy(Arc::new(AskAllExecPolicy)),
    );

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: ConversationMessageContent::Text("Write a file".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };
    let thread_id =
        control.spawn("session-file-approval".into(), config, messages).await.expect("spawn");
    let final_status = wait_for_control_terminal_status(&control, &thread_id).await;
    assert_eq!(final_status, ThreadStatus::Completed);

    let events = notify.events.lock().unwrap().clone();
    let file_approvals: Vec<&crate::protocol::FileChangeRequestApprovalParams> = events
        .iter()
        .filter_map(|event| match event {
            EventMsg::FileChangeRequestApproval(p) if p.thread_id == thread_id => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(file_approvals.len(), 1, "FileChange render must request via the file channel");
    let params = file_approvals[0];
    assert_eq!(params.item_id, "call-1", "approval id must match the lifecycle item id");
    assert_eq!(params.changes.len(), 1);
    assert_eq!(params.changes[0].path, "a.txt");
    assert_eq!(params.changes[0].change_type, "edit");
    assert_eq!(params.changes[0].diff.as_deref(), Some("+hello"));

    assert!(
        !events.iter().any(|event| matches!(
            event,
            EventMsg::CommandExecutionRequestApproval(p) if p.thread_id == thread_id
        )),
        "file-change approvals must not also emit the command-style notification"
    );
}

/// Reproduces the post-approval hang with a tool that streams output via
/// `ctx.output` (like the real shell tool), driven through the FULL
/// `handle_tool_call` path: approval → `tokio::join!(run, drain)`. The hard
/// timeout turns a hang into a test failure.
#[tokio::test]
async fn streaming_tool_after_approval_completes_without_hang() {
    use crate::tool::ToolOutputStream;

    struct StreamingEchoTool;
    #[async_trait]
    impl ToolHandler for StreamingEchoTool {
        fn name(&self) -> &str {
            "streaming_echo"
        }
        fn description(&self) -> &str {
            "Echo with streaming output."
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object","properties":{"message":{"type":"string"}},"required":["message"]})
        }
        fn describe_operation(
            &self,
            args: &serde_json::Value,
        ) -> Option<crate::OperationDescriptor> {
            Some(crate::OperationDescriptor::shell(
                args.get("message").and_then(serde_json::Value::as_str).unwrap_or(""),
            ))
        }
        async fn execute(
            &self,
            ctx: &ToolContext,
            args: &serde_json::Value,
        ) -> Result<ToolOutput, AgentError> {
            if let Some(observer) = ctx.output.as_ref() {
                observer.on_output(ToolOutputStream::Stdout, "chunk-1\n");
                observer.on_output(ToolOutputStream::Stdout, "chunk-2\n");
            }
            let msg = args.get("message").and_then(serde_json::Value::as_str).unwrap_or("");
            Ok(ToolOutput { content: format!("streamed: {msg}"), metadata: None })
        }
    }

    struct StreamingLlm {
        count: Mutex<u32>,
    }
    impl StreamingLlm {
        fn new() -> Self {
            Self { count: Mutex::new(0) }
        }
    }
    #[async_trait]
    impl LlmPort for StreamingLlm {
        async fn chat_completion(
            &self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _config: &AgentConfig,
            _trace_context: &AgentTraceContext,
        ) -> Result<LlmResponse, AgentError> {
            let mut c = self.count.lock().unwrap();
            *c += 1;
            if *c == 1 {
                Ok(LlmResponse {
                    content: None,
                    content_already_streamed: false,
                    tool_calls: vec![ParsedToolCall {
                        id: "sc-1".into(),
                        name: "streaming_echo".into(),
                        arguments: r#"{"message":"hi"}"#.into(),
                    }],
                    finish_reason: Some("tool_calls".into()),
                    usage: None,
                })
            } else {
                Ok(LlmResponse {
                    content: Some("done".into()),
                    content_already_streamed: false,
                    tool_calls: vec![],
                    finish_reason: Some("stop".into()),
                    usage: None,
                })
            }
        }
    }

    let llm = Arc::new(StreamingLlm::new());
    let store_port: Arc<dyn AgentStorePort> = Arc::new(RecordingStore::default());
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(StreamingEchoTool));
    let approval = Arc::clone(&notify);
    let control = Arc::new(
        AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4)
            .with_exec_policy(Arc::new(AskAllExecPolicy)),
    );
    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("go".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };

    let thread_id = control.spawn("sess-stream".into(), config, messages).await.expect("spawn");
    let mut rx = control.subscribe(&thread_id).await.expect("subscribe");
    let status = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            rx.changed().await.expect("status channel closed");
            let s = *rx.borrow();
            if matches!(s, ThreadStatus::Completed | ThreadStatus::Errored) {
                break s;
            }
        }
    })
    .await
    .expect("HANG: streaming tool after approval did not complete in 10s");
    assert_eq!(status, ThreadStatus::Completed);
}

#[tokio::test]
async fn rejected_approval_tool_is_recorded_pending_then_failed() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(RecordingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);

    let router = ToolRouter::new();
    router.register(Box::new(ApprovalEchoTool));

    let control = AgentControl::new(
        llm,
        store_port,
        notify,
        Arc::new(RejectingApproval),
        Arc::new(router),
        8,
        4,
    )
    .with_exec_policy(Arc::new(AskAllExecPolicy));

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("Please echo".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };

    let thread_id =
        control.spawn("session-approval-rejected".into(), config, messages).await.expect("spawn");
    let final_status = wait_for_control_terminal_status(&control, &thread_id).await;

    assert_eq!(final_status, ThreadStatus::Completed);
}

#[tokio::test]
async fn invalid_tool_arguments_are_recorded_failed() {
    let llm = Arc::new(InvalidToolArgsLlm::new());
    let store = Arc::new(RecordingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let approval = Arc::clone(&notify);
    let control =
        AgentControl::new(llm, store_port, notify, approval, Arc::new(ToolRouter::new()), 8, 4);

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("Please use a tool".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };

    let thread_id =
        control.spawn("session-invalid-tool-args".into(), config, messages).await.expect("spawn");
    let final_status = wait_for_control_terminal_status(&control, &thread_id).await;

    assert_eq!(final_status, ThreadStatus::Completed);
}

#[tokio::test]
async fn hook_blocked_tool_call_is_recorded_failed() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(RecordingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);

    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&notify);
    let control = AgentControl::new_with_hooks(
        llm,
        store_port,
        notify,
        approval,
        Arc::new(router),
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        vec![Arc::new(BlockingHook)],
    );

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("Please echo".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };

    let thread_id =
        control.spawn("session-hook-blocked".into(), config, messages).await.expect("spawn");
    let final_status = wait_for_control_terminal_status(&control, &thread_id).await;

    assert_eq!(final_status, ThreadStatus::Completed);
}

#[tokio::test]
async fn send_input_replays_persisted_thread_messages() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());

    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&Arc::new(NoopNotify));
    let control = Arc::new(AgentControl::new(
        llm,
        store_port,
        notify.clone(),
        approval,
        Arc::new(router),
        8,
        4,
    ));

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("first prompt".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };

    let thread_id = control.spawn("session-replay".into(), config, messages).await.expect("spawn");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    // slab-agent no longer reads conversation data itself (the trait
    // method is gone). `resume_with_input` mirrors the hoisted
    // `AgentCore::send_input` read+sort+max over the mock's seeded messages.
    // Seed the spawn-time user message as a turn-0 record (what the OLD adapter
    // persisted) so the helper has NON-EMPTY history to replay — without this
    // the helper builds an empty history and the replay/sort/max logic this test
    // names is never exercised (a false-green). End-to-end replay coverage over
    // the REAL rollout reader lives in app-core `harness_tests.rs`.
    store.messages.lock().unwrap().push(ThreadMessageRecord {
        id: "replay-0".into(),
        thread_id: thread_id.clone(),
        turn_index: 0,
        message: ConversationMessage {
            role: "user".into(),
            content: slab_types::ConversationMessageContent::Text("first prompt".into()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        },
        created_at: "2026-01-01T00:00:00Z".into(),
    });

    resume_with_input(&store, &control, &thread_id, "second prompt".into())
        .await
        .expect("send input");
    // slab-agent emits `MessageAppended` events (no store writes).
    wait_for_emitted_message(&notify, &thread_id, "second prompt").await;

    let emitted = notify.emitted_messages(&thread_id);
    assert!(
        emitted.iter().any(|message| message.rendered_text().contains("first prompt")),
        "initial prompt emitted on spawn: {:?}",
        emitted
    );
    assert!(
        emitted.iter().any(|message| message.rendered_text().contains("second prompt")),
        "resume prompt emitted: {:?}",
        emitted
    );
    // The resume read the seeded turn-0 record and computed
    // `starting_turn_index = max(turn_index) + 1 = 1`, so the resume's emitted
    // `MessageAppended("second prompt")` must carry `turn_index = 1`. If the
    // helper failed to read/sort/max the history, this would be 0.
    let resume_turn = notify
        .events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            EventMsg::MessageAppended(p)
                if p.thread_id == thread_id
                    && p.message.rendered_text().contains("second prompt") =>
            {
                Some(p.turn_index)
            }
            _ => None,
        })
        .next()
        .expect("resume MessageAppended for second prompt");
    assert_eq!(
        resume_turn, 1,
        "resume computed starting_turn_index from the seeded turn-0 history"
    );
}

// ── OnAgentStart init-batch re-injection regression (context inflation) ─────
//
// A hook that injects a TAGGED init batch on every `OnAgentStart` (what
// `ContextInstructionHook` does in production) must not duplicate the batch
// across turns: the thread-level merge REPLACES same-tagged messages, and the
// trailing-count emit anchor emits only the new user message. Pre-fix, the
// second run re-INSERTED the batch (a second system message reached the LLM
// and the rollout TurnState snapshots) and the absolute `emit_from` anchor —
// shifted by the injected batch — re-emitted a tail of old history as
// `MessageAppended` (duplicating it in the rollout).

/// Test hook mirroring `ContextInstructionHook`: injects a tagged init batch
/// (system + two developer fragments) on every agent start.
struct TaggedInitHook;

#[async_trait]
impl crate::AgentHook for TaggedInitHook {
    async fn on_event(&self, event: &crate::HookEvent) -> crate::HookOutcome {
        let crate::HookEvent::OnAgentStart { .. } = event else {
            return crate::HookOutcome::Continue;
        };
        let batch = vec![
            ConversationMessage {
                role: "system".into(),
                content: ConversationMessageContent::Text("You are a test agent.".into()),
                name: Some("slab_system".into()),
                tool_call_id: None,
                tool_calls: vec![],
            },
            ConversationMessage {
                role: "developer".into(),
                content: ConversationMessageContent::Text(
                    "<environment_context>\n<cwd>/w</cwd>".into(),
                ),
                name: Some("slab_environment".into()),
                tool_call_id: None,
                tool_calls: vec![],
            },
            ConversationMessage {
                role: "developer".into(),
                content: ConversationMessageContent::Text(
                    "<permissions_instructions>\nmode: test".into(),
                ),
                name: Some("slab_permissions".into()),
                tool_call_id: None,
                tool_calls: vec![],
            },
        ];
        crate::HookOutcome::Effects {
            tool_action: crate::HookToolAction::Continue,
            injected_messages: batch,
            observations: Vec::new(),
        }
    }
}

#[tokio::test]
async fn on_agent_start_batch_replaces_and_does_not_reemit_history() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let approval = Arc::clone(&Arc::new(NoopNotify));
    let control = Arc::new(AgentControl::new(
        llm,
        store_port,
        notify.clone(),
        approval,
        Arc::new(ToolRouter::new()),
        8,
        4,
    ));
    // Register the init hook BEFORE the first spawn so BOTH runs inject.
    control.replace_hooks(vec![Arc::new(TaggedInitHook)]);

    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };
    let thread_id = control
        .spawn(
            "session-injection".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("first prompt".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    // Seed the turn-0 record (mirrors what the persisted read path returns),
    // then run a second turn with the hook injecting AGAIN.
    store.messages.lock().unwrap().push(ThreadMessageRecord {
        id: "injection-0".into(),
        thread_id: thread_id.clone(),
        turn_index: 0,
        message: ConversationMessage {
            role: "user".into(),
            content: ConversationMessageContent::Text("first prompt".into()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        },
        created_at: "2026-01-01T00:00:00Z".into(),
    });
    resume_with_input(&store, &control, &thread_id, "second prompt".into())
        .await
        .expect("send input");
    wait_for_emitted_message(&notify, &thread_id, "second prompt").await;

    let emitted = notify.emitted_messages(&thread_id);
    // The init batch is emitted EXACTLY once (on spawn) — the second run's
    // merge replaces in place and re-emits nothing for it.
    assert_eq!(
        emitted.iter().filter(|m| m.role == "system").count(),
        1,
        "system message emitted once total (not once per turn): {emitted:?}"
    );
    for tag in ["slab_environment", "slab_permissions"] {
        assert_eq!(
            emitted.iter().filter(|m| m.name.as_deref() == Some(tag)).count(),
            1,
            "{tag} fragment emitted exactly once"
        );
    }
    // No anchor-drift re-emission of history: each user message exactly once.
    for text in ["first prompt", "second prompt"] {
        assert_eq!(
            emitted.iter().filter(|m| m.rendered_text().contains(text)).count(),
            1,
            "{text} emitted exactly once (no drift re-emission): {emitted:?}"
        );
    }
}

#[tokio::test]
async fn tool_context_includes_thread_workspace_and_plan_scope() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    let workspaces = Arc::new(Mutex::new(Vec::new()));
    let plans = Arc::new(Mutex::new(Vec::new()));
    router.register(Box::new(CapturingContextTool {
        workspaces: Arc::clone(&workspaces),
        plans: Arc::clone(&plans),
    }));

    let approval = Arc::clone(&notify);
    let workspace_root = PathBuf::from("C:/workspace/demo");
    let thread_context = AgentThreadContext::new()
        .with_workspace(WorkspaceRef { root: workspace_root.clone(), session_id: None })
        .with_plan_id("plan-1");
    let control = Arc::new(
        AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4)
            .with_thread_context(thread_context),
    );

    let config = AgentConfig { model: "mock".into(), max_turns: 2, ..AgentConfig::default() };
    let thread_id = control
        .spawn(
            "session-tool-context".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("capture context".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    assert_eq!(
        workspaces.lock().unwrap().as_slice(),
        &[Some(WorkspaceRef {
            root: workspace_root,
            session_id: Some("session-tool-context".to_owned()),
        })]
    );
    assert_eq!(
        plans.lock().unwrap().as_slice(),
        &[Some(PlanRef { thread_id, plan_id: Some("plan-1".to_owned()) })]
    );
}

// ── task.complete default-deny / structured completion (B-3, 双轨 2) ─────────────

/// Test double for the `task.complete` tool contract. On success it returns the
/// `task_complete` metadata marker that `turn_tool_call` recognizes to finalize
/// the run. With `fail_first_call` it errors on the first invocation, simulating
/// a denied completion that is fed back to the LLM as a tool result.
struct TaskCompleteMarkerTool {
    fail_first_call: bool,
    calls: Mutex<u32>,
}

impl TaskCompleteMarkerTool {
    fn always_succeeds() -> Self {
        Self { fail_first_call: false, calls: Mutex::new(0) }
    }
    fn failing_once() -> Self {
        Self { fail_first_call: true, calls: Mutex::new(0) }
    }
}

#[async_trait]
impl ToolHandler for TaskCompleteMarkerTool {
    fn name(&self) -> &str {
        "task.complete"
    }
    fn description(&self) -> &str {
        "Test double for the task.complete structured-completion tool."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }
    async fn execute(
        &self,
        _ctx: &ToolContext,
        _arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        let mut calls = self.calls.lock().unwrap();
        *calls += 1;
        if self.fail_first_call && *calls == 1 {
            return Err(AgentError::ToolExecution(
                "task.complete denied: 1 plan item(s) are not completed".to_owned(),
            ));
        }
        let metadata = serde_json::json!({
            "task_complete": {
                "summary": "shipped it",
                "artifact_refs": [{ "path": "src/main.rs", "kind": "file" }],
            }
        });
        Ok(ToolOutput { content: "task complete: shipped it".to_owned(), metadata: Some(metadata) })
    }
}

/// Mock LLM that always asks the agent to call `task.complete`.
struct TaskCompleteLlm {
    call_count: Mutex<u32>,
}

impl TaskCompleteLlm {
    fn new() -> Self {
        Self { call_count: Mutex::new(0) }
    }
}

#[async_trait]
impl LlmPort for TaskCompleteLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        Ok(LlmResponse {
            content: None,
            content_already_streamed: false,
            tool_calls: vec![ParsedToolCall {
                id: format!("call-task-{count}"),
                name: "task.complete".into(),
                arguments: r#"{"summary":"shipped it","plan":[{"step":"x","status":"completed"}]}"#
                    .into(),
            }],
            finish_reason: Some("tool_calls".into()),
            usage: None,
        })
    }
}

#[tokio::test]
async fn task_complete_finalizes_run_on_success() {
    let llm = Arc::new(TaskCompleteLlm::new());
    let llm_handle = Arc::clone(&llm);
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let router = ToolRouter::new();
    router.register(Box::new(TaskCompleteMarkerTool::always_succeeds()));
    let approval = Arc::clone(&Arc::new(NoopNotify));
    let control = Arc::new(AgentControl::new(
        llm,
        store_port,
        notify.clone(),
        approval,
        Arc::new(router),
        8,
        4,
    ));
    let config = AgentConfig { model: "mock".into(), max_turns: 3, ..AgentConfig::default() };
    let thread_id = control
        .spawn(
            "session-task-complete".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("finish the task".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    // task.complete must short-circuit to Final after exactly one LLM call.
    let calls = *llm_handle.call_count.lock().unwrap();
    assert_eq!(calls, 1, "task.complete should finalize without a second LLM turn");

    // slab-agent emits `MessageAppended` (no store writes).
    let final_text = notify
        .emitted_messages(&thread_id)
        .iter()
        .rev()
        .find(|message| message.role == "assistant")
        .and_then(|message| match &message.content {
            ConversationMessageContent::Text(text) => Some(text.clone()),
            _ => None,
        });
    assert_eq!(final_text.as_deref(), Some("shipped it"));
}

/// Tool double that seeds the durable plan store (stands in for `update_plan`)
/// and flags that it ran, so the teardown test can distinguish "cleared after
/// seeding" from "never seeded".
struct SeedPlanTool {
    ran: Arc<AtomicBool>,
}

#[async_trait]
impl ToolHandler for SeedPlanTool {
    fn name(&self) -> &str {
        "seed_plan"
    }

    fn description(&self) -> &str {
        "Test double that writes a plan into the durable plan store."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        _arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        ctx.plan_store
            .replace_plan(
                &ctx.thread_id,
                crate::Plan {
                    plan_id: "plan-teardown".to_owned(),
                    summary: Some("teardown probe".to_owned()),
                    items: vec![crate::PlanItem {
                        step: "work".to_owned(),
                        status: crate::PlanStatus::Completed,
                        depends_on: None,
                        result_ref: None,
                    }],
                    counts: crate::PlanCounts {
                        pending: 0,
                        in_progress: 0,
                        completed: 1,
                        blocked: 0,
                    },
                    current_step: None,
                },
            )
            .await?;
        self.ran.store(true, Ordering::SeqCst);
        Ok(ToolOutput { content: "seeded".to_owned(), metadata: None })
    }
}

/// Mock LLM: first call seeds the plan store, second call completes the task.
struct PlanThenCompleteLlm {
    call_count: Mutex<u32>,
}

impl PlanThenCompleteLlm {
    fn new() -> Self {
        Self { call_count: Mutex::new(0) }
    }
}

#[async_trait]
impl LlmPort for PlanThenCompleteLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        let call = if *count == 1 {
            ParsedToolCall {
                id: "call-seed".into(),
                name: "seed_plan".into(),
                arguments: "{}".into(),
            }
        } else {
            ParsedToolCall {
                id: "call-complete".into(),
                name: "task.complete".into(),
                arguments: r#"{"summary":"shipped it","plan":[{"step":"x","status":"completed"}]}"#
                    .into(),
            }
        };
        Ok(LlmResponse {
            content: None,
            content_already_streamed: false,
            tool_calls: vec![call],
            finish_reason: Some("tool_calls".into()),
            usage: None,
        })
    }
}

/// A finished run must retire its per-thread residue (durable plan, exec
/// mode): without the teardown clear, a resumed run saw the stale
/// all-completed plan in its init context and re-finalized on the first tool
/// batch — the A2 silent-termination loop.
#[tokio::test]
async fn run_teardown_retires_durable_plan() {
    let llm = Arc::new(PlanThenCompleteLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(TaskCompleteMarkerTool::always_succeeds()));
    let seed_ran = Arc::new(AtomicBool::new(false));
    router.register(Box::new(SeedPlanTool { ran: Arc::clone(&seed_ran) }));
    let plan_store: Arc<dyn crate::PlanStorePort> = Arc::new(InMemoryPlanStore::default());
    let approval = Arc::clone(&Arc::new(NoopNotify));
    let control = Arc::new(
        AgentControl::new(llm, store_port, notify.clone(), approval, Arc::new(router), 8, 4)
            .with_plan_store(plan_store.clone()),
    );
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };
    let thread_id = control
        .spawn(
            "session-teardown-clear".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("plan then finish".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    assert!(seed_ran.load(Ordering::SeqCst), "seed_plan must have run mid-run");
    assert!(
        plan_store.current_plan(&thread_id).await.is_none(),
        "run teardown must retire the durable plan (stale plans made resumed runs re-finalize)"
    );
}

#[tokio::test]
async fn task_complete_denial_does_not_finalize() {
    let llm = Arc::new(TaskCompleteLlm::new());
    let llm_handle = Arc::clone(&llm);
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(TaskCompleteMarkerTool::failing_once()));
    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4));
    let config = AgentConfig { model: "mock".into(), max_turns: 3, ..AgentConfig::default() };
    let thread_id = control
        .spawn(
            "session-task-denied".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("finish the task".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    // First call is denied (error fed back), second call succeeds → Final.
    let calls = *llm_handle.call_count.lock().unwrap();
    assert_eq!(calls, 2, "denied task.complete must not finalize on the first turn");
}

#[tokio::test]
async fn echo_tool_returns_input() {
    use crate::tool::{ToolContext, ToolHandler};

    let ctx = ToolContext::for_thread("t1").build();
    let args = serde_json::json!({"message": "test message"});

    let output = TestEchoTool.execute(&ctx, &args).await.expect("echo should succeed");
    assert_eq!(output.content, "test message");
}

#[tokio::test]
async fn echo_tool_missing_message_returns_empty() {
    use crate::tool::{ToolContext, ToolHandler};

    let ctx = ToolContext::for_thread("t1").build();
    let args = serde_json::json!({});

    let output = TestEchoTool.execute(&ctx, &args).await.expect("echo should succeed");
    assert_eq!(output.content, "");
}

// ── Tool router tests ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn tool_router_registers_and_retrieves_tools() {
    use crate::tool::ToolRouter;

    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let tool = router.get("echo");
    assert!(tool.is_some(), "echo tool should be registered");
    assert_eq!(tool.unwrap().name(), "echo");
}

#[tokio::test]
async fn tool_router_returns_none_for_unregistered_tool() {
    use crate::tool::ToolRouter;

    let router = ToolRouter::new();
    let tool = router.get("nonexistent");
    assert!(tool.is_none(), "unregistered tool should return None");
}

#[tokio::test]
async fn tool_router_overwrites_existing_tool() {
    use crate::tool::{ToolContext, ToolHandler, ToolRouter};

    // Create a custom test tool that returns "custom"
    #[derive(Debug)]
    struct CustomTool;

    #[async_trait]
    impl ToolHandler for CustomTool {
        fn name(&self) -> &str {
            "custom"
        }

        fn description(&self) -> &str {
            "A custom test tool"
        }

        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }

        async fn execute(
            &self,
            _ctx: &ToolContext,
            _arguments: &serde_json::Value,
        ) -> Result<crate::tool::ToolOutput, AgentError> {
            Ok(crate::tool::ToolOutput { content: "custom".to_string(), metadata: None })
        }
    }

    let router = ToolRouter::new();
    router.register(Box::new(CustomTool));

    let ctx = ToolContext::for_thread("t1").build();
    let args = serde_json::json!({});

    let output = router
        .get("custom")
        .unwrap()
        .execute(&ctx, &args)
        .await
        .expect("custom tool should succeed");

    assert_eq!(output.content, "custom");
}

#[tokio::test]
async fn tool_router_generates_tool_specs() {
    use crate::tool::ToolRouter;

    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let specs = router.tool_specs();
    assert_eq!(specs.len(), 1, "should have one tool spec");
    assert_eq!(specs[0].name, "echo");
    assert!(!specs[0].description.is_empty());
}

// ── Thread limit enforcement tests ────────────────────────────────────────────────────

#[tokio::test]
async fn agent_control_enforces_thread_limit() {
    let llm = Arc::new(SlowLlm);
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());

    // Set max_threads to 1
    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new(llm, store, notify, approval, router, 1, 4));

    let config = AgentConfig { model: "mock".into(), max_turns: 1, ..AgentConfig::default() };

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("test".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];

    // First thread should spawn successfully
    let thread_id_1 = control
        .spawn("session-1".into(), config.clone(), messages.clone())
        .await
        .expect("first thread should spawn");

    // Second thread should fail with ThreadLimitExceeded
    let result = control.spawn("session-2".into(), config, messages).await;
    assert!(
        matches!(result, Err(AgentError::ThreadLimitExceeded { .. })),
        "second thread should exceed limit"
    );

    // Clean up the first thread
    control.shutdown(&thread_id_1).await.expect("shutdown should succeed");
}

#[tokio::test]
async fn active_thread_ids_and_interrupt_all_target_active_threads() {
    let llm = Arc::new(MockLlm::new());
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());
    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new(llm, store, notify, approval, router, 8, 4));

    // No active threads ⇒ empty enumeration and empty interrupt sweep.
    assert!(control.active_thread_ids().await.is_empty());
    assert!(control.interrupt_all().await.is_empty());

    let config = AgentConfig { model: "mock".into(), max_turns: 1, ..AgentConfig::default() };
    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("hi".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let thread_id = control.spawn("session-migrate".into(), config, messages).await.expect("spawn");

    // The thread is registered before spawn returns, so it is enumerable now.
    let active = control.active_thread_ids().await;
    assert!(active.contains(&thread_id), "active threads should include the spawned thread");

    // interrupt_all targets every active thread and reports what it interrupted.
    let interrupted = control.interrupt_all().await;
    assert_eq!(interrupted, vec![thread_id.clone()]);

    let _ = control.shutdown(&thread_id).await;
}

#[tokio::test]
async fn agent_control_enforces_depth_limit() {
    let llm = Arc::new(MockLlm::new());
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());

    // Set max_depth to 0 (only root agents allowed)
    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new(llm.clone(), store, notify, approval, router, 8, 0));

    let config = AgentConfig { model: "mock".into(), max_turns: 1, ..AgentConfig::default() };

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("test".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];

    // Root agent at depth 0 should succeed
    let result = control.spawn("session-1".into(), config.clone(), messages.clone()).await;
    assert!(result.is_ok(), "root agent at depth 0 should spawn");

    // Clean up
    let _ = control.shutdown(&result.unwrap()).await;

    // Child agent at depth 1 should fail
    let result =
        control.spawn_child("session-2".into(), "parent-1".into(), 1, config, messages).await;
    assert!(
        matches!(result, Err(AgentError::DepthLimitExceeded { .. })),
        "child agent at depth 1 should exceed limit of 0"
    );
}

#[tokio::test]
async fn fork_thread_clones_history_at_depth_plus_one() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());
    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port.clone(), notify, approval, router, 8, 4));

    // Seed a parent thread (depth 0) with two messages and one turn state.
    let now = "2026-01-01T00:00:00Z".to_owned();
    let parent_config = AgentConfig { model: "parent-model".into(), ..AgentConfig::default() };
    store_port
        .upsert_thread(&ThreadSnapshot {
            id: "parent-1".into(),
            session_id: "session-1".into(),
            parent_id: None,
            depth: 0,
            status: ThreadStatus::Completed,
            role_name: None,
            config_json: serde_json::to_string(&parent_config).expect("config"),
            completion_text: Some("done".into()),
            created_at: now.clone(),
            updated_at: now.clone(),
            archived_at: None,
        })
        .await
        .expect("seed parent");
    for (id, turn_index) in [("pmsg-0", 0u32), ("pmsg-1", 1)] {
        // `insert_thread_message` left the slab-agent store trait;
        // seed the mock's in-memory messages vec directly.
        store.messages.lock().unwrap().push(ThreadMessageRecord {
            id: id.into(),
            thread_id: "parent-1".into(),
            turn_index,
            message: ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text(id.into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            created_at: now.clone(),
        });
    }
    // `upsert_turn_state` left the slab-agent store trait (slab-agent
    // emits `TurnStateChanged` events now). Seed the mock's in-memory turn-state
    // vec directly so the parent has a realistic history; fork does not copy
    // per-record turn states, so this is not asserted on below.
    store.turn_states.lock().unwrap().push(TurnStateRecord {
        thread_id: "parent-1".into(),
        turn_index: 1,
        status: "completed".into(),
        input_messages_json: None,
        tool_specs_json: None,
        llm_response_json: None,
        error: None,
        started_at: now.clone(),
        completed_at: Some(now.clone()),
    });

    // Fork with a model override.
    let child_id = control
        .fork_thread("parent-1", Some("child-model".into()))
        .await
        .expect("fork should succeed");

    let child = control.thread_snapshot(&child_id).await.expect("read").expect("child present");
    assert_eq!(child.depth, 1, "child is one level deeper than the parent");
    assert_eq!(child.parent_id.as_deref(), Some("parent-1"));
    assert_eq!(child.status, ThreadStatus::Pending);
    let child_config: AgentConfig =
        serde_json::from_str(&child.config_json).expect("child config parses");
    assert_eq!(child_config.model, "child-model", "model override applied");

    // `AgentControl::fork_thread` no longer clones the parent's
    // per-record history — the production fork path (HarnessService::fork_thread)
    // snapshots the parent rollout file wholesale into the child. At this layer
    // the child is just new metadata, so the parent's history stays untouched
    // under the parent id (no per-record copy into the child).
    //
    // `list_thread_messages` left the slab-agent store trait (slab-agent
    // is pure now); assert directly against the mock's in-memory messages.
    let parent_message_count =
        store.messages.lock().unwrap().iter().filter(|m| m.thread_id == "parent-1").count();
    assert_eq!(parent_message_count, 2, "parent history untouched by fork");
}

#[tokio::test]
async fn apply_agent_override_sets_and_clears_agent_type_and_prompt() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());
    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port.clone(), notify, approval, router, 8, 4));

    // Seed a default thread (no agent_type, no system_prompt).
    let now = "2026-01-01T00:00:00Z".to_owned();
    store_port
        .upsert_thread(&ThreadSnapshot {
            id: "thread-1".into(),
            session_id: "session-1".into(),
            parent_id: None,
            depth: 0,
            status: ThreadStatus::Completed,
            role_name: None,
            config_json: serde_json::to_string(&AgentConfig::default()).expect("config"),
            completion_text: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            archived_at: None,
        })
        .await
        .expect("seed thread");

    let plan_def = AgentDefinition {
        agent_type: "plan".into(),
        description: "read-only planner".into(),
        tools: ToolConstraint::Denylist(vec!["shell".into()]),
        system_prompt: "You are a planning agent.".into(),
        model: ModelPolicy::Inherit,
    };

    // Setting the override persists agent_type + system_prompt on the config.
    control.apply_agent_override("thread-1", Some(&plan_def)).await.expect("override applies");
    let snap = control.thread_snapshot("thread-1").await.expect("read").expect("thread present");
    let cfg: AgentConfig = serde_json::from_str(&snap.config_json).expect("config parses");
    assert_eq!(cfg.agent_type.as_deref(), Some("plan"));
    assert_eq!(cfg.system_prompt.as_deref(), Some("You are a planning agent."));

    // Clearing the override removes both so the next turn runs as the default agent.
    control.apply_agent_override("thread-1", None).await.expect("clear applies");
    let snap = control.thread_snapshot("thread-1").await.expect("read").expect("thread present");
    let cfg: AgentConfig = serde_json::from_str(&snap.config_json).expect("config parses");
    assert!(cfg.agent_type.is_none(), "agent_type cleared");
    assert!(cfg.system_prompt.is_none(), "system_prompt cleared");
}

#[tokio::test]
async fn set_thread_reasoning_effort_sets_and_clears() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());
    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port.clone(), notify, approval, router, 8, 4));

    // Seed a default thread (no reasoning_effort).
    let now = "2026-01-01T00:00:00Z".to_owned();
    store_port
        .upsert_thread(&ThreadSnapshot {
            id: "thread-1".into(),
            session_id: "session-1".into(),
            parent_id: None,
            depth: 0,
            status: ThreadStatus::Completed,
            role_name: None,
            config_json: serde_json::to_string(&AgentConfig::default()).expect("config"),
            completion_text: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            archived_at: None,
        })
        .await
        .expect("seed thread");

    // Setting the effort persists it on the config the next turn re-reads.
    control
        .set_thread_reasoning_effort("thread-1", Some(slab_types::chat::ChatReasoningEffort::High))
        .await
        .expect("effort applies");
    let snap = control.thread_snapshot("thread-1").await.expect("read").expect("thread present");
    let cfg: AgentConfig = serde_json::from_str(&snap.config_json).expect("config parses");
    assert_eq!(cfg.reasoning_effort, Some(slab_types::chat::ChatReasoningEffort::High));

    // Clearing removes the override so no sticky effort survives the turn.
    control.set_thread_reasoning_effort("thread-1", None).await.expect("clear applies");
    let snap = control.thread_snapshot("thread-1").await.expect("read").expect("thread present");
    let cfg: AgentConfig = serde_json::from_str(&snap.config_json).expect("config parses");
    assert!(cfg.reasoning_effort.is_none(), "reasoning_effort cleared");
}

// ── Error propagation tests ───────────────────────────────────────────────────────────

struct FailingLlm;

#[async_trait]
impl LlmPort for FailingLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        Err(AgentError::Llm("simulated LLM failure".into()))
    }
}

struct SlowLlm;

#[async_trait]
impl LlmPort for SlowLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        Ok(LlmResponse {
            content: Some("too late".into()),
            content_already_streamed: false,
            tool_calls: Vec::new(),
            finish_reason: Some("stop".into()),
            usage: None,
        })
    }
}

#[tokio::test]
async fn agent_propagates_llm_errors() {
    let llm = Arc::new(FailingLlm);
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());

    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new(llm, store, notify, approval, router, 8, 4));

    let config = AgentConfig { model: "mock".into(), max_turns: 1, ..AgentConfig::default() };

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("test".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];

    let thread_id =
        control.spawn("session-1".into(), config, messages).await.expect("spawn should succeed");

    // Wait for the thread to reach an error state
    let mut status_rx = control.subscribe(&thread_id).await.expect("subscribe should succeed");

    let final_status = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            status_rx.changed().await.expect("status channel closed");
            let status = *status_rx.borrow();
            if matches!(status, ThreadStatus::Errored | ThreadStatus::Shutdown) {
                return status;
            }
        }
    })
    .await
    .expect("agent should error within timeout");

    assert_eq!(final_status, ThreadStatus::Errored, "agent should error when LLM fails");
}

#[tokio::test]
async fn interrupt_cancels_running_turn_and_allows_follow_up_input() {
    let llm = Arc::new(SlowLlm);
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());

    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new(llm, store_port, notify, approval, router, 8, 4));

    let config = AgentConfig { model: "mock".into(), max_turns: 1, ..AgentConfig::default() };
    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("slow".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];

    let thread_id =
        control.spawn("session-interrupt".into(), config, messages).await.expect("spawn");
    control.interrupt(&thread_id).await.expect("interrupt");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Interrupted).await;

    assert_eq!(control.active_thread_count().await, 0);
    let result = resume_with_input(&store, &control, &thread_id, "continue".into()).await;
    assert!(result.is_ok(), "interrupted thread should accept follow-up input");
    let _ = control.shutdown(&thread_id).await;
}

/// LLM double that always requests one long-running echo tool call — used to
/// hold a tool item in flight so an interrupt lands mid-execution.
struct DelayedToolCallLlm;

#[async_trait]
impl LlmPort for DelayedToolCallLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            content: None,
            content_already_streamed: false,
            tool_calls: vec![ParsedToolCall {
                id: "call-1".into(),
                name: "echo".into(),
                arguments: r#"{"message":"slow tool","delay_ms":30000}"#.into(),
            }],
            finish_reason: Some("tool_calls".into()),
            usage: None,
        })
    }
}

#[tokio::test]
async fn interrupt_mid_tool_emits_single_authoritative_abort_and_closes_items() {
    let llm = Arc::new(DelayedToolCallLlm);
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let router = ToolRouter::new();
    router.register(Box::new(DelayEchoTool));

    let approval: Arc<dyn ApprovalPort> = notify.clone();
    let control = Arc::new(AgentControl::new(
        llm,
        store_port,
        notify.clone(),
        approval,
        Arc::new(router),
        8,
        4,
    ));

    let config = AgentConfig { model: "mock".into(), max_turns: 3, ..AgentConfig::default() };
    let thread_id = control
        .spawn(
            "session-midtool-interrupt".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("use slow tool".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    // Hold until the tool item is genuinely in flight (ItemStarted observed).
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let started =
                notify.events.lock().unwrap().iter().any(
                    |event| matches!(event, EventMsg::ItemStarted(p) if p.item.id() == "call-1"),
                );
            if started {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("tool item never started");

    control.interrupt(&thread_id).await.expect("interrupt");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Interrupted).await;

    let events = notify.events.lock().unwrap().clone();

    // Exactly ONE TurnAborted — the authoritative one from the run teardown,
    // referencing the real turn id (regression: the eager controller
    // placeholder used turn id "current" and the loop emitted nothing).
    let aborted: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            EventMsg::TurnAborted(p) => Some(p.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(aborted.len(), 1, "expected a single authoritative TurnAborted, got {aborted:?}");
    assert_eq!(aborted[0].turn.id, "0");
    assert_eq!(aborted[0].reason.as_deref(), Some("interrupted"));

    // The in-flight item was closed with a synthetic ItemCompleted (no
    // perpetual "running" card in the timeline).
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EventMsg::ItemCompleted(p) if p.item.id() == "call-1")),
        "interrupted in-flight tool item must receive a terminal ItemCompleted"
    );

    // Terminal interrupted turn state landed (previously NO terminal turn
    // state was written on the interrupt path).
    assert!(
        events.iter().any(
            |event| matches!(event, EventMsg::TurnStateChanged(p) if p.status == "interrupted")
        ),
        "interrupt path must persist a terminal interrupted TurnState"
    );

    // The authoritative thread status reached the wire: running →
    // interrupting → interrupted.
    let statuses: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            EventMsg::ThreadStatusChanged(p) => Some(p.status.clone()),
            _ => None,
        })
        .collect();
    assert!(statuses.contains(&"running".to_owned()), "statuses: {statuses:?}");
    assert!(statuses.contains(&"interrupting".to_owned()), "statuses: {statuses:?}");
    assert!(statuses.contains(&"interrupted".to_owned()), "statuses: {statuses:?}");

    // Residue repair: the persisted history ends balanced — the assistant
    // tool-request IS followed by a synthesized "[interrupted]" tool result,
    // so a resumed thread never sends a dangling tool_calls tail to strict
    // providers.
    let messages = notify.emitted_messages(&thread_id);
    assert!(
        messages.iter().any(|m| m.role == "assistant" && !m.tool_calls.is_empty()),
        "assistant tool-request must be persisted before the tool batch"
    );
    assert!(
        messages.iter().any(|m| m.role == "tool"
            && m.tool_call_id.as_deref() == Some("call-1")
            && m.rendered_text().contains("[interrupted]")),
        "an interrupted in-flight tool call must land a synthesized failed result"
    );

    let _ = control.shutdown(&thread_id).await;
}

#[tokio::test]
async fn llm_error_path_emits_turn_aborted_with_error_reason() {
    let llm = Arc::new(FailingLlm);
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let router = Arc::new(ToolRouter::new());

    let approval: Arc<dyn ApprovalPort> = notify.clone();
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify.clone(), approval, router, 8, 4));

    let config = AgentConfig { model: "mock".into(), max_turns: 1, ..AgentConfig::default() };
    let thread_id = control
        .spawn(
            "session-llm-error-abort".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("will fail".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Errored).await;

    let events = notify.events.lock().unwrap().clone();
    // The error run still closes its turn on the wire before the terminal
    // `error` notification (clients treat `error` as stream-terminal).
    let aborted: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            EventMsg::TurnAborted(p) => Some(p.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(aborted.len(), 1, "got {aborted:?}");
    assert_eq!(aborted[0].reason.as_deref(), Some("error"));
    assert_eq!(aborted[0].turn.id, "0");
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EventMsg::TurnStateChanged(p) if p.status == "failed")),
        "error path must persist a terminal failed TurnState"
    );
}

/// Hook that records every `OnAgentEnd` status — proves the graceful-shutdown
/// run tail executed (the old hard-abort skipped it entirely).
struct EndRecordingHook {
    ends: Mutex<Vec<ThreadStatus>>,
}

#[async_trait]
impl AgentHook for EndRecordingHook {
    async fn on_event(&self, event: &HookEvent) -> HookOutcome {
        if let HookEvent::OnAgentEnd { status, .. } = event {
            self.ends.lock().unwrap().push(*status);
        }
        HookOutcome::Continue
    }
}

#[tokio::test]
async fn graceful_shutdown_runs_run_tail_and_persists_terminal_status() {
    let llm = Arc::new(SlowLlm);
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let router = Arc::new(ToolRouter::new());
    let hook = Arc::new(EndRecordingHook { ends: Mutex::new(Vec::new()) });

    let approval: Arc<dyn ApprovalPort> = notify.clone();
    let control = Arc::new(AgentControl::new_with_hooks(
        llm,
        store_port,
        notify,
        approval,
        router,
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        vec![hook.clone()],
    ));

    let thread_id = control
        .spawn(
            "session-graceful-shutdown".into(),
            AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() },
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("slow".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");
    // Hold until the run is genuinely in flight (LLM streaming).
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Running).await;

    control.shutdown(&thread_id).await.expect("shutdown");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Shutdown).await;

    // The run tail executed: OnAgentEnd fired (memory extraction et al. ride
    // this hook in the host). The old hard-abort killed the task before it.
    assert!(
        !hook.ends.lock().unwrap().is_empty(),
        "graceful shutdown must run the OnAgentEnd hook"
    );
}

#[tokio::test]
async fn concurrent_resume_is_serialized_exactly_one_starts() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new(llm, store_port, notify, approval, router, 8, 4));

    // Complete a first run so a resumable snapshot exists.
    let thread_id = control
        .spawn(
            "session-toctou".into(),
            AgentConfig { model: "mock".into(), max_turns: 3, ..AgentConfig::default() },
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("use tool".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    // Race two resumes on the (currently idle) thread: the registry-slot
    // reservation is atomic with the busy check, so exactly one wins — the
    // other observes ThreadBusy instead of double-running the thread.
    let (first, second) = tokio::join!(
        resume_with_input(&store, &control, &thread_id, "race one".into()),
        resume_with_input(&store, &control, &thread_id, "race two".into()),
    );
    let outcomes = [first.is_ok(), second.is_ok()];
    assert_eq!(
        outcomes.iter().filter(|won| **won).count(),
        1,
        "exactly one concurrent resume must win (got {outcomes:?})"
    );
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;
}

// ── S4: LLM resilience ───────────────────────────────────────────────────────

/// Fast-retry config so backoff sleeps stay sub-millisecond in tests.
fn fast_retry_config() -> AgentConfig {
    AgentConfig {
        model: "mock".into(),
        max_turns: 3,
        llm_retry_base_delay_ms: 1,
        ..AgentConfig::default()
    }
}

/// Fails the first `failures` calls with a TRANSIENT error, then answers with
/// plain final text. Counts every call.
struct TransientThenOkLlm {
    failures_left: Mutex<u32>,
    calls: Mutex<u32>,
}

impl TransientThenOkLlm {
    fn new(failures: u32) -> Self {
        Self { failures_left: Mutex::new(failures), calls: Mutex::new(0) }
    }
}

#[async_trait]
impl LlmPort for TransientThenOkLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        *self.calls.lock().unwrap() += 1;
        let mut left = self.failures_left.lock().unwrap();
        if *left > 0 {
            *left -= 1;
            return Err(AgentError::LlmTransient("HTTP 503 service unavailable".to_owned()));
        }
        Ok(LlmResponse {
            content: Some("recovered after retry".into()),
            content_already_streamed: false,
            tool_calls: vec![],
            finish_reason: Some("stop".into()),
            usage: None,
        })
    }
}

/// Streams a text delta, THEN fails transiently — the withhold constraint
/// must refuse to retry (the client already saw the prefix).
struct PartialStreamThenFailLlm {
    calls: Mutex<u32>,
}

#[async_trait]
impl LlmPort for PartialStreamThenFailLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        unreachable!("streaming path must be used")
    }

    async fn chat_completion_streaming(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
        observer: &mut dyn crate::port::LlmStreamObserver,
    ) -> Result<LlmResponse, AgentError> {
        *self.calls.lock().unwrap() += 1;
        observer.on_text_delta("partial prefix").await?;
        Err(AgentError::LlmTransient("HTTP 502 bad gateway".to_owned()))
    }
}

/// Test compact port: deterministic shrink to the first message so the
/// context-overflow recovery path has something to do. Cheap fixed token
/// estimate (len × 10) drives the pre-flight budget gate deterministically.
struct ShrinkOnceCompactPort;

#[async_trait]
impl CompactPort for ShrinkOnceCompactPort {
    async fn compact(
        &self,
        messages: &[ConversationMessage],
        _ctx: &CompactContext<'_>,
    ) -> Result<CompactOutcome, AgentError> {
        let replaced = messages.len().saturating_sub(1);
        let kept = messages.iter().take(1).cloned().collect();
        Ok(CompactOutcome::Replaced {
            messages: kept,
            output_tokens: 4,
            replaced_messages: replaced,
            stubbed_messages: 0,
        })
    }

    fn estimate_tokens(&self, messages: &[ConversationMessage]) -> usize {
        messages.len().saturating_mul(10)
    }

    fn threshold_tokens(&self) -> usize {
        usize::MAX
    }

    fn policy_name(&self) -> &'static str {
        "test-shrink"
    }
}

/// Test compact port: replaces once (identity messages, zero removals) while
/// REPORTING a stub count, then skips — lets a notification-level test assert
/// `ContextCompacted` carries the micro-tier stub count through `maybe_compact`.
struct StubReportingCompactPort {
    calls: std::sync::atomic::AtomicU8,
}

#[async_trait]
impl CompactPort for StubReportingCompactPort {
    async fn compact(
        &self,
        messages: &[ConversationMessage],
        _ctx: &CompactContext<'_>,
    ) -> Result<CompactOutcome, AgentError> {
        if self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 0 {
            return Ok(CompactOutcome::Skipped { reason: "already reported".to_owned() });
        }
        Ok(CompactOutcome::Replaced {
            messages: messages.to_vec(),
            output_tokens: 42,
            replaced_messages: 0,
            stubbed_messages: 3,
        })
    }

    fn estimate_tokens(&self, messages: &[ConversationMessage]) -> usize {
        messages.len().saturating_mul(10)
    }

    fn threshold_tokens(&self) -> usize {
        usize::MAX
    }

    fn policy_name(&self) -> &'static str {
        "test-stub-reporting"
    }
}

async fn run_to_terminal(
    control: &AgentControl,
    store: &PersistingStore,
    thread_id: &str,
) -> ThreadStatus {
    let _ = control;
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            // Tolerate the spawn-race window before the run's initial
            // snapshot lands in the store.
            if let Some(snapshot) = store.get_thread(thread_id).await.unwrap()
                && matches!(
                    snapshot.status,
                    ThreadStatus::Completed
                        | ThreadStatus::Errored
                        | ThreadStatus::Interrupted
                        | ThreadStatus::Shutdown
                )
            {
                return snapshot.status;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("thread reached a terminal status")
}

#[tokio::test]
async fn transient_llm_failures_retry_with_backoff_and_recover() {
    let llm = Arc::new(TransientThenOkLlm::new(2));
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());

    let approval = Arc::clone(&notify);
    let control = AgentControl::new(llm.clone(), store_port, notify, approval, router, 8, 4);

    let thread_id = control
        .spawn(
            "session-transient-retry".into(),
            fast_retry_config(),
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("hello".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");
    let status = run_to_terminal(&control, &store, &thread_id).await;
    assert_eq!(status, ThreadStatus::Completed);
    assert_eq!(*llm.calls.lock().unwrap(), 3, "two transient failures + one success");
}

#[tokio::test]
async fn partially_streamed_failure_is_never_retried() {
    let llm = Arc::new(PartialStreamThenFailLlm { calls: Mutex::new(0) });
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());

    let approval = Arc::clone(&notify);
    let control = AgentControl::new(llm.clone(), store_port, notify, approval, router, 8, 4);

    let thread_id = control
        .spawn(
            "session-withhold".into(),
            fast_retry_config(),
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("hello".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");
    let status = run_to_terminal(&control, &store, &thread_id).await;
    assert_eq!(status, ThreadStatus::Errored);
    assert_eq!(*llm.calls.lock().unwrap(), 1, "withhold: no retry after a streamed prefix");
}

#[tokio::test]
async fn token_budget_preflight_skips_the_llm_call_entirely() {
    let llm = Arc::new(TransientThenOkLlm::new(0));
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());

    let approval = Arc::clone(&notify);
    let control = AgentControl::new_with_ports(
        llm.clone(),
        store_port,
        notify,
        approval,
        router,
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        Arc::new(ShrinkOnceCompactPort),
        Arc::new(crate::risk::BasicToolRiskAnalyzer::default()),
    );

    // One message × 10 estimated tokens; budget 5 → the pre-flight gate must
    // refuse BEFORE any LLM request.
    let config = AgentConfig { token_budget: Some(5), ..fast_retry_config() };
    let thread_id = control
        .spawn(
            "session-budget-preflight".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("hello".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");
    let status = run_to_terminal(&control, &store, &thread_id).await;
    assert_eq!(status, ThreadStatus::Interrupted, "budget exhaustion is an interrupted end");
    assert_eq!(*llm.calls.lock().unwrap(), 0, "preflight must skip the LLM call");
}

/// A micro-tier compaction (content-only shrink: `replaced_messages == 0`,
/// `stubbed_messages > 0`) must surface BOTH counts on the terminal
/// `ContextCompacted` notification — the stub count is the only signal that
/// anything happened at all.
#[tokio::test]
async fn maybe_compact_notifies_stubbed_messages() {
    let llm = Arc::new(TransientThenOkLlm::new(0));
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let router = Arc::new(ToolRouter::new());

    let approval = Arc::clone(&notify) as Arc<dyn ApprovalPort>;
    let control = AgentControl::new_with_ports(
        llm,
        store_port,
        notify.clone(),
        approval,
        router,
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        Arc::new(StubReportingCompactPort { calls: std::sync::atomic::AtomicU8::new(0) }),
        Arc::new(crate::risk::BasicToolRiskAnalyzer::default()),
    );

    let thread_id = control
        .spawn(
            "session-stub-count".into(),
            fast_retry_config(),
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("hello".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");
    let status = run_to_terminal(&control, &store, &thread_id).await;
    assert_eq!(status, ThreadStatus::Completed);

    let compacted: Vec<crate::protocol::ContextCompactedParams> = notify
        .events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            EventMsg::ContextCompacted(params) => Some(params.clone()),
            _ => None,
        })
        .collect();
    assert!(
        compacted.iter().any(|params| params.thread_id == thread_id
            && params.status.as_deref() == Some("compacted")
            && params.removed_messages == Some(0)
            && params.stubbed_messages == Some(3)),
        "ContextCompacted must carry the stub count: {compacted:?}"
    );
}

/// Returns a tool call with usage large enough to trip the POST-response
/// budget gate — exercises the budget-with-tool-calls residue repair.
struct BudgetExhaustingToolCallLlm;

#[async_trait]
impl LlmPort for BudgetExhaustingToolCallLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            content: None,
            content_already_streamed: false,
            tool_calls: vec![ParsedToolCall {
                id: "call-budget".into(),
                name: "echo".into(),
                arguments: r#"{"message":"never runs"}"#.into(),
            }],
            finish_reason: Some("tool_calls".into()),
            usage: Some(LlmUsage {
                prompt_tokens: 60,
                completion_tokens: 40,
                total_tokens: 100,
                estimated: false,
            }),
        })
    }
}

#[tokio::test]
async fn budget_exceeded_with_tool_calls_persists_balanced_history() {
    let llm = Arc::new(BudgetExhaustingToolCallLlm);
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let approval: Arc<dyn ApprovalPort> = notify.clone();
    let control =
        AgentControl::new(llm, store_port, notify.clone(), approval, Arc::new(router), 8, 4);

    let config = AgentConfig { token_budget: Some(100), ..fast_retry_config() };
    let thread_id = control
        .spawn(
            "session-budget-residue".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("use tool".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");
    let status = run_to_terminal(&control, &store, &thread_id).await;
    assert_eq!(status, ThreadStatus::Interrupted);

    let messages = notify.emitted_messages(&thread_id);
    // The assistant tool-request IS persisted (previously dropped silently)...
    assert!(
        messages.iter().any(|m| m.role == "assistant" && !m.tool_calls.is_empty()),
        "assistant tool-request must be persisted on the budget path"
    );
    // ...and every requested call has a synthesized failed tool result, so the
    // replayed history never ends with a dangling tool_calls tail.
    assert!(
        messages.iter().any(|m| m.role == "tool"
            && m.tool_call_id.as_deref() == Some("call-budget")
            && m.rendered_text().contains("[token budget exceeded]")),
        "a synthesized tool result must balance the dangling tool call"
    );
}

/// Fails the first call with a context-overflow error, then answers.
struct ContextOverflowOnceLlm {
    calls: Mutex<u32>,
}

#[async_trait]
impl LlmPort for ContextOverflowOnceLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let mut calls = self.calls.lock().unwrap();
        *calls += 1;
        if *calls == 1 {
            return Err(AgentError::LlmContextTooLong(
                "this model's maximum context length is 8 tokens".to_owned(),
            ));
        }
        Ok(LlmResponse {
            content: Some("recovered after compaction".into()),
            content_already_streamed: false,
            tool_calls: vec![],
            finish_reason: Some("stop".into()),
            usage: None,
        })
    }
}

#[tokio::test]
async fn context_overflow_recovers_via_forced_compaction_once() {
    let llm = Arc::new(ContextOverflowOnceLlm { calls: Mutex::new(0) });
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());

    let approval = Arc::clone(&notify);
    let control = AgentControl::new_with_ports(
        llm.clone(),
        store_port,
        notify,
        approval,
        router,
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        Arc::new(ShrinkOnceCompactPort),
        Arc::new(crate::risk::BasicToolRiskAnalyzer::default()),
    );

    let thread_id = control
        .spawn(
            "session-overflow-recovery".into(),
            fast_retry_config(),
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("hello".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");
    let status = run_to_terminal(&control, &store, &thread_id).await;
    assert_eq!(status, ThreadStatus::Completed, "overflow + compact + retry must complete");
    assert_eq!(*llm.calls.lock().unwrap(), 2);
}

// ── S3: queued steering ─────────────────────────────────────────────────────

/// Requests a slow echo tool call on the first turn, a final answer after;
/// captures the messages of every LLM call.
struct SteeringCaptureLlm {
    calls: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
}

#[async_trait]
impl LlmPort for SteeringCaptureLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let call_index = {
            let mut calls = self.calls.lock().unwrap();
            calls.push(messages.to_vec());
            calls.len()
        };
        if call_index == 1 {
            return Ok(LlmResponse {
                content: None,
                content_already_streamed: false,
                tool_calls: vec![ParsedToolCall {
                    id: "call-1".into(),
                    name: "echo".into(),
                    arguments: r#"{"message":"slow","delay_ms":300}"#.into(),
                }],
                finish_reason: Some("tool_calls".into()),
                usage: None,
            });
        }
        Ok(LlmResponse {
            content: Some("done after steering".into()),
            content_already_streamed: false,
            tool_calls: Vec::new(),
            finish_reason: Some("stop".into()),
            usage: None,
        })
    }
}

async fn wait_for_item_started(notify: &RecordingNotify, item_id: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let started =
                notify.events.lock().unwrap().iter().any(
                    |event| matches!(event, EventMsg::ItemStarted(p) if p.item.id() == item_id),
                );
            if started {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("item never started");
}

fn user_text_message(text: &str) -> ConversationMessage {
    ConversationMessage {
        role: "user".into(),
        content: ConversationMessageContent::Text(text.into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }
}

#[tokio::test]
async fn queued_input_extends_the_run_at_the_iteration_boundary() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let llm = Arc::new(SteeringCaptureLlm { calls: calls.clone() });
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let router = ToolRouter::new();
    router.register(Box::new(DelayEchoTool));

    let approval: Arc<dyn ApprovalPort> = notify.clone();
    let control = Arc::new(AgentControl::new(
        llm,
        store_port,
        notify.clone(),
        approval,
        Arc::new(router),
        8,
        4,
    ));

    let thread_id = control
        .spawn(
            "session-steering".into(),
            AgentConfig { model: "mock".into(), max_turns: 4, ..AgentConfig::default() },
            vec![user_text_message("first request")],
        )
        .await
        .expect("spawn");

    // Hold until the tool is genuinely in flight, then steer.
    wait_for_item_started(&notify, "call-1").await;
    let outcome = control
        .queue_input(&thread_id, user_text_message("steered mid-run"))
        .await
        .expect("queue input on a live thread");
    assert!(
        matches!(outcome, crate::SendOutcome::Queued { position: 1 }),
        "expected Queued, got {outcome:?}"
    );

    let status = run_to_terminal(&control, &store, &thread_id).await;
    assert_eq!(status, ThreadStatus::Completed);

    // The second LLM call saw the steered user input (drained after the tool
    // batch, before the next iteration).
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2, "run must extend past the tool turn");
    let second = message_texts(&calls[1]);
    assert!(
        second.iter().any(|text| text.contains("steered mid-run")),
        "second LLM call must carry the steered input: {second:?}"
    );
    assert!(
        second.iter().any(|text| text.contains("first request")),
        "original input must still be present: {second:?}"
    );

    // Both user messages are persisted (MessageAppended).
    let emitted = notify.emitted_messages(&thread_id);
    assert!(emitted.iter().any(|m| m.rendered_text().contains("steered mid-run")));
    assert!(emitted.iter().any(|m| m.rendered_text().contains("first request")));
}

#[tokio::test]
async fn queued_input_on_idle_thread_signals_needs_resume() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new(llm, store_port, notify, approval, router, 8, 4));

    // Spawn-and-complete so the thread exists but is idle.
    let thread_id = control
        .spawn(
            "session-steering-idle".into(),
            AgentConfig { model: "mock".into(), max_turns: 3, ..AgentConfig::default() },
            vec![user_text_message("use tool")],
        )
        .await
        .expect("spawn");
    run_to_terminal(&control, &store, &thread_id).await;

    let outcome = control
        .queue_input(&thread_id, user_text_message("next"))
        .await
        .expect("queue input probe");
    assert_eq!(outcome, crate::SendOutcome::NeedsResume);
}

#[tokio::test]
async fn interrupt_persists_undelivered_queued_input() {
    let llm = Arc::new(DelayedToolCallLlm);
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let router = ToolRouter::new();
    router.register(Box::new(DelayEchoTool));

    let approval: Arc<dyn ApprovalPort> = notify.clone();
    let control = Arc::new(AgentControl::new(
        llm,
        store_port,
        notify.clone(),
        approval,
        Arc::new(router),
        8,
        4,
    ));

    let thread_id = control
        .spawn(
            "session-steering-interrupt".into(),
            AgentConfig { model: "mock".into(), max_turns: 3, ..AgentConfig::default() },
            vec![user_text_message("slow work")],
        )
        .await
        .expect("spawn");

    wait_for_item_started(&notify, "call-1").await;
    control
        .queue_input(&thread_id, user_text_message("queued but interrupted"))
        .await
        .expect("queue");
    control.interrupt(&thread_id).await.expect("interrupt");
    run_to_terminal(&control, &store, &thread_id).await;

    // Undelivered steering input is persisted into the history — the next
    // resume (and rollout replay) sees it instead of silently dropping it.
    let emitted = notify.emitted_messages(&thread_id);
    assert!(
        emitted.iter().any(|m| m.rendered_text().contains("queued but interrupted")),
        "queued input must be persisted on interrupt, got: {:?}",
        emitted.iter().map(ConversationMessage::rendered_text).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn max_turns_exhaustion_is_interrupted_with_reason_not_completed() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4));

    let config = AgentConfig { model: "mock".into(), max_turns: 1, ..AgentConfig::default() };
    let thread_id = control
        .spawn(
            "session-max-turns".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("use the tool once".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Interrupted).await;
    let snapshot =
        store.get_thread(&thread_id).await.expect("load snapshot").expect("snapshot should exist");

    assert_eq!(snapshot.status, ThreadStatus::Interrupted);
    assert_eq!(snapshot.completion_text.as_deref(), Some("max_turns_reached"));
    assert_eq!(control.active_thread_count().await, 0);
    assert!(
        resume_with_input(&store, &control, &thread_id, "continue".into()).await.is_ok(),
        "max-turns interrupted threads should remain resumable"
    );
}

#[tokio::test]
async fn repeated_side_effect_tool_call_interrupts_with_reason_and_trace_event() {
    let llm = Arc::new(RepeatingToolCallLlm::new(
        "write_file",
        r#"{"content":"same","path":"notes.txt"}"#,
    ));
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(JsonNoopTool { name: "write_file" }));
    let trace = Arc::new(RecordingTraceSink::default());
    let trace_sink: Arc<dyn AgentTraceSink> = trace.clone();

    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new_with_hooks_and_tracing(
        llm,
        store_port,
        notify.clone(),
        approval,
        Arc::new(router),
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        Vec::new(),
        trace_sink,
        None,
    ));

    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };
    let thread_id = control
        .spawn(
            "session-repetition".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("repeat the write".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Interrupted).await;
    let snapshot =
        store.get_thread(&thread_id).await.expect("load snapshot").expect("snapshot should exist");

    assert_eq!(snapshot.status, ThreadStatus::Interrupted);
    assert_eq!(snapshot.completion_text.as_deref(), Some("repetition_detected"));
    assert_eq!(control.active_thread_count().await, 0);
    assert!(
        resume_with_input(&store, &control, &thread_id, "continue".into()).await.is_ok(),
        "repetition-interrupted threads should remain resumable"
    );

    let trace_events = trace.events.lock().unwrap().clone();
    assert_trace_event(&trace_events, "loop_detected");
    let loop_event = trace_events
        .iter()
        .find(|(_context, event)| event.event == "loop_detected")
        .expect("loop_detected event");
    assert_eq!(loop_event.1.payload["hit_count"], 3);
    assert!(loop_event.1.payload["signature_hash"].as_str().is_some());
    assert_trace_event(&trace_events, "thread_repetition_detected");
}

#[tokio::test]
async fn repeated_read_only_tool_call_is_exempt_from_repetition_guard() {
    let llm = Arc::new(RepeatingToolCallLlm::new("read_file", r#"{"path":"notes.txt"}"#));
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(JsonNoopTool { name: "read_file" }));

    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4));

    let config = AgentConfig { model: "mock".into(), max_turns: 3, ..AgentConfig::default() };
    let thread_id = control
        .spawn(
            "session-readonly-repetition".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("keep reading".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Interrupted).await;
    let snapshot =
        store.get_thread(&thread_id).await.expect("load snapshot").expect("snapshot should exist");

    assert_eq!(snapshot.status, ThreadStatus::Interrupted);
    assert_eq!(snapshot.completion_text.as_deref(), Some("max_turns_reached"));
}

#[tokio::test]
async fn token_budget_exhaustion_interrupts_with_reason_and_keeps_thread_resumable() {
    let llm = Arc::new(BudgetedToolCallLlm);
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4));

    let config = AgentConfig {
        model: "mock".into(),
        max_turns: 5,
        token_budget: Some(7),
        ..AgentConfig::default()
    };
    let thread_id = control
        .spawn(
            "session-token-budget".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("use tokens".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Interrupted).await;
    let snapshot =
        store.get_thread(&thread_id).await.expect("load snapshot").expect("snapshot should exist");

    assert_eq!(snapshot.status, ThreadStatus::Interrupted);
    assert_eq!(snapshot.completion_text.as_deref(), Some("budget_exhausted"));
    assert_eq!(control.active_thread_count().await, 0);
    assert!(
        resume_with_input(&store, &control, &thread_id, "continue".into()).await.is_ok(),
        "budget-interrupted threads should remain resumable"
    );
}

#[tokio::test]
async fn token_budget_exhaustion_interrupts_before_executing_tool_calls() {
    let llm = Arc::new(BudgetedToolCallLlm);
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    let executions = Arc::new(Mutex::new(0));
    router.register(Box::new(CountingEchoTool { executions: Arc::clone(&executions) }));

    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4));

    let config = AgentConfig {
        model: "mock".into(),
        max_turns: 5,
        token_budget: Some(7),
        ..AgentConfig::default()
    };
    let thread_id = control
        .spawn(
            "session-token-budget-tool-gate".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("use expensive tool".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");

    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Interrupted).await;
    let snapshot =
        store.get_thread(&thread_id).await.expect("load snapshot").expect("snapshot should exist");

    assert_eq!(snapshot.completion_text.as_deref(), Some("budget_exhausted"));
    assert_eq!(*executions.lock().unwrap(), 0);
}

#[tokio::test]
async fn high_risk_tool_calls_require_approval_even_without_tool_metadata() {
    let llm = Arc::new(SecretToolCallLlm::new());
    let store = Arc::new(RecordingPersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(SecretTool { executions: Arc::new(Mutex::new(0)) }));

    let control = AgentControl::new_with_ports(
        llm,
        store_port,
        notify,
        Arc::new(RejectingApproval),
        Arc::new(router),
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        Arc::new(SlidingWindowCompactPort::default()),
        Arc::new(HighRiskToolAnalyzer),
    )
    .with_exec_policy(Arc::new(AskAllExecPolicy));

    let config = AgentConfig { model: "mock".into(), max_turns: 2, ..AgentConfig::default() };
    let thread_id = control
        .spawn(
            "session-risk-approval".into(),
            config,
            vec![ConversationMessage {
                role: "user".into(),
                content: ConversationMessageContent::Text("try the secret tool".into()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
        )
        .await
        .expect("spawn");
    let snapshot = control
        .wait_for_terminal_snapshot(&thread_id)
        .await
        .expect("terminal snapshot should be available");

    assert_eq!(snapshot.status, ThreadStatus::Completed);
}

#[tokio::test]
async fn shutdown_prevents_follow_up_input() {
    let llm = Arc::new(SlowLlm);
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());

    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new(llm, store_port, notify, approval, router, 8, 4));

    let config = AgentConfig { model: "mock".into(), max_turns: 1, ..AgentConfig::default() };
    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("slow".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];

    let thread_id =
        control.spawn("session-shutdown".into(), config, messages).await.expect("spawn");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Running).await;
    control.shutdown(&thread_id).await.expect("shutdown");

    let result = resume_with_input(&store, &control, &thread_id, "continue".into()).await;
    assert!(
        matches!(result, Err(AgentError::ThreadNotResumable { .. })),
        "shutdown thread should reject follow-up input"
    );
}

// ── Thread lifecycle tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn agent_control_shutdown_nonexistent_thread_fails() {
    let llm = Arc::new(MockLlm::new());
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());

    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new(llm, store, notify, approval, router, 8, 4));

    let result = control.shutdown("nonexistent-thread").await;
    assert!(
        matches!(result, Err(AgentError::ThreadNotFound(_))),
        "shutdown of nonexistent thread should fail"
    );
}

#[tokio::test]
async fn agent_control_subscribe_to_nonexistent_thread_fails() {
    let llm = Arc::new(MockLlm::new());
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let notify = Arc::new(NoopNotify);
    let router = Arc::new(ToolRouter::new());

    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new(llm, store, notify, approval, router, 8, 4));

    let result = control.subscribe("nonexistent-thread").await;
    assert!(
        matches!(result, Err(AgentError::ThreadNotFound(_))),
        "subscribe to nonexistent thread should fail"
    );
}

// ── Context-budget system: per-turn token accounting + central result net ─────

/// Tool that returns a large payload, exercising the dispatch-layer bounds.
struct BigOutputTool {
    name: &'static str,
    payload_bytes: usize,
}

#[async_trait]
impl ToolHandler for BigOutputTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        "Returns a large fixed payload for context-budget tests."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object" })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        _arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        let payload = "m".repeat(self.payload_bytes);
        Ok(ToolOutput { content: payload, metadata: None })
    }
}

/// First call emits a tool request (with real usage numbers); every later
/// call returns a final answer.
struct OneToolCallWithUsageLlm {
    tool_name: &'static str,
    call_count: Mutex<u32>,
}

#[async_trait]
impl LlmPort for OneToolCallWithUsageLlm {
    async fn chat_completion(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        let call_index = {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            *count
        };
        if call_index > 1 {
            return Ok(LlmResponse {
                content: Some("done".into()),
                content_already_streamed: false,
                tool_calls: vec![],
                finish_reason: Some("stop".into()),
                usage: Some(LlmUsage {
                    prompt_tokens: 130,
                    completion_tokens: 5,
                    total_tokens: 135,
                    estimated: false,
                }),
            });
        }
        Ok(LlmResponse {
            content: None,
            content_already_streamed: false,
            tool_calls: vec![ParsedToolCall {
                id: "call-big".into(),
                name: self.tool_name.to_owned(),
                arguments: "{}".into(),
            }],
            finish_reason: Some("tool_calls".into()),
            usage: Some(LlmUsage {
                prompt_tokens: 100,
                completion_tokens: 20,
                total_tokens: 120,
                estimated: false,
            }),
        })
    }
}

#[tokio::test]
async fn turn_token_accounting_breaks_down_by_role_and_tool_and_reports_usage() {
    let trace = Arc::new(RecordingTraceSink::default());
    let llm = Arc::new(OneToolCallWithUsageLlm { tool_name: "bigdata", call_count: Mutex::new(0) });
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(BigOutputTool { name: "bigdata", payload_bytes: 5 * 1024 }));
    let control = Arc::new(AgentControl::new_with_hooks_and_tracing(
        llm,
        store_port,
        notify.clone(),
        notify,
        Arc::new(router),
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        Vec::new(),
        trace.clone(),
        None,
    ));

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: ConversationMessageContent::Text("run bigdata".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };
    let thread_id = control.spawn("accounting".into(), config, messages).await.expect("spawn");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    let events = trace.events.lock().unwrap();
    let accounting: Vec<&AgentTraceEvent> = events
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.event == "turn_token_accounting")
        .collect();
    assert!(accounting.len() >= 2, "expected accounting on tool and final turns");

    // The post-tool accounting line describes the tool result it will re-send.
    let with_tool = accounting
        .iter()
        .find(|event| event.payload["by_tool"]["bigdata"]["messages"].as_u64() == Some(1))
        .expect("accounting line attributing tokens to the bigdata tool");
    assert!(
        with_tool.payload["by_tool"]["bigdata"]["bytes"].as_u64().unwrap_or(0) >= 5_000,
        "tool bytes accounted: {}",
        with_tool.payload
    );
    assert!(with_tool.payload["by_segment"]["tool"].as_u64().unwrap_or(0) > 0);
    assert!(with_tool.payload["by_segment"]["user"].as_u64().unwrap_or(0) > 0);
    assert!(with_tool.payload["estimated_total_tokens"].as_u64().unwrap_or(0) > 0);
    assert_eq!(with_tool.payload["model"], "mock");
    // Actual provider usage lands on the line.
    assert_eq!(with_tool.payload["usage"]["total_tokens"], 120);
    assert!(!with_tool.payload["largest_tool_results"].as_array().expect("largest").is_empty());
}

/// The dispatch-layer net: a 100KB tool result enters history bounded, with
/// the original size recorded on the trace line.
#[tokio::test]
async fn oversized_tool_output_is_bounded_before_entering_history() {
    let trace = Arc::new(RecordingTraceSink::default());
    let llm =
        Arc::new(OneToolCallWithUsageLlm { tool_name: "firehose", call_count: Mutex::new(0) });
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());
    let events_notify = Arc::clone(&notify);
    let router = ToolRouter::new();
    router.register(Box::new(BigOutputTool { name: "firehose", payload_bytes: 100 * 1024 }));
    let control = Arc::new(AgentControl::new_with_hooks_and_tracing(
        llm,
        store_port,
        notify.clone(),
        notify,
        Arc::new(router),
        AgentControlLimits { max_threads: 8, max_depth: 4 },
        Vec::new(),
        trace.clone(),
        None,
    ));

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: ConversationMessageContent::Text("run firehose".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };
    let thread_id = control.spawn("bounded".into(), config, messages).await.expect("spawn");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    // Emitted history: the tool message is bounded with a marker.
    let tool_messages: Vec<String> = events_notify
        .emitted_messages(&thread_id)
        .into_iter()
        .filter(|message| message.role == "tool")
        .filter_map(|message| match message.content {
            ConversationMessageContent::Text(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(tool_messages.len(), 1, "one tool result persisted");
    let bounded = &tool_messages[0];
    assert!(bounded.len() < 64 * 1024 + 128, "tool result not bounded: {}", bounded.len());
    assert!(bounded.contains("bytes omitted"), "marker missing");
    assert!(bounded.starts_with('m'), "head survived");

    // Trace line records the true size and the truncation.
    let events = trace.events.lock().unwrap();
    let output_event = events
        .iter()
        .map(|(_, event)| event)
        .find(|event| event.event == "tool_call_output" && event.payload["tool_name"] == "firehose")
        .expect("tool_call_output trace line");
    assert_eq!(output_event.payload["output_bytes"], 100 * 1024);
    assert_eq!(output_event.payload["output_truncated"], true);
}
