//! Single-turn execution logic (private to the crate).

use chrono::Utc;
use tracing::{debug, warn};
use uuid::Uuid;

use slab_types::{
    ConversationMessage, ConversationMessageContent, ConversationToolCall,
    ConversationToolFunction, agent::ToolCallStatus,
};

use crate::{
    config::AgentConfig,
    error::AgentError,
    port::{AgentStorePort, LlmPort, ToolCallRecord},
    tool::{ToolContext, ToolRouter},
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
}

pub(crate) async fn execute_turn(
    context: TurnExecutionContext<'_>,
    messages: &mut Vec<ConversationMessage>,
) -> Result<bool, AgentError> {
    // Build the list of allowed tool specs for this turn.
    let tool_specs: Vec<_> = if context.config.allowed_tools.is_empty() {
        context.tools.tool_specs()
    } else {
        context
            .tools
            .tool_specs()
            .into_iter()
            .filter(|s| context.config.allowed_tools.contains(&s.name))
            .collect()
    };

    debug!(thread_id = context.thread_id, turn_index = context.turn_index, "executing turn");

    let response = context
        .llm
        .chat_completion(&context.config.model, messages, &tool_specs, context.config)
        .await?;

    if response.tool_calls.is_empty() {
        // Model produced a final answer — no more turns required.
        let content = response.content.unwrap_or_default();
        messages.push(ConversationMessage {
            role: "assistant".to_owned(),
            content: ConversationMessageContent::Text(content),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        });
        return Ok(false);
    }

    // Model requested tool calls — build the assistant message and execute.
    let assistant_tool_calls: Vec<ConversationToolCall> = response
        .tool_calls
        .iter()
        .map(|tc| ConversationToolCall {
            id: Some(tc.id.clone()),
            r#type: "function".to_owned(),
            function: ConversationToolFunction {
                name: tc.name.clone(),
                arguments: tc.arguments.clone(),
            },
        })
        .collect();

    messages.push(ConversationMessage {
        role: "assistant".to_owned(),
        content: ConversationMessageContent::Text(response.content.unwrap_or_default()),
        name: None,
        tool_call_id: None,
        tool_calls: assistant_tool_calls,
    });

    let ctx = ToolContext {
        thread_id: context.thread_id.to_owned(),
        turn_index: context.turn_index,
        depth: context.depth,
    };
    let now = Utc::now().to_rfc3339();

    for tc in &response.tool_calls {
        let call_id = Uuid::new_v4().to_string();

        let record = ToolCallRecord {
            id: call_id.clone(),
            thread_id: context.thread_id.to_owned(),
            tool_name: tc.name.clone(),
            arguments: tc.arguments.clone(),
            output: None,
            status: ToolCallStatus::Running,
            created_at: now.clone(),
            completed_at: None,
        };

        if let Err(e) = context.store.insert_tool_call(&record).await {
            warn!(error = %e, call_id, "failed to persist tool call record");
        }

        let (output, call_status) = match context.tools.get(&tc.name) {
            None => {
                warn!(thread_id = context.thread_id, tool = %tc.name, "tool not found");
                (format!("tool not found: {}", tc.name), ToolCallStatus::Failed)
            }
            Some(handler) => match serde_json::from_str::<serde_json::Value>(&tc.arguments) {
                Err(e) => {
                    warn!(
                        thread_id = context.thread_id,
                        tool = %tc.name,
                        error = %e,
                        "failed to parse tool call arguments as JSON"
                    );
                    (format!("invalid tool call arguments: {e}"), ToolCallStatus::Failed)
                }
                Ok(args) => match handler.execute(&ctx, &args).await {
                    Ok(out) => (out.content, ToolCallStatus::Completed),
                    Err(e) => {
                        warn!(
                            thread_id = context.thread_id,
                            tool = %tc.name,
                            error = %e,
                            "tool execution failed"
                        );
                        (e.to_string(), ToolCallStatus::Failed)
                    }
                },
            },
        };

        let completed_at = Utc::now().to_rfc3339();
        if let Err(e) = context
            .store
            .update_tool_call(&call_id, Some(&output), call_status, &completed_at)
            .await
        {
            warn!(error = %e, call_id, "failed to update tool call record");
        }

        messages.push(ConversationMessage {
            role: "tool".to_owned(),
            content: ConversationMessageContent::Text(output),
            name: None,
            tool_call_id: Some(tc.id.clone()),
            tool_calls: vec![],
        });
    }

    Ok(true)
}
