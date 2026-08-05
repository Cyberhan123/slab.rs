//! Persistence helpers for turn tool calls.
//!
//! The `agent_tool_calls` audit table + its trait methods were removed: tool
//! calls are now captured by the rollout `TurnItem::CommandExecution` /
//! `McpToolCall` stream instead of a side-channel audit row. What remains here
//! is the tool-result message construction + the rollout/trace recording of the
//! persisted tool message (NOT a per-call audit row).

use slab_agent_tracing::record_json;
use slab_types::ConversationMessage;

use crate::{
    port::ParsedToolCall,
    turn::{TurnExecutionContext, emit_message_appended},
};

pub(crate) async fn record_failed_tool_call(
    context: &TurnExecutionContext<'_>,
    call_id: &str,
    tool_call: &ParsedToolCall,
    output: String,
    created_at: &str,
    messages: &mut Vec<ConversationMessage>,
) -> Result<(), crate::error::AgentError> {
    let message = record_failed_tool_call_without_persisting_message(
        context, call_id, tool_call, output, created_at,
    )
    .await?;
    persist_tool_message_record(context, message, messages).await;
    Ok(())
}

/// Build the tool-result message for a failed tool call. The `call_id` /
/// `created_at` / `context` parameters are retained in the signature so callers
/// do not need to branch, but are no longer used to persist a side-channel
/// audit row (that path was removed alongside `agent_tool_calls`).
#[allow(unused_variables)]
pub(crate) async fn record_failed_tool_call_without_persisting_message(
    context: &TurnExecutionContext<'_>,
    call_id: &str,
    tool_call: &ParsedToolCall,
    output: String,
    created_at: &str,
) -> Result<ConversationMessage, crate::error::AgentError> {
    Ok(tool_message(tool_call, output))
}

pub(crate) fn tool_message(tool_call: &ParsedToolCall, output: String) -> ConversationMessage {
    ConversationMessage {
        role: "tool".to_owned(),
        content: slab_types::ConversationMessageContent::Text(output),
        name: None,
        tool_call_id: Some(tool_call.id.clone()),
        tool_calls: vec![],
    }
}

pub(crate) async fn persist_tool_message_record(
    context: &TurnExecutionContext<'_>,
    message: ConversationMessage,
    messages: &mut Vec<ConversationMessage>,
) {
    emit_message_appended(context.notify, context.thread_id, context.turn_index, &message).await;
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "tool_message_persisted",
        serde_json::json!({
            "turn_index": context.turn_index,
            "message": message,
        }),
    );
    messages.push(message);
}
