//! Single-turn execution logic (private to the crate).

use chrono::Utc;
use tracing::{debug, warn};
use uuid::Uuid;

use slab_types::{
    agent::ToolCallStatus, ConversationMessage, ConversationMessageContent,
    ConversationToolCall, ConversationToolFunction,
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
pub(crate) async fn execute_turn(
    thread_id: &str,
    turn_index: u32,
    depth: u32,
    messages: &mut Vec<ConversationMessage>,
    config: &AgentConfig,
    llm: &dyn LlmPort,
    tools: &ToolRouter,
    store: &dyn AgentStorePort,
) -> Result<bool, AgentError> {
    // Build the list of allowed tool specs for this turn.
    let tool_specs: Vec<_> = if config.allowed_tools.is_empty() {
        tools.tool_specs()
    } else {
        tools
            .tool_specs()
            .into_iter()
            .filter(|s| config.allowed_tools.contains(&s.name))
            .collect()
    };

    debug!(thread_id, turn_index, "executing turn");

    let response = llm
        .chat_completion(&config.model, messages, &tool_specs, config)
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
        content: ConversationMessageContent::Text(
            response.content.unwrap_or_default(),
        ),
        name: None,
        tool_call_id: None,
        tool_calls: assistant_tool_calls,
    });

    let ctx = ToolContext { thread_id: thread_id.to_owned(), turn_index, depth };
    let now = Utc::now().to_rfc3339();

    for tc in &response.tool_calls {
        let call_id = Uuid::new_v4().to_string();

        let record = ToolCallRecord {
            id: call_id.clone(),
            thread_id: thread_id.to_owned(),
            tool_name: tc.name.clone(),
            arguments: tc.arguments.clone(),
            output: None,
            status: ToolCallStatus::Running,
            created_at: now.clone(),
            completed_at: None,
        };

        if let Err(e) = store.insert_tool_call(&record).await {
            warn!(error = %e, call_id, "failed to persist tool call record");
        }

        let (output, call_status) = match tools.get(&tc.name) {
            None => {
                warn!(thread_id, tool = %tc.name, "tool not found");
                (format!("tool not found: {}", tc.name), ToolCallStatus::Failed)
            }
            Some(handler) => {
                let args: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
                match handler.execute(&ctx, &args).await {
                    Ok(out) => (out.content, ToolCallStatus::Completed),
                    Err(e) => {
                        warn!(thread_id, tool = %tc.name, error = %e, "tool execution failed");
                        (e.to_string(), ToolCallStatus::Failed)
                    }
                }
            }
        };

        let completed_at = Utc::now().to_rfc3339();
        if let Err(e) =
            store.update_tool_call(&call_id, Some(&output), call_status, &completed_at).await
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
