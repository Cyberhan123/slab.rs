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
    sync::{Arc, Mutex},
};

use crate::{
    AgentControl, AgentControlLimits, AgentError, AgentHook, AgentThreadContext, HookEvent,
    HookOutcome, PlanRef, ToolApprovalRequest, ToolContext, ToolHandler, ToolOutput, ToolRouter,
    WorkspaceRef,
    compact::{CompactPort, SlidingWindowCompactPort},
    config::{AgentConfig, AgentToolChoice},
    event::AgentEventKind,
    port::{
        AgentNotifyPort, AgentStorePort, ApprovalDecision, ApprovalPort, LlmPort, LlmResponse,
        LlmStreamObserver, LlmUsage, ParsedToolCall, ThreadMessageRecord, ThreadSnapshot,
        ThreadStatus, ToolCallRecord, ToolSpec, TurnEvent, TurnStateRecord,
    },
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

    fn approval_request(&self, arguments: &serde_json::Value) -> Option<ToolApprovalRequest> {
        Some(ToolApprovalRequest {
            command: arguments
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
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

struct StreamingLlm;

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
        Ok(LlmResponse {
            content: Some("hello".into()),
            content_already_streamed: false,
            tool_calls: Vec::new(),
            finish_reason: Some("stop".into()),
            usage: None,
        })
    }

    async fn chat_completion_streaming(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
        observer: &mut dyn LlmStreamObserver,
    ) -> Result<LlmResponse, AgentError> {
        observer.on_text_delta("hel").await?;
        observer.on_reasoning_delta("thinking").await?;
        observer.on_reasoning_done("thinking").await?;
        observer.on_text_delta("lo").await?;
        Ok(LlmResponse {
            content: Some("hello".into()),
            content_already_streamed: true,
            tool_calls: Vec::new(),
            finish_reason: Some("stop".into()),
            usage: None,
        })
    }
}

struct StreamingToolCallLlm {
    call_count: Mutex<u32>,
}

impl StreamingToolCallLlm {
    fn new() -> Self {
        Self { call_count: Mutex::new(0) }
    }
}

#[async_trait]
impl LlmPort for StreamingToolCallLlm {
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
            tool_calls: Vec::new(),
            finish_reason: Some("stop".into()),
            usage: None,
        })
    }

    async fn chat_completion_streaming(
        &self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _config: &AgentConfig,
        _trace_context: &AgentTraceContext,
        observer: &mut dyn LlmStreamObserver,
    ) -> Result<LlmResponse, AgentError> {
        let next_call = {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            *count
        };

        if next_call == 1 {
            observer.on_text_delta("checking ").await?;
            Ok(LlmResponse {
                content: Some("checking ".into()),
                content_already_streamed: true,
                tool_calls: vec![ParsedToolCall {
                    id: "call-1".into(),
                    name: "echo".into(),
                    arguments: r#"{"message":"hello"}"#.into(),
                }],
                finish_reason: Some("tool_calls".into()),
                usage: None,
            })
        } else {
            observer.on_text_delta("done").await?;
            Ok(LlmResponse {
                content: Some("done".into()),
                content_already_streamed: true,
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

    async fn insert_thread_message(&self, _record: &ThreadMessageRecord) -> Result<(), AgentError> {
        Ok(())
    }

    async fn list_thread_messages(
        &self,
        _thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AgentError> {
        Ok(Vec::new())
    }
}

#[derive(Default)]
struct RecordingStore {
    inserted_statuses: Mutex<Vec<slab_types::agent::ToolCallStatus>>,
    updated_statuses: Mutex<Vec<slab_types::agent::ToolCallStatus>>,
}

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

    async fn insert_tool_call(&self, record: &ToolCallRecord) -> Result<(), AgentError> {
        self.inserted_statuses.lock().unwrap().push(record.status);
        Ok(())
    }

    async fn update_tool_call_status(
        &self,
        _id: &str,
        status: slab_types::agent::ToolCallStatus,
    ) -> Result<(), AgentError> {
        self.updated_statuses.lock().unwrap().push(status);
        Ok(())
    }

    async fn update_tool_call(
        &self,
        _id: &str,
        _output: Option<&str>,
        status: slab_types::agent::ToolCallStatus,
        _completed_at: &str,
    ) -> Result<(), AgentError> {
        self.updated_statuses.lock().unwrap().push(status);
        Ok(())
    }

    async fn insert_thread_message(&self, _record: &ThreadMessageRecord) -> Result<(), AgentError> {
        Ok(())
    }

    async fn list_thread_messages(
        &self,
        _thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AgentError> {
        Ok(Vec::new())
    }
}

#[derive(Default)]
struct RecordingPersistingStore {
    snapshots: Mutex<HashMap<String, ThreadSnapshot>>,
    messages: Mutex<Vec<ThreadMessageRecord>>,
    inserted_statuses: Mutex<Vec<slab_types::agent::ToolCallStatus>>,
    updated_statuses: Mutex<Vec<slab_types::agent::ToolCallStatus>>,
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

    async fn insert_tool_call(&self, record: &ToolCallRecord) -> Result<(), AgentError> {
        self.inserted_statuses.lock().unwrap().push(record.status);
        Ok(())
    }

    async fn update_tool_call_status(
        &self,
        _id: &str,
        status: slab_types::agent::ToolCallStatus,
    ) -> Result<(), AgentError> {
        self.updated_statuses.lock().unwrap().push(status);
        Ok(())
    }

    async fn update_tool_call(
        &self,
        _id: &str,
        _output: Option<&str>,
        status: slab_types::agent::ToolCallStatus,
        _completed_at: &str,
    ) -> Result<(), AgentError> {
        self.updated_statuses.lock().unwrap().push(status);
        Ok(())
    }

    async fn insert_thread_message(&self, record: &ThreadMessageRecord) -> Result<(), AgentError> {
        self.messages.lock().unwrap().push(record.clone());
        Ok(())
    }

    async fn list_thread_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AgentError> {
        Ok(self
            .messages
            .lock()
            .unwrap()
            .iter()
            .filter(|record| record.thread_id == thread_id)
            .cloned()
            .collect())
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

    async fn insert_thread_message(&self, record: &ThreadMessageRecord) -> Result<(), AgentError> {
        self.messages.lock().unwrap().push(record.clone());
        Ok(())
    }

    async fn list_thread_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AgentError> {
        Ok(self
            .messages
            .lock()
            .unwrap()
            .iter()
            .filter(|record| record.thread_id == thread_id)
            .cloned()
            .collect())
    }

    async fn upsert_turn_state(&self, record: &TurnStateRecord) -> Result<(), AgentError> {
        self.turn_states.lock().unwrap().push(record.clone());
        Ok(())
    }
}

// ── Mock notify ───────────────────────────────────────────────────────────────

struct NoopNotify;

#[async_trait]
impl AgentNotifyPort for NoopNotify {
    async fn on_status_change(&self, _thread_id: &str, _status: ThreadStatus) {}
}

#[derive(Default)]
struct RecordingNotify {
    events: Mutex<Vec<TurnEvent>>,
}

#[async_trait]
impl AgentNotifyPort for RecordingNotify {
    async fn on_status_change(&self, _thread_id: &str, status: ThreadStatus) {
        self.events.lock().unwrap().push(TurnEvent::Response {
            turn_index: None,
            event: AgentEventKind::AgentStatus { status },
        });
    }

    async fn on_turn_event(&self, _thread_id: &str, event: &TurnEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

#[async_trait]
impl ApprovalPort for RecordingNotify {
    async fn request_approval(
        &self,
        _thread_id: &str,
        _call_id: &str,
        _tool_name: &str,
        _command: &str,
        _risk: Option<crate::ToolRiskAssessment>,
    ) -> ApprovalDecision {
        ApprovalDecision::Approved
    }
}

#[async_trait]
impl ApprovalPort for NoopNotify {
    async fn request_approval(
        &self,
        _thread_id: &str,
        _call_id: &str,
        _tool_name: &str,
        _command: &str,
        _risk: Option<crate::ToolRiskAssessment>,
    ) -> ApprovalDecision {
        ApprovalDecision::Approved
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
        _command: &str,
        _risk: Option<crate::ToolRiskAssessment>,
    ) -> ApprovalDecision {
        ApprovalDecision::Rejected
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

async fn wait_for_persisted_message(store: &PersistingStore, thread_id: &str, text: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let found = store.messages.lock().unwrap().iter().any(|record| {
                record.thread_id == thread_id && record.message.rendered_text().contains(text)
            });
            if found {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("persisted message did not appear");
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

    let messages = store.messages.lock().unwrap();
    let tool_output = messages
        .iter()
        .find(|record| record.thread_id == thread_id && record.message.role == "tool")
        .expect("tool message")
        .message
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
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4));
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

    let statuses = store
        .turn_states
        .lock()
        .unwrap()
        .iter()
        .filter(|record| record.thread_id == thread_id)
        .map(|record| record.status.clone())
        .collect::<Vec<_>>();
    assert!(statuses.contains(&"running".to_owned()));
    assert!(statuses.contains(&"llm_completed".to_owned()));
    assert!(statuses.contains(&"tool_calls_completed".to_owned()));
    assert!(statuses.contains(&"completed".to_owned()));
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

    let approval = notify.clone();
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4));
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
    wait_for_persisted_message(&store, &thread_id, "invalid tool call: tool not allowed: secret")
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
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(DelayEchoTool));

    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4));
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
    let tool_outputs = store
        .messages
        .lock()
        .unwrap()
        .iter()
        .filter(|record| record.thread_id == thread_id && record.message.role == "tool")
        .map(|record| match &record.message.content {
            ConversationMessageContent::Text(text) => text.clone(),
            ConversationMessageContent::Parts(_) => record.message.rendered_text(),
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

    let outcome = compact.compact(&messages).await.expect("compact");
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
async fn approval_required_tool_is_recorded_pending_then_completed() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(RecordingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);

    let router = ToolRouter::new();
    router.register(Box::new(ApprovalEchoTool));

    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4));

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
    assert_eq!(
        store.inserted_statuses.lock().unwrap().as_slice(),
        &[slab_types::agent::ToolCallStatus::Pending]
    );
    assert_eq!(
        store.updated_statuses.lock().unwrap().as_slice(),
        &[slab_types::agent::ToolCallStatus::Running, slab_types::agent::ToolCallStatus::Completed,]
    );
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
        control.spawn("session-approval-rejected".into(), config, messages).await.expect("spawn");
    let final_status = wait_for_control_terminal_status(&control, &thread_id).await;

    assert_eq!(final_status, ThreadStatus::Completed);
    assert_eq!(
        store.inserted_statuses.lock().unwrap().as_slice(),
        &[slab_types::agent::ToolCallStatus::Pending]
    );
    assert_eq!(
        store.updated_statuses.lock().unwrap().as_slice(),
        &[slab_types::agent::ToolCallStatus::Failed]
    );
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
    assert_eq!(
        store.inserted_statuses.lock().unwrap().as_slice(),
        &[slab_types::agent::ToolCallStatus::Running]
    );
    assert_eq!(
        store.updated_statuses.lock().unwrap().as_slice(),
        &[slab_types::agent::ToolCallStatus::Failed]
    );
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
    assert_eq!(
        store.inserted_statuses.lock().unwrap().as_slice(),
        &[slab_types::agent::ToolCallStatus::Running]
    );
    assert_eq!(
        store.updated_statuses.lock().unwrap().as_slice(),
        &[slab_types::agent::ToolCallStatus::Failed]
    );
}

#[tokio::test]
async fn response_style_events_include_text_tool_and_metrics() {
    let llm = Arc::new(MockLlm::new());
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let notify = Arc::new(RecordingNotify::default());

    let router = ToolRouter::new();
    router.register(Box::new(ApprovalEchoTool));

    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store, notify.clone(), approval, Arc::new(router), 8, 4));

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("Please echo".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };

    let thread_id = control.spawn("session-events".into(), config, messages).await.expect("spawn");
    let mut status_rx = control.subscribe(&thread_id).await.expect("subscribe");
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            status_rx.changed().await.expect("status channel closed");
            if *status_rx.borrow() == ThreadStatus::Completed {
                return;
            }
        }
    })
    .await
    .expect("thread should complete");

    let events = notify.events.lock().unwrap();
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnEvent::Response {
                event: AgentEventKind::ResponseFunctionCallArgumentsDone { risk: Some(_), .. },
                ..
            }
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnEvent::Response { event: AgentEventKind::ResponseOutputTextDone { .. }, .. }
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(event, TurnEvent::Response { event: AgentEventKind::ResponseMetrics { .. }, .. })
    }));
}

#[tokio::test]
async fn streaming_llm_deltas_arrive_before_text_done() {
    let llm = Arc::new(StreamingLlm);
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let notify = Arc::new(RecordingNotify::default());
    let router = Arc::new(ToolRouter::new());

    let approval = Arc::clone(&notify);
    let control = Arc::new(AgentControl::new(llm, store, notify.clone(), approval, router, 8, 4));

    let messages = vec![ConversationMessage {
        role: "user".into(),
        content: slab_types::ConversationMessageContent::Text("Say hello".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 1, ..AgentConfig::default() };

    let thread_id =
        control.spawn("session-streaming".into(), config, messages).await.expect("spawn");
    let mut status_rx = control.subscribe(&thread_id).await.expect("subscribe");
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            status_rx.changed().await.expect("status channel closed");
            if *status_rx.borrow() == ThreadStatus::Completed {
                return;
            }
        }
    })
    .await
    .expect("thread should complete");

    let events = notify.events.lock().unwrap();
    let first_delta = events
        .iter()
        .position(|event| {
            matches!(
                event,
                TurnEvent::Response {
                    event: AgentEventKind::ResponseOutputTextDelta { delta, .. },
                    ..
                } if delta == "hel"
            )
        })
        .expect("first text delta");
    let done = events
        .iter()
        .position(|event| {
            matches!(
                event,
                TurnEvent::Response {
                    event: AgentEventKind::ResponseOutputTextDone { text, .. },
                    ..
                } if text == "hello"
            )
        })
        .expect("text done");
    let reasoning_delta = events
        .iter()
        .position(|event| {
            matches!(
                event,
                TurnEvent::Response {
                    event: AgentEventKind::ResponseReasoningTextDelta { delta, .. },
                    ..
                } if delta == "thinking"
            )
        })
        .expect("reasoning delta");
    let reasoning_done = events
        .iter()
        .position(|event| {
            matches!(
                event,
                TurnEvent::Response {
                    event: AgentEventKind::ResponseReasoningTextDone { text, .. },
                    ..
                } if text == "thinking"
            )
        })
        .expect("reasoning done");

    assert!(first_delta < done);
    assert!(reasoning_delta < reasoning_done);
    assert!(reasoning_done < done);
}

#[tokio::test]
async fn streaming_tool_call_emits_text_before_function_call_without_duplicate_delta() {
    let llm = Arc::new(StreamingToolCallLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(RecordingNotify::default());

    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&notify);
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
        content: slab_types::ConversationMessageContent::Text("Please echo".into()),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let config = AgentConfig { model: "mock".into(), max_turns: 5, ..AgentConfig::default() };

    let thread_id =
        control.spawn("session-streaming-tool".into(), config, messages).await.expect("spawn");
    wait_for_persisted_status(&store, &thread_id, ThreadStatus::Completed).await;

    let events = notify.events.lock().unwrap();
    let text_delta_positions = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            TurnEvent::Response {
                event: AgentEventKind::ResponseOutputTextDelta { delta, .. },
                ..
            } if delta == "checking " => Some(index),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(text_delta_positions.len(), 1);

    let function_call_position = events
        .iter()
        .position(|event| {
            matches!(
                event,
                TurnEvent::Response {
                    event: AgentEventKind::ResponseFunctionCallArgumentsDone {
                        item_id,
                        name,
                        arguments,
                        ..
                    },
                    ..
                } if item_id == "call-1" && name == "echo" && arguments == r#"{"message":"hello"}"#
            )
        })
        .expect("function call event");
    assert!(text_delta_positions[0] < function_call_position);
    drop(events);

    let records = store.messages.lock().unwrap();
    let debug_records = records
        .iter()
        .map(|record| {
            format!(
                "{}:{}:{}:{:?}:{}",
                record.thread_id,
                record.turn_index,
                record.message.role,
                record.message.tool_call_id,
                record.message.rendered_text()
            )
        })
        .collect::<Vec<_>>();
    assert!(
        records.iter().any(|record| {
            record.thread_id == thread_id
                && record.message.role == "tool"
                && record.message.tool_call_id.as_deref() == Some("call-1")
                && record.message.rendered_text().contains("hello")
        }),
        "{debug_records:#?}"
    );
}

#[tokio::test]
async fn send_input_replays_persisted_thread_messages() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(PersistingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);

    let router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4));

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

    control.send_input(&thread_id, "second prompt".into()).await.expect("send input");
    wait_for_persisted_message(&store, &thread_id, "second prompt").await;

    let records = store.messages.lock().unwrap();
    assert!(
        records
            .iter()
            .filter(|record| record.thread_id == thread_id)
            .any(|record| record.message.rendered_text().contains("first prompt"))
    );
    assert!(
        records
            .iter()
            .filter(|record| record.thread_id == thread_id)
            .any(|record| record.message.rendered_text().contains("second prompt"))
    );
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
    let notify = Arc::new(NoopNotify);
    let router = ToolRouter::new();
    router.register(Box::new(TaskCompleteMarkerTool::always_succeeds()));
    let approval = Arc::clone(&notify);
    let control =
        Arc::new(AgentControl::new(llm, store_port, notify, approval, Arc::new(router), 8, 4));
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

    let final_text = store
        .messages
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|record| record.thread_id == thread_id && record.message.role == "assistant")
        .and_then(|record| match &record.message.content {
            ConversationMessageContent::Text(text) => Some(text.clone()),
            _ => None,
        });
    assert_eq!(final_text.as_deref(), Some("shipped it"));
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
    let result = control.send_input(&thread_id, "continue".into()).await;
    assert!(result.is_ok(), "interrupted thread should accept follow-up input");
    let _ = control.shutdown(&thread_id).await;
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
        control.send_input(&thread_id, "continue".into()).await.is_ok(),
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
    let notify = Arc::new(RecordingNotify::default());
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
        control.send_input(&thread_id, "continue".into()).await.is_ok(),
        "repetition-interrupted threads should remain resumable"
    );

    let events = notify.events.lock().unwrap();
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnEvent::Response {
                event: AgentEventKind::ResponseCancelled { reason, .. },
                ..
            } if reason == "repetition_detected"
        )
    }));
    drop(events);

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
        control.send_input(&thread_id, "continue".into()).await.is_ok(),
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
    );

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
    assert_eq!(
        store.inserted_statuses.lock().unwrap().as_slice(),
        &[slab_types::agent::ToolCallStatus::Pending]
    );
    assert_eq!(
        store.updated_statuses.lock().unwrap().as_slice(),
        &[slab_types::agent::ToolCallStatus::Failed]
    );
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

    let result = control.send_input(&thread_id, "continue".into()).await;
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
