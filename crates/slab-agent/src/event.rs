use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::port::ThreadStatus;

/// Response-style event emitted by the Slab agent stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentStreamEvent {
    pub thread_id: String,
    pub turn_index: Option<u32>,
    pub sequence_number: u64,
    #[serde(flatten)]
    pub event: AgentEventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

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
    ResponseFailed { response: AgentResponseRef, error: String },
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
    },
    #[serde(rename = "response.function_call_arguments.done")]
    ResponseFunctionCallArgumentsDone {
        item_id: String,
        call_id: String,
        name: String,
        output_index: i32,
        arguments: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        risk: Option<ToolRiskAssessment>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolRiskAssessment {
    pub level: ToolRiskLevel,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMetrics {
    pub name: String,
    pub duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
}

impl AgentStreamEvent {
    pub fn new(
        thread_id: impl Into<String>,
        turn_index: Option<u32>,
        sequence_number: u64,
        event: AgentEventKind,
    ) -> Self {
        Self { thread_id: thread_id.into(), turn_index, sequence_number, event, metadata: None }
    }
}
