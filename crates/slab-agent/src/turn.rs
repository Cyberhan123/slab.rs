//! Single-turn execution logic (private to the crate).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use uuid::Uuid;

use slab_agent_tracing::{AgentTraceContext, AgentTraceSink, record_json};
use slab_types::{
    ConversationMessage, ConversationMessageContent, ConversationToolCall, ConversationToolFunction,
};

use crate::{
    config::AgentConfig,
    error::AgentError,
    event::{AgentEventKind, AgentMetrics},
    hook::AgentHook,
    port::{
        AgentNotifyPort, AgentStorePort, ApprovalPort, LlmPort, LlmStreamObserver, ParsedToolCall,
        ThreadMessageRecord, ToolSpec, TurnEvent,
    },
    risk::ToolRiskAnalyzer,
    tool::ToolRouter,
    turn_tool_call::handle_tool_calls,
};

/// Execute a single LLM turn.
///
/// Returns `true` if another turn is needed (i.e. the model emitted tool
/// calls), or `false` when the model produced a final answer.
pub(crate) struct TurnExecutionContext<'a> {
    pub thread_id: &'a str,
    pub turn_index: u32,
    pub depth: u32,
    pub config: &'a AgentConfig,
    pub llm: &'a dyn LlmPort,
    pub tools: &'a ToolRouter,
    pub store: &'a dyn AgentStorePort,
    pub notify: &'a dyn AgentNotifyPort,
    pub approval: &'a dyn ApprovalPort,
    pub hooks: &'a [Arc<dyn AgentHook>],
    pub risk: &'a dyn ToolRiskAnalyzer,
    pub trace: &'a dyn AgentTraceSink,
    pub trace_context: AgentTraceContext,
    pub cancellation: &'a CancellationToken,
}

pub(crate) async fn execute_turn(
    context: TurnExecutionContext<'_>,
    messages: &mut Vec<ConversationMessage>,
) -> Result<bool, AgentError> {
    let turn_started_at = std::time::Instant::now();
    if context.cancellation.is_cancelled() {
        return Err(AgentError::Interrupted);
    }

    let tool_specs = allowed_tool_specs(&context);

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

    let mut stream_observer = TurnTextDeltaObserver {
        thread_id: context.thread_id,
        turn_index: context.turn_index,
        notify: context.notify,
    };
    let response = tokio::select! {
        response = context.llm.chat_completion_streaming(
            &context.config.model,
            messages,
            &tool_specs,
            context.config,
            &context.trace_context,
            &mut stream_observer,
        ) => response?,
        _ = context.cancellation.cancelled() => return Err(AgentError::Interrupted),
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
        }),
    );

    if response.tool_calls.is_empty() {
        persist_final_answer(&context, messages, response.content.unwrap_or_default()).await;
        emit_turn_metrics(&context, turn_started_at, true).await;
        record_json(
            context.trace,
            &context.trace_context,
            "slab-agent",
            "turn_completed",
            serde_json::json!({ "more_turns": false }),
        );
        return Ok(false);
    }

    emit_unstreamed_tool_text(
        &context,
        response.content.as_deref(),
        response.content_already_streamed,
    )
    .await;
    persist_assistant_tool_request(&context, messages, &response).await;
    handle_tool_calls(&context, &response.tool_calls, messages).await?;

    emit_turn_metrics(&context, turn_started_at, true).await;
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "turn_completed",
        serde_json::json!({ "more_turns": true }),
    );
    Ok(true)
}

fn allowed_tool_specs(context: &TurnExecutionContext<'_>) -> Vec<ToolSpec> {
    if context.config.allowed_tools.is_empty() {
        return context.tools.tool_specs();
    }

    context
        .tools
        .tool_specs()
        .into_iter()
        .filter(|tool| context.config.allowed_tools.contains(&tool.name))
        .collect()
}

async fn persist_final_answer(
    context: &TurnExecutionContext<'_>,
    messages: &mut Vec<ConversationMessage>,
    content: String,
) {
    context
        .notify
        .on_turn_event(
            context.thread_id,
            &TurnEvent::Response {
                turn_index: Some(context.turn_index),
                event: AgentEventKind::ResponseOutputTextDone {
                    item_id: assistant_item_id(context.turn_index),
                    output_index: 0,
                    content_index: 0,
                    text: content.clone(),
                },
            },
        )
        .await;

    let message = ConversationMessage {
        role: "assistant".to_owned(),
        content: ConversationMessageContent::Text(content),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    };
    persist_thread_message(context.store, context.thread_id, context.turn_index, &message).await;
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

    context
        .notify
        .on_turn_event(
            context.thread_id,
            &TurnEvent::Response {
                turn_index: Some(context.turn_index),
                event: AgentEventKind::ResponseOutputTextDelta {
                    item_id: assistant_item_id(context.turn_index),
                    output_index: 0,
                    content_index: 0,
                    delta: text.to_owned(),
                },
            },
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
    persist_thread_message(
        context.store,
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

pub(crate) async fn persist_thread_message(
    store: &dyn AgentStorePort,
    thread_id: &str,
    turn_index: u32,
    message: &ConversationMessage,
) {
    let record = ThreadMessageRecord {
        id: Uuid::new_v4().to_string(),
        thread_id: thread_id.to_owned(),
        turn_index,
        message: message.clone(),
        created_at: Utc::now().to_rfc3339(),
    };
    if let Err(error) = store.insert_thread_message(&record).await {
        warn!(error = %error, thread_id, "failed to persist thread message");
    }
}

async fn emit_turn_metrics(
    context: &TurnExecutionContext<'_>,
    started_at: std::time::Instant,
    success: bool,
) {
    context
        .notify
        .on_turn_event(
            context.thread_id,
            &TurnEvent::Response {
                turn_index: Some(context.turn_index),
                event: AgentEventKind::ResponseMetrics {
                    metrics: AgentMetrics {
                        name: "agent_turn".to_owned(),
                        duration_ms: started_at.elapsed().as_millis(),
                        success: Some(success),
                    },
                },
            },
        )
        .await;
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

struct TurnTextDeltaObserver<'a> {
    thread_id: &'a str,
    turn_index: u32,
    notify: &'a dyn AgentNotifyPort,
}

#[async_trait]
impl LlmStreamObserver for TurnTextDeltaObserver<'_> {
    async fn on_text_delta(&mut self, delta: &str) -> Result<(), AgentError> {
        if delta.is_empty() {
            return Ok(());
        }

        self.notify
            .on_turn_event(
                self.thread_id,
                &TurnEvent::Response {
                    turn_index: Some(self.turn_index),
                    event: AgentEventKind::ResponseOutputTextDelta {
                        item_id: assistant_item_id(self.turn_index),
                        output_index: 0,
                        content_index: 0,
                        delta: delta.to_owned(),
                    },
                },
            )
            .await;
        Ok(())
    }

    async fn on_reasoning_delta(&mut self, delta: &str) -> Result<(), AgentError> {
        if delta.is_empty() {
            return Ok(());
        }

        self.notify
            .on_turn_event(
                self.thread_id,
                &TurnEvent::Response {
                    turn_index: Some(self.turn_index),
                    event: AgentEventKind::ResponseReasoningTextDelta {
                        item_id: assistant_item_id(self.turn_index),
                        output_index: 0,
                        content_index: 0,
                        delta: delta.to_owned(),
                    },
                },
            )
            .await;
        Ok(())
    }

    async fn on_reasoning_done(&mut self, text: &str) -> Result<(), AgentError> {
        if text.trim().is_empty() {
            return Ok(());
        }

        self.notify
            .on_turn_event(
                self.thread_id,
                &TurnEvent::Response {
                    turn_index: Some(self.turn_index),
                    event: AgentEventKind::ResponseReasoningTextDone {
                        item_id: assistant_item_id(self.turn_index),
                        output_index: 0,
                        content_index: 0,
                        text: text.to_owned(),
                    },
                },
            )
            .await;
        Ok(())
    }
}
