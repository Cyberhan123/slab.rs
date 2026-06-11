use std::collections::HashSet;

use crate::{
    config::AgentToolChoice,
    port::{ParsedToolCall, ToolSpec},
};

#[derive(Debug, Clone)]
pub(crate) struct InvalidToolCall {
    pub tool_call: ParsedToolCall,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolCallValidation {
    pub valid: Vec<ParsedToolCall>,
    pub invalid: Vec<InvalidToolCall>,
}

pub(crate) fn validate_tool_calls(
    tool_choice: &AgentToolChoice,
    allowed_tools: &[String],
    tool_specs: &[ToolSpec],
    tool_calls: &[ParsedToolCall],
) -> ToolCallValidation {
    let available_names = tool_specs.iter().map(|tool| tool.name.as_str()).collect::<HashSet<_>>();
    let mut valid = Vec::new();
    let mut invalid = Vec::new();

    for tool_call in tool_calls {
        if let Some(reason) =
            validate_tool_call(tool_choice, allowed_tools, &available_names, tool_call)
        {
            invalid.push(InvalidToolCall { tool_call: tool_call.clone(), reason });
        } else {
            valid.push(tool_call.clone());
        }
    }

    ToolCallValidation { valid, invalid }
}

fn validate_tool_call(
    tool_choice: &AgentToolChoice,
    allowed_tools: &[String],
    available_names: &HashSet<&str>,
    tool_call: &ParsedToolCall,
) -> Option<String> {
    match tool_choice {
        AgentToolChoice::None => {
            return Some("tool_choice none disallows tool calls".to_owned());
        }
        AgentToolChoice::Tool { name } if tool_call.name != *name => {
            return Some(format!("tool_choice requires tool '{name}'"));
        }
        AgentToolChoice::Auto | AgentToolChoice::Required | AgentToolChoice::Tool { .. } => {}
    }

    if !available_names.contains(tool_call.name.as_str()) {
        if !allowed_tools.is_empty() && !allowed_tools.contains(&tool_call.name) {
            return Some(format!("tool not allowed: {}", tool_call.name));
        }
        return Some(format!("tool not available: {}", tool_call.name));
    }

    if let Err(error) = serde_json::from_str::<serde_json::Value>(&tool_call.arguments) {
        return Some(format!("invalid tool call arguments: {error}"));
    }

    None
}
