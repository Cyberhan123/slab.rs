//! Streaming wire projection for the OpenAI Responses transport.
//!
//! Relocated from `bin/slab-server/.../openai_compat.rs` (slice 1e): pure
//! conversion from slab-owned agent domain events into canonical
//! `ResponsesServerEvent`s. HTTP / SSE / WebSocket framing stays in the server
//! crate; this module only owns the streaming state machine + event synthesis.

use std::collections::{HashMap, HashSet};

use slab_agent::AgentEventKind;

// Glob import: the streaming state machine references ~70 slab-proto types. Glob
// imports do not trigger `unused_imports`, which keeps this module
// `clippy -D warnings` clean without hand-maintaining the full type list.
#[allow(unused_imports)]
use slab_proto::openai::*;

use super::projection::{parse_mcp_status, parse_phase, parse_shell_output_content};
use crate::infra::agent::event_hub::AgentEventEnvelope;

/// Per-response streaming state. Carries the assembled output items, the
/// running sequence-number counter, and response-level metadata so synthesized
/// wrapper events (`response.output_item.added`, `response.content_part.added`,
// ...) and the terminal `response.completed` payload match the fixtures.
pub struct StreamCtx {
    pub response_id: String,
    pub model: String,
    pub created_at_unix: f64,
    pub service_tier: Option<ServiceTier>,
    pub next_sequence: i32,
    /// Items finalized via `output_item.done`, in output order.
    pub items: Vec<OutputItem>,
    /// Skeleton [`Response`] cloned for lifecycle events (`response.created`,
    /// `response.in_progress`, `response.failed`) and as the base for
    /// `response.completed`. Defaults to a minimal response built from the
    /// `new()` inputs; tests/handler replace it with the full request-echo
    /// config when matching richer fixtures.
    skeleton: Response,
    /// Resolved service tier applied to the `response.completed` payload.
    /// Captures the OpenAI behaviour where the created/in_progress skeleton
    /// echoes the unresolved request tier (often `auto`) while the completed
    /// event echoes the resolved tier (often `default`). Defaults to
    /// [`StreamCtx::service_tier`].
    completed_service_tier: Option<ServiceTier>,
    /// `item_id`s that have already emitted `response.output_item.added`, so
    /// the wrapper fires exactly once per item even when multiple deltas land.
    seen_items: HashSet<String>,
    /// `item_id`s that have already emitted `response.reasoning_summary_part.
    /// added` (set on the first `ResponseReasoningTextDelta`). Tracked
    /// separately from `seen_items` so a reasoning done event can decide
    /// whether to synthesize the summary.done / summary_part.done wrappers.
    summary_started: HashSet<String>,
    /// Per-item delta split counts for tool streams slab-agent does NOT model
    /// at the delta granularity (code-interpreter `code`, mcp `arguments`).
    /// slab only carries the finalized payload, so the adapter re-splits it
    /// into N chunks to match the canonical event count. Entries default to
    /// `1` (emit the whole payload as a single delta) when absent — the
    /// production-faithful behaviour since slab never saw the per-token stream.
    /// Tests populate this from the fixture's delta count.
    tool_delta_splits: HashMap<String, usize>,
    /// Response-level shell environment (`(type, container_id?)`) applied to
    /// every `shell_call` output item the adapter emits. slab-agent's
    /// `ResponseShellCallCommandDelta/Done` variants carry the command stream
    /// but not the `environment` discriminator (it lives on the `shell` tool
    /// config), so the handler/test pins it on the context. `None` omits the
    /// field (local environment with no container).
    shell_environment: Option<(String, Option<String>)>,
    /// Unix-seconds completion timestamp applied to the terminal
    /// `response.completed` payload. The created/in_progress skeleton leaves
    /// `completed_at` unset (matching the fixture's `null`), and the completed
    /// event echoes this value when `Some`.
    completed_at_unix: Option<f64>,
}

impl StreamCtx {
    pub fn new(
        response_id: String,
        model: String,
        created_at_unix: f64,
        service_tier: Option<ServiceTier>,
    ) -> Self {
        let skeleton = Response {
            id: response_id.clone(),
            object: ResponseObject::Response,
            created_at: created_at_unix,
            status: Some(ResponseStatus::InProgress),
            model: Some(Box::new(ModelIdsResponses::StringValue(model.clone()))),
            service_tier: service_tier.map(Some),
            output: Vec::new(),
            ..Default::default()
        };
        Self {
            response_id,
            model,
            created_at_unix,
            service_tier,
            next_sequence: 0,
            items: Vec::new(),
            skeleton,
            completed_service_tier: service_tier,
            seen_items: HashSet::new(),
            summary_started: HashSet::new(),
            tool_delta_splits: HashMap::new(),
            shell_environment: None,
            completed_at_unix: None,
        }
    }

    /// Allocate the next monotonic sequence number.
    pub fn next_seq(&mut self) -> i32 {
        let n = self.next_sequence;
        self.next_sequence += 1;
        n
    }

    /// Override the service tier applied to the `response.completed` payload.
    pub fn set_completed_service_tier(&mut self, tier: Option<ServiceTier>) {
        self.completed_service_tier = tier;
    }

    /// Override the `completed_at` timestamp applied to the terminal
    /// `response.completed` payload.
    pub fn set_completed_at(&mut self, ts: Option<f64>) {
        self.completed_at_unix = ts;
    }

    /// Pin the response-level shell environment applied to every `shell_call`
    /// output item. `env_type` is `"local"` or `"container_reference"`;
    /// `container_id` is only set for the container form.
    pub fn set_shell_environment(&mut self, env_type: String, container_id: Option<String>) {
        self.shell_environment = Some((env_type, container_id));
    }

    /// Record how many delta chunks a tool stream lacking a slab-agent delta
    /// variant (code-interpreter `code`, mcp `arguments`) should be split into.
    /// The adapter splits the finalized payload into exactly `n` chunks so the
    /// emitted delta-event count matches the canonical fixture.
    pub fn set_tool_delta_split(&mut self, item_id: &str, n: usize) {
        self.tool_delta_splits.insert(item_id.to_owned(), n);
    }

    /// Reset per-response streaming state for a fresh response cycle within the
    /// same multi-response stream (keeps `response_id`/`model`/skeleton config).
    /// Used by tests that drive one `.chunks.txt` carrying several independent
    /// response cycles (e.g. multi-turn reasoning fixtures).
    pub fn reset_for_new_response(&mut self) {
        self.next_sequence = 0;
        self.items = Vec::new();
        self.seen_items = HashSet::new();
        self.summary_started = HashSet::new();
        self.tool_delta_splits = HashMap::new();
        self.shell_environment = None;
        self.completed_at_unix = None;
    }

    /// Replace the lifecycle skeleton [`Response`] verbatim. Use this when a
    /// fixture (or the handler) needs the `response.created` / `in_progress` /
    /// `failed` skeleton to echo the full request config (background, reasoning,
    /// store, temperature, text, tool_choice, tools, top_logprobs, top_p,
    /// truncation, metadata, parallel_tool_calls, ...). The status field is
    /// overwritten per-event by [`skeleton_with_status`]; the output array is
    /// cleared for lifecycle skeleton events.
    pub fn set_skeleton(&mut self, skeleton: Response) {
        self.skeleton = skeleton;
    }

    /// Clone the skeleton, apply the requested status, and clear the output
    /// array (lifecycle skeleton events carry `output: []`).
    fn skeleton_with_status(&self, status: ResponseStatus) -> Response {
        let mut r = self.skeleton.clone();
        r.status = Some(status);
        r.output = Vec::new();
        r
    }

    /// Clone the skeleton for the terminal `response.completed` event: apply
    /// the resolved service tier, the assembled output items, default usage,
    /// and a `completed` status.
    fn completed_response(&self) -> Response {
        let mut r = self.skeleton.clone();
        r.status = Some(ResponseStatus::Completed);
        r.service_tier = self.completed_service_tier.map(Some);
        r.completed_at = self.completed_at_unix.map(Some);
        r.output = self.items.clone();
        r.usage = Some(Box::new(ResponseUsage::default()));
        r
    }
}

/// Convert a single slab [`AgentEventEnvelope`] into 0..N canonical
/// [`ResponsesServerEvent`]s. N>1 when slab's coarse event requires synthesizing
/// the canonical wrapper events (`output_item.added`, `content_part.added/done`,
/// `output_item.done`) the fixtures expect.
pub fn envelope_to_events(
    env: &AgentEventEnvelope,
    ctx: &mut StreamCtx,
) -> Vec<ResponsesServerEvent> {
    let slab_agent::TurnEvent::Response { event, .. } = &env.event;
    match event {
        // --- Lifecycle -------------------------------------------------------
        AgentEventKind::ResponseQueued { .. } => {
            let response = ctx.skeleton_with_status(ResponseStatus::InProgress);
            vec![ResponsesServerEvent::ResponseCreatedEvent(Box::new(ResponseCreatedEvent::new(
                ResponseCreatedType::ResponseCreated,
                response,
                ctx.next_seq(),
            )))]
        }
        AgentEventKind::ResponseInProgress { .. } => {
            let response = ctx.skeleton_with_status(ResponseStatus::InProgress);
            vec![ResponsesServerEvent::ResponseInProgressEvent(Box::new(
                ResponseInProgressEvent::new(
                    ResponseInProgressType::ResponseInProgress,
                    response,
                    ctx.next_seq(),
                ),
            ))]
        }
        AgentEventKind::ResponseCompleted { .. } => {
            vec![ResponsesServerEvent::ResponseCompletedEvent(Box::new(
                ResponseCompletedEvent::new(
                    ResponseCompletedType::ResponseCompleted,
                    ctx.completed_response(),
                    ctx.next_seq(),
                ),
            ))]
        }
        AgentEventKind::ResponseFailed { error, error_code, error_type, .. } => {
            // OpenAI emits a standalone nested `error` event before `response.failed`.
            let err_payload = slab_proto::openai::Error::new(
                error_code.clone(),
                error.clone(),
                None,
                error_type.clone().unwrap_or_else(|| "server_error".to_owned()),
            );
            let mut out =
                vec![ResponsesServerEvent::ResponseErrorEvent(Box::new(ResponseErrorEvent::new(
                    slab_proto::openai::ErrorType::Error,
                    ctx.next_seq(),
                    err_payload.clone(),
                )))];
            let mut response = ctx.skeleton_with_status(ResponseStatus::Failed);
            // `response.failed.response.error` carries only `{code, message}`
            // in the canonical OpenAI wire format — no `type` or `param`. The
            // standalone nested `error` event above carries the fuller payload.
            response.error = Some(Box::new(slab_proto::openai::ResponseError {
                code: error_code.clone(),
                message: error.clone(),
                param: None,
                r#type: None,
            }));
            out.push(ResponsesServerEvent::ResponseFailedEvent(Box::new(
                ResponseFailedEvent::new(
                    ResponseFailedType::ResponseFailed,
                    ctx.next_seq(),
                    response,
                ),
            )));
            out
        }

        // --- Output text streaming ------------------------------------------
        AgentEventKind::ResponseOutputTextDelta {
            item_id,
            output_index,
            content_index,
            delta,
            ..
        } => {
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    skeleton_message(item_id),
                ));
                out.push(content_part_added(
                    item_id,
                    *output_index,
                    *content_index,
                    ctx.next_seq(),
                    OutputTextContent { text: String::new(), ..Default::default() },
                ));
            }
            out.push(ResponsesServerEvent::ResponseTextDeltaEvent(Box::new(
                ResponseTextDeltaEvent::new(
                    TextDeltaType::ResponseOutputTextDelta,
                    item_id.clone(),
                    *output_index,
                    *content_index,
                    delta.clone(),
                    ctx.next_seq(),
                    Vec::new(),
                ),
            )));
            out
        }
        AgentEventKind::ResponseOutputTextDone {
            item_id,
            output_index,
            content_index,
            text,
            phase,
            ..
        } => {
            let phase_value = phase.as_deref().map(parse_phase);
            let finalized = OutputMessage {
                id: item_id.clone(),
                r#type: CommonOutputType::Message,
                role: OutputMessageRole::Assistant,
                content: vec![OutputMessageContent::OutputTextContent(Box::new(
                    OutputTextContent { text: text.clone(), ..Default::default() },
                ))],
                status: Status::Completed,
                phase: phase_value.map(Some),
            };

            let out = vec![
                ResponsesServerEvent::ResponseTextDoneEvent(Box::new(ResponseTextDoneEvent::new(
                    TextDoneType::ResponseOutputTextDone,
                    item_id.clone(),
                    *output_index,
                    *content_index,
                    text.clone(),
                    ctx.next_seq(),
                    Vec::new(),
                ))),
                content_part_done(
                    item_id,
                    *output_index,
                    *content_index,
                    ctx.next_seq(),
                    OutputTextContent { text: text.clone(), ..Default::default() },
                ),
                output_item_done(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::OutputMessage(Box::new(finalized.clone())),
                ),
            ];
            ctx.items.push(OutputItem::OutputMessage(Box::new(finalized)));
            out
        }

        // --- Function-call streaming ----------------------------------------
        // Delta: fire `output_item.added` (in_progress skeleton) once on the
        // first delta, then emit each arguments delta as a standalone event.
        // Function-call items have NO `content_part` lifecycle: the wrapper
        // sequence is `output_item.added` -> `args.delta*` -> `args.done` ->
        // `output_item.done`.
        AgentEventKind::ResponseFunctionCallArgumentsDelta {
            item_id,
            call_id,
            name,
            output_index,
            delta,
            ..
        } => {
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                let skeleton_call = FunctionToolCall {
                    call_id: call_id.clone(),
                    name: name.clone(),
                    arguments: String::new(),
                    id: Some(item_id.clone()),
                    status: Some(FunctionToolCallStatus::InProgress),
                    ..Default::default()
                };
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::FunctionToolCall(Box::new(skeleton_call)),
                ));
            }
            out.push(ResponsesServerEvent::ResponseFunctionCallArgumentsDeltaEvent(Box::new(
                ResponseFunctionCallArgumentsDeltaEvent::new(
                    FuncArgsDeltaType::ResponseFunctionCallArgumentsDelta,
                    item_id.clone(),
                    *output_index,
                    ctx.next_seq(),
                    delta.clone(),
                ),
            )));
            out
        }
        AgentEventKind::ResponseFunctionCallArgumentsDone {
            item_id,
            call_id,
            name,
            arguments,
            namespace,
            output_index,
            ..
        } => {
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                let skeleton_call = FunctionToolCall {
                    call_id: call_id.clone(),
                    name: name.clone(),
                    arguments: String::new(),
                    id: Some(item_id.clone()),
                    status: Some(FunctionToolCallStatus::InProgress),
                    namespace: namespace.clone(),
                    ..Default::default()
                };
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::FunctionToolCall(Box::new(skeleton_call)),
                ));
            }
            out.push(ResponsesServerEvent::ResponseFunctionCallArgumentsDoneEvent(Box::new(
                ResponseFunctionCallArgumentsDoneEvent::new(
                    FuncArgsDoneType::ResponseFunctionCallArgumentsDone,
                    item_id.clone(),
                    *output_index,
                    ctx.next_seq(),
                    arguments.clone(),
                ),
            )));
            let finalized = FunctionToolCall {
                call_id: call_id.clone(),
                name: name.clone(),
                arguments: arguments.clone(),
                id: Some(item_id.clone()),
                status: Some(FunctionToolCallStatus::Completed),
                namespace: namespace.clone(),
                ..Default::default()
            };
            out.push(output_item_done(
                *output_index,
                ctx.next_seq(),
                OutputItem::FunctionToolCall(Box::new(finalized.clone())),
            ));
            ctx.items.push(OutputItem::FunctionToolCall(Box::new(finalized)));
            out
        }

        // --- Custom-tool streaming -----------------------------------------
        // Mirrors function-call streaming but emits
        // `response.custom_tool_call_input.delta`/`.done` events and uses the
        // `CustomToolCall` output item. Skeleton has no `status` field.
        AgentEventKind::ResponseCustomToolCallInputDelta {
            item_id,
            call_id,
            name,
            output_index,
            delta,
            ..
        } => {
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                let skeleton_call = CustomToolCall {
                    r#type: CustomToolCallType::CustomToolCall,
                    call_id: call_id.clone(),
                    name: name.clone(),
                    input: String::new(),
                    id: Some(item_id.clone()),
                    ..Default::default()
                };
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::CustomToolCall(Box::new(skeleton_call)),
                ));
            }
            out.push(ResponsesServerEvent::ResponseCustomToolCallInputDeltaEvent(Box::new(
                ResponseCustomToolCallInputDeltaEvent::new(
                    CustomToolCallInputDeltaType::ResponseCustomToolCallInputDelta,
                    ctx.next_seq(),
                    *output_index,
                    item_id.clone(),
                    delta.clone(),
                ),
            )));
            out
        }
        AgentEventKind::ResponseCustomToolCallInputDone {
            item_id,
            call_id,
            name,
            input,
            output_index,
            namespace,
            ..
        } => {
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                let skeleton_call = CustomToolCall {
                    r#type: CustomToolCallType::CustomToolCall,
                    call_id: call_id.clone(),
                    name: name.clone(),
                    input: String::new(),
                    id: Some(item_id.clone()),
                    namespace: namespace.clone(),
                    ..Default::default()
                };
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::CustomToolCall(Box::new(skeleton_call)),
                ));
            }
            // Note: the OpenAI fixture omits the standalone
            // `response.custom_tool_call_input.done` event and goes directly
            // from the last `.delta` to `response.output_item.done`. Should a
            // future fixture require it, slab-proto's
            // `ResponseCustomToolCallInputDoneEvent` is the type to emit here.
            let finalized = CustomToolCall {
                r#type: CustomToolCallType::CustomToolCall,
                call_id: call_id.clone(),
                name: name.clone(),
                input: input.clone(),
                id: Some(item_id.clone()),
                namespace: namespace.clone(),
                status: Some(FunctionCallStatus::Completed),
            };
            out.push(output_item_done(
                *output_index,
                ctx.next_seq(),
                OutputItem::CustomToolCall(Box::new(finalized.clone())),
            ));
            ctx.items.push(OutputItem::CustomToolCall(Box::new(finalized)));
            out
        }

        // --- Apply-patch streaming -----------------------------------------
        // No delta stream — `output_item.added` carries the in_progress
        // skeleton (operation already populated), then `output_item.done`
        // finalizes with `status: completed`.
        AgentEventKind::ResponseApplyPatchCallDone {
            item_id,
            call_id,
            operation_type,
            path,
            diff,
            output_index,
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
                _ => ApplyPatchOperation::ApplyPatchCreateFileOperation(Box::new(
                    ApplyPatchCreateFileOperation::new(
                        CreateFileOperationType::CreateFile,
                        path.clone(),
                        diff.clone().unwrap_or_default(),
                    ),
                )),
            };
            let skeleton_call = ApplyPatchToolCall {
                r#type: ApplyPatchToolCallType::ApplyPatchCall,
                id: item_id.clone(),
                call_id: call_id.clone(),
                status: ApplyPatchCallStatus::InProgress,
                operation: Box::new(operation.clone()),
                ..Default::default()
            };
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::ApplyPatchToolCall(Box::new(skeleton_call)),
                ));
            }
            let finalized = ApplyPatchToolCall {
                r#type: ApplyPatchToolCallType::ApplyPatchCall,
                id: item_id.clone(),
                call_id: call_id.clone(),
                status: ApplyPatchCallStatus::Completed,
                operation: Box::new(operation),
                ..Default::default()
            };
            out.push(output_item_done(
                *output_index,
                ctx.next_seq(),
                OutputItem::ApplyPatchToolCall(Box::new(finalized.clone())),
            ));
            ctx.items.push(OutputItem::ApplyPatchToolCall(Box::new(finalized)));
            out
        }

        // --- Local-shell streaming -----------------------------------------
        // `output_item.added` carries an in_progress skeleton with an empty
        // command vector; `output_item.done` finalizes with the full command
        // and `status: completed`.
        AgentEventKind::ResponseLocalShellCallDone {
            item_id,
            call_id,
            command,
            env,
            working_directory,
            output_index,
        } => {
            let skeleton_action =
                LocalShellExecAction::new(LocalShellExecActionType::Exec, Vec::new(), env.clone());
            let skeleton_call = LocalShellToolCall::new(
                LocalShellToolCallType::LocalShellCall,
                item_id.clone(),
                call_id.clone(),
                skeleton_action,
                LocalShellToolCallStatus::InProgress,
            );
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::LocalShellToolCall(Box::new(skeleton_call)),
                ));
            }
            let mut final_action = LocalShellExecAction::new(
                LocalShellExecActionType::Exec,
                command.clone(),
                env.clone(),
            );
            final_action.working_directory = working_directory.clone().map(Some);
            let finalized = LocalShellToolCall::new(
                LocalShellToolCallType::LocalShellCall,
                item_id.clone(),
                call_id.clone(),
                final_action,
                LocalShellToolCallStatus::Completed,
            );
            out.push(output_item_done(
                *output_index,
                ctx.next_seq(),
                OutputItem::LocalShellToolCall(Box::new(finalized.clone())),
            ));
            ctx.items.push(OutputItem::LocalShellToolCall(Box::new(finalized)));
            out
        }

        // --- Reasoning streaming -------------------------------------------
        // Delta: synthesize the OpenAI `reasoning_summary_text.delta` stream
        // from slab's `ResponseReasoningTextDelta`. On the first delta for an
        // item, also emit `output_item.added` (reasoning skeleton carrying the
        // encrypted content) and `reasoning_summary_part.added` (empty text).
        AgentEventKind::ResponseReasoningTextDelta { item_id, output_index, delta, .. } => {
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                // The reasoning skeleton's `encrypted_content` arrives with
                // the slab `ResponseReasoningTextDone` event, not with deltas.
                // The OpenAI fixture's `output_item.added` skeleton carries it
                // eagerly; tests normalize that field away on both sides.
                let skeleton_reasoning = ReasoningItem {
                    r#type: ReasoningItemType::Reasoning,
                    id: item_id.clone(),
                    summary: Vec::new(),
                    ..Default::default()
                };
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::ReasoningItem(Box::new(skeleton_reasoning)),
                ));
            }
            if ctx.summary_started.insert(item_id.clone()) {
                out.push(ResponsesServerEvent::ResponseReasoningSummaryPartAddedEvent(Box::new(
                    ResponseReasoningSummaryPartAddedEvent::new(
                        ReasoningSummaryPartAddedType::ResponseReasoningSummaryPartAdded,
                        item_id.clone(),
                        *output_index,
                        0,
                        ctx.next_seq(),
                        ResponseReasoningSummaryPartAddedEventPart::new(
                            SummaryTextType::SummaryText,
                            String::new(),
                        ),
                    ),
                )));
            }
            out.push(ResponsesServerEvent::ResponseReasoningSummaryTextDeltaEvent(Box::new(
                ResponseReasoningSummaryTextDeltaEvent::new(
                    ReasoningSummaryTextDeltaType::ResponseReasoningSummaryTextDelta,
                    item_id.clone(),
                    *output_index,
                    0,
                    delta.clone(),
                    ctx.next_seq(),
                ),
            )));
            out
        }
        // Done: if any summary deltas fired, synthesize the canonical
        // `reasoning_summary_text.done` + `reasoning_summary_part.done` closing
        // wrappers in addition to `output_item.done`.
        AgentEventKind::ResponseReasoningTextDone {
            item_id,
            output_index,
            encrypted_content,
            summary,
            ..
        } => {
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                let skeleton_reasoning = ReasoningItem {
                    r#type: ReasoningItemType::Reasoning,
                    id: item_id.clone(),
                    summary: Vec::new(),
                    encrypted_content: encrypted_content.clone().map(Some),
                    ..Default::default()
                };
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::ReasoningItem(Box::new(skeleton_reasoning)),
                ));
            }
            let summary_text = summary.clone().unwrap_or_default();
            if ctx.summary_started.contains(item_id) {
                out.push(ResponsesServerEvent::ResponseReasoningSummaryTextDoneEvent(Box::new(
                    ResponseReasoningSummaryTextDoneEvent::new(
                        ReasoningSummaryTextDoneType::ResponseReasoningSummaryTextDone,
                        item_id.clone(),
                        *output_index,
                        0,
                        summary_text.clone(),
                        ctx.next_seq(),
                    ),
                )));
                out.push(ResponsesServerEvent::ResponseReasoningSummaryPartDoneEvent(Box::new(
                    ResponseReasoningSummaryPartDoneEvent::new(
                        ReasoningSummaryPartDoneType::ResponseReasoningSummaryPartDone,
                        item_id.clone(),
                        *output_index,
                        0,
                        ctx.next_seq(),
                        ResponseReasoningSummaryPartDoneEventPart::new(
                            SummaryTextType::SummaryText,
                            summary_text.clone(),
                        ),
                    ),
                )));
            }
            let summary_items = if summary.is_some() {
                vec![SummaryTextContent {
                    r#type: SummaryTextContentType::SummaryText,
                    text: summary_text,
                }]
            } else {
                Vec::new()
            };
            let finalized = ReasoningItem {
                r#type: ReasoningItemType::Reasoning,
                id: item_id.clone(),
                summary: summary_items,
                encrypted_content: encrypted_content.clone().map(Some),
                ..Default::default()
            };
            out.push(output_item_done(
                *output_index,
                ctx.next_seq(),
                OutputItem::ReasoningItem(Box::new(finalized.clone())),
            ));
            ctx.items.push(OutputItem::ReasoningItem(Box::new(finalized)));
            out
        }

        // --- MCP streaming ------------------------------------------------
        // slab emits a single finalized event per MCP item; the adapter
        // synthesizes the canonical lifecycle. `mcp_list_tools`:
        // `output_item.added` (empty tools) → `mcp_list_tools.in_progress` →
        // `mcp_list_tools.completed` → `output_item.done` (tools populated).
        AgentEventKind::ResponseMcpListToolsDone {
            item_id,
            output_index,
            server_label,
            tools,
            error,
            ..
        } => {
            let parsed_tools: Vec<McpListToolsTool> = tools
                .iter()
                .filter_map(|v| serde_json::from_value::<McpListToolsTool>(v.clone()).ok())
                .collect();
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                let skeleton = McpListTools::new(
                    McpListToolsType::McpListTools,
                    item_id.clone(),
                    server_label.clone(),
                    Vec::new(),
                );
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::McpListTools(Box::new(skeleton)),
                ));
            }
            out.push(ResponsesServerEvent::ResponseMcpListToolsInProgressEvent(Box::new(
                ResponseMcpListToolsInProgressEvent::new(
                    McpListToolsInProgressType::ResponseMcpListToolsInProgress,
                    item_id.clone(),
                    *output_index,
                    ctx.next_seq(),
                ),
            )));
            out.push(ResponsesServerEvent::ResponseMcpListToolsCompletedEvent(Box::new(
                ResponseMcpListToolsCompletedEvent::new(
                    McpListToolsCompletedType::ResponseMcpListToolsCompleted,
                    item_id.clone(),
                    *output_index,
                    ctx.next_seq(),
                ),
            )));
            let mut finalized = McpListTools::new(
                McpListToolsType::McpListTools,
                item_id.clone(),
                server_label.clone(),
                parsed_tools,
            );
            finalized.error = error.clone().map(Some);
            out.push(output_item_done(
                *output_index,
                ctx.next_seq(),
                OutputItem::McpListTools(Box::new(finalized.clone())),
            ));
            ctx.items.push(OutputItem::McpListTools(Box::new(finalized)));
            out
        }

        // `mcp_call`: `output_item.added` (in_progress skeleton) →
        // `mcp_call.in_progress` → `mcp_call_arguments.delta`×N →
        // `mcp_call_arguments.done` → `mcp_call.completed` → `output_item.done`.
        // slab lacks an mcp-arguments delta variant, so the finalized arguments
        // are re-split into N chunks (N pinned on `ctx`; default 1).
        AgentEventKind::ResponseMcpCallDone {
            item_id,
            output_index,
            server_label,
            name,
            arguments,
            output: call_output,
            error,
            status,
            approval_request_id,
            ..
        } => {
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                let mut skeleton = McpToolCall::new(
                    McpToolCallType::McpCall,
                    item_id.clone(),
                    server_label.clone(),
                    name.clone(),
                    String::new(),
                );
                skeleton.status = Some(McpToolCallStatus::InProgress);
                skeleton.approval_request_id = approval_request_id.clone().map(Some);
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::McpToolCall(Box::new(skeleton)),
                ));
            }
            out.push(ResponsesServerEvent::ResponseMcpCallInProgressEvent(Box::new(
                ResponseMcpCallInProgressEvent::new(
                    McpCallInProgressType::ResponseMcpCallInProgress,
                    ctx.next_seq(),
                    *output_index,
                    item_id.clone(),
                ),
            )));
            let n = ctx.tool_delta_splits.get(item_id).copied().unwrap_or(1);
            for chunk in split_string_into(arguments, n) {
                out.push(ResponsesServerEvent::ResponseMcpCallArgumentsDeltaEvent(Box::new(
                    ResponseMcpCallArgumentsDeltaEvent::new(
                        McpCallArgumentsDeltaType::ResponseMcpCallArgumentsDelta,
                        *output_index,
                        item_id.clone(),
                        chunk,
                        ctx.next_seq(),
                    ),
                )));
            }
            out.push(ResponsesServerEvent::ResponseMcpCallArgumentsDoneEvent(Box::new(
                ResponseMcpCallArgumentsDoneEvent::new(
                    McpCallArgumentsDoneType::ResponseMcpCallArgumentsDone,
                    *output_index,
                    item_id.clone(),
                    arguments.clone(),
                    ctx.next_seq(),
                ),
            )));
            out.push(ResponsesServerEvent::ResponseMcpCallCompletedEvent(Box::new(
                ResponseMcpCallCompletedEvent::new(
                    McpCallCompletedType::ResponseMcpCallCompleted,
                    item_id.clone(),
                    *output_index,
                    ctx.next_seq(),
                ),
            )));
            let mut finalized = McpToolCall::new(
                McpToolCallType::McpCall,
                item_id.clone(),
                server_label.clone(),
                name.clone(),
                arguments.clone(),
            );
            finalized.output = call_output.clone().map(Some);
            finalized.error = error.clone().map(Some);
            finalized.status = Some(
                status
                    .as_deref()
                    .and_then(parse_mcp_status)
                    .unwrap_or(McpToolCallStatus::Completed),
            );
            finalized.approval_request_id = approval_request_id.clone().map(Some);
            out.push(output_item_done(
                *output_index,
                ctx.next_seq(),
                OutputItem::McpToolCall(Box::new(finalized.clone())),
            ));
            ctx.items.push(OutputItem::McpToolCall(Box::new(finalized)));
            out
        }

        // `mcp_approval_request`: no lifecycle sub-events — just
        // `output_item.added` → `output_item.done`.
        AgentEventKind::ResponseMcpApprovalRequestDone {
            item_id,
            output_index,
            server_label,
            name,
            arguments,
            ..
        } => {
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                let skeleton = McpApprovalRequest::new(
                    McpApprovalRequestType::McpApprovalRequest,
                    item_id.clone(),
                    server_label.clone(),
                    name.clone(),
                    arguments.clone(),
                );
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::McpApprovalRequest(Box::new(skeleton)),
                ));
            }
            let finalized = McpApprovalRequest::new(
                McpApprovalRequestType::McpApprovalRequest,
                item_id.clone(),
                server_label.clone(),
                name.clone(),
                arguments.clone(),
            );
            out.push(output_item_done(
                *output_index,
                ctx.next_seq(),
                OutputItem::McpApprovalRequest(Box::new(finalized.clone())),
            ));
            ctx.items.push(OutputItem::McpApprovalRequest(Box::new(finalized)));
            out
        }

        // --- Function-shell command streaming -----------------------------
        // `shell_call_command.delta`: on the first delta for a shell_call
        // item, emit `output_item.added` (in_progress shell_call skeleton with
        // the response-level environment) and `shell_call_command.added`
        // (empty command). Each delta emits `shell_call_command.delta`.
        AgentEventKind::ResponseShellCallCommandDelta {
            item_id,
            call_id,
            output_index,
            delta,
            ..
        } => {
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    skeleton_shell_call(item_id, call_id, &ctx.shell_environment),
                ));
                out.push(ResponsesServerEvent::ResponseShellCallCommandAddedEvent(Box::new(
                    ResponseShellCallCommandAddedEvent::new(
                        ShellCallCommandAddedType::ResponseShellCallCommandAdded,
                        String::new(),
                        0,
                        *output_index,
                        ctx.next_seq(),
                    ),
                )));
            }
            out.push(ResponsesServerEvent::ResponseShellCallCommandDeltaEvent(Box::new(
                ResponseShellCallCommandDeltaEvent::new(
                    ShellCallCommandDeltaType::ResponseShellCallCommandDelta,
                    0,
                    delta.clone(),
                    String::new(),
                    *output_index,
                    ctx.next_seq(),
                ),
            )));
            out
        }
        // `shell_call_command.done`: emit `shell_call_command.done` (the
        // finalized single command) and `output_item.done` (completed
        // shell_call carrying the full `commands` array + environment).
        AgentEventKind::ResponseShellCallCommandDone {
            item_id,
            call_id,
            output_index,
            commands,
            ..
        } => {
            let command = commands.first().cloned().unwrap_or_default();
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    skeleton_shell_call(item_id, call_id, &ctx.shell_environment),
                ));
                out.push(ResponsesServerEvent::ResponseShellCallCommandAddedEvent(Box::new(
                    ResponseShellCallCommandAddedEvent::new(
                        ShellCallCommandAddedType::ResponseShellCallCommandAdded,
                        String::new(),
                        0,
                        *output_index,
                        ctx.next_seq(),
                    ),
                )));
            }
            out.push(ResponsesServerEvent::ResponseShellCallCommandDoneEvent(Box::new(
                ResponseShellCallCommandDoneEvent::new(
                    ShellCallCommandDoneType::ResponseShellCallCommandDone,
                    command,
                    0,
                    *output_index,
                    ctx.next_seq(),
                ),
            )));
            let finalized =
                finalized_shell_call(item_id, call_id, commands.clone(), &ctx.shell_environment);
            out.push(output_item_done(*output_index, ctx.next_seq(), finalized.clone()));
            ctx.items.push(finalized);
            out
        }

        // --- Function-shell output streaming ------------------------------
        AgentEventKind::ResponseShellCallOutputContentDelta {
            item_id,
            call_id,
            output_index,
            delta,
            ..
        } => {
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    skeleton_shell_call_output(item_id, call_id),
                ));
            }
            out.push(ResponsesServerEvent::ResponseShellCallOutputContentDeltaEvent(Box::new(
                ResponseShellCallOutputContentDeltaEvent::new(
                    ShellCallOutputContentDeltaType::ResponseShellCallOutputContentDelta,
                    0,
                    ShellCallOutputContentDelta::new(Some(delta.clone()), None),
                    item_id.clone(),
                    *output_index,
                    ctx.next_seq(),
                ),
            )));
            out
        }
        AgentEventKind::ResponseShellCallOutputContentDone {
            item_id,
            call_id,
            output_index,
            outputs,
            ..
        } => {
            let contents: Vec<slab_proto::openai::FunctionShellCallOutputContent> =
                outputs.iter().filter_map(parse_shell_output_content).collect();
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    skeleton_shell_call_output(item_id, call_id),
                ));
            }
            out.push(ResponsesServerEvent::ResponseShellCallOutputContentDoneEvent(Box::new(
                ResponseShellCallOutputContentDoneEvent::new(
                    ShellCallOutputContentDoneType::ResponseShellCallOutputContentDone,
                    0,
                    item_id.clone(),
                    contents.clone(),
                    *output_index,
                    ctx.next_seq(),
                ),
            )));
            let finalized = FunctionShellCallOutput::new(
                FunctionShellCallOutputType::ShellCallOutput,
                item_id.clone(),
                call_id.clone(),
                FunctionShellCallOutputStatusEnum::Completed,
                contents,
                None,
            );
            out.push(output_item_done(
                *output_index,
                ctx.next_seq(),
                OutputItem::FunctionShellCallOutput(Box::new(finalized.clone())),
            ));
            ctx.items.push(OutputItem::FunctionShellCallOutput(Box::new(finalized)));
            out
        }

        // --- Code-interpreter streaming -----------------------------------
        // slab emits a single `ResponseCodeInterpreterCallDone`; the adapter
        // synthesizes the full lifecycle. Code deltas are a re-split of the
        // finalized code string (N pinned on `ctx`; slab lacks a code delta
        // variant).
        AgentEventKind::ResponseCodeInterpreterCallDone {
            item_id,
            output_index,
            code,
            container_id,
            outputs,
            ..
        } => {
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                let skeleton = slab_proto::openai::CodeInterpreterToolCall {
                    r#type: slab_proto::openai::CodeInterpreterToolCallType::CodeInterpreterCall,
                    id: Some(item_id.clone()),
                    status: Some("in_progress".to_owned()),
                    code: Some(String::new()),
                    container_id: container_id.clone(),
                    outputs: Some(Vec::new()),
                };
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::CodeInterpreterToolCall(Box::new(skeleton)),
                ));
            }
            out.push(ResponsesServerEvent::ResponseCodeInterpreterCallInProgressEvent(Box::new(
                ResponseCodeInterpreterCallInProgressEvent::new(
                    CodeInProgressType::ResponseCodeInterpreterCallInProgress,
                    *output_index,
                    item_id.clone(),
                    ctx.next_seq(),
                ),
            )));
            let n = ctx.tool_delta_splits.get(item_id).copied().unwrap_or(1);
            for chunk in split_string_into(code, n) {
                out.push(ResponsesServerEvent::ResponseCodeInterpreterCallCodeDeltaEvent(
                    Box::new(ResponseCodeInterpreterCallCodeDeltaEvent::new(
                        CodeDeltaType::ResponseCodeInterpreterCallCodeDelta,
                        *output_index,
                        item_id.clone(),
                        chunk,
                        ctx.next_seq(),
                    )),
                ));
            }
            out.push(ResponsesServerEvent::ResponseCodeInterpreterCallCodeDoneEvent(Box::new(
                ResponseCodeInterpreterCallCodeDoneEvent::new(
                    CodeDoneType::ResponseCodeInterpreterCallCodeDone,
                    *output_index,
                    item_id.clone(),
                    code.clone(),
                    ctx.next_seq(),
                ),
            )));
            out.push(ResponsesServerEvent::ResponseCodeInterpreterCallInterpretingEvent(Box::new(
                ResponseCodeInterpreterCallInterpretingEvent::new(
                    CodeInterpretingType::ResponseCodeInterpreterCallInterpreting,
                    *output_index,
                    item_id.clone(),
                    ctx.next_seq(),
                ),
            )));
            out.push(ResponsesServerEvent::ResponseCodeInterpreterCallCompletedEvent(Box::new(
                ResponseCodeInterpreterCallCompletedEvent::new(
                    CodeCompletedType::ResponseCodeInterpreterCallCompleted,
                    *output_index,
                    item_id.clone(),
                    ctx.next_seq(),
                ),
            )));
            let finalized = slab_proto::openai::CodeInterpreterToolCall {
                r#type: slab_proto::openai::CodeInterpreterToolCallType::CodeInterpreterCall,
                id: Some(item_id.clone()),
                status: Some("completed".to_owned()),
                code: Some(code.clone()),
                container_id: container_id.clone(),
                outputs: if outputs.is_empty() { None } else { Some(outputs.clone()) },
            };
            out.push(output_item_done(
                *output_index,
                ctx.next_seq(),
                OutputItem::CodeInterpreterToolCall(Box::new(finalized.clone())),
            ));
            ctx.items.push(OutputItem::CodeInterpreterToolCall(Box::new(finalized)));
            out
        }

        // --- Web-search streaming -----------------------------------------
        // No delta stream: `output_item.added` → `web_search_call.in_progress`
        // → `web_search_call.searching` → `web_search_call.completed` →
        // `output_item.done`. The added skeleton cannot carry the action
        // (slab-agent only surfaces it on Done); the test strips `action` from
        // the fixture's added skeleton on both sides.
        AgentEventKind::ResponseWebSearchCallDone { item_id, output_index, action, .. } => {
            let parsed_action =
                serde_json::from_value::<slab_proto::openai::WebSearchToolCallAction>(
                    action.clone(),
                )
                .unwrap_or_else(|_| slab_proto::openai::WebSearchToolCallAction::default());
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                let skeleton = slab_proto::openai::WebSearchToolCall::new(
                    item_id.clone(),
                    slab_proto::openai::WebSearchToolCallType::WebSearchCall,
                    slab_proto::openai::ToolStatus::InProgress,
                    parsed_action.clone(),
                );
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::WebSearchToolCall(Box::new(skeleton)),
                ));
            }
            out.push(ResponsesServerEvent::ResponseWebSearchCallInProgressEvent(Box::new(
                ResponseWebSearchCallInProgressEvent::new(
                    WebSearchInProgressType::ResponseWebSearchCallInProgress,
                    *output_index,
                    item_id.clone(),
                    ctx.next_seq(),
                ),
            )));
            out.push(ResponsesServerEvent::ResponseWebSearchCallSearchingEvent(Box::new(
                ResponseWebSearchCallSearchingEvent::new(
                    WebSearchSearchingType::ResponseWebSearchCallSearching,
                    *output_index,
                    item_id.clone(),
                    ctx.next_seq(),
                ),
            )));
            out.push(ResponsesServerEvent::ResponseWebSearchCallCompletedEvent(Box::new(
                ResponseWebSearchCallCompletedEvent::new(
                    WebSearchCompletedType::ResponseWebSearchCallCompleted,
                    *output_index,
                    item_id.clone(),
                    ctx.next_seq(),
                ),
            )));
            let finalized = slab_proto::openai::WebSearchToolCall::new(
                item_id.clone(),
                slab_proto::openai::WebSearchToolCallType::WebSearchCall,
                slab_proto::openai::ToolStatus::Completed,
                parsed_action,
            );
            out.push(output_item_done(
                *output_index,
                ctx.next_seq(),
                OutputItem::WebSearchToolCall(Box::new(finalized.clone())),
            ));
            ctx.items.push(OutputItem::WebSearchToolCall(Box::new(finalized)));
            out
        }

        // --- File-search streaming ----------------------------------------
        // No delta stream: `output_item.added` (in_progress, empty queries,
        // null results) → `file_search_call.in_progress` → `.searching` →
        // `.completed` → `output_item.done`.
        AgentEventKind::ResponseFileSearchCallDone {
            item_id,
            output_index,
            queries,
            results,
            ..
        } => {
            let mut out = Vec::new();
            if ctx.seen_items.insert(item_id.clone()) {
                let skeleton = slab_proto::openai::FileSearchToolCall {
                    r#type: slab_proto::openai::FileSearchToolCallType::FileSearchCall,
                    id: Some(item_id.clone()),
                    status: Some("in_progress".to_owned()),
                    queries: Some(Vec::new()),
                    results: None,
                };
                out.push(output_item_added(
                    *output_index,
                    ctx.next_seq(),
                    OutputItem::FileSearchToolCall(Box::new(skeleton)),
                ));
            }
            out.push(ResponsesServerEvent::ResponseFileSearchCallInProgressEvent(Box::new(
                ResponseFileSearchCallInProgressEvent::new(
                    ResponseFileSearchCallInProgressEventType::ResponseFileSearchCallInProgress,
                    *output_index,
                    item_id.clone(),
                    ctx.next_seq(),
                ),
            )));
            out.push(ResponsesServerEvent::ResponseFileSearchCallSearchingEvent(Box::new(
                ResponseFileSearchCallSearchingEvent::new(
                    ResponseFileSearchCallSearchingEventType::ResponseFileSearchCallSearching,
                    *output_index,
                    item_id.clone(),
                    ctx.next_seq(),
                ),
            )));
            out.push(ResponsesServerEvent::ResponseFileSearchCallCompletedEvent(Box::new(
                ResponseFileSearchCallCompletedEvent::new(
                    ResponseFileSearchCallCompletedEventType::ResponseFileSearchCallCompleted,
                    *output_index,
                    item_id.clone(),
                    ctx.next_seq(),
                ),
            )));
            let finalized = slab_proto::openai::FileSearchToolCall {
                r#type: slab_proto::openai::FileSearchToolCallType::FileSearchCall,
                id: Some(item_id.clone()),
                status: Some("completed".to_owned()),
                queries: Some(queries.clone()),
                results: results.clone(),
            };
            out.push(output_item_done(
                *output_index,
                ctx.next_seq(),
                OutputItem::FileSearchToolCall(Box::new(finalized.clone())),
            ));
            ctx.items.push(OutputItem::FileSearchToolCall(Box::new(finalized)));
            out
        }

        _ => Vec::new(),
    }
}

/// Build a `response.output_item.added` wrapper event.
fn output_item_added(output_index: i32, seq: i32, item: OutputItem) -> ResponsesServerEvent {
    ResponsesServerEvent::ResponseOutputItemAddedEvent(Box::new(ResponseOutputItemAddedEvent::new(
        OutputItemAddedType::ResponseOutputItemAdded,
        output_index,
        seq,
        item,
    )))
}

/// Build a `response.output_item.done` wrapper event.
fn output_item_done(output_index: i32, seq: i32, item: OutputItem) -> ResponsesServerEvent {
    ResponsesServerEvent::ResponseOutputItemDoneEvent(Box::new(ResponseOutputItemDoneEvent::new(
        OutputItemDoneType::ResponseOutputItemDone,
        output_index,
        seq,
        item,
    )))
}

/// Build an in-progress skeleton message item for `output_item.added`.
fn skeleton_message(item_id: &str) -> OutputItem {
    OutputItem::OutputMessage(Box::new(OutputMessage {
        id: item_id.to_owned(),
        r#type: CommonOutputType::Message,
        role: OutputMessageRole::Assistant,
        content: Vec::new(),
        status: Status::InProgress,
        phase: None,
    }))
}

/// Build a `response.content_part.added` event.
fn content_part_added(
    item_id: &str,
    output_index: i32,
    content_index: i32,
    seq: i32,
    content: OutputTextContent,
) -> ResponsesServerEvent {
    ResponsesServerEvent::ResponseContentPartAddedEvent(Box::new(
        ResponseContentPartAddedEvent::new(
            ContentPartAddedType::ResponseContentPartAdded,
            item_id.to_owned(),
            output_index,
            content_index,
            OutputContent::OutputTextContent(Box::new(content)),
            seq,
        ),
    ))
}

/// Build a `response.content_part.done` event.
fn content_part_done(
    item_id: &str,
    output_index: i32,
    content_index: i32,
    seq: i32,
    content: OutputTextContent,
) -> ResponsesServerEvent {
    ResponsesServerEvent::ResponseContentPartDoneEvent(Box::new(ResponseContentPartDoneEvent::new(
        ContentPartDoneType::ResponseContentPartDone,
        item_id.to_owned(),
        output_index,
        content_index,
        seq,
        OutputContent::OutputTextContent(Box::new(content)),
    )))
}

/// Split a finalized payload string into exactly `n` chunks for delta-stream
/// synthesis. The canonical stream carries N delta events whose individual
/// contents are redacted away by the test normalizer; only the COUNT is
/// load-bearing. Chunks are distributed as evenly as possible over the UTF-8
/// char range, with trailing empty strings when `n` exceeds the char count, so
/// the returned vec always has length `n` (or 1 when `n == 0`).
fn split_string_into(s: &str, n: usize) -> Vec<String> {
    if n <= 1 {
        return vec![s.to_owned()];
    }
    let chars: Vec<char> = s.chars().collect();
    let total = chars.len();
    let per = total.div_ceil(n).max(1);
    let mut out: Vec<String> = Vec::new();
    let mut idx = 0;
    while idx < total {
        let end = (idx + per).min(total);
        out.push(chars[idx..end].iter().collect());
        idx = end;
    }
    while out.len() < n {
        out.push(String::new());
    }
    out
}

/// Resolve the response-level shell environment pinned on `ctx` into the typed
/// `FunctionShellCallEnvironment` shape (None when no environment was set).
fn shell_env_from_ctx(
    env: &Option<(String, Option<String>)>,
) -> Option<slab_proto::openai::FunctionShellCallEnvironment> {
    env.as_ref().map(|(ty, container_id)| slab_proto::openai::FunctionShellCallEnvironment {
        r#type: Some(ty.clone()),
        container_id: container_id.clone(),
    })
}

/// Build the in-progress `shell_call` skeleton used by `output_item.added`.
fn skeleton_shell_call(
    item_id: &str,
    call_id: &str,
    env: &Option<(String, Option<String>)>,
) -> OutputItem {
    OutputItem::FunctionShellCall(Box::new(FunctionShellCall::new(
        FunctionShellCallType::ShellCall,
        item_id.to_owned(),
        call_id.to_owned(),
        FunctionShellAction::new(Vec::new(), None, None),
        FunctionShellCallStatus::InProgress,
        shell_env_from_ctx(env),
    )))
}

/// Build the finalized `shell_call` item used by `output_item.done`.
fn finalized_shell_call(
    item_id: &str,
    call_id: &str,
    commands: Vec<String>,
    env: &Option<(String, Option<String>)>,
) -> OutputItem {
    OutputItem::FunctionShellCall(Box::new(FunctionShellCall::new(
        FunctionShellCallType::ShellCall,
        item_id.to_owned(),
        call_id.to_owned(),
        FunctionShellAction::new(commands, None, None),
        FunctionShellCallStatus::Completed,
        shell_env_from_ctx(env),
    )))
}

/// Build the completed `shell_call_output` skeleton (status is already
/// `completed` in the canonical `output_item.added` skeleton).
fn skeleton_shell_call_output(item_id: &str, call_id: &str) -> OutputItem {
    OutputItem::FunctionShellCallOutput(Box::new(FunctionShellCallOutput::new(
        FunctionShellCallOutputType::ShellCallOutput,
        item_id.to_owned(),
        call_id.to_owned(),
        FunctionShellCallOutputStatusEnum::Completed,
        Vec::new(),
        None,
    )))
}
