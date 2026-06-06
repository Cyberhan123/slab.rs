//! P6 smoke test: agent runs the echo tool and completes.
//!
//! This test exercises the entire agent loop in isolation, using a mock
//! [`LlmPort`] instead of a real model.  The mock:
//! 1. First call → returns a tool call to `echo` with `message = "hello"`.
//! 2. Second call (after the tool result is appended) → returns a plain-text
//!    final answer so the loop terminates.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    AgentControl, AgentControlLimits, AgentError, ToolApprovalRequest, ToolContext, ToolHandler,
    ToolOutput, ToolRouter,
    config::AgentConfig,
    event::AgentEventKind,
    port::{
        AgentNotifyPort, AgentStorePort, ApprovalDecision, ApprovalPort, LlmPort, LlmResponse,
        LlmStreamObserver, ParsedToolCall, ThreadMessageRecord, ThreadSnapshot, ThreadStatus,
        ToolCallRecord, ToolSpec, TurnEvent,
    },
};
use async_trait::async_trait;
use slab_agent_tracing::{AgentTraceContext, AgentTraceEvent, AgentTraceSink};
use slab_types::ConversationMessage;

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
            })
        } else {
            // Second turn: final text answer after receiving the tool result.
            Ok(LlmResponse {
                content: Some("echo completed: hello from agent".into()),
                content_already_streamed: false,
                tool_calls: vec![],
                finish_reason: Some("stop".into()),
            })
        }
    }
}

// ── Mock store ────────────────────────────────────────────────────────────────

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
            })
        } else {
            observer.on_text_delta("done").await?;
            Ok(LlmResponse {
                content: Some("done".into()),
                content_already_streamed: true,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".into()),
            })
        }
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
struct PersistingStore {
    snapshots: Mutex<HashMap<String, ThreadSnapshot>>,
    messages: Mutex<Vec<ThreadMessageRecord>>,
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
async fn smoke_echo_tool_agent_completes() {
    // Wire up the agent control with the echo tool registered.
    let llm = Arc::new(MockLlm::new());
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let notify = Arc::new(NoopNotify);

    let mut router = ToolRouter::new();
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
    let mut router = ToolRouter::new();
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
async fn approval_required_tool_is_recorded_pending_then_completed() {
    let llm = Arc::new(MockLlm::new());
    let store = Arc::new(RecordingStore::default());
    let store_port: Arc<dyn AgentStorePort> = store.clone();
    let notify = Arc::new(NoopNotify);

    let mut router = ToolRouter::new();
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
        &[slab_types::agent::ToolCallStatus::Completed]
    );
}

#[tokio::test]
async fn response_style_events_include_text_tool_and_metrics() {
    let llm = Arc::new(MockLlm::new());
    let store: Arc<dyn AgentStorePort> = Arc::new(NoopStore);
    let notify = Arc::new(RecordingNotify::default());

    let mut router = ToolRouter::new();
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

    let mut router = ToolRouter::new();
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

    let mut router = ToolRouter::new();
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
async fn echo_tool_returns_input() {
    use crate::tool::{ToolContext, ToolHandler};

    let ctx = ToolContext { thread_id: "t1".into(), turn_index: 0, depth: 0 };
    let args = serde_json::json!({"message": "test message"});

    let output = TestEchoTool.execute(&ctx, &args).await.expect("echo should succeed");
    assert_eq!(output.content, "test message");
}

#[tokio::test]
async fn echo_tool_missing_message_returns_empty() {
    use crate::tool::{ToolContext, ToolHandler};

    let ctx = ToolContext { thread_id: "t1".into(), turn_index: 0, depth: 0 };
    let args = serde_json::json!({});

    let output = TestEchoTool.execute(&ctx, &args).await.expect("echo should succeed");
    assert_eq!(output.content, "");
}

// ── Tool router tests ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn tool_router_registers_and_retrieves_tools() {
    use crate::tool::ToolRouter;

    let mut router = ToolRouter::new();
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

    let mut router = ToolRouter::new();
    router.register(Box::new(CustomTool));

    let ctx = ToolContext { thread_id: "t1".into(), turn_index: 0, depth: 0 };
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

    let mut router = ToolRouter::new();
    router.register(Box::new(TestEchoTool));

    let specs = router.tool_specs();
    assert_eq!(specs.len(), 1, "should have one tool spec");
    assert_eq!(specs[0].name, "echo");
    assert!(!specs[0].description.is_empty());
}

// ── Thread limit enforcement tests ────────────────────────────────────────────────────

#[tokio::test]
async fn agent_control_enforces_thread_limit() {
    let llm = Arc::new(MockLlm::new());
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
