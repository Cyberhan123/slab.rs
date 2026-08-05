//! OpenAI-Responses-canonical `Response` assembler for pure conversion from
//! slab-owned agent domain events into the canonical
//! [`slab_proto::openai::Response`] wire/persistence type.
//!
//! Lives inside the [`ResponseService`](super::ResponseService) module so the
//! HTTP handler (immediate non-streaming POST return) and the
//! response-persistence observer (run completion) share one assembler.
//!
//! Boundary rules (unchanged from the original adapter):
//! - Pure conversion only: never calls the agent services, never touches
//!   `tokio`/`axum`/sqlx.
//! - HTTP / SSE / WebSocket framing stays in the server crate.

use std::collections::HashMap;

use super::event::{AgentEventEnvelope, AgentEventKind, TurnEvent};
use super::single_shot::SingleShotOutcome;
use crate::domain::models::TextGenerationUsage;

// Glob import: `build_response` references ~70 slab-proto types. Glob imports do
// not trigger `unused_imports`, which keeps this module `clippy -D warnings` clean
// without hand-maintaining the full type list.
#[allow(unused_imports)]
use slab_proto::openai::*;

/// Input the converter needs from the caller. Decouples from the agent services
/// (concrete structs, not traits) so unit tests can construct this directly
/// without mocking a service.
///
/// Every field beyond the core identifiers is an optional echoed request-config
/// value: the caller fills in whichever fields the upstream request carried, and
/// [`build_response`] only emits the ones that are `Some`.
#[derive(Default)]
pub struct AdapterInput<'a> {
    /// `resp_<...>` identifier (== the slab run id for stored responses).
    pub response_id: &'a str,
    /// Model id echoed on the response (e.g. `gpt-5.3-codex`).
    pub model: &'a str,
    /// Unix seconds timestamp the response was created.
    pub created_at_unix: f64,
    /// Unix seconds timestamp the response was completed. Emitted as the
    /// `completed_at` field when `Some`; `None` omits the field entirely.
    pub completed_at: Option<f64>,
    /// Service tier echoed on the response (e.g. `default` / `auto`).
    pub service_tier: Option<ServiceTier>,
    /// Ordered slab event envelopes for a single run.
    pub envelopes: &'a [AgentEventEnvelope],
    /// Whether to run the model response in the background.
    pub background: Option<bool>,
    /// Billing attribution echoed on the response.
    pub billing: Option<serde_json::Value>,
    /// Whether the generated response is stored for later retrieval.
    pub store: Option<bool>,
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Nucleus sampling `top_p`.
    pub top_p: Option<f64>,
    /// Maximum number of top log-probabilities returned per token.
    pub top_logprobs: Option<i32>,
    /// Truncation strategy (`auto` / `disabled`).
    pub truncation: Option<ResponseTruncation>,
    /// Tool-choice directive echoed on the response.
    pub tool_choice: Option<ToolChoiceParam>,
    /// Tool definitions echoed on the response.
    pub tools: Option<Vec<Tool>>,
    /// Text response formatting (`format` + `verbosity`).
    pub text: Option<ResponseTextParam>,
    /// Reasoning configuration (`effort` + `summary`).
    pub reasoning: Option<Reasoning>,
    /// Request metadata (16 key-value pairs).
    pub metadata: Option<HashMap<String, String>>,
    /// Maximum number of built-in tool calls allowed in the response.
    pub max_tool_calls: Option<i32>,
    /// Frequency penalty applied to token sampling.
    pub frequency_penalty: Option<f64>,
    /// Presence penalty applied to token sampling.
    pub presence_penalty: Option<f64>,
    /// Input items submitted on the request, echoed when applicable.
    pub input: Option<Vec<slab_proto::openai::InputItem>>,
    /// Retention policy for the prompt cache.
    pub prompt_cache_retention: Option<ResponsePromptCacheRetention>,
    /// Whether the model may run tool calls in parallel.
    pub parallel_tool_calls: Option<bool>,
}

/// Assemble a complete ordered slab event sequence into a bare canonical
/// [`Response`] (the non-streaming POST return value / the persisted shape).
pub fn build_response(input: AdapterInput<'_>) -> Response {
    let mut output: Vec<OutputItem> = Vec::new();

    for envelope in input.envelopes {
        let TurnEvent::Response { event, .. } = &envelope.event;
        match event {
            AgentEventKind::ResponseOutputTextDone { item_id, text, phase, .. } => {
                let message = OutputMessage {
                    id: item_id.clone(),
                    r#type: CommonOutputType::Message,
                    role: OutputMessageRole::Assistant,
                    content: vec![OutputMessageContent::OutputTextContent(Box::new(
                        OutputTextContent { text: text.clone(), ..Default::default() },
                    ))],
                    status: Status::Completed,
                    phase: phase.as_deref().map(parse_phase).map(Some),
                };
                output.push(OutputItem::OutputMessage(Box::new(message)));
            }
            AgentEventKind::ResponseReasoningTextDone {
                item_id,
                encrypted_content,
                summary,
                ..
            } => {
                let summary_items = summary
                    .clone()
                    .map(|text| {
                        vec![SummaryTextContent {
                            r#type: SummaryTextContentType::SummaryText,
                            text,
                        }]
                    })
                    .unwrap_or_default();
                output.push(OutputItem::ReasoningItem(Box::new(ReasoningItem {
                    r#type: ReasoningItemType::Reasoning,
                    id: item_id.clone(),
                    summary: summary_items,
                    encrypted_content: encrypted_content.clone().map(Some),
                    ..Default::default()
                })));
            }
            AgentEventKind::ResponseFunctionCallArgumentsDone {
                item_id,
                call_id,
                name,
                arguments,
                namespace,
                ..
            } => {
                output.push(OutputItem::FunctionToolCall(Box::new(FunctionToolCall {
                    call_id: call_id.clone(),
                    name: name.clone(),
                    arguments: arguments.clone(),
                    id: Some(item_id.clone()),
                    status: Some(FunctionToolCallStatus::Completed),
                    namespace: namespace.clone(),
                    ..Default::default()
                })));
            }
            AgentEventKind::ResponseCustomToolCallInputDone {
                item_id,
                call_id,
                name,
                input,
                namespace,
                ..
            } => {
                output.push(OutputItem::CustomToolCall(Box::new(CustomToolCall {
                    r#type: CustomToolCallType::CustomToolCall,
                    call_id: call_id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                    id: Some(item_id.clone()),
                    namespace: namespace.clone(),
                    status: Some(FunctionCallStatus::Completed),
                })));
            }
            AgentEventKind::ResponseApplyPatchCallDone {
                item_id,
                call_id,
                operation_type,
                path,
                diff,
                ..
            } => {
                let operation = match operation_type.as_str() {
                    "delete_file" => ApplyPatchOperation::ApplyPatchDeleteFileOperation(Box::new(
                        ApplyPatchDeleteFileOperation::new(
                            DeleteFileOperationType::DeleteFile,
                            path.clone(),
                        ),
                    )),
                    "update_file" => ApplyPatchOperation::ApplyPatchUpdateFileOperation(Box::new(
                        ApplyPatchUpdateFileOperation::new(
                            UpdateFileOperationType::UpdateFile,
                            path.clone(),
                            diff.clone().unwrap_or_default(),
                        ),
                    )),
                    // default to create_file
                    _ => ApplyPatchOperation::ApplyPatchCreateFileOperation(Box::new(
                        ApplyPatchCreateFileOperation::new(
                            CreateFileOperationType::CreateFile,
                            path.clone(),
                            diff.clone().unwrap_or_default(),
                        ),
                    )),
                };
                output.push(OutputItem::ApplyPatchToolCall(Box::new(ApplyPatchToolCall {
                    r#type: ApplyPatchToolCallType::ApplyPatchCall,
                    id: item_id.clone(),
                    call_id: call_id.clone(),
                    status: ApplyPatchCallStatus::Completed,
                    operation: Box::new(operation),
                    ..Default::default()
                })));
            }
            AgentEventKind::ResponseLocalShellCallDone {
                item_id,
                call_id,
                command,
                env,
                working_directory,
                ..
            } => {
                let mut action = LocalShellExecAction::new(
                    LocalShellExecActionType::Exec,
                    command.clone(),
                    env.clone(),
                );
                action.working_directory = working_directory.clone().map(Some);
                output.push(OutputItem::LocalShellToolCall(Box::new(LocalShellToolCall::new(
                    LocalShellToolCallType::LocalShellCall,
                    item_id.clone(),
                    call_id.clone(),
                    action,
                    LocalShellToolCallStatus::Completed,
                ))));
            }
            AgentEventKind::ResponseCompactionDone { item_id, encrypted_content, .. } => {
                output.push(OutputItem::CompactionBody(Box::new(CompactionBody::new(
                    CompactionBodyType::Compaction,
                    item_id.clone(),
                    encrypted_content.clone(),
                ))));
            }
            AgentEventKind::ResponseFileSearchCallDone { item_id, queries, results, .. } => {
                output.push(OutputItem::FileSearchToolCall(Box::new(
                    slab_proto::openai::FileSearchToolCall {
                        r#type: slab_proto::openai::FileSearchToolCallType::FileSearchCall,
                        id: Some(item_id.clone()),
                        status: Some("completed".to_owned()),
                        queries: Some(queries.clone()),
                        results: results.clone(),
                    },
                )));
            }
            AgentEventKind::ResponseImageGenCallDone {
                item_id,
                result,
                revised_prompt,
                background,
                output_format,
                quality,
                size,
                ..
            } => {
                output.push(OutputItem::ImageGenToolCall(Box::new(
                    slab_proto::openai::ImageGenToolCall {
                        r#type: slab_proto::openai::ImageGenToolCallType::ImageGenerationCall,
                        id: item_id.clone(),
                        status: slab_proto::openai::ImageParamsStatus::Completed,
                        result: Some(result.clone()),
                        background: Some(background.clone()),
                        output_format: Some(output_format.clone()),
                        quality: Some(quality.clone()),
                        revised_prompt: revised_prompt.clone(),
                        size: Some(size.clone()),
                    },
                )));
            }
            AgentEventKind::ResponseToolSearchCallDone {
                item_id,
                execution,
                call_id,
                arguments,
                ..
            } => {
                let exec = match execution.as_str() {
                    "client" => ToolSearchExecutionType::Client,
                    _ => ToolSearchExecutionType::Server,
                };
                output.push(OutputItem::ToolSearchCall(Box::new(ToolSearchCall::new(
                    ToolSearchCallType::ToolSearchCall,
                    item_id.clone(),
                    call_id.clone(),
                    exec,
                    Some(arguments.clone()),
                    FunctionCallStatus::Completed,
                ))));
            }
            AgentEventKind::ResponseToolSearchOutputDone {
                item_id,
                execution,
                call_id,
                tools,
                ..
            } => {
                let exec = match execution.as_str() {
                    "client" => ToolSearchExecutionType::Client,
                    _ => ToolSearchExecutionType::Server,
                };
                // `tools` arrives as opaque JSON from slab-agent; parse each into
                // the typed `Tool` shape, dropping any that do not fit.
                let parsed_tools: Vec<Tool> = tools
                    .iter()
                    .filter_map(|v| serde_json::from_value::<Tool>(v.clone()).ok())
                    .collect();
                output.push(OutputItem::ToolSearchOutput(Box::new(ToolSearchOutput::new(
                    ToolSearchOutputType::ToolSearchOutput,
                    item_id.clone(),
                    call_id.clone(),
                    exec,
                    parsed_tools,
                    FunctionCallOutputStatusEnum::Completed,
                ))));
            }
            AgentEventKind::ResponseFunctionShellCallDone {
                item_id,
                call_id,
                commands,
                max_output_length,
                timeout_ms,
                environment_type,
                container_id,
                ..
            } => {
                let action =
                    FunctionShellAction::new(commands.clone(), *timeout_ms, *max_output_length);
                let env =
                    build_shell_environment(environment_type.as_deref(), container_id.as_deref());
                output.push(OutputItem::FunctionShellCall(Box::new(FunctionShellCall::new(
                    FunctionShellCallType::ShellCall,
                    item_id.clone(),
                    call_id.clone(),
                    action,
                    FunctionShellCallStatus::Completed,
                    env,
                ))));
            }
            AgentEventKind::ResponseMcpListToolsDone {
                item_id,
                server_label,
                tools,
                error,
                ..
            } => {
                let parsed_tools: Vec<McpListToolsTool> = tools
                    .iter()
                    .filter_map(|v| serde_json::from_value::<McpListToolsTool>(v.clone()).ok())
                    .collect();
                let mut list = McpListTools::new(
                    McpListToolsType::McpListTools,
                    item_id.clone(),
                    server_label.clone(),
                    parsed_tools,
                );
                list.error = error.clone().map(Some);
                output.push(OutputItem::McpListTools(Box::new(list)));
            }
            AgentEventKind::ResponseMcpCallDone {
                item_id,
                server_label,
                name,
                arguments,
                output: call_output,
                error,
                status,
                approval_request_id,
                ..
            } => {
                let mut call = McpToolCall::new(
                    McpToolCallType::McpCall,
                    item_id.clone(),
                    server_label.clone(),
                    name.clone(),
                    arguments.clone(),
                );
                call.output = call_output.clone().map(Some);
                call.error = error.clone().map(Some);
                call.status = status.as_deref().and_then(parse_mcp_status);
                call.approval_request_id = approval_request_id.clone().map(Some);
                output.push(OutputItem::McpToolCall(Box::new(call)));
            }
            AgentEventKind::ResponseMcpApprovalRequestDone {
                item_id,
                server_label,
                name,
                arguments,
                ..
            } => {
                output.push(OutputItem::McpApprovalRequest(Box::new(McpApprovalRequest::new(
                    McpApprovalRequestType::McpApprovalRequest,
                    item_id.clone(),
                    server_label.clone(),
                    name.clone(),
                    arguments.clone(),
                ))));
            }
            AgentEventKind::ResponseCodeInterpreterCallDone {
                item_id,
                code,
                container_id,
                outputs,
                ..
            } => {
                output.push(OutputItem::CodeInterpreterToolCall(Box::new(
                    slab_proto::openai::CodeInterpreterToolCall {
                        r#type:
                            slab_proto::openai::CodeInterpreterToolCallType::CodeInterpreterCall,
                        id: Some(item_id.clone()),
                        status: Some("completed".to_owned()),
                        code: Some(code.clone()),
                        container_id: container_id.clone(),
                        outputs: if outputs.is_empty() { None } else { Some(outputs.clone()) },
                    },
                )));
            }
            AgentEventKind::ResponseWebSearchCallDone { item_id, action, .. } => {
                let parsed_action = serde_json::from_value::<
                    slab_proto::openai::WebSearchToolCallAction,
                >(action.clone())
                .unwrap_or_else(|_| slab_proto::openai::WebSearchToolCallAction::default());
                output.push(OutputItem::WebSearchToolCall(Box::new(
                    slab_proto::openai::WebSearchToolCall::new(
                        item_id.clone(),
                        slab_proto::openai::WebSearchToolCallType::WebSearchCall,
                        slab_proto::openai::ToolStatus::Completed,
                        parsed_action,
                    ),
                )));
            }
            AgentEventKind::ResponseShellCallOutputContentDone {
                item_id,
                call_id,
                outputs,
                ..
            } => {
                let contents: Vec<slab_proto::openai::FunctionShellCallOutputContent> =
                    outputs.iter().filter_map(parse_shell_output_content).collect();
                output.push(OutputItem::FunctionShellCallOutput(Box::new(
                    slab_proto::openai::FunctionShellCallOutput::new(
                        slab_proto::openai::FunctionShellCallOutputType::ShellCallOutput,
                        item_id.clone(),
                        call_id.clone(),
                        slab_proto::openai::FunctionShellCallOutputStatusEnum::Completed,
                        contents,
                        None,
                    ),
                )));
            }
            _ => {}
        }
    }

    Response {
        id: input.response_id.to_owned(),
        object: ResponseObject::Response,
        created_at: input.created_at_unix,
        completed_at: input.completed_at.map(Some),
        status: Some(ResponseStatus::Completed),
        model: Some(Box::new(ModelIdsResponses::StringValue(input.model.to_owned()))),
        service_tier: input.service_tier.map(Some),
        output,
        usage: Some(Box::new(ResponseUsage::default())),
        background: input.background,
        billing: input.billing,
        store: input.store,
        temperature: input.temperature,
        top_p: input.top_p,
        top_logprobs: input.top_logprobs,
        truncation: input.truncation,
        tool_choice: input.tool_choice.map(Box::new),
        tools: input.tools,
        text: input.text.map(Box::new),
        reasoning: input.reasoning.map(Box::new),
        metadata: input.metadata,
        max_tool_calls: input.max_tool_calls,
        frequency_penalty: input.frequency_penalty,
        presence_penalty: input.presence_penalty,
        input: input.input,
        prompt_cache_retention: input.prompt_cache_retention,
        parallel_tool_calls: input.parallel_tool_calls,
        ..Default::default()
    }
}

pub fn parse_mcp_status(raw: &str) -> Option<McpToolCallStatus> {
    match raw {
        "in_progress" => Some(McpToolCallStatus::InProgress),
        "completed" => Some(McpToolCallStatus::Completed),
        "incomplete" => Some(McpToolCallStatus::Incomplete),
        "calling" => Some(McpToolCallStatus::Calling),
        "failed" => Some(McpToolCallStatus::Failed),
        _ => None,
    }
}

pub fn parse_phase(raw: &str) -> MessagePhase {
    match raw {
        "final_answer" => MessagePhase::FinalAnswer,
        _ => MessagePhase::Commentary,
    }
}

/// Post-process a [`Response`] assembled by [`build_response`] for the
/// single-shot Model:
/// - set `status` + `incomplete_details` from the outcome — tool calls present
///   ⇒ `Incomplete { reason: ToolCalls }` (client-side tool loop); failure ⇒
///   `Failed`; otherwise `Completed` (the projection itself defaults to
///   `Completed`, so this only diverges on tool calls / failure),
/// - populate `usage` from the LLM call's token usage (the projection defaults
///   it to `ResponseUsage::default()`).
pub(crate) fn apply_terminal(response: &mut Response, outcome: &SingleShotOutcome) {
    match outcome {
        SingleShotOutcome::Failed { .. } => {
            response.status = Some(ResponseStatus::Failed);
            response.incomplete_details = None;
        }
        _ if outcome.has_tool_calls() => {
            response.status = Some(ResponseStatus::Incomplete);
            response.incomplete_details =
                Some(Box::new(ResponseAllOfIncompleteDetails { reason: Some(Reason::ToolCalls) }));
        }
        _ => {
            response.status = Some(ResponseStatus::Completed);
            response.incomplete_details = None;
        }
    }
    if let Some(usage) = outcome.usage() {
        response.usage = Some(Box::new(response_usage_from_text(usage)));
    }
}

pub(crate) fn response_usage_from_text(usage: &TextGenerationUsage) -> ResponseUsage {
    ResponseUsage {
        input_tokens: usage.prompt_tokens as i32,
        output_tokens: usage.completion_tokens as i32,
        total_tokens: usage.total_tokens as i32,
        ..Default::default()
    }
}

/// Build the optional `environment` field for a function-shell tool call.
fn build_shell_environment(
    environment_type: Option<&str>,
    container_id: Option<&str>,
) -> Option<slab_proto::openai::FunctionShellCallEnvironment> {
    let ty = environment_type?;
    Some(slab_proto::openai::FunctionShellCallEnvironment {
        r#type: Some(ty.to_owned()),
        container_id: container_id.map(|s| s.to_owned()),
    })
}

/// Parse one opaque shell-output element into the typed
/// [`slab_proto::openai::FunctionShellCallOutputContent`].
pub fn parse_shell_output_content(
    v: &serde_json::Value,
) -> Option<slab_proto::openai::FunctionShellCallOutputContent> {
    let obj = v.as_object()?;
    let stdout = obj.get("stdout").and_then(|s| s.as_str()).unwrap_or_default().to_owned();
    let stderr = obj.get("stderr").and_then(|s| s.as_str()).unwrap_or_default().to_owned();
    let outcome = match obj.get("outcome").and_then(|o| o.get("type")).and_then(|t| t.as_str()) {
        Some("timeout") => {
            slab_proto::openai::ShellCallOutcome::FunctionShellCallOutputTimeoutOutcome(Box::new(
                slab_proto::openai::FunctionShellCallOutputTimeoutOutcome::new(),
            ))
        }
        _ => slab_proto::openai::ShellCallOutcome::FunctionShellCallOutputExitOutcome(Box::new(
            slab_proto::openai::FunctionShellCallOutputExitOutcome::new(
                obj.get("outcome")
                    .and_then(|o| o.get("exit_code"))
                    .and_then(|c| c.as_i64())
                    .unwrap_or(0) as i32,
            ),
        )),
    };
    Some(slab_proto::openai::FunctionShellCallOutputContent::new(stdout, stderr, outcome))
}
