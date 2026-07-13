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
use slab_agent::port::{ThreadMessageRecord, ThreadSnapshot};
use slab_agent::{AgentConfig, AgentEventKind, AgentResponseRef, ThreadStatus, TurnEvent};
use slab_proto::openai::{Reason, Response};
use uuid::Uuid;

use super::projection::{AdapterInput, apply_terminal, build_response};
use crate::context::ModelState;
use crate::domain::models::{
    ChatCompletionCommand, ChatCompletionOutput, ChatCompletionResult, CloudChatParams,
    CommonChatParams, ConversationMessage, ConversationMessageContent, ConversationToolCall,
    LocalChatParams, TextGenerationUsage,
};
use crate::domain::services::agent::AgentCore;
use crate::domain::services::chat::ChatService;
use crate::error::AppCoreError;
use crate::infra::agent::event_hub::AgentEventEnvelope;
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
    /// [`super::stream::build_terminal_event`]).
    Terminal(TerminalKind),
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
// `previous_response_id` (= thread id) keeps chaining. C2 is text-only: the
// request carries no `tools`, so tool-call synthesis is correct but dormant.

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
                let records = core.store().list_thread_messages(thread_id).await?;
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
        tools: Vec::new(),
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
        local: LocalChatParams { gbnf: None, structured_output: config.structured_output.clone() },
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
        core.store()
            .insert_thread_message(&ThreadMessageRecord {
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
            core.store()
                .insert_thread_message(&ThreadMessageRecord {
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
        SingleShotOutcome::Failed { .. } => {
            core.store().update_thread_status(response_id, ThreadStatus::Errored, None).await?;
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

/// Run one non-streaming LLM call and return the canonical OpenAI [`Response`].
pub(crate) async fn run_create_response(
    core: &AgentCore,
    state: &ModelState,
    req: &OpenAICreateRequest,
    session_id: &str,
) -> Result<Response, AppCoreError> {
    let config: AgentConfig = req.to_config_input().into();
    let input = resolve_input(core, req).await?;
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

/// Run one LLM call and return the response id plus the synthesized frame
/// list (lifecycle + output-item envelopes + terminal). The service boxes the
/// frames into a stream; the handler seeds `StreamCtx` from the response id.
/// C2 emits the full sequence as a burst; true token streaming is a follow-up.
pub(crate) async fn run_stream_response(
    core: &AgentCore,
    state: &ModelState,
    req: &OpenAICreateRequest,
    session_id: &str,
) -> Result<(String, Vec<StreamFrame>), AppCoreError> {
    let config: AgentConfig = req.to_config_input().into();
    let input = resolve_input(core, req).await?;
    let command = build_command(req, input.messages.clone(), &config);
    persist_input(core, &input, session_id, &config).await?;

    let outcome = run_llm_or_failure(state, command).await;
    persist_assistant_and_complete(core, &input.response_id, input.turn_index, &outcome).await?;

    let (envs, terminal) = synthesize_envelopes(&input.response_id, &outcome);
    let mut frames: Vec<StreamFrame> = envs.into_iter().map(StreamFrame::Envelope).collect();
    frames.push(StreamFrame::Terminal(terminal));
    Ok((input.response_id, frames))
}

/// Reconstruct a [`Response`] for an already-completed run (GET SSE resume /
/// `get_response`). Best-effort: response state is not persisted, so this
/// rebuilds from the persisted thread messages.
pub(crate) async fn run_get_response(
    core: &AgentCore,
    response_id: &str,
) -> Result<Response, AppCoreError> {
    let thread =
        core.store().get_thread(response_id).await?.ok_or_else(|| {
            AppCoreError::BadRequest(format!("unknown response id: {response_id}"))
        })?;
    let records = core.store().list_thread_messages(response_id).await?;

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
    let outcome = if text_parts.is_empty() && tool_calls.is_empty() {
        SingleShotOutcome::Empty
    } else {
        let joined = text_parts.join("\n");
        SingleShotOutcome::Completed {
            text: if joined.is_empty() { None } else { Some(joined) },
            reasoning: None,
            tool_calls,
            usage: None,
        }
    };

    let (envs, _terminal) = synthesize_envelopes(response_id, &outcome);
    let mut response = build_response(AdapterInput {
        response_id,
        model: &thread.config_json,
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
