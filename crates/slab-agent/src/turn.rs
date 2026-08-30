//! Single-turn execution logic (private to the crate).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use uuid::Uuid;

use slab_agent_tracing::{AgentTraceContext, AgentTraceSink, record_json};
use slab_types::{
    ConversationContentPart, ConversationMessage, ConversationMessageContent, ConversationToolCall,
    ConversationToolFunction,
};

use crate::{
    compact::{CompactContext, CompactOutcome, CompactPort},
    config::{AgentConfig, AgentToolChoice},
    error::AgentError,
    hook::{AgentHookRegistry, HookEvent, dispatch_registered_hooks},
    port::{
        AgentNotifyPort, ApprovalPort, ExecPolicyPort, LlmPort, LlmStreamObserver, LlmUsage,
        ParsedToolCall, PlanStorePort, ToolSpec,
    },
    protocol::{
        AgentMessageDeltaParams, EventMsg, ItemCompletedParams, ItemStartedParams,
        MessageAppendedParams, ReasoningText, ReasoningTextDeltaParams, TurnItem,
        TurnStateChangedParams,
    },
    repetition_guard::ToolCallSignature,
    risk::ToolRiskAnalyzer,
    tool::{AgentThreadContext, ToolDiscoveryState, ToolRouter},
    tool_validation::{InvalidToolCall, validate_tool_calls},
    turn_state::{OpenItemTracker, TurnLifecycle, TurnPhase},
    turn_tool_call::{emit_tool_item_failed, handle_tool_calls},
    turn_tool_record::record_failed_tool_call,
};

/// Execute a single LLM turn.
///
/// Returns `true` if another turn is needed (i.e. the model emitted tool
/// calls), or `false` when the model produced a final answer.
pub(crate) struct TurnExecutionContext<'a> {
    pub thread_id: &'a str,
    pub session_id: &'a str,
    pub turn_index: u32,
    pub depth: u32,
    pub config: &'a AgentConfig,
    pub llm: &'a dyn LlmPort,
    pub tools: &'a ToolRouter,
    pub tool_discovery: &'a ToolDiscoveryState,
    pub notify: &'a dyn AgentNotifyPort,
    pub approval: &'a dyn ApprovalPort,
    pub exec_policy: &'a dyn ExecPolicyPort,
    /// Built-in agent registry (Slice 4). Read-only turn use — drives
    /// [`crate::agent::filter_tools_for_agent`] from `config.agent_type`.
    pub agent_registry: &'a dyn crate::agent::AgentRegistry,
    /// Owned `Arc` (not `&'a dyn`) because it is cloned into each call's
    /// [`crate::ToolContext`] so the plan tools can persist/query the durable plan.
    pub plan_store: Arc<dyn PlanStorePort>,
    pub hooks: &'a AgentHookRegistry,
    pub risk: &'a dyn ToolRiskAnalyzer,
    pub trace: &'a dyn AgentTraceSink,
    pub trace_context: AgentTraceContext,
    pub cancellation: &'a CancellationToken,
    pub thread_context: &'a AgentThreadContext,
    pub consumed_tokens: u32,
    /// Per-run turn lifecycle — the typed phase choke point. `execute_turn`
    /// validates every transition through it; `started_at` for
    /// `TurnStateChanged` emits comes from here (stamped once per iteration).
    pub lifecycle: &'a TurnLifecycle,
    /// Open tool items for this run — guarantees every `ItemStarted` gets a
    /// terminal `ItemCompleted` even on interrupt/error teardown.
    pub items: &'a OpenItemTracker,
    /// Compact port for the context-overflow recovery path (one forced
    /// compaction per run, then retry the LLM call).
    pub compact: &'a dyn CompactPort,
    /// Estimated prompt token count for the iteration (from the pre-iteration
    /// compaction policy check) — drives the pre-flight token-budget gate.
    pub prompt_token_estimate: Option<u32>,
    /// Death-spiral guard: a forced compaction for context overflow may run
    /// AT MOST ONCE per run. A second overflow after compaction fails the
    /// turn (compacting again cannot shrink further).
    pub context_overflow_recovered: &'a AtomicBool,
    /// Run-scoped context-budget net: bounds every tool result (byte cap +
    /// exact-duplicate dedup) before it becomes conversation history.
    pub tool_result_guard: &'a crate::tool_result_guard::ToolResultGuard,
}

pub(crate) enum TurnOutcome {
    Final {
        usage: Option<LlmUsage>,
    },
    BudgetExceeded {
        usage: Option<LlmUsage>,
    },
    ToolCalls {
        invalid_tool_calls: usize,
        signatures: Vec<ToolCallSignature>,
        usage: Option<LlmUsage>,
    },
}

pub(crate) async fn execute_turn(
    context: TurnExecutionContext<'_>,
    messages: &mut Vec<ConversationMessage>,
) -> Result<TurnOutcome, AgentError> {
    if context.cancellation.is_cancelled() {
        return Err(AgentError::Interrupted);
    }

    let tool_specs = allowed_tool_specs(&context)?;
    let llm_start_effects = dispatch_registered_hooks(
        context.hooks,
        &HookEvent::OnLlmStart {
            thread_id: context.thread_id.to_owned(),
            session_id: context.session_id.to_owned(),
            turn_index: context.turn_index,
            messages: messages.clone(),
            tools: tool_specs.clone(),
        },
    )
    .await;
    insert_injected_messages(messages, llm_start_effects.injected_messages);
    append_observations(messages, llm_start_effects.observations);
    // Sampling entry snapshot — the per-iteration anchor line. MUST carry the
    // full input messages: rollout replay uses the last non-empty TurnState to
    // replace the history, and item attribution advances on TurnContext lines.
    // The transition also exits a lingering Compacting phase (auto-compaction
    // ran just before this call).
    transition_turn(&context, TurnPhase::Sampling).await;
    emit_turn_state_changed(
        &context,
        TurnPhase::Sampling,
        Some(messages.as_slice()),
        Some(&tool_specs),
        None,
        None,
        None,
    )
    .await;

    // Pre-flight token-budget gate: refuse BEFORE the request when the
    // estimated prompt alone would exhaust the budget. The post-response
    // check below stays as the authoritative accounting; this one prevents
    // paying for a call whose answer can never be delivered.
    if let Some(estimate) = context.prompt_token_estimate
        && token_budget_would_be_exhausted(
            context.config.token_budget,
            context.consumed_tokens,
            estimate,
        )
    {
        transition_turn(&context, TurnPhase::Interrupted).await;
        emit_turn_state_changed(
            &context,
            TurnPhase::Interrupted,
            Some(messages.as_slice()),
            None,
            None,
            Some("token budget exceeded before request"),
            Some(Utc::now().to_rfc3339()),
        )
        .await;
        record_json(
            context.trace,
            &context.trace_context,
            "slab-agent",
            "turn_token_budget_preflight_exhausted",
            serde_json::json!({
                "prompt_token_estimate": estimate,
                "consumed_tokens": context.consumed_tokens,
                "token_budget": context.config.token_budget,
            }),
        );
        return Ok(TurnOutcome::BudgetExceeded { usage: None });
    }

    debug!(thread_id = context.thread_id, turn_index = context.turn_index, "executing turn");
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "turn_started",
        serde_json::json!({
            "thread_id": context.thread_id,
            "turn_index": context.turn_index,
            "depth": context.depth,
            "message_count": messages.len(),
        }),
    );
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "agent_llm_request",
        serde_json::json!({
            "model": context.config.model,
            "messages": messages,
            "tools": tool_specs_trace_payload(&tool_specs),
            "config": context.config,
        }),
    );
    if let Some(structured_output) = context.config.structured_output.as_ref() {
        record_json(
            context.trace,
            &context.trace_context,
            "slab-agent",
            "structured_output_requested",
            serde_json::json!({ "structured_output": structured_output }),
        );
    }

    // LLM call with bounded recovery:
    // - TRANSIENT failures (transport reset / timeout / 429 / 5xx) retry with
    //   exponential backoff, at most `llm_max_retries` times.
    // - Context-overflow failures trigger ONE forced compaction per run, then
    //   retry (death-spiral guard: a second overflow fails the turn).
    // - A partially-streamed response is NEVER retried (withhold: the client
    //   would see the streamed prefix twice).
    // - No `Error` event is emitted until retries are exhausted — SDK clients
    //   treat `error` as terminal, and the recovery loop must stay invisible
    //   while it still has moves left.
    let max_retries = context.config.effective_llm_max_retries();
    let mut llm_attempts: u8 = 0;
    let response = loop {
        let mut stream_observer = TurnTextDeltaObserver {
            thread_id: context.thread_id,
            turn_index: context.turn_index,
            notify: context.notify,
            text_started: false,
            any_delta_emitted: false,
        };
        let response_result = tokio::select! {
            response = context.llm.chat_completion_streaming(
                &context.config.model,
                messages,
                &tool_specs,
                context.config,
                &context.trace_context,
                &mut stream_observer,
            ) => response,
            _ = context.cancellation.cancelled() => return Err(AgentError::Interrupted),
        };
        match response_result {
            Ok(response) => break response,
            Err(error) => {
                let streamed = stream_observer.any_delta_emitted;
                let recovery_available = match &error {
                    AgentError::LlmContextTooLong(_) => {
                        !context.context_overflow_recovered.load(Ordering::SeqCst)
                    }
                    AgentError::LlmTransient(_) => true,
                    _ => false,
                };
                let can_retry = !streamed && llm_attempts < max_retries && recovery_available;
                if !can_retry {
                    if streamed {
                        warn!(
                            thread_id = context.thread_id,
                            turn_index = context.turn_index,
                            error = %error,
                            "llm failed after partial stream; not retrying (client already saw the prefix)"
                        );
                    }
                    fail_turn_llm(&context, messages, &tool_specs, &error).await;
                    return Err(error);
                }
                llm_attempts += 1;
                if matches!(error, AgentError::LlmContextTooLong(_)) {
                    // Claim the one-shot recovery slot (atomic swap; the guard
                    // above makes the already-claimed branch unreachable, but
                    // the swap keeps the claim authoritative).
                    let already_claimed =
                        context.context_overflow_recovered.swap(true, Ordering::SeqCst);
                    debug_assert!(!already_claimed, "recovery slot raced");
                    warn!(
                        thread_id = context.thread_id,
                        turn_index = context.turn_index,
                        error = %error,
                        "context overflow; forcing compaction then retrying (once per run)"
                    );
                    emit_turn_phase(&context, TurnPhase::Compacting).await;
                    let compact_ctx = CompactContext {
                        model_id: &context.config.model,
                        summary_instructions: None,
                        force: true,
                        memory_pressure_hint: None,
                        progress: None,
                    };
                    let compacted = match context.compact.compact(messages, &compact_ctx).await {
                        Ok(CompactOutcome::Replaced { messages, .. }) => Some(messages),
                        Ok(_) => {
                            warn!(
                                thread_id = context.thread_id,
                                "forced compaction did not replace the message set; failing turn"
                            );
                            None
                        }
                        Err(compact_error) => {
                            warn!(
                                thread_id = context.thread_id,
                                error = %compact_error,
                                "forced compaction failed; failing turn"
                            );
                            None
                        }
                    };
                    let Some(compacted) = compacted else {
                        fail_turn_llm(&context, messages, &tool_specs, &error).await;
                        return Err(error);
                    };
                    *messages = compacted;
                    emit_turn_phase(&context, TurnPhase::Sampling).await;
                    record_json(
                        context.trace,
                        &context.trace_context,
                        "slab-agent",
                        "llm_context_overflow_recovered",
                        serde_json::json!({
                            "attempt": llm_attempts,
                            "message_count": messages.len(),
                        }),
                    );
                    // Compaction already took time; retry immediately without
                    // an additional backoff delay.
                    continue;
                }
                let delay_ms = context
                    .config
                    .llm_retry_base_delay_ms
                    .saturating_mul(2u64.saturating_pow((llm_attempts - 1) as u32));
                warn!(
                    thread_id = context.thread_id,
                    turn_index = context.turn_index,
                    attempt = llm_attempts,
                    max_retries,
                    delay_ms,
                    error = %error,
                    "transient llm failure; retrying with backoff"
                );
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_millis(delay_ms)) => {}
                    _ = context.cancellation.cancelled() => return Err(AgentError::Interrupted),
                }
            }
        }
    };
    if context.cancellation.is_cancelled() {
        return Err(AgentError::Interrupted);
    }

    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "llm_response_normalized",
        serde_json::json!({
            "content": &response.content,
            "content_already_streamed": response.content_already_streamed,
            "finish_reason": &response.finish_reason,
            "tool_calls": parsed_tool_calls_trace_payload(&response.tool_calls),
            "usage": response.usage,
        }),
    );
    let llm_end_effects = dispatch_registered_hooks(
        context.hooks,
        &HookEvent::OnLlmEnd {
            thread_id: context.thread_id.to_owned(),
            session_id: context.session_id.to_owned(),
            turn_index: context.turn_index,
            messages: messages.clone(),
            response: response.clone(),
        },
    )
    .await;
    insert_injected_messages(messages, llm_end_effects.injected_messages);
    append_observations(messages, llm_end_effects.observations);

    let usage = response.usage.clone();
    let token_usage = usage.as_ref().map(|usage| usage.total_tokens).unwrap_or_default();
    if token_budget_would_be_exhausted(
        context.config.token_budget,
        context.consumed_tokens,
        token_usage,
    ) {
        // Protocol completeness: the budget path returns BEFORE
        // `persist_assistant_tool_request`, so a response with tool calls
        // would vanish with no persisted trace AND leave nothing for the
        // model to answer. Persist the request + synthesize failed results
        // so the rollout never carries a dangling tool_calls tail.
        finalize_unresolved_tool_calls(
            context.notify,
            context.thread_id,
            context.turn_index,
            messages,
            Some(&response),
            "[token budget exceeded]",
        )
        .await;
        // Terminal Interrupted state with the reason recorded — the budget
        // path previously left the turn in a non-terminal `budget_exhausted`
        // string with no error detail.
        transition_turn(&context, TurnPhase::Interrupted).await;
        emit_turn_state_changed(
            &context,
            TurnPhase::Interrupted,
            Some(messages.as_slice()),
            Some(&tool_specs),
            Some(&response),
            Some("token budget exceeded"),
            Some(Utc::now().to_rfc3339()),
        )
        .await;
        record_json(
            context.trace,
            &context.trace_context,
            "slab-agent",
            "turn_token_budget_exhausted",
            serde_json::json!({
                "token_usage": token_usage,
                "consumed_tokens": context.consumed_tokens,
                "token_budget": context.config.token_budget,
                "has_tool_calls": !response.tool_calls.is_empty(),
            }),
        );
        return Ok(TurnOutcome::BudgetExceeded { usage: usage.clone() });
    }

    if response.tool_calls.is_empty() {
        if let Err(error) = reject_missing_required_tool_call(&context) {
            transition_turn(&context, TurnPhase::Failed).await;
            emit_turn_state_changed(
                &context,
                TurnPhase::Failed,
                Some(messages.as_slice()),
                Some(&tool_specs),
                Some(&response),
                Some(&error.to_string()),
                Some(Utc::now().to_rfc3339()),
            )
            .await;
            return Err(error);
        }
        persist_final_answer(&context, messages, response.content.unwrap_or_default()).await;
        transition_turn(&context, TurnPhase::Completed).await;
        emit_turn_state_changed(
            &context,
            TurnPhase::Completed,
            Some(messages.as_slice()),
            Some(&tool_specs),
            None,
            None,
            Some(Utc::now().to_rfc3339()),
        )
        .await;
        record_json(
            context.trace,
            &context.trace_context,
            "slab-agent",
            "turn_completed",
            serde_json::json!({ "more_turns": false }),
        );
        emit_turn_token_accounting(&context, messages, usage.as_ref());
        return Ok(TurnOutcome::Final { usage: usage.clone() });
    }

    let validation = validate_tool_calls(
        &context.config.tool_choice,
        &context.config.allowed_tools,
        &tool_specs,
        &response.tool_calls,
    );
    emit_unstreamed_tool_text(
        &context,
        response.content.as_deref(),
        response.content_already_streamed,
    )
    .await;
    persist_assistant_tool_request(&context, messages, &response).await;
    if !validation.invalid.is_empty() {
        record_invalid_tool_calls(&context, &validation.invalid, messages).await?;
    }
    if !validation.valid.is_empty() {
        let task_completion = handle_tool_calls(&context, &validation.valid, messages).await?;
        if let Some(completion) = task_completion {
            // 双轨 2: the deterministic `task.complete` gate passed; emit the
            // summary as the final answer and end the run (alongside the
            // existing `tool_calls.is_empty()` Final path).
            //
            // Retire the durable plan with the completion itself: a steering
            // continuation that replays `task.complete` without fresh planning
            // must hit the tool's no-active-plan denial instead of silently
            // finalizing again on the already-completed plan.
            context.plan_store.clear(context.thread_id).await;
            persist_final_answer(&context, messages, completion.summary).await;
            transition_turn(&context, TurnPhase::Completed).await;
            emit_turn_state_changed(
                &context,
                TurnPhase::Completed,
                Some(messages.as_slice()),
                Some(&tool_specs),
                None,
                None,
                Some(Utc::now().to_rfc3339()),
            )
            .await;
            record_json(
                context.trace,
                &context.trace_context,
                "slab-agent",
                "turn_completed",
                serde_json::json!({ "more_turns": false, "task_complete": true }),
            );
            emit_turn_token_accounting(&context, messages, usage.as_ref());
            return Ok(TurnOutcome::Final { usage: usage.clone() });
        }
    }

    // No intermediate `tool_calls_completed` full snapshot anymore — the
    // ExecutingTools entry phase is emitted (status-only) inside
    // `handle_tool_calls`, and the next iteration's Sampling line carries the
    // authoritative replay snapshot.
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "turn_completed",
        serde_json::json!({ "more_turns": true }),
    );
    // Emitted after `handle_tool_calls` so this turn's tool results are
    // included in the breakdown — the accounting describes what the NEXT
    // sampling request would re-send.
    emit_turn_token_accounting(&context, messages, usage.as_ref());
    Ok(TurnOutcome::ToolCalls {
        invalid_tool_calls: validation.invalid.len(),
        signatures: validation.valid.iter().map(ToolCallSignature::new).collect(),
        usage: usage.clone(),
    })
}

fn token_budget_would_be_exhausted(
    token_budget: Option<u32>,
    consumed_tokens: u32,
    token_usage: u32,
) -> bool {
    token_budget
        .is_some_and(|budget| budget > 0 && consumed_tokens.saturating_add(token_usage) >= budget)
}

/// Per-turn context accounting (context-budget system): a pure estimate of
/// what the next sampling request will re-send, broken down by message role
/// and by tool, alongside the actual usage the provider reported for this
/// turn. One free-form trace line — no LLM calls.
fn emit_turn_token_accounting(
    context: &TurnExecutionContext<'_>,
    messages: &[ConversationMessage],
    usage: Option<&LlmUsage>,
) {
    // Tool messages carry only `tool_call_id`; resolve tool names from the
    // assistant `tool_calls` that produced them.
    let mut tool_names: HashMap<&str, &str> = HashMap::new();
    for message in messages {
        for call in &message.tool_calls {
            if let Some(id) = call.id.as_deref() {
                tool_names.insert(id, call.function.name.as_str());
            }
        }
    }

    #[derive(Default)]
    struct ToolStats {
        messages: u64,
        tokens: u64,
        bytes: u64,
    }

    let mut segments: BTreeMap<&str, u64> = BTreeMap::new();
    let mut by_tool: BTreeMap<&str, ToolStats> = BTreeMap::new();
    // (bytes, tokens, call_id, tool name)
    let mut largest: Vec<(u64, u64, &str, &str)> = Vec::new();

    for message in messages {
        let tokens = crate::compact::estimate_message_tokens(message) as u64;
        *segments.entry(message.role.as_str()).or_default() += tokens;
        if message.role == "tool" {
            let bytes = message_content_bytes(message) as u64;
            let call_id = message.tool_call_id.as_deref().unwrap_or("");
            let name = message
                .tool_call_id
                .as_deref()
                .and_then(|id| tool_names.get(id).copied())
                .unwrap_or("unknown");
            let stats = by_tool.entry(name).or_default();
            stats.messages += 1;
            stats.tokens += tokens;
            stats.bytes += bytes;
            largest.push((bytes, tokens, call_id, name));
        }
    }

    largest.sort_by_key(|(bytes, ..)| std::cmp::Reverse(*bytes));
    let largest_tool_results: Vec<serde_json::Value> = largest
        .into_iter()
        .take(5)
        .map(|(bytes, tokens, call_id, name)| {
            serde_json::json!({ "tool": name, "call_id": call_id, "bytes": bytes, "tokens": tokens })
        })
        .collect();

    let by_tool_json: serde_json::Map<String, serde_json::Value> = by_tool
        .iter()
        .map(|(name, stats)| {
            (
                (*name).to_owned(),
                serde_json::json!({
                    "messages": stats.messages,
                    "tokens": stats.tokens,
                    "bytes": stats.bytes,
                }),
            )
        })
        .collect();

    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "turn_token_accounting",
        serde_json::json!({
            "turn_index": context.turn_index,
            "model": context.config.model,
            "message_count": messages.len(),
            "estimated_total_tokens": crate::compact::estimate_tokens(messages),
            "by_segment": segments,
            "by_tool": by_tool_json,
            "largest_tool_results": largest_tool_results,
            "usage": usage,
            "consumed_tokens": context.consumed_tokens,
            "token_budget": context.config.token_budget,
        }),
    );
}

/// Byte length of a message's textual payload (image parts are not
/// text-injected and the estimator already caps them).
fn message_content_bytes(message: &ConversationMessage) -> usize {
    match &message.content {
        ConversationMessageContent::Text(text) => text.len(),
        ConversationMessageContent::Parts(parts) => parts
            .iter()
            .map(|part| match part {
                ConversationContentPart::Text { text }
                | ConversationContentPart::InputText { text }
                | ConversationContentPart::OutputText { text }
                | ConversationContentPart::Refusal { text } => text.len(),
                ConversationContentPart::ToolResult { value, .. }
                | ConversationContentPart::Json { value } => value.to_string().len(),
                ConversationContentPart::Image { .. } => 0,
            })
            .sum(),
    }
}

/// Land the terminal Failed turn state for an unrecoverable LLM failure.
async fn fail_turn_llm(
    context: &TurnExecutionContext<'_>,
    messages: &[ConversationMessage],
    tool_specs: &[ToolSpec],
    error: &AgentError,
) {
    transition_turn(context, TurnPhase::Failed).await;
    emit_turn_state_changed(
        context,
        TurnPhase::Failed,
        Some(messages),
        Some(tool_specs),
        None,
        Some(&error.to_string()),
        Some(Utc::now().to_rfc3339()),
    )
    .await;
}

/// Protocol-completeness repair: the persisted history must carry a tool
/// result for every tool call in the trailing assistant message. Strict
/// OpenAI-compatible providers REJECT a follow-up request whose history ends
/// with a dangling `tool_calls` pair — previously both the budget-exceeded
/// path (assistant request dropped silently) and the interrupt teardown
/// (assistant request persisted, results never landed) produced exactly that.
///
/// Step 1: when the caller still holds an UN-persisted response with tool
/// calls (the budget path returns before `persist_assistant_tool_request`),
/// persist the assistant tool-request first.
/// Step 2: synthesize a `role: "tool"` failed result carrying `note` for
/// every unanswered call id in the trailing assistant message. Each lands as
/// a `MessageAppended` event so the rollout true source stays balanced.
pub(crate) async fn finalize_unresolved_tool_calls(
    notify: &dyn AgentNotifyPort,
    thread_id: &str,
    turn_index: u32,
    messages: &mut Vec<ConversationMessage>,
    response: Option<&crate::port::LlmResponse>,
    note: &str,
) {
    // Step 1: persist the assistant tool-request when the caller still holds it.
    if let Some(response) = response
        && !response.tool_calls.is_empty()
    {
        let response_ids: HashSet<&str> =
            response.tool_calls.iter().map(|call| call.id.as_str()).collect();
        let already_persisted = messages.iter().any(|message| {
            message.role == "assistant"
                && message
                    .tool_calls
                    .iter()
                    .filter_map(|call| call.id.as_deref())
                    .any(|id| response_ids.contains(id))
        });
        if !already_persisted {
            let assistant_tool_calls: Vec<ConversationToolCall> = response
                .tool_calls
                .iter()
                .map(|tool_call| ConversationToolCall {
                    id: Some(tool_call.id.clone()),
                    r#type: "function".to_owned(),
                    function: ConversationToolFunction {
                        name: tool_call.name.clone(),
                        arguments: tool_call.arguments.clone(),
                    },
                })
                .collect();
            let assistant_message = ConversationMessage {
                role: "assistant".to_owned(),
                content: ConversationMessageContent::Text(
                    response.content.clone().unwrap_or_default(),
                ),
                name: None,
                tool_call_id: None,
                tool_calls: assistant_tool_calls,
            };
            emit_message_appended(notify, thread_id, turn_index, &assistant_message).await;
            messages.push(assistant_message);
        }
    }

    // Step 2: synthesize failed tool results for unanswered calls in the
    // trailing assistant message with tool calls.
    let Some(trailing_calls) = messages.iter().rev().find_map(|message| {
        if message.role == "assistant" && !message.tool_calls.is_empty() {
            Some(
                message
                    .tool_calls
                    .iter()
                    .map(|call| (call.id.clone().unwrap_or_default(), call.function.name.clone()))
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        }
    }) else {
        return;
    };
    let answered: HashSet<String> = messages
        .iter()
        .filter(|message| message.role == "tool")
        .filter_map(|message| message.tool_call_id.clone())
        .collect();
    for (call_id, tool_name) in trailing_calls {
        if call_id.is_empty() || answered.contains(&call_id) {
            continue;
        }
        let tool_result = ConversationMessage {
            role: "tool".to_owned(),
            content: ConversationMessageContent::Text(note.to_owned()),
            name: Some(tool_name),
            tool_call_id: Some(call_id),
            tool_calls: Vec::new(),
        };
        emit_message_appended(notify, thread_id, turn_index, &tool_result).await;
        messages.push(tool_result);
    }
}

/// External tools that require provider/network reachability and are removed
/// from the agent's tool list in offline mode (INFRA-07). Local filesystem,
/// shell, plan, and verify tools remain available offline.
fn is_external_tool_name(name: &str) -> bool {
    matches!(name, "web_search" | "mcp_call" | "mcp_list_tools") || name.starts_with("mcp__")
}

fn allowed_tool_specs(context: &TurnExecutionContext<'_>) -> Result<Vec<ToolSpec>, AgentError> {
    // Visibility (Direct/Deferred/Hidden) + category-exposure projection.
    // Computed fresh each turn from the live per-thread permission mode, so a
    // mid-thread mode/permission flip is reflected immediately. `injected_deferred`
    // is the set of Deferred tools `tool_search` has injected for this thread;
    // empty means Deferred tools stay hidden from the base list until discovered.
    let exposure = context.exec_policy.permission_state_for(context.thread_id).exposure;
    let injected_deferred = context.tool_discovery.snapshot();
    let mut specs = context.tools.visible_tool_specs(exposure, &injected_deferred);
    // Slice 4: per-agent tool constraint, layered above visibility/exposure and
    // below `config.allowed_tools`. `agent_type` is set only by `delegate_subagent`
    // after a successful registry lookup; a miss here is misconfiguration, so we
    // fail open (the constraint is tool-shaping, not a security boundary) with a
    // warning rather than starving the agent of tools.
    let agent_constraint = context.config.agent_type.as_deref().and_then(|agent_type| {
        let constraint = context.agent_registry.get(agent_type).map(|def| def.tools);
        if constraint.is_none() {
            warn!(
                thread_id = context.thread_id,
                agent_type, "agent_type not found in registry; skipping tool constraint"
            );
        }
        constraint
    });
    specs = crate::agent::filter_tools_for_agent(&specs, agent_constraint.as_ref());
    if !context.config.allowed_tools.is_empty() {
        specs.retain(|tool| context.config.allowed_tools.contains(&tool.name));
    }
    if context.thread_context.offline {
        // INFRA-07: offline mode narrows the toolset to local-only tools,
        // dropping anything that needs external network/provider reachability.
        specs.retain(|tool| !is_external_tool_name(&tool.name));
    }

    match &context.config.tool_choice {
        AgentToolChoice::Auto => Ok(specs),
        AgentToolChoice::None => Ok(Vec::new()),
        AgentToolChoice::Required => {
            if specs.is_empty() {
                Err(AgentError::Internal(
                    "tool_choice required but no tools are available".to_owned(),
                ))
            } else {
                Ok(specs)
            }
        }
        AgentToolChoice::Tool { name } => {
            let name = name.trim();
            if name.is_empty() {
                return Err(AgentError::Internal(
                    "tool_choice tool name must not be blank".to_owned(),
                ));
            }
            let Some(spec) = specs.into_iter().find(|tool| tool.name == name) else {
                return Err(AgentError::Internal(format!(
                    "tool_choice tool is not available or allowed: {name}"
                )));
            };
            Ok(vec![spec])
        }
    }
}

fn reject_missing_required_tool_call(context: &TurnExecutionContext<'_>) -> Result<(), AgentError> {
    match &context.config.tool_choice {
        AgentToolChoice::Required => Err(AgentError::Internal(
            "tool_choice required but the model returned no tool calls".to_owned(),
        )),
        AgentToolChoice::Tool { name } => Err(AgentError::Internal(format!(
            "tool_choice requires tool '{name}' but the model returned no tool calls"
        ))),
        AgentToolChoice::Auto | AgentToolChoice::None => Ok(()),
    }
}

async fn record_invalid_tool_calls(
    context: &TurnExecutionContext<'_>,
    invalid: &[InvalidToolCall],
    messages: &mut Vec<ConversationMessage>,
) -> Result<(), AgentError> {
    let created_at = Utc::now().to_rfc3339();
    for invalid_call in invalid {
        let call_id = Uuid::new_v4().to_string();
        record_json(
            context.trace,
            &context.trace_context,
            "slab-agent",
            "invalid_tool_call",
            serde_json::json!({
                "item_id": invalid_call.tool_call.id,
                "call_id": call_id,
                "tool_name": invalid_call.tool_call.name,
                "arguments": invalid_call.tool_call.arguments,
                "reason": invalid_call.reason,
            }),
        );
        let invalid_output = format!("invalid tool call: {}", invalid_call.reason);
        emit_tool_item_failed(
            context,
            &invalid_call.tool_call,
            &serde_json::Value::Null,
            &invalid_output,
        )
        .await;
        record_failed_tool_call(
            context,
            &call_id,
            &invalid_call.tool_call,
            invalid_output,
            &created_at,
            messages,
        )
        .await?;
    }
    Ok(())
}

async fn persist_final_answer(
    context: &TurnExecutionContext<'_>,
    messages: &mut Vec<ConversationMessage>,
    content: String,
) {
    // `content` is the LLM-grade form (reasoning embedded as a
    // `<think status="done">…</think>` block for the next prompt's chat
    // template). The UI-grade agentMessage item must carry only the visible
    // text — history renders item text verbatim, so the block is stripped
    // here while the appended ConversationMessage keeps it.
    emit_agent_message_completed(
        context.notify,
        context.thread_id,
        context.turn_index,
        &assistant_item_id(context.turn_index),
        &strip_think_blocks(&content),
    )
    .await;

    let message = ConversationMessage {
        role: "assistant".to_owned(),
        content: ConversationMessageContent::Text(content),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    };
    emit_message_appended(context.notify, context.thread_id, context.turn_index, &message).await;
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "assistant_message_persisted",
        serde_json::json!({
            "turn_index": context.turn_index,
            "message": message,
        }),
    );
    messages.push(message);
}

async fn emit_unstreamed_tool_text(
    context: &TurnExecutionContext<'_>,
    content: Option<&str>,
    content_already_streamed: bool,
) {
    let Some(text) = content else {
        return;
    };
    if content_already_streamed || text.is_empty() {
        return;
    }

    // This path only runs when the content was NOT streamed, so the streaming
    // observer never announced the item — emit the harness ItemStarted here,
    // then the delta (mirrors the projection's first-delta behavior). The
    // delta is UI-grade: strip the LLM-context `<think>` block the adapter may
    // have embedded, matching what the streamed-delta path delivers.
    let text = strip_think_blocks(text);
    if text.is_empty() {
        return;
    }
    let item_id = assistant_item_id(context.turn_index);
    emit_agent_message_started(context.notify, context.thread_id, context.turn_index, &item_id)
        .await;
    emit_agent_message_delta(
        context.notify,
        context.thread_id,
        context.turn_index,
        &item_id,
        &text,
    )
    .await;
}

async fn persist_assistant_tool_request(
    context: &TurnExecutionContext<'_>,
    messages: &mut Vec<ConversationMessage>,
    response: &crate::port::LlmResponse,
) {
    let assistant_tool_calls: Vec<ConversationToolCall> = response
        .tool_calls
        .iter()
        .map(|tool_call| ConversationToolCall {
            id: Some(tool_call.id.clone()),
            r#type: "function".to_owned(),
            function: ConversationToolFunction {
                name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
            },
        })
        .collect();

    let assistant_message = ConversationMessage {
        role: "assistant".to_owned(),
        content: ConversationMessageContent::Text(response.content.clone().unwrap_or_default()),
        name: None,
        tool_call_id: None,
        tool_calls: assistant_tool_calls,
    };
    emit_message_appended(
        context.notify,
        context.thread_id,
        context.turn_index,
        &assistant_message,
    )
    .await;
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "assistant_message_persisted",
        serde_json::json!({
            "turn_index": context.turn_index,
            "message": assistant_message,
        }),
    );
    messages.push(assistant_message);
}

/// Emit a conversation message append as a persistence-grade
/// `MessageAppended` event. The app-core rollout observer lands it in the
/// rollout true source (`TurnContext::MessageAppend`), replacing the old
/// slab-agent store-trait `insert_thread_message` route. Carries the original
/// record `id` + `created_at` (F3) so replay recovers them verbatim.
pub(crate) async fn emit_message_appended(
    notify: &dyn AgentNotifyPort,
    thread_id: &str,
    turn_index: u32,
    message: &ConversationMessage,
) {
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    let event = EventMsg::MessageAppended(MessageAppendedParams {
        thread_id: thread_id.to_owned(),
        turn_index,
        message: message.clone(),
        id,
        created_at,
    });
    notify.on_event_msg(thread_id, &event).await;
}

/// Validate a phase transition through the run's [`TurnLifecycle`] before a
/// terminal emit. Advisory (status-only) callers use [`emit_turn_phase`]
/// instead; this is for the load-bearing terminal transitions whose failure
/// should be visible, not silent.
async fn transition_turn(context: &TurnExecutionContext<'_>, to: TurnPhase) {
    if let Err(error) = context.lifecycle.transition(to) {
        warn!(
            thread_id = context.thread_id,
            turn_index = context.turn_index,
            error = %error,
            "invalid turn phase transition"
        );
    }
}

/// Emit a status-only phase transition (advisory UI state — `executing_tools`
/// / `awaiting_approval`). Carries an empty `input_messages` vec, which
/// rollout replay treats as a no-op, so these lines cost a tiny rollout line
/// without disturbing the replayed history. An invalid transition is logged
/// and skipped rather than failing the turn.
pub(crate) async fn emit_turn_phase(context: &TurnExecutionContext<'_>, to: TurnPhase) {
    if let Err(error) = context.lifecycle.transition(to) {
        warn!(
            thread_id = context.thread_id,
            turn_index = context.turn_index,
            error = %error,
            "skipping advisory turn phase transition"
        );
        return;
    }
    let event = EventMsg::TurnStateChanged(TurnStateChangedParams {
        thread_id: context.thread_id.to_owned(),
        turn_index: context.turn_index,
        status: to.as_str().to_owned(),
        input_messages: Vec::new(),
        tool_specs_json: None,
        llm_response_json: None,
        error: None,
        started_at: context.lifecycle.started_at(),
        completed_at: None,
    });
    context.notify.on_event_msg(context.thread_id, &event).await;
}

/// Emit the terminal turn state from the run teardown in
/// [`crate::thread::AgentThread::run`] (the interrupt path emits no terminal
/// state inside `execute_turn`). Skips when the last iteration already
/// emitted its own terminal state (LLM-error / budget paths).
pub(crate) async fn emit_run_terminal_turn_state(
    notify: &dyn AgentNotifyPort,
    thread_id: &str,
    turn_index: u32,
    lifecycle: &TurnLifecycle,
    phase: TurnPhase,
    messages: &[ConversationMessage],
    error: Option<&str>,
) {
    if lifecycle.phase().is_terminal() {
        return;
    }
    if let Err(err) = lifecycle.transition(phase) {
        warn!(thread_id, turn_index, error = %err, "run teardown terminal transition rejected");
        return;
    }
    let event = EventMsg::TurnStateChanged(TurnStateChangedParams {
        thread_id: thread_id.to_owned(),
        turn_index,
        status: phase.as_str().to_owned(),
        input_messages: messages.to_vec(),
        tool_specs_json: None,
        llm_response_json: None,
        error: error.map(str::to_owned),
        started_at: lifecycle.started_at(),
        completed_at: Some(Utc::now().to_rfc3339()),
    });
    notify.on_event_msg(thread_id, &event).await;
}

/// Emit a turn-state snapshot as a persistence-grade `TurnStateChanged`
/// event. The app-core rollout observer lands it in the rollout true source
/// (`TurnContext::TurnState`), replacing the old slab-agent store-trait
/// `upsert_turn_state` route. Carries the typed input-messages vec directly (so
/// the F6 raw-blob recovery path is dead here). `started_at` comes from the
/// run's [`TurnLifecycle`] — stamped once per iteration, not per emit.
async fn emit_turn_state_changed(
    context: &TurnExecutionContext<'_>,
    phase: TurnPhase,
    messages: Option<&[ConversationMessage]>,
    tool_specs: Option<&[ToolSpec]>,
    response: Option<&crate::port::LlmResponse>,
    error: Option<&str>,
    completed_at: Option<String>,
) {
    let tool_specs_json = tool_specs.and_then(|tool_specs| serde_json::to_string(tool_specs).ok());
    let llm_response_json = response.and_then(|response| serde_json::to_string(response).ok());
    let event = EventMsg::TurnStateChanged(TurnStateChangedParams {
        thread_id: context.thread_id.to_owned(),
        turn_index: context.turn_index,
        status: phase.as_str().to_owned(),
        input_messages: messages.unwrap_or(&[]).to_vec(),
        tool_specs_json,
        llm_response_json,
        error: error.map(str::to_owned),
        started_at: context.lifecycle.started_at(),
        completed_at,
    });
    context.notify.on_event_msg(context.thread_id, &event).await;
}

fn insert_injected_messages(
    messages: &mut Vec<ConversationMessage>,
    injected: Vec<ConversationMessage>,
) {
    if injected.is_empty() {
        return;
    }
    let insert_at = messages
        .iter()
        .position(|message| message.role != "system" && message.role != "developer")
        .unwrap_or(messages.len());
    for (offset, message) in injected.into_iter().enumerate() {
        messages.insert(insert_at + offset, message);
    }
}

fn append_observations(messages: &mut Vec<ConversationMessage>, observations: Vec<String>) {
    for observation in observations.into_iter().filter(|value| !value.trim().is_empty()) {
        messages.push(ConversationMessage {
            role: "developer".to_owned(),
            content: ConversationMessageContent::Text(format!(
                "Local hook observation:\n{observation}"
            )),
            name: Some("slab_hook".to_owned()),
            tool_call_id: None,
            tool_calls: Vec::new(),
        });
    }
}

fn assistant_item_id(turn_index: u32) -> String {
    format!("assistant-{turn_index}")
}

fn tool_specs_trace_payload(tool_specs: &[ToolSpec]) -> serde_json::Value {
    serde_json::Value::Array(
        tool_specs
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name,
                    "description": tool.description,
                    "parameters_schema": tool.parameters_schema,
                })
            })
            .collect(),
    )
}

fn parsed_tool_calls_trace_payload(tool_calls: &[ParsedToolCall]) -> serde_json::Value {
    serde_json::Value::Array(
        tool_calls
            .iter()
            .map(|tool_call| {
                serde_json::json!({
                    "id": tool_call.id,
                    "name": tool_call.name,
                    "arguments": tool_call.arguments,
                })
            })
            .collect(),
    )
}

// ── Harness-protocol (EventMsg) text/reasoning emits ──────────────────────────
//
// The harness text/reasoning emits. slab-agent speaks `EventMsg` (its harness
// protocol) exclusively — the legacy `AgentEventKind`/`/responses` wire left
// this crate. These mirror, byte-for-byte, what `HarnessProjection`
// used to derive so the harness wire is unchanged.

fn harness_turn_id(turn_index: u32) -> String {
    turn_index.to_string()
}

async fn emit_agent_message_started(
    notify: &dyn AgentNotifyPort,
    thread_id: &str,
    turn_index: u32,
    item_id: &str,
) {
    let msg = EventMsg::ItemStarted(ItemStartedParams {
        item: TurnItem::AgentMessage { id: item_id.to_owned(), text: String::new() },
        thread_id: thread_id.to_owned(),
        turn_id: harness_turn_id(turn_index),
    });
    notify.on_event_msg(thread_id, &msg).await;
}

async fn emit_agent_message_delta(
    notify: &dyn AgentNotifyPort,
    thread_id: &str,
    turn_index: u32,
    item_id: &str,
    delta: &str,
) {
    let msg = EventMsg::AgentMessageDelta(AgentMessageDeltaParams {
        thread_id: thread_id.to_owned(),
        turn_id: harness_turn_id(turn_index),
        item_id: item_id.to_owned(),
        delta: delta.to_owned(),
    });
    notify.on_event_msg(thread_id, &msg).await;
}

async fn emit_agent_message_completed(
    notify: &dyn AgentNotifyPort,
    thread_id: &str,
    turn_index: u32,
    item_id: &str,
    text: &str,
) {
    let msg = EventMsg::ItemCompleted(ItemCompletedParams {
        item: TurnItem::AgentMessage { id: item_id.to_owned(), text: text.to_owned() },
        thread_id: thread_id.to_owned(),
        turn_id: harness_turn_id(turn_index),
    });
    notify.on_event_msg(thread_id, &msg).await;
}

async fn emit_reasoning_delta_msg(
    notify: &dyn AgentNotifyPort,
    thread_id: &str,
    turn_index: u32,
    item_id: &str,
    delta: &str,
) {
    let msg = EventMsg::ReasoningTextDelta(ReasoningTextDeltaParams {
        thread_id: thread_id.to_owned(),
        turn_id: harness_turn_id(turn_index),
        item_id: item_id.to_owned(),
        content_index: 0,
        delta: delta.to_owned(),
    });
    notify.on_event_msg(thread_id, &msg).await;
}

async fn emit_reasoning_completed(
    notify: &dyn AgentNotifyPort,
    thread_id: &str,
    turn_index: u32,
    item_id: &str,
    text: &str,
) {
    let msg = EventMsg::ItemCompleted(ItemCompletedParams {
        item: TurnItem::Reasoning {
            id: item_id.to_owned(),
            summary: ReasoningText::one(text),
            content: ReasoningText::one(text),
        },
        thread_id: thread_id.to_owned(),
        turn_id: harness_turn_id(turn_index),
    });
    notify.on_event_msg(thread_id, &msg).await;
}

const THINK_OPEN_MARKER: &str = "<think";
const THINK_CLOSE_TAG: &str = "</think>";

/// Find the start of a `<think …>` open tag whose next char is `>` or
/// whitespace (so `<thinking>` and friends are not matched).
fn find_think_open(text: &str) -> Option<usize> {
    let mut search = 0;
    while let Some(found) = text[search..].find(THINK_OPEN_MARKER) {
        let at = search + found;
        let after = &text[at + THINK_OPEN_MARKER.len()..];
        let is_tag = after.chars().next().is_some_and(|c| c == '>' || c.is_whitespace());
        if is_tag {
            return Some(at);
        }
        search = at + THINK_OPEN_MARKER.len();
    }
    None
}

/// Remove complete `<think …>…</think>` blocks from assistant text.
///
/// The app-core adapter embeds the turn's reasoning into the LLM-grade
/// assistant text (`format_assistant_content`) so the next prompt's chat
/// template slots it correctly. That form must not reach the UI-grade
/// agentMessage item — history renders the item text verbatim, which would
/// show the raw thinking block in the message body. Unterminated blocks
/// (streaming truncation) are kept verbatim.
///
/// Public so app-core can produce UI-grade text on its own surfaces (e.g. the
/// REST history endpoint reads the LLM-grade rollout messages directly).
pub fn strip_think_blocks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = find_think_open(rest) {
        let after_open = &rest[open + THINK_OPEN_MARKER.len()..];
        let Some(tag_end) = after_open.find('>') else {
            break;
        };
        let body = &after_open[tag_end + 1..];
        let Some(close) = body.find(THINK_CLOSE_TAG) else {
            break;
        };
        out.push_str(&rest[..open]);
        rest = &body[close + THINK_CLOSE_TAG.len()..];
    }
    out.push_str(rest);
    out.trim().to_owned()
}

struct TurnTextDeltaObserver<'a> {
    thread_id: &'a str,
    turn_index: u32,
    notify: &'a dyn AgentNotifyPort,
    /// Whether the `ItemStarted(AgentMessage)` for this turn's assistant item
    /// has been emitted. Mirrors the projection's `started_items` dedup: the
    /// first text delta announces the item, later deltas only carry content.
    text_started: bool,
    /// Whether ANY delta (text or reasoning) reached the client. Gates the
    /// LLM retry loop: a partially-streamed response must never be retried —
    /// the client would see the streamed prefix twice.
    any_delta_emitted: bool,
}

#[async_trait]
impl LlmStreamObserver for TurnTextDeltaObserver<'_> {
    async fn on_text_delta(&mut self, delta: &str) -> Result<(), AgentError> {
        if delta.is_empty() {
            return Ok(());
        }
        self.any_delta_emitted = true;

        let item_id = assistant_item_id(self.turn_index);
        if !self.text_started {
            self.text_started = true;
            emit_agent_message_started(self.notify, self.thread_id, self.turn_index, &item_id)
                .await;
        }
        emit_agent_message_delta(self.notify, self.thread_id, self.turn_index, &item_id, delta)
            .await;
        Ok(())
    }

    async fn on_reasoning_delta(&mut self, delta: &str) -> Result<(), AgentError> {
        if delta.is_empty() {
            return Ok(());
        }
        self.any_delta_emitted = true;

        emit_reasoning_delta_msg(
            self.notify,
            self.thread_id,
            self.turn_index,
            &assistant_item_id(self.turn_index),
            delta,
        )
        .await;
        Ok(())
    }

    async fn on_reasoning_done(&mut self, text: &str) -> Result<(), AgentError> {
        if text.trim().is_empty() {
            return Ok(());
        }

        emit_reasoning_completed(
            self.notify,
            self.thread_id,
            self.turn_index,
            &assistant_item_id(self.turn_index),
            text,
        )
        .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{is_external_tool_name, strip_think_blocks};

    #[test]
    fn strip_think_blocks_removes_embedded_reasoning_wrapper() {
        let text = "<think status=\"done\">\n\nplan the reply\n\n</think>\n\nfinal answer";
        assert_eq!(strip_think_blocks(text), "final answer");
    }

    #[test]
    fn strip_think_blocks_reasoning_only_message_becomes_empty() {
        let text = "<think status=\"done\">\n\nonly thinking\n\n</think>";
        assert_eq!(strip_think_blocks(text), "");
    }

    #[test]
    fn strip_think_blocks_keeps_unterminated_block_verbatim() {
        let text = "before<think>never closes";
        assert_eq!(strip_think_blocks(text), "before<think>never closes");
    }

    #[test]
    fn strip_think_blocks_ignores_similar_tags() {
        assert_eq!(
            strip_think_blocks("<thinking>not a think tag</thinking>"),
            "<thinking>not a think tag</thinking>"
        );
        assert_eq!(strip_think_blocks("plain text"), "plain text");
    }

    #[test]
    fn strip_think_blocks_handles_multiple_blocks() {
        assert_eq!(strip_think_blocks("<think>a</think>x<think>b</think>y"), "xy",);
    }

    #[test]
    fn is_external_tool_name_classifies_offline_droppable_tools() {
        for external in ["web_search", "mcp_call", "mcp_list_tools", "mcp__server__tool"] {
            assert!(is_external_tool_name(external), "{external} should be external");
        }
        for local in [
            "read_file",
            "write_file",
            "shell",
            "grep",
            "plan",
            "update_plan",
            "present_plan",
            "task.complete",
            "verify",
        ] {
            assert!(!is_external_tool_name(local), "{local} should stay available offline");
            assert!(!is_external_tool_name(local), "{local} should stay available offline");
        }
    }
}
