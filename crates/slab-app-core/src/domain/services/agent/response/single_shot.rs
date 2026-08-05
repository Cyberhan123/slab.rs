//! Single-shot Model orchestration for `/responses`.
//!
//! Slice C2 turns `/responses` into a standalone single-shot Model: one LLM call
//! via `llm-service`, OpenAI Responses wire produced by feeding *synthesized*
//! [`AgentEventKind`] envelopes into the existing pure projections
//! ([`super::projection::build_response`] / [`super::stream::envelope_to_events`]).
//!
//! This module owns:
//! - the shared outcome / terminal / frame types used across the projections and
//!   the transport,
//! - [`synthesize_envelopes`] — the pure mapping from a finalized LLM outcome to a
//!   slab event sequence + terminal kind,
//! - the orchestration helpers (resolve model → route → call `llm-service` →
//!   persist) invoked by `ResponseService`.
//!
//! See `slab-agent-3-snuggly-eich.md` (slice C2) for the design.

use chrono::Utc;
use futures::stream::{self, BoxStream, StreamExt};
use slab_agent::port::{ThreadMessageRecord, ThreadSnapshot};
use slab_agent::{AgentConfig, ThreadStatus};
use slab_proto::openai::{Reason, Response};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use super::event::{AgentEventEnvelope, AgentEventKind, AgentResponseRef, TurnEvent};
use super::projection::{AdapterInput, apply_terminal, build_response};
use crate::context::ModelState;
use crate::domain::models::{
    ChatCompletionCommand, ChatCompletionOutput, ChatCompletionResult, CloudChatParams,
    CommonChatParams, ConversationMessage, ConversationMessageContent, ConversationToolCall,
    LocalChatParams, TextGenerationUsage,
};
use crate::domain::services::agent::{AgentCore, maybe_compact_messages};
use crate::domain::services::chat::ChatService;
use crate::domain::services::chat::local::{LocalChatRequestConfig, build_local_runtime_request};
use crate::domain::services::llm::cloud::{
    CloudChatRequestConfig, CloudDelta, cloud_chat_stream, resolve_cloud_model,
};
use crate::domain::services::llm::local::{local_chat_stream, text_usage_from_runtime};
use crate::domain::services::llm::should_route_to_cloud;
use crate::error::AppCoreError;
use crate::schemas::agent::OpenAICreateRequest;

/// Terminal outcome of the single-shot LLM call.
///
/// `Empty` covers the degenerate "model produced neither text, reasoning, nor
/// tool calls" case (terminal `Completed` with empty output); `Failed` carries
/// the error surface mapped to `response.failed`.
#[derive(Debug, Clone, Default)]
pub(crate) enum SingleShotOutcome {
    #[default]
    Empty,
    Completed {
        text: Option<String>,
        reasoning: Option<String>,
        tool_calls: Vec<ConversationToolCall>,
        usage: Option<TextGenerationUsage>,
    },
    Failed {
        message: String,
        code: Option<String>,
        error_type: Option<String>,
    },
}

impl SingleShotOutcome {
    /// `true` when the outcome carries at least one tool call (drives the
    /// client-side loop: terminal `Incomplete { reason: ToolCalls }`).
    pub(crate) fn has_tool_calls(&self) -> bool {
        matches!(
            self,
            SingleShotOutcome::Completed { tool_calls, .. } if !tool_calls.is_empty()
        )
    }

    pub(crate) fn usage(&self) -> Option<&TextGenerationUsage> {
        match self {
            SingleShotOutcome::Completed { usage, .. } => usage.as_ref(),
            _ => None,
        }
    }
}

/// How the single-shot response terminates on the wire.
#[derive(Debug, Clone)]
pub enum TerminalKind {
    Completed,
    Incomplete { reason: Reason },
    Failed { message: String, code: Option<String>, error_type: Option<String> },
}

impl TerminalKind {
    /// Derive the terminal kind from a finalized outcome:
    /// failure → `Failed`; tool calls present → `Incomplete { ToolCalls }`
    /// (client-side loop); otherwise → `Completed`.
    pub(crate) fn from_outcome(outcome: &SingleShotOutcome) -> Self {
        match outcome {
            SingleShotOutcome::Failed { message, code, error_type } => Self::Failed {
                message: message.clone(),
                code: code.clone(),
                error_type: error_type.clone(),
            },
            other if other.has_tool_calls() => Self::Incomplete { reason: Reason::ToolCalls },
            _ => Self::Completed,
        }
    }
}

/// One frame produced by a streaming single-shot run.
#[derive(Clone)]
pub enum StreamFrame {
    /// A slab agent event to be expanded into 0..N wire events by
    /// [`super::stream::envelope_to_events`].
    Envelope(AgentEventEnvelope),
    /// The terminal `response.completed` / `response.incomplete` /
    /// `response.failed` event (expanded by
    /// [`super::stream::build_terminal_event`]), carrying the finalized token
    /// usage (when the backend reported any) so the terminal event can populate
    /// `response.usage` instead of defaulting to zero.
    Terminal(TerminalKind, Option<TextGenerationUsage>),
}

// ── Envelope constructors ───────────────────────────────────────────────────
//
// Small constructors shared by the non-streaming assembler
// ([`synthesize_envelopes`]) and the streaming orchestration in `ResponseService`.
// Each takes an explicit envelope `id` (the per-stream monotonic ordering key;
// the projections never read it — the handler uses it for SSE `Last-Event-Id`).

fn envelope(id: u64, kind: AgentEventKind) -> AgentEventEnvelope {
    AgentEventEnvelope { id, event: TurnEvent::Response { turn_index: Some(0), event: kind } }
}

fn response_ref(response_id: &str, status: ThreadStatus) -> AgentResponseRef {
    AgentResponseRef { id: response_id.to_owned(), status }
}

pub(crate) fn queued_envelope(id: u64, response_id: &str) -> AgentEventEnvelope {
    envelope(
        id,
        AgentEventKind::ResponseQueued {
            response: response_ref(response_id, ThreadStatus::Pending),
        },
    )
}

pub(crate) fn in_progress_envelope(id: u64, response_id: &str) -> AgentEventEnvelope {
    envelope(
        id,
        AgentEventKind::ResponseInProgress {
            response: response_ref(response_id, ThreadStatus::Running),
        },
    )
}

pub(crate) fn text_delta_envelope(
    id: u64,
    item_id: &str,
    output_index: i32,
    delta: &str,
) -> AgentEventEnvelope {
    envelope(
        id,
        AgentEventKind::ResponseOutputTextDelta {
            item_id: item_id.to_owned(),
            output_index,
            content_index: 0,
            delta: delta.to_owned(),
        },
    )
}

pub(crate) fn text_done_envelope(
    id: u64,
    item_id: &str,
    output_index: i32,
    text: &str,
) -> AgentEventEnvelope {
    envelope(
        id,
        AgentEventKind::ResponseOutputTextDone {
            item_id: item_id.to_owned(),
            output_index,
            content_index: 0,
            text: text.to_owned(),
            artifact_refs: Vec::new(),
            reason: None,
            phase: Some("final_answer".to_owned()),
        },
    )
}

pub(crate) fn reasoning_delta_envelope(
    id: u64,
    item_id: &str,
    output_index: i32,
    delta: &str,
) -> AgentEventEnvelope {
    envelope(
        id,
        AgentEventKind::ResponseReasoningTextDelta {
            item_id: item_id.to_owned(),
            output_index,
            content_index: 0,
            delta: delta.to_owned(),
        },
    )
}

pub(crate) fn reasoning_done_envelope(
    id: u64,
    item_id: &str,
    output_index: i32,
    text: &str,
) -> AgentEventEnvelope {
    let owned = text.to_owned();
    envelope(
        id,
        AgentEventKind::ResponseReasoningTextDone {
            item_id: item_id.to_owned(),
            output_index,
            content_index: 0,
            text: owned.clone(),
            encrypted_content: Some(owned.clone()),
            summary: Some(owned),
        },
    )
}

pub(crate) fn function_call_done_envelope(
    id: u64,
    item_id: &str,
    call_id: &str,
    name: &str,
    arguments: &str,
    output_index: i32,
) -> AgentEventEnvelope {
    envelope(
        id,
        AgentEventKind::ResponseFunctionCallArgumentsDone {
            item_id: item_id.to_owned(),
            call_id: call_id.to_owned(),
            name: name.to_owned(),
            output_index,
            arguments: arguments.to_owned(),
            namespace: None,
            risk: None,
        },
    )
}

fn nonempty(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|s| !s.is_empty())
}

/// Synthesize the full slab event sequence for a finalized single-shot outcome:
/// `response.queued` → `response.in_progress` → (reasoning delta+done if any) →
/// (text delta+done if any) → (`function_call_arguments.done` per tool call).
///
/// The terminal event is returned separately as a [`TerminalKind`] — the
/// non-streaming path feeds the envelopes to
/// [`super::projection::build_response`] + [`super::projection::apply_terminal`];
/// the burst-streaming path (tools present, or non-streaming internal) emits each
/// envelope followed by `StreamFrame::Terminal`. The per-token streaming path
/// (no tools) builds its delta envelopes inline and only reuses these
/// constructors for the `*Done` tail.
pub(crate) fn synthesize_envelopes(
    response_id: &str,
    outcome: &SingleShotOutcome,
) -> (Vec<AgentEventEnvelope>, TerminalKind) {
    let terminal = TerminalKind::from_outcome(outcome);
    let mut envs = Vec::new();
    let mut id: u64 = 0;

    envs.push(queued_envelope(id, response_id));
    id += 1;
    envs.push(in_progress_envelope(id, response_id));
    id += 1;

    if let SingleShotOutcome::Completed { text, reasoning, tool_calls, .. } = outcome {
        let mut output_index: i32 = 0;
        if let Some(r) = nonempty(reasoning) {
            envs.push(reasoning_delta_envelope(id, "rs_0", output_index, r));
            id += 1;
            envs.push(reasoning_done_envelope(id, "rs_0", output_index, r));
            id += 1;
            output_index += 1;
        }
        if let Some(t) = nonempty(text) {
            envs.push(text_delta_envelope(id, "msg_0", output_index, t));
            id += 1;
            envs.push(text_done_envelope(id, "msg_0", output_index, t));
            id += 1;
            output_index += 1;
        }
        for (i, tc) in tool_calls.iter().enumerate() {
            let call_id = tc.id.clone().unwrap_or_else(|| format!("call_{i}"));
            envs.push(function_call_done_envelope(
                id,
                &format!("fc_{i}"),
                &call_id,
                &tc.function.name,
                &tc.function.arguments,
                output_index,
            ));
            id += 1;
            output_index += 1;
        }
    }

    (envs, terminal)
}

// ── Single-shot orchestration ────────────────────────────────────────────────
//
// `/responses` performs ONE LLM completion — via `ChatService`, which owns the
// cloud/local routing + prompt engineering over `llm-service` — and feeds the
// result through the pure projections. No slab-agent turn loop, no
// `AgentEventHub` subscription. Thread-store persistence is reused so
// `previous_response_id` (= thread id) keeps chaining. The request may carry
// `tools` (function definitions); when the model returns tool calls the
// terminal is `Incomplete { ToolCalls }` and the client drives the tool loop by
// POSTing again with `previous_response_id` + `function_call_output` input.

const DEFAULT_RESPONSE_MAX_TOKENS: u32 = 1024;

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn system_message(text: &str) -> ConversationMessage {
    ConversationMessage {
        role: "system".into(),
        content: ConversationMessageContent::Text(text.to_owned()),
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
    }
}

fn assistant_message(text: &str, tool_calls: &[ConversationToolCall]) -> ConversationMessage {
    ConversationMessage {
        role: "assistant".into(),
        content: ConversationMessageContent::Text(text.to_owned()),
        name: None,
        tool_call_id: None,
        tool_calls: tool_calls.to_vec(),
    }
}

/// Resolved inputs for one `/responses` run: the response/thread id, the full
/// message list for the LLM (`[system(instr)?] ++ history ++ new input`), the
/// new user messages to persist, the turn index for persistence, and whether a
/// fresh thread must be created.
struct ResolvedInput {
    response_id: String,
    messages: Vec<ConversationMessage>,
    new_user_messages: Vec<ConversationMessage>,
    turn_index: u32,
    is_new: bool,
}

async fn resolve_input(
    core: &AgentCore,
    req: &OpenAICreateRequest,
) -> Result<ResolvedInput, AppCoreError> {
    let new_user_messages: Vec<ConversationMessage> =
        req.to_messages().into_iter().map(Into::into).collect();

    let (response_id, history, turn_index, is_new) =
        match req.previous_response_id.as_deref().filter(|s| !s.is_empty()) {
            Some(thread_id) => {
                if core.store().get_thread(thread_id).await?.is_none() {
                    return Err(AppCoreError::BadRequest(format!(
                        "unknown previous_response_id: {thread_id}"
                    )));
                }
                let records = core.reader().list_thread_messages(thread_id).await?;
                let next_turn =
                    records.iter().map(|r| r.turn_index).max().map(|m| m + 1).unwrap_or(0);
                let history = records.into_iter().map(|r| r.message).collect::<Vec<_>>();
                (thread_id.to_owned(), history, next_turn, false)
            }
            None => (format!("resp_{}", Uuid::new_v4().simple()), Vec::new(), 0, true),
        };

    let mut messages = Vec::new();
    if let Some(instr) = req.instructions.as_deref().filter(|s| !s.is_empty()) {
        messages.push(system_message(instr));
    }
    messages.extend(history);
    messages.extend(new_user_messages.clone());

    Ok(ResolvedInput { response_id, messages, new_user_messages, turn_index, is_new })
}

fn build_command(
    req: &OpenAICreateRequest,
    messages: Vec<ConversationMessage>,
    config: &AgentConfig,
) -> ChatCompletionCommand {
    ChatCompletionCommand {
        id: None,
        model: req.model.clone().unwrap_or_default(),
        messages,
        tools: req.function_tools(),
        agent_trace: None,
        continue_generation: false,
        common: CommonChatParams {
            max_tokens: config.max_tokens.or(Some(DEFAULT_RESPONSE_MAX_TOKENS)),
            temperature: config.temperature,
            top_p: config.top_p,
            top_k: config.top_k,
            min_p: config.min_p,
            presence_penalty: config.presence_penalty,
            repetition_penalty: config.repetition_penalty,
            n: 1,
            stream: false,
            stop: Vec::new(),
            stream_options: Default::default(),
        },
        local: LocalChatParams {
            gbnf: None,
            structured_output: config.structured_output.clone(),
            session_key: None,
            reasoning_guidance_in_context: false,
        },
        cloud: CloudChatParams {
            reasoning_effort: config.reasoning_effort,
            verbosity: config.verbosity,
            structured_output: config.structured_output.clone(),
        },
    }
}

fn outcome_from_chat_result(result: ChatCompletionResult) -> SingleShotOutcome {
    let Some(choice) = result.choices.into_iter().next() else {
        return SingleShotOutcome::Empty;
    };
    let text = match &choice.message.content {
        ConversationMessageContent::Text(t) if !t.is_empty() => Some(t.clone()),
        _ => None,
    };
    let tool_calls = choice.message.tool_calls;
    if text.is_none() && tool_calls.is_empty() {
        SingleShotOutcome::Empty
    } else {
        SingleShotOutcome::Completed { text, reasoning: None, tool_calls, usage: result.usage }
    }
}

async fn persist_input(
    core: &AgentCore,
    input: &ResolvedInput,
    session_id: &str,
    config: &AgentConfig,
) -> Result<(), AppCoreError> {
    if input.is_new {
        let now = now_rfc3339();
        core.store()
            .upsert_thread(&ThreadSnapshot {
                id: input.response_id.clone(),
                session_id: session_id.to_owned(),
                parent_id: None,
                depth: 0,
                status: ThreadStatus::Running,
                role_name: None,
                config_json: serde_json::to_string(config).unwrap_or_default(),
                completion_text: None,
                created_at: now.clone(),
                updated_at: now,
                archived_at: None,
            })
            .await?;
    }
    for msg in &input.new_user_messages {
        // Slice E.2 (option c): single_shot has NO turn loop and emits NO
        // `EventMsg`, so it is NOT an agent thread and the rollout persistence
        // observer never runs for it. Its conversation writes flow directly
        // through the `RolloutConversationStore::append_message` trait (the
        // app-core-internal out-of-band writer), bypassing the event hub.
        core.reader()
            .append_message(&ThreadMessageRecord {
                id: format!("msg_{}_{}", Uuid::new_v4().simple(), input.turn_index),
                thread_id: input.response_id.clone(),
                turn_index: input.turn_index,
                message: msg.clone(),
                created_at: now_rfc3339(),
            })
            .await?;
    }
    Ok(())
}

async fn persist_assistant_and_complete(
    core: &AgentCore,
    response_id: &str,
    turn_index: u32,
    outcome: &SingleShotOutcome,
) -> Result<(), AppCoreError> {
    match outcome {
        SingleShotOutcome::Completed { text, tool_calls, .. } => {
            let text = text.clone().unwrap_or_default();
            // Slice E.2 (option c): single_shot writes out-of-band via the
            // `RolloutConversationStore` trait (no EventMsg, no observer).
            core.reader()
                .append_message(&ThreadMessageRecord {
                    id: format!("msg_{}_{}", Uuid::new_v4().simple(), turn_index),
                    thread_id: response_id.to_owned(),
                    turn_index,
                    message: assistant_message(&text, tool_calls),
                    created_at: now_rfc3339(),
                })
                .await?;
            let completion = if text.is_empty() { None } else { Some(text.as_str()) };
            core.store()
                .update_thread_status(response_id, ThreadStatus::Completed, completion)
                .await?;
        }
        SingleShotOutcome::Empty => {
            core.store().update_thread_status(response_id, ThreadStatus::Completed, None).await?;
        }
        SingleShotOutcome::Failed { message, .. } => {
            // Stash the error message on the thread's completion_text so a later
            // GET can reconstruct `response.failed` instead of degrading to
            // `completed` (response state is not persisted separately).
            core.store()
                .update_thread_status(response_id, ThreadStatus::Errored, Some(message.as_str()))
                .await?;
        }
    }
    Ok(())
}

/// Map a finalized single-shot outcome, converting a provider error into a
/// [`SingleShotOutcome::Failed`] (OpenAI-faithful `response.failed`) instead of
/// propagating an HTTP error.
async fn run_llm_or_failure(
    state: &ModelState,
    command: ChatCompletionCommand,
) -> SingleShotOutcome {
    match run_llm_non_streaming(state, command).await {
        Ok(outcome) => outcome,
        Err(error) => SingleShotOutcome::Failed {
            message: error.to_string(),
            code: None,
            error_type: Some("server_error".to_owned()),
        },
    }
}

/// Reject requests that cannot produce a response. OpenAI Responses requires a
/// `model`, and the input must resolve to at least one message — restoring the
/// pre-refactor 400 (BadRequest) for malformed requests instead of letting them
/// reach runtime dispatch and surface as a 500.
fn validate_create_response_request(req: &OpenAICreateRequest) -> Result<(), AppCoreError> {
    if req.model.as_deref().map(str::trim).filter(|value| !value.is_empty()).is_none() {
        return Err(AppCoreError::BadRequest(
            "a `model` is required for /v1/agents/responses".to_owned(),
        ));
    }
    if req.to_messages().is_empty() {
        return Err(AppCoreError::BadRequest(
            "a non-empty `input` is required for /v1/agents/responses".to_owned(),
        ));
    }
    Ok(())
}

/// Run one non-streaming LLM call and return the canonical OpenAI [`Response`].
pub(crate) async fn run_create_response(
    core: &AgentCore,
    state: &ModelState,
    req: &OpenAICreateRequest,
    session_id: &str,
) -> Result<Response, AppCoreError> {
    let config: AgentConfig = req.to_config_input().into();
    validate_create_response_request(req)?;
    let mut input = resolve_input(core, req).await?;
    // Auto-compaction before the single LLM call. Non-fatal — log + continue.
    if let Err(error) =
        maybe_compact_messages(core.compact().as_ref(), &config.model, &mut input.messages, false)
            .await
    {
        tracing::warn!(%error, "context compaction skipped before /responses create");
    }
    let command = build_command(req, input.messages.clone(), &config);
    persist_input(core, &input, session_id, &config).await?;

    let outcome = run_llm_or_failure(state, command).await;
    persist_assistant_and_complete(core, &input.response_id, input.turn_index, &outcome).await?;

    let (envs, _terminal) = synthesize_envelopes(&input.response_id, &outcome);
    let model = req.model.clone().unwrap_or_default();
    let mut response = build_response(AdapterInput {
        response_id: &input.response_id,
        model: &model,
        created_at_unix: Utc::now().timestamp() as f64,
        envelopes: &envs,
        ..Default::default()
    });
    apply_terminal(&mut response, &outcome);
    Ok(response)
}

/// One normalized event from the underlying LLM token stream. Reasoning deltas
/// are intentionally dropped for now — `/responses` does not surface reasoning
/// (the non-streaming path sets `reasoning: None`), so streaming matches that
/// output shape; reasoning streaming is a follow-up.
enum DeltaEvent {
    Text(String),
    Done { usage: Option<TextGenerationUsage> },
    Failed(String),
}

/// Streaming state shared between the per-delta mapper and the terminal future
/// (run after the delta stream ends). Behind an `Arc<Mutex>` so the terminal can
/// read the finalized text/usage/failure.
#[derive(Clone, Default)]
struct StreamAccumulator {
    text: String,
    usage: Option<TextGenerationUsage>,
    failed: Option<String>,
    /// Next envelope id (the lifecycle prefix uses 0 and 1; deltas start at 2).
    next_id: u64,
}

/// Build the cloud chat request config from the `/responses` request + resolved
/// agent config (mirrors `chat/mod.rs`'s `CloudChatRequestConfig` mapping).
fn cloud_stream_config(req: &OpenAICreateRequest, config: &AgentConfig) -> CloudChatRequestConfig {
    CloudChatRequestConfig {
        max_tokens: config.max_tokens.unwrap_or(DEFAULT_RESPONSE_MAX_TOKENS),
        temperature: config.temperature.unwrap_or(0.7),
        top_p: config.top_p,
        structured_output: config.structured_output.clone(),
        reasoning_effort: config.reasoning_effort,
        verbosity: config.verbosity,
        tools: req.function_tools(),
        stream: true,
        include_usage: false,
    }
}

/// Build the local chat request config. Defaults mirror `chat/mod.rs`
/// (`temperature` 0.7, `max_tokens` DEFAULT_RESPONSE_MAX_TOKENS).
fn local_stream_config(req: &OpenAICreateRequest, config: &AgentConfig) -> LocalChatRequestConfig {
    LocalChatRequestConfig {
        session_id: None,
        max_tokens: config
            .max_tokens
            .or(Some(DEFAULT_RESPONSE_MAX_TOKENS))
            .unwrap_or(DEFAULT_RESPONSE_MAX_TOKENS),
        temperature: config.temperature.unwrap_or(0.7),
        top_p: config.top_p,
        top_k: config.top_k,
        min_p: config.min_p,
        presence_penalty: config.presence_penalty,
        repetition_penalty: config.repetition_penalty,
        reasoning_effort: config.reasoning_effort,
        verbosity: config.verbosity,
        reasoning_guidance_in_context: false,
        gbnf: None,
        structured_output: config.structured_output.clone(),
        tools: req.function_tools(),
        stop: Vec::new(),
        agent_trace: None,
        stream: true,
        include_usage: true,
    }
}

/// Resolve the route and start the LLM token stream, normalized to
/// [`DeltaEvent`]. Cloud uses [`cloud_chat_stream`] directly (text + reasoning
/// deltas); local uses [`build_local_runtime_request`] + [`local_chat_stream`]
/// and holds the inference guard for the whole stream.
async fn build_delta_events(
    state: &ModelState,
    req: &OpenAICreateRequest,
    input: &ResolvedInput,
    config: &AgentConfig,
) -> Result<BoxStream<'static, DeltaEvent>, AppCoreError> {
    let model = req.model.clone().unwrap_or_default();
    if should_route_to_cloud(state, &model).await? {
        let target = resolve_cloud_model(state, &model).await?;
        let cfg = cloud_stream_config(req, config);
        let trace_http = state.pmid().config().server.cloud_http_trace;
        let raw = cloud_chat_stream(&target, &input.messages, cfg, trace_http).await?;
        let mapped = raw.filter_map(|item| match item {
            Ok(CloudDelta::Content(t)) => futures::future::ready(Some(DeltaEvent::Text(t))),
            // Reasoning + tool-call chunks are not surfaced (see DeltaEvent doc).
            Ok(CloudDelta::Reasoning(_)) => futures::future::ready(None),
            Err(error) => futures::future::ready(Some(DeltaEvent::Failed(error.to_string()))),
        });
        Ok(Box::pin(mapped))
    } else {
        let cfg = local_stream_config(req, config);
        let built = build_local_runtime_request(state, &model, &input.messages, &cfg).await?;
        let (raw, guard) = local_chat_stream(state, built.backend_id, built.request).await?;
        let mapped = raw.flat_map(|item| match item {
            Ok(chunk) => {
                // A chunk is a reasoning chunk when the runtime tagged its
                // metadata with `reasoning_content`; drop those (reasoning is
                // not surfaced for `/responses`).
                let is_reasoning =
                    chunk.metadata.get("reasoning_content").map(|v| !v.is_null()).unwrap_or(false);
                let text =
                    if !is_reasoning && !chunk.delta.is_empty() { Some(chunk.delta) } else { None };
                let mut events: Vec<DeltaEvent> = Vec::new();
                if let Some(t) = text {
                    events.push(DeltaEvent::Text(t));
                }
                if chunk.done {
                    events
                        .push(DeltaEvent::Done { usage: chunk.usage.map(text_usage_from_runtime) });
                }
                stream::iter(events)
            }
            Err(error) => stream::iter(vec![DeltaEvent::Failed(error.to_string())]),
        });
        // Keep the inference guard alive for the whole stream so the model is
        // not auto-unloaded mid-stream (see `llm::local::local_chat_stream`).
        let with_guard = mapped.map(move |event| {
            let _keep_alive = &guard;
            event
        });
        Ok(Box::pin(with_guard))
    }
}

/// Map one delta event to 0..1 frames, mutating the shared accumulator.
fn map_delta_event(event: DeltaEvent, acc: &mut StreamAccumulator) -> Option<StreamFrame> {
    match event {
        DeltaEvent::Text(t) => {
            acc.text.push_str(&t);
            let id = acc.next_id;
            acc.next_id += 1;
            Some(StreamFrame::Envelope(text_delta_envelope(id, "msg_0", 0, &t)))
        }
        DeltaEvent::Done { usage } => {
            acc.usage = usage;
            None
        }
        DeltaEvent::Failed(message) => {
            acc.failed = Some(message);
            None
        }
    }
}

/// Derive the finalized single-shot outcome from the accumulator.
fn outcome_from_accumulator(acc: &StreamAccumulator) -> SingleShotOutcome {
    match &acc.failed {
        Some(message) => SingleShotOutcome::Failed {
            message: message.clone(),
            code: None,
            error_type: Some("server_error".to_owned()),
        },
        None => {
            let text = if acc.text.is_empty() { None } else { Some(acc.text.clone()) };
            if text.is_none() && acc.usage.is_none() {
                SingleShotOutcome::Empty
            } else {
                SingleShotOutcome::Completed {
                    text,
                    reasoning: None,
                    tool_calls: Vec::new(),
                    usage: acc.usage.clone(),
                }
            }
        }
    }
}

/// Build the tail frames (text-done + terminal) for a finalized outcome.
fn terminal_frames(acc: &StreamAccumulator, outcome: &SingleShotOutcome) -> Vec<StreamFrame> {
    let mut frames: Vec<StreamFrame> = Vec::new();
    if let SingleShotOutcome::Completed { text: Some(t), .. } = outcome
        && !t.is_empty()
    {
        frames.push(StreamFrame::Envelope(text_done_envelope(acc.next_id, "msg_0", 0, t)));
    }
    frames
        .push(StreamFrame::Terminal(TerminalKind::from_outcome(outcome), outcome.usage().cloned()));
    frames
}

/// Wrap a normalized delta-event stream into the `/responses` frame stream:
/// `queued` → `in_progress` → per-delta `text.delta` envelopes → (on end)
/// `text.done` + terminal. The terminal persists the finalized outcome.
fn build_frame_stream(
    events: BoxStream<'static, DeltaEvent>,
    acc: Arc<Mutex<StreamAccumulator>>,
    core: AgentCore,
    response_id: String,
    turn_index: u32,
) -> super::StreamFrameStream {
    let response_id_for_lifecycle = response_id.clone();
    let acc_for_deltas = Arc::clone(&acc);
    let delta_frames = events.filter_map(move |event| {
        let mut a = acc_for_deltas.lock().expect("stream accumulator lock");
        futures::future::ready(map_delta_event(event, &mut a))
    });

    let core_for_terminal = core;
    let response_id_for_terminal = response_id.clone();
    let acc_for_terminal = acc;
    let terminal = stream::once(async move {
        let a = acc_for_terminal.lock().expect("stream accumulator lock").clone();
        let outcome = outcome_from_accumulator(&a);
        persist_assistant_and_complete(
            &core_for_terminal,
            &response_id_for_terminal,
            turn_index,
            &outcome,
        )
        .await
        .ok();
        stream::iter(terminal_frames(&a, &outcome))
    })
    .flatten();

    let lifecycle = stream::iter([
        StreamFrame::Envelope(queued_envelope(0, &response_id_for_lifecycle)),
        StreamFrame::Envelope(in_progress_envelope(1, &response_id_for_lifecycle)),
    ]);
    Box::pin(lifecycle.chain(delta_frames).chain(terminal))
}

/// Run one `/responses` stream. Text-only requests stream token-by-token
/// (cloud via `cloud_chat_stream`, local via `local_chat_stream`); a request
/// carrying `tools` falls back to the whole-envelope burst (tool calls only
/// surface in the non-streaming result). A pre-stream error becomes a
/// `response.failed` (consistent with [`run_llm_or_failure`]).
pub(crate) async fn run_stream_response(
    core: &AgentCore,
    state: &ModelState,
    req: &OpenAICreateRequest,
    session_id: &str,
) -> Result<(String, super::StreamFrameStream), AppCoreError> {
    let config: AgentConfig = req.to_config_input().into();
    validate_create_response_request(req)?;
    let mut input = resolve_input(core, req).await?;
    // Auto-compaction before the streaming LLM call. Non-fatal — log + continue.
    if let Err(error) =
        maybe_compact_messages(core.compact().as_ref(), &config.model, &mut input.messages, false)
            .await
    {
        tracing::warn!(%error, "context compaction skipped before /responses stream");
    }
    persist_input(core, &input, session_id, &config).await?;

    // Burst fallback: a request carrying tools may yield tool_calls, which only
    // surface in the non-streaming result (cloud_chat_stream filters
    // ToolCallChunk; RuntimeTextGenerationChunk has no tool channel).
    if !req.function_tools().is_empty() {
        let command = build_command(req, input.messages.clone(), &config);
        let outcome = run_llm_or_failure(state, command).await;
        persist_assistant_and_complete(core, &input.response_id, input.turn_index, &outcome)
            .await?;
        let (envs, terminal) = synthesize_envelopes(&input.response_id, &outcome);
        let mut frames: Vec<StreamFrame> = envs.into_iter().map(StreamFrame::Envelope).collect();
        frames.push(StreamFrame::Terminal(terminal, outcome.usage().cloned()));
        return Ok((input.response_id, Box::pin(stream::iter(frames))));
    }

    let acc = Arc::new(Mutex::new(StreamAccumulator { next_id: 2, ..Default::default() }));
    let events = match build_delta_events(state, req, &input, &config).await {
        Ok(events) => events,
        Err(error) => {
            acc.lock().expect("stream accumulator lock").failed = Some(error.to_string());
            Box::pin(stream::empty())
        }
    };
    let stream =
        build_frame_stream(events, acc, core.clone(), input.response_id.clone(), input.turn_index);
    Ok((input.response_id, stream))
}

/// Reconstruct a [`Response`] for an already-completed run (GET SSE resume /
/// `get_response`). Best-effort: response state is not persisted, so this
/// rebuilds from the persisted thread messages. An errored thread reconstructs
/// `response.failed` (the error message was stashed on `completion_text` by
/// [`persist_assistant_and_complete`]). Usage/reasoning are not recovered
/// (documented limitation).
pub(crate) async fn run_get_response(
    core: &AgentCore,
    response_id: &str,
) -> Result<Response, AppCoreError> {
    let thread =
        core.store().get_thread(response_id).await?.ok_or_else(|| {
            AppCoreError::BadRequest(format!("unknown response id: {response_id}"))
        })?;
    let records = core.reader().list_thread_messages(response_id).await?;

    let outcome = if thread.status == ThreadStatus::Errored {
        SingleShotOutcome::Failed {
            message: thread.completion_text.clone().unwrap_or_else(|| "response failed".to_owned()),
            code: None,
            error_type: Some("server_error".to_owned()),
        }
    } else {
        let mut text_parts: Vec<String> = Vec::new();
        let mut tool_calls: Vec<ConversationToolCall> = Vec::new();
        for rec in &records {
            if rec.message.role == "assistant" {
                if let ConversationMessageContent::Text(t) = &rec.message.content
                    && !t.is_empty()
                {
                    text_parts.push(t.clone());
                }
                tool_calls.extend(rec.message.tool_calls.clone());
            }
        }
        if text_parts.is_empty() && tool_calls.is_empty() {
            SingleShotOutcome::Empty
        } else {
            let joined = text_parts.join("\n");
            SingleShotOutcome::Completed {
                text: if joined.is_empty() { None } else { Some(joined) },
                reasoning: None,
                tool_calls,
                usage: None,
            }
        }
    };

    let (envs, _terminal) = synthesize_envelopes(response_id, &outcome);
    // Recover the model id from the persisted AgentConfig (config_json is the
    // serialized AgentConfig written by persist_input); fall back to empty.
    let model = serde_json::from_str::<AgentConfig>(&thread.config_json)
        .ok()
        .map(|c| c.model)
        .unwrap_or_default();
    let mut response = build_response(AdapterInput {
        response_id,
        model: &model,
        created_at_unix: Utc::now().timestamp() as f64,
        envelopes: &envs,
        ..Default::default()
    });
    apply_terminal(&mut response, &outcome);
    Ok(response)
}

async fn run_llm_non_streaming(
    state: &ModelState,
    command: ChatCompletionCommand,
) -> Result<SingleShotOutcome, AppCoreError> {
    let chat = ChatService::new(state.clone());
    match chat.create_chat_completion(command).await? {
        ChatCompletionOutput::Json(result) => Ok(outcome_from_chat_result(result)),
        ChatCompletionOutput::Stream(_) => Err(AppCoreError::Internal(
            "chat service returned a stream for a non-stream /responses request".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::ConversationToolFunction;

    fn kind(env: &AgentEventEnvelope) -> &AgentEventKind {
        let TurnEvent::Response { event, .. } = &env.event;
        event
    }

    fn completed(
        text: Option<&str>,
        reasoning: Option<&str>,
        tool_calls: Vec<ConversationToolCall>,
    ) -> SingleShotOutcome {
        SingleShotOutcome::Completed {
            text: text.map(str::to_owned),
            reasoning: reasoning.map(str::to_owned),
            tool_calls,
            usage: None,
        }
    }

    fn tool_call(name: &str, args: &str) -> ConversationToolCall {
        ConversationToolCall {
            id: Some(format!("call_{name}")),
            r#type: "function".to_owned(),
            function: ConversationToolFunction {
                name: name.to_owned(),
                arguments: args.to_owned(),
            },
        }
    }

    #[test]
    fn terminal_from_outcome_tools_is_incomplete() {
        assert!(matches!(
            TerminalKind::from_outcome(&completed(None, None, vec![tool_call("foo", "{}")])),
            TerminalKind::Incomplete { reason: Reason::ToolCalls }
        ));
        assert!(matches!(
            TerminalKind::from_outcome(&completed(Some("hi"), None, Vec::new())),
            TerminalKind::Completed
        ));
        assert!(matches!(
            TerminalKind::from_outcome(&SingleShotOutcome::Failed {
                message: "boom".into(),
                code: None,
                error_type: None,
            }),
            TerminalKind::Failed { .. }
        ));
        assert!(matches!(
            TerminalKind::from_outcome(&SingleShotOutcome::Empty),
            TerminalKind::Completed
        ));
    }

    #[test]
    fn text_only_envelopes_and_indices() {
        let (envs, terminal) =
            synthesize_envelopes("resp_1", &completed(Some("hello"), None, Vec::new()));
        assert!(matches!(terminal, TerminalKind::Completed));
        // queued, in_progress, text delta, text done
        assert_eq!(envs.len(), 4);
        assert!(matches!(kind(&envs[0]), AgentEventKind::ResponseQueued { .. }));
        assert!(matches!(kind(&envs[1]), AgentEventKind::ResponseInProgress { .. }));
        match kind(&envs[2]) {
            AgentEventKind::ResponseOutputTextDelta { item_id, output_index, delta, .. } => {
                assert_eq!(item_id, "msg_0");
                assert_eq!(*output_index, 0);
                assert_eq!(delta, "hello");
            }
            other => panic!("unexpected delta: {other:?}"),
        }
        match kind(&envs[3]) {
            AgentEventKind::ResponseOutputTextDone {
                item_id, output_index, text, phase, ..
            } => {
                assert_eq!(item_id, "msg_0");
                assert_eq!(*output_index, 0);
                assert_eq!(text, "hello");
                assert_eq!(phase.as_deref(), Some("final_answer"));
            }
            other => panic!("unexpected done: {other:?}"),
        }
    }

    #[test]
    fn reasoning_then_text_share_output_index() {
        let (envs, _) = synthesize_envelopes(
            "resp_2",
            &completed(Some("answer"), Some("thinking"), Vec::new()),
        );
        // queued, in_progress, reasoning delta(0), reasoning done(0), text delta(1), text done(1)
        assert_eq!(envs.len(), 6);
        let idx = |env: &AgentEventEnvelope, k: fn(&AgentEventKind) -> bool| -> i32 {
            let e = kind(env);
            assert!(k(e));
            output_index_of(e)
        };
        assert_eq!(idx(&envs[2], is_reasoning_delta), 0);
        assert_eq!(idx(&envs[3], is_reasoning_done), 0);
        assert_eq!(idx(&envs[4], is_text_delta), 1);
        assert_eq!(idx(&envs[5], is_text_done), 1);
    }

    #[test]
    fn tool_calls_get_incomplete_terminal_and_indices() {
        let (envs, terminal) = synthesize_envelopes(
            "resp_3",
            &completed(Some("ok"), None, vec![tool_call("foo", "{}"), tool_call("bar", "[]")]),
        );
        assert!(matches!(terminal, TerminalKind::Incomplete { reason: Reason::ToolCalls }));
        // queued, in_progress, text delta(0), text done(0), fc_0(1), fc_1(2)
        assert_eq!(envs.len(), 6);
        assert!(
            matches!(kind(&envs[4]), AgentEventKind::ResponseFunctionCallArgumentsDone { output_index: 1, item_id, .. } if item_id == "fc_0")
        );
        assert!(
            matches!(kind(&envs[5]), AgentEventKind::ResponseFunctionCallArgumentsDone { output_index: 2, item_id, .. } if item_id == "fc_1")
        );
    }

    #[test]
    fn empty_outcome_emits_only_lifecycle() {
        let (envs, terminal) = synthesize_envelopes("resp_4", &SingleShotOutcome::Empty);
        assert!(matches!(terminal, TerminalKind::Completed));
        assert_eq!(envs.len(), 2);
        assert!(matches!(kind(&envs[0]), AgentEventKind::ResponseQueued { .. }));
        assert!(matches!(kind(&envs[1]), AgentEventKind::ResponseInProgress { .. }));
    }

    #[test]
    fn failed_emits_no_item_envelopes() {
        let (envs, terminal) = synthesize_envelopes(
            "resp_5",
            &SingleShotOutcome::Failed { message: "boom".into(), code: None, error_type: None },
        );
        assert!(matches!(terminal, TerminalKind::Failed { .. }));
        assert_eq!(envs.len(), 2); // lifecycle only
    }

    #[test]
    fn stream_mapping_text_deltas_then_terminal() {
        let mut acc = StreamAccumulator { next_id: 2, ..Default::default() };
        let d1 = map_delta_event(DeltaEvent::Text("hel".into()), &mut acc);
        let d2 = map_delta_event(DeltaEvent::Text("lo".into()), &mut acc);
        let d3 = map_delta_event(DeltaEvent::Done { usage: None }, &mut acc);
        // Text deltas emit text-delta envelopes with monotonic ids; Done emits none.
        assert!(d3.is_none());
        let delta_frames: Vec<StreamFrame> = [d1, d2].into_iter().flatten().collect();
        assert_eq!(delta_frames.len(), 2);
        assert!(matches!(
            delta_frames[0],
            StreamFrame::Envelope(ref e) if matches!(
                kind(e),
                AgentEventKind::ResponseOutputTextDelta { delta, output_index, .. }
                    if delta.as_str() == "hel" && *output_index == 0
            )
        ));
        assert!(matches!(
            delta_frames[1],
            StreamFrame::Envelope(ref e) if matches!(
                kind(e),
                AgentEventKind::ResponseOutputTextDelta { delta, .. } if delta.as_str() == "lo"
            )
        ));
        assert_eq!(acc.text, "hello");
        // Terminal: one text-done (msg_0) + Completed.
        let outcome = outcome_from_accumulator(&acc);
        assert!(matches!(outcome, SingleShotOutcome::Completed { .. }));
        let term = terminal_frames(&acc, &outcome);
        assert_eq!(term.len(), 2);
        assert!(matches!(
            term[0],
            StreamFrame::Envelope(ref e) if matches!(
                kind(e),
                AgentEventKind::ResponseOutputTextDone { text, .. } if text.as_str() == "hello"
            )
        ));
        assert!(matches!(term[1], StreamFrame::Terminal(TerminalKind::Completed, _)));
    }

    #[test]
    fn stream_mapping_failed_emits_no_text_done() {
        let mut acc = StreamAccumulator { next_id: 2, ..Default::default() };
        map_delta_event(DeltaEvent::Text("partial".into()), &mut acc);
        map_delta_event(DeltaEvent::Failed("boom".into()), &mut acc);
        let outcome = outcome_from_accumulator(&acc);
        assert!(matches!(outcome, SingleShotOutcome::Failed { .. }));
        // Failed → no text-done envelope, just the terminal.
        let term = terminal_frames(&acc, &outcome);
        assert_eq!(term.len(), 1);
        assert!(matches!(term[0], StreamFrame::Terminal(TerminalKind::Failed { .. }, _)));
    }

    #[test]
    fn stream_mapping_empty_emits_completed_only() {
        let acc = StreamAccumulator { next_id: 2, ..Default::default() };
        let outcome = outcome_from_accumulator(&acc);
        assert!(matches!(outcome, SingleShotOutcome::Empty));
        // Empty → Completed terminal, no text-done envelope.
        let term = terminal_frames(&acc, &outcome);
        assert_eq!(term.len(), 1);
        assert!(matches!(term[0], StreamFrame::Terminal(TerminalKind::Completed, _)));
    }

    fn output_index_of(e: &AgentEventKind) -> i32 {
        match e {
            AgentEventKind::ResponseOutputTextDelta { output_index, .. }
            | AgentEventKind::ResponseOutputTextDone { output_index, .. }
            | AgentEventKind::ResponseReasoningTextDelta { output_index, .. }
            | AgentEventKind::ResponseReasoningTextDone { output_index, .. }
            | AgentEventKind::ResponseFunctionCallArgumentsDone { output_index, .. } => {
                *output_index
            }
            _ => unreachable!("not an item event"),
        }
    }

    fn is_reasoning_delta(e: &AgentEventKind) -> bool {
        matches!(e, AgentEventKind::ResponseReasoningTextDelta { .. })
    }
    fn is_reasoning_done(e: &AgentEventKind) -> bool {
        matches!(e, AgentEventKind::ResponseReasoningTextDone { .. })
    }
    fn is_text_delta(e: &AgentEventKind) -> bool {
        matches!(e, AgentEventKind::ResponseOutputTextDelta { .. })
    }
    fn is_text_done(e: &AgentEventKind) -> bool {
        matches!(e, AgentEventKind::ResponseOutputTextDone { .. })
    }
}
