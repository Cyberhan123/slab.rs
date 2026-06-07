use serde_json::Value;
use slab_agent::AgentError;

pub(crate) fn string_arg<'a>(arguments: &'a Value, name: &str) -> Result<&'a str, AgentError> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| AgentError::ToolExecution(format!("missing '{name}' argument")))
}
