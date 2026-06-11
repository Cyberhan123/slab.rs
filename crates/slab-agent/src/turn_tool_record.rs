//! Persistence and status recording helpers for turn tool calls.

use chrono::Utc;
use tracing::warn;

use slab_agent_tracing::record_json;
use slab_types::{ConversationMessage, ConversationMessageContent, agent::ToolCallStatus};

use crate::{
    error::AgentError,
    event::{AgentEventKind, ToolExecutionStatus},
    port::{ParsedToolCall, ToolCallRecord, TurnEvent},
    state::ToolCallStateMachine,
    turn::{TurnExecutionContext, persist_thread_message},
};

pub(crate) async fn record_failed_tool_call(
    context: &TurnExecutionContext<'_>,
    call_id: &str,
    tool_call: &ParsedToolCall,
    output: String,
    created_at: &str,
    messages: &mut Vec<ConversationMessage>,
) -> Result<(), AgentError> {
    let message = record_failed_tool_call_without_persisting_message(
        context, call_id, tool_call, output, created_at,
    )
    .await?;
    persist_tool_message_record(context, message, messages).await;
    Ok(())
}

pub(crate) async fn record_failed_tool_call_without_persisting_message(
    context: &TurnExecutionContext<'_>,
    call_id: &str,
    tool_call: &ParsedToolCall,
    output: String,
    created_at: &str,
) -> Result<ConversationMessage, AgentError> {
    let mut tool_state = ToolCallStateMachine::new(ToolCallStatus::Running);
    insert_tool_call_record(context, call_id, tool_call, tool_state.status(), created_at).await;
    let call_status = tool_state.transition(ToolCallStatus::Failed)?;
    context
        .notify
        .on_turn_event(
            context.thread_id,
            &TurnEvent::Response {
                turn_index: Some(context.turn_index),
                event: AgentEventKind::ResponseToolCallOutput {
                    item_id: tool_call.id.clone(),
                    call_id: call_id.to_owned(),
                    output: output.clone(),
                    status: ToolExecutionStatus::Failed,
                },
            },
        )
        .await;
    update_tool_call_record(context, call_id, Some(&output), call_status).await;
    Ok(tool_message(tool_call, output))
}

pub(crate) fn tool_message(tool_call: &ParsedToolCall, output: String) -> ConversationMessage {
    ConversationMessage {
        role: "tool".to_owned(),
        content: ConversationMessageContent::Text(output),
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
    persist_thread_message(context.store, context.thread_id, context.turn_index, &message).await;
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

pub(crate) async fn insert_tool_call_record(
    context: &TurnExecutionContext<'_>,
    call_id: &str,
    tool_call: &ParsedToolCall,
    status: ToolCallStatus,
    created_at: &str,
) {
    let record = ToolCallRecord {
        id: call_id.to_owned(),
        thread_id: context.thread_id.to_owned(),
        tool_name: tool_call.name.clone(),
        arguments: tool_call.arguments.clone(),
        output: None,
        status,
        created_at: created_at.to_owned(),
        completed_at: None,
    };

    if let Err(error) = context.store.insert_tool_call(&record).await {
        warn!(error = %error, call_id, "failed to persist tool call record");
    }
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "tool_call_record_persisted",
        serde_json::json!({
            "record": {
                "id": &record.id,
                "thread_id": &record.thread_id,
                "tool_name": &record.tool_name,
                "arguments": &record.arguments,
                "output": &record.output,
                "status": record.status,
                "created_at": &record.created_at,
                "completed_at": &record.completed_at,
            }
        }),
    );
}

pub(crate) async fn update_tool_call_status(
    context: &TurnExecutionContext<'_>,
    call_id: &str,
    status: ToolCallStatus,
) {
    if let Err(error) = context.store.update_tool_call_status(call_id, status).await {
        warn!(error = %error, call_id, "failed to update tool call status");
    }
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "tool_call_record_status_updated",
        serde_json::json!({
            "call_id": call_id,
            "status": status,
        }),
    );
}

pub(crate) async fn update_tool_call_record(
    context: &TurnExecutionContext<'_>,
    call_id: &str,
    output: Option<&str>,
    status: ToolCallStatus,
) {
    let completed_at = Utc::now().to_rfc3339();
    if let Err(error) = context.store.update_tool_call(call_id, output, status, &completed_at).await
    {
        warn!(error = %error, call_id, "failed to update tool call record");
    }
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "tool_call_record_updated",
        serde_json::json!({
            "call_id": call_id,
            "status": status,
            "completed_at": completed_at,
            "output": output,
        }),
    );
}

pub(crate) fn tool_execution_status(status: ToolCallStatus) -> ToolExecutionStatus {
    match status {
        ToolCallStatus::Pending | ToolCallStatus::Running => ToolExecutionStatus::Failed,
        ToolCallStatus::Completed => ToolExecutionStatus::Completed,
        ToolCallStatus::Failed => ToolExecutionStatus::Failed,
    }
}
