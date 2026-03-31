//! Built-in `echo` tool.
//!
//! The echo tool simply returns whatever text is passed as its `message`
//! parameter.  It is useful for smoke-testing the agent loop without a real
//! LLM backend: configure a mock [`LlmPort`] that emits a single tool call to
//! `echo`, and verify that the agent receives the response and completes.

use async_trait::async_trait;
use serde_json::Value;

use crate::error::AgentError;
use crate::tool::{ToolContext, ToolHandler, ToolOutput};

/// A trivial tool that echoes its `message` argument back to the agent.
pub struct EchoTool;

#[async_trait]
impl ToolHandler for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echo the provided message back verbatim. \
         Useful for testing the agent tool-call loop."
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
