//! MCP proxy tools.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};
use slab_mcp::{McpClient, McpToolSpec};

use crate::args::string_arg;

pub struct McpCallTool {
    client: Arc<McpClient>,
}

impl McpCallTool {
    pub fn new(client: Arc<McpClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ToolHandler for McpCallTool {
    fn name(&self) -> &str {
        "mcp_call"
    }

    fn description(&self) -> &str {
        "Call a tool on a configured external MCP server."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "server_name": { "type": "string" },
                "tool_name": { "type": "string" },
                "arguments": {
                    "type": "object",
                    "default": {}
                }
            },
            "required": ["server_name", "tool_name"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let server_name = string_arg(arguments, "server_name")?;
        let tool_name = string_arg(arguments, "tool_name")?;
        let tool_arguments =
            arguments.get("arguments").cloned().unwrap_or_else(|| serde_json::json!({}));
        let result = self
            .client
            .call_tool(server_name, tool_name, tool_arguments)
            .await
            .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
        Ok(ToolOutput {
            content: serde_json::to_string(&result)
                .map_err(|error| AgentError::ToolExecution(error.to_string()))?,
            metadata: None,
        })
    }
}

pub struct McpProxyTool {
    client: Arc<McpClient>,
    spec: McpToolSpec,
    name: String,
}

impl McpProxyTool {
    pub fn new(client: Arc<McpClient>, spec: McpToolSpec) -> Self {
        let name = proxy_tool_name(&spec.server_name, &spec.tool.name);
        Self { client, spec, name }
    }
}

#[async_trait]
impl ToolHandler for McpProxyTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        self.spec.tool.description.as_deref().unwrap_or("Remote MCP tool proxy.")
    }

    fn parameters_schema(&self) -> Value {
        if self.spec.tool.input_schema.is_null() {
            return serde_json::json!({ "type": "object", "properties": {} });
        }
        self.spec.tool.input_schema.clone()
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let result = self
            .client
            .call_tool(&self.spec.server_name, &self.spec.tool.name, arguments.clone())
            .await
            .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
        Ok(ToolOutput {
            content: serde_json::to_string(&result)
                .map_err(|error| AgentError::ToolExecution(error.to_string()))?,
            metadata: None,
        })
    }
}

fn proxy_tool_name(server_name: &str, tool_name: &str) -> String {
    format!("mcp__{}__{}", sanitize_name(server_name), sanitize_name(tool_name))
}

fn sanitize_name(value: &str) -> String {
    value.chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' { ch } else { '_' }).collect()
}
