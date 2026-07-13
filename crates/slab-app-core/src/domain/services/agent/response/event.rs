//! OpenAI-Responses wire event vocabulary for the `/responses` surface.
//!
//! Slice C3 relocated these types out of `slab-agent` (the engine crate) into the
//! response layer (the `/responses` host): `slab-agent` now owns only its harness
//! protocol (`EventMsg`/`TurnItem`) and holds zero OpenAI-Responses wire types.
//! `AgentEventKind` is the event model the pure projections
//! ([`super::projection::build_response`]/[`super::stream::envelope_to_events`]) consume;
//! [`single_shot`](super::single_shot) synthesizes the envelopes locally from one LLM call.
//!
//! `ToolRiskAssessment` is imported back from `slab-agent` — it stays there because it is
//! the `ApprovalPort::request_approval` signature. The serde shape of every type here is
//! byte-identical to the pre-C3 definitions (the 44 openai_compat fixtures are the canary).

use serde::{Deserialize, Serialize};

use slab_agent::ToolRiskAssessment;
use slab_types::agent::AgentThreadStatus as ThreadStatus;

/// Slab-owned agent event payloads aligned with OpenAI Responses event names.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum AgentEventKind {
    #[serde(rename = "response.queued")]
    ResponseQueued { response: AgentResponseRef },
    #[serde(rename = "response.in_progress")]
    ResponseInProgress { response: AgentResponseRef },
    #[serde(rename = "response.completed")]
    ResponseCompleted { response: AgentResponseRef },
    #[serde(rename = "response.failed")]
    ResponseFailed {
        response: AgentResponseRef,
        error: String,
        /// OpenAI error `code` (e.g. `insufficient_quota`). Forwarded onto the
        /// canonical `response.error.code` and the standalone error event.
        #[serde(skip_serializing_if = "Option::is_none")]
        error_code: Option<String>,
        /// OpenAI error `type` (e.g. `insufficient_quota`, `server_error`).
        #[serde(skip_serializing_if = "Option::is_none")]
        error_type: Option<String>,
    },
    #[serde(rename = "response.cancelled")]
    ResponseCancelled { response: AgentResponseRef, reason: String },
    #[serde(rename = "response.output_text.delta")]
    ResponseOutputTextDelta {
        item_id: String,
        output_index: i32,
        content_index: i32,
        delta: String,
    },
    #[serde(rename = "response.output_text.done")]
    ResponseOutputTextDone {
        item_id: String,
        output_index: i32,
        content_index: i32,
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        artifact_refs: Vec<AgentArtifactRef>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        /// OpenAI `message.phase` discriminator (`commentary` / `final_answer`).
        #[serde(skip_serializing_if = "Option::is_none")]
        phase: Option<String>,
    },
    #[serde(rename = "response.reasoning_text.delta")]
    ResponseReasoningTextDelta {
        item_id: String,
        output_index: i32,
        content_index: i32,
        delta: String,
    },
    #[serde(rename = "response.reasoning_text.done")]
    ResponseReasoningTextDone {
        item_id: String,
        output_index: i32,
        content_index: i32,
        text: String,
        /// slab does not encrypt reasoning. This carries the reasoning
        /// content verbatim as an OpenAI wire-shape bridge
        /// (`reasoning.encrypted_content`). For local models it is
        /// semantically equivalent to the `reasoning_summary_text.delta`
        /// content; cloud reasoning with explicit text is bridged the same
        /// way — type/format conversion only, never encryption.
        #[serde(skip_serializing_if = "Option::is_none")]
        encrypted_content: Option<String>,
        /// Reasoning summary text. The adapter wraps this as the canonical
        /// `summary: [{ type: "summary_text", text }]` array on the reasoning
        /// output item.
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    #[serde(rename = "response.function_call_arguments.done")]
    ResponseFunctionCallArgumentsDone {
        item_id: String,
        call_id: String,
        name: String,
        output_index: i32,
        arguments: String,
        /// Optional namespace the function call is scoped to (echoed on the
        /// canonical `function_call.namespace` field when `Some`).
        #[serde(skip_serializing_if = "Option::is_none")]
        namespace: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        risk: Option<ToolRiskAssessment>,
    },
    #[serde(rename = "response.function_call_arguments.delta")]
    ResponseFunctionCallArgumentsDelta {
        item_id: String,
        call_id: String,
        name: String,
        output_index: i32,
        delta: String,
    },
    #[serde(rename = "response.custom_tool_call_input.delta")]
    ResponseCustomToolCallInputDelta {
        item_id: String,
        call_id: String,
        name: String,
        output_index: i32,
        delta: String,
    },
    #[serde(rename = "response.custom_tool_call_input.done")]
    ResponseCustomToolCallInputDone {
        item_id: String,
        call_id: String,
        name: String,
        output_index: i32,
        input: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        namespace: Option<String>,
    },
    #[serde(rename = "response.apply_patch_call.done")]
    ResponseApplyPatchCallDone {
        item_id: String,
        call_id: String,
        output_index: i32,
        /// One of `create_file`, `delete_file`, `update_file`.
        operation_type: String,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        diff: Option<String>,
    },
    #[serde(rename = "response.local_shell_call.done")]
    ResponseLocalShellCallDone {
        item_id: String,
        call_id: String,
        output_index: i32,
        command: Vec<String>,
        #[serde(default)]
        env: std::collections::HashMap<String, String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        working_directory: Option<String>,
    },
    #[serde(rename = "response.tool_call.output")]
    ResponseToolCallOutput {
        item_id: String,
        call_id: String,
        output: String,
        status: ToolExecutionStatus,
    },
    #[serde(rename = "response.tool_call.approval_required")]
    ResponseToolCallApprovalRequired {
        item_id: String,
        call_id: String,
        tool_name: String,
        command: String,
        category: slab_exec_policy::OperationCategory,
        #[serde(skip_serializing_if = "Option::is_none")]
        risk: Option<ToolRiskAssessment>,
    },
    #[serde(rename = "response.tool_call.approval_resolved")]
    ResponseToolCallApprovalResolved {
        item_id: String,
        call_id: String,
        tool_name: String,
        approved: bool,
    },
    #[serde(rename = "response.tool_call.validation_failed")]
    ResponseToolCallValidationFailed {
        item_id: String,
        call_id: String,
        tool_name: String,
        reason: String,
    },
    #[serde(rename = "response.tool_call.concurrency_started")]
    ResponseToolCallConcurrencyStarted { total: usize, concurrency: usize },
    #[serde(rename = "response.tool_call.concurrency_completed")]
    ResponseToolCallConcurrencyCompleted { total: usize, completed: usize, failed: usize },
    #[serde(rename = "response.context.compact_started")]
    ResponseContextCompactStarted { input_tokens: usize, threshold_tokens: usize },
    #[serde(rename = "response.context.compact_completed")]
    ResponseContextCompactCompleted {
        input_tokens: usize,
        output_tokens: usize,
        replaced_messages: usize,
    },
    #[serde(rename = "response.context.compact_skipped")]
    ResponseContextCompactSkipped { input_tokens: usize, threshold_tokens: usize, reason: String },
    /// Finalized compaction output item (`type: "compaction"`). slab does
    /// not encrypt compaction content. The `encrypted_content` field carries
    /// the compacted-context content verbatim as an OpenAI wire-shape bridge;
    /// type/format conversion only, never encryption. The adapter maps this
    /// to a `CompactionBody` output item.
    #[serde(rename = "response.compaction.done")]
    ResponseCompactionDone { item_id: String, output_index: i32, encrypted_content: String },
    /// Finalized file-search tool call (`type: "file_search_call"`).
    #[serde(rename = "response.file_search_call.done")]
    ResponseFileSearchCallDone {
        item_id: String,
        output_index: i32,
        queries: Vec<String>,
        /// Opaque result hits; `None` serializes as an absent field (the
        /// fixture's `null` is normalized away by the test redactor).
        #[serde(skip_serializing_if = "Option::is_none")]
        results: Option<Vec<serde_json::Value>>,
    },
    /// Finalized image-generation tool call (`type: "image_generation_call"`).
    #[serde(rename = "response.image_generation_call.done")]
    ResponseImageGenCallDone {
        item_id: String,
        output_index: i32,
        result: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        revised_prompt: Option<String>,
        background: String,
        output_format: String,
        quality: String,
        size: String,
    },
    /// Finalized tool-search call (`type: "tool_search_call"`).
    #[serde(rename = "response.tool_search_call.done")]
    ResponseToolSearchCallDone {
        item_id: String,
        output_index: i32,
        /// `server` or `client`.
        execution: String,
        /// `Some` for client execution (carries a `call_id`), `None` for server.
        #[serde(skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
        /// Free-form arguments object (e.g. `{ "paths": [...] }` or
        /// `{ "goal": "..." }`).
        arguments: serde_json::Value,
    },
    /// Finalized tool-search output (`type: "tool_search_output"`). Carries the
    /// resolved tool definitions the search discovered.
    #[serde(rename = "response.tool_search_output.done")]
    ResponseToolSearchOutputDone {
        item_id: String,
        output_index: i32,
        /// `server` or `client`.
        execution: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
        /// Resolved tool definitions the search discovered, carried as opaque
        /// JSON; the projection parses each into the typed `Tool` shape.
        tools: Vec<serde_json::Value>,
    },
    /// Finalized MCP list-tools call (`type: "mcp_list_tools"`). Carries the
    /// tools a server exposed as opaque JSON.
    #[serde(rename = "response.mcp_list_tools.done")]
    ResponseMcpListToolsDone {
        item_id: String,
        output_index: i32,
        server_label: String,
        tools: Vec<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// Finalized MCP tool call (`type: "mcp_tool_call"`).
    #[serde(rename = "response.mcp_call.done")]
    ResponseMcpCallDone {
        item_id: String,
        output_index: i32,
        server_label: String,
        name: String,
        arguments: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        approval_request_id: Option<String>,
    },
    /// Finalized MCP approval request (`type: "mcp_approval_request"`).
    #[serde(rename = "response.mcp_approval_request.done")]
    ResponseMcpApprovalRequestDone {
        item_id: String,
        output_index: i32,
        server_label: String,
        name: String,
        arguments: String,
    },
    /// Finalized code-interpreter tool call (`type: "code_interpreter_call"`).
    /// `outputs` is opaque JSON (e.g. `{ type: "logs", logs }`).
    #[serde(rename = "response.code_interpreter_call.done")]
    ResponseCodeInterpreterCallDone {
        item_id: String,
        output_index: i32,
        code: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        container_id: Option<String>,
        outputs: Vec<serde_json::Value>,
    },
    /// Finalized web-search tool call (`type: "web_search_call"`). `action` is
    /// opaque JSON (search / open_page / find_in_page).
    #[serde(rename = "response.web_search_call.done")]
    ResponseWebSearchCallDone { item_id: String, output_index: i32, action: serde_json::Value },
    /// Finalized function-shell tool call (`type: "shell_call"`). Distinct from
    /// `ResponseLocalShellCallDone` (`local_shell_call`) — the function-shell
    /// variant carries an `environment` discriminator (`local` /
    /// `container_reference`) and uses a `commands` array action shape.
    #[serde(rename = "response.function_shell_call.done")]
    ResponseFunctionShellCallDone {
        item_id: String,
        call_id: String,
        output_index: i32,
        commands: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_output_length: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<i32>,
        /// `None` = absent; `Some("local")` / `Some("container_reference")`.
        #[serde(skip_serializing_if = "Option::is_none")]
        environment_type: Option<String>,
        /// Container id when `environment_type == "container_reference"`.
        #[serde(skip_serializing_if = "Option::is_none")]
        container_id: Option<String>,
    },
    /// Streaming delta for a function-shell command payload.
    #[serde(rename = "response.shell_call_command.delta")]
    ResponseShellCallCommandDelta {
        item_id: String,
        call_id: String,
        output_index: i32,
        delta: String,
    },
    /// Finalized function-shell command payload (`commands` array). Distinct
    /// from `ResponseFunctionShellCallDone` (the call item): this carries only
    /// the command-stream terminal event.
    #[serde(rename = "response.shell_call_command.done")]
    ResponseShellCallCommandDone {
        item_id: String,
        call_id: String,
        output_index: i32,
        commands: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_output_length: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<i32>,
    },
    /// Streaming delta for function-shell command output (stdout/stderr).
    #[serde(rename = "response.shell_call_output_content.delta")]
    ResponseShellCallOutputContentDelta {
        item_id: String,
        call_id: String,
        output_index: i32,
        delta: String,
    },
    /// Finalized function-shell command output. `outputs` is opaque JSON
    /// (e.g. `{ type: "stdout", text }` / `{ type: "exit_code", code }`).
    #[serde(rename = "response.shell_call_output_content.done")]
    ResponseShellCallOutputContentDone {
        item_id: String,
        call_id: String,
        output_index: i32,
        outputs: Vec<serde_json::Value>,
    },
    #[serde(rename = "response.metrics")]
    ResponseMetrics { metrics: AgentMetrics },
    #[serde(rename = "response.background")]
    ResponseBackground { message: String },
    #[serde(rename = "agent.status")]
    AgentStatus { status: ThreadStatus },
    #[serde(rename = "agent.stream.lagged")]
    AgentStreamLagged,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentResponseRef {
    pub id: String,
    pub status: ThreadStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentArtifactRef {
    pub path: String,
    pub kind: AgentArtifactKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentArtifactKind {
    Diff,
    File,
    Image,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionStatus {
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMetrics {
    pub name: String,
    pub duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
}

/// A streaming event emitted during a single LLM turn, wrapped by the
/// response layer. The engine no longer produces this — `/responses`
/// synthesizes envelopes directly in [`single_shot`](super::single_shot).
#[derive(Debug, Clone)]
pub enum TurnEvent {
    Response { turn_index: Option<u32>, event: AgentEventKind },
}

/// Envelope for a response-layer event sequence. `id` is the per-stream
/// monotonic ordering key (used for SSE `Last-Event-Id`); the projections never
/// read it. Built by [`single_shot`](super::single_shot) and consumed by
/// [`super::projection::build_response`] / [`super::stream::envelope_to_events`].
#[derive(Clone)]
pub struct AgentEventEnvelope {
    pub id: u64,
    pub event: TurnEvent,
}
