//! Built-in tool implementations for the Slab agent runtime.
//!
//! `slab-agent` owns the orchestration kernel and tool traits. This crate
//! contains host-provided deterministic tools and registration helpers.

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput, ToolRouter};

/// Register all built-in host tools with the provided router.
pub fn register_builtin_tools(router: &mut ToolRouter) {
    router.register(Box::new(EchoTool));
}

/// A trivial tool that echoes its `message` argument back to the agent.
pub struct EchoTool;

#[async_trait]
impl ToolHandler for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echo the provided message back verbatim. Useful for testing the agent tool-call loop."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The text to echo back."
                }
            },
            "required": ["message"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let message = arguments.get("message").and_then(Value::as_str).unwrap_or("").to_owned();
        Ok(ToolOutput { content: message, metadata: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn echo_tool_returns_input() {
        let ctx = ToolContext { thread_id: "t1".into(), turn_index: 0, depth: 0 };
        let args = serde_json::json!({"message": "test message"});

        let output = EchoTool.execute(&ctx, &args).await.expect("echo should succeed");
        assert_eq!(output.content, "test message");
    }

    #[tokio::test]
    async fn echo_tool_missing_message_returns_empty() {
        let ctx = ToolContext { thread_id: "t1".into(), turn_index: 0, depth: 0 };
        let args = serde_json::json!({});

        let output = EchoTool.execute(&ctx, &args).await.expect("echo should succeed");
        assert_eq!(output.content, "");
    }

    #[test]
    fn register_builtin_tools_adds_echo() {
        let mut router = ToolRouter::new();
        register_builtin_tools(&mut router);

        let specs = router.tool_specs();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, "echo");
    }
}
