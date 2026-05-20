use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use slab_agent::{ToolContext, ToolRouter};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Error)]
pub enum McpServerError {
    #[error("invalid JSON-RPC message")]
    InvalidMessage,
    #[error("tool not found: {0}")]
    ToolNotFound(String),
    #[error("tool execution failed: {0}")]
    ToolExecution(String),
}

pub struct McpServerToolRouter {
    config: McpServerConfig,
    router: Arc<ToolRouter>,
}

impl McpServerToolRouter {
    pub fn new(config: McpServerConfig, router: Arc<ToolRouter>) -> Self {
        Self { config, router }
    }

    pub async fn handle_message(&self, message: Value) -> Result<Option<Value>, McpServerError> {
        let method =
            message.get("method").and_then(Value::as_str).ok_or(McpServerError::InvalidMessage)?;
        let id = message.get("id").cloned();

        if id.is_none() {
            return Ok(None);
        }
        let id = id.unwrap_or(Value::Null);

        let result = match method {
            "initialize" => json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": self.config.name,
                    "version": self.config.version,
                }
            }),
            "ping" => json!({}),
            "tools/list" => {
                let tools = self
                    .router
                    .tool_specs()
                    .into_iter()
                    .map(|spec| {
                        json!({
                            "name": spec.name,
                            "description": spec.description,
                            "inputSchema": spec.parameters_schema,
                        })
                    })
                    .collect::<Vec<_>>();
                json!({ "tools": tools })
            }
            "tools/call" => {
                let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or(McpServerError::InvalidMessage)?;
                let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                let handler = self
                    .router
                    .get(name)
                    .ok_or_else(|| McpServerError::ToolNotFound(name.to_string()))?;
                let output = handler
                    .execute(
                        &ToolContext { thread_id: "mcp".to_string(), turn_index: 0, depth: 0 },
                        &arguments,
                    )
                    .await
                    .map_err(|e| McpServerError::ToolExecution(e.to_string()))?;
                json!({
                    "content": [{
                        "type": "text",
                        "text": output.content
                    }]
                })
            }
            _ => return Err(McpServerError::InvalidMessage),
        };

        Ok(Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use slab_agent::{AgentError, ToolHandler, ToolOutput};

    struct EchoTool;

    #[async_trait]
    impl ToolHandler for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn description(&self) -> &str {
            "Echo a message."
        }

        fn parameters_schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                }
            })
        }

        async fn execute(
            &self,
            _ctx: &ToolContext,
            arguments: &Value,
        ) -> Result<ToolOutput, AgentError> {
            Ok(ToolOutput {
                content: arguments.get("message").and_then(Value::as_str).unwrap_or("").to_string(),
                metadata: None,
            })
        }
    }

    #[tokio::test]
    async fn handles_initialize_list_tools_and_call_tool() {
        let mut router = ToolRouter::new();
        router.register(Box::new(EchoTool));
        let server = McpServerToolRouter::new(
            McpServerConfig { name: "test".to_string(), version: "0.1.0".to_string() },
            Arc::new(router),
        );

        let initialize = server
            .handle_message(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize"
            }))
            .await
            .expect("initialize")
            .expect("initialize response");
        assert_eq!(initialize["result"]["serverInfo"]["name"], "test");

        let list = server
            .handle_message(json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list"
            }))
            .await
            .expect("tools list")
            .expect("tools list response");
        assert_eq!(list["result"]["tools"][0]["name"], "echo");

        let call = server
            .handle_message(json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "echo",
                    "arguments": { "message": "hello" }
                }
            }))
            .await
            .expect("tools call")
            .expect("tools call response");
        assert_eq!(call["result"]["content"][0]["text"], "hello");
    }
}
