use serde_json::Value;
use slab_agent::AgentError;

pub(crate) fn string_arg<'a>(arguments: &'a Value, name: &str) -> Result<&'a str, AgentError> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| AgentError::ToolExecution(format!("missing '{name}' argument")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn string_arg_requires_present_string_values() {
        assert_eq!(string_arg(&json!({"path": "src/lib.rs"}), "path").expect("path"), "src/lib.rs");
        assert!(matches!(
            string_arg(&json!({"path": 42}), "path"),
            Err(AgentError::ToolExecution(message)) if message == "missing 'path' argument"
        ));
        assert!(matches!(
            string_arg(&json!({}), "path"),
            Err(AgentError::ToolExecution(message)) if message == "missing 'path' argument"
        ));
    }
}
