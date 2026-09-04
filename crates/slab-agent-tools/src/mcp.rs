//! MCP proxy tools.

use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slab_agent::{
    AgentError, ToolCallRender, ToolContext, ToolHandler, ToolOutput, ToolVisibility,
    parse_tool_input, protocol::TurnItem, typed_input_schema,
};
use slab_mcp::{McpClient, McpToolSpec};

/// Arguments for the `mcp_call` tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct McpCallArgs {
    server_name: String,
    tool_name: String,
    #[serde(default = "default_arguments")]
    arguments: Value,
}

fn default_arguments() -> Value {
    Value::Object(serde_json::Map::new())
}

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
        typed_input_schema::<McpCallArgs>()
    }

    fn category(&self) -> slab_agent::OperationCategory {
        slab_agent::OperationCategory::Network
    }

    fn render_turn_item(&self, render: &ToolCallRender<'_>) -> TurnItem {
        TurnItem::McpToolCall {
            id: render.call.id.clone(),
            // Schema keys are `server_name` / `tool_name` (NOT `server` / `tool`).
            server: render
                .args
                .get("server_name")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>")
                .to_owned(),
            tool: render
                .args
                .get("tool_name")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>")
                .to_owned(),
            arguments: render.args.get("arguments").cloned().unwrap_or_else(|| render.args.clone()),
            status: render.status.to_owned(),
            result: render.output.and_then(|o| serde_json::from_str(o).ok()),
            error: None,
            duration_ms: None,
        }
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args = parse_tool_input::<McpCallArgs>(arguments)?;
        let result = self
            .client
            .call_tool(&args.server_name, &args.tool_name, args.arguments)
            .await
            .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
        Ok(ToolOutput {
            content: serde_json::to_string(&result)
                .map_err(|error| AgentError::ToolExecution(error.to_string()))?,
            metadata: None,
        })
    }
}

pub struct McpListToolsTool {
    client: Arc<McpClient>,
}

impl McpListToolsTool {
    pub fn new(client: Arc<McpClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ToolHandler for McpListToolsTool {
    fn name(&self) -> &str {
        "mcp_list_tools"
    }

    fn description(&self) -> &str {
        "List tools exposed by configured external MCP servers."
    }

    fn parameters_schema(&self) -> Value {
        // No-arg tool: `Value` keeps any stray arguments tolerated at parse
        // time (an empty struct would reject non-object calls).
        typed_input_schema::<Value>()
    }

    fn category(&self) -> slab_agent::OperationCategory {
        slab_agent::OperationCategory::Network
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        _arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let tools = self
            .client
            .list_tools()
            .await
            .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
        Ok(ToolOutput {
            content: serde_json::to_string(&tools)
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

    fn category(&self) -> slab_agent::OperationCategory {
        slab_agent::OperationCategory::Network
    }

    fn namespace(&self) -> slab_agent::ToolNamespace {
        // Wire form is `mcp__{server}__{tool}`; the structured namespace is `mcp`.
        slab_agent::ToolNamespace::new("mcp")
    }

    fn visibility(&self) -> ToolVisibility {
        // MCP proxies are Deferred: kept out of the base tool list until the
        // model discovers them via `tool_search`, so many MCP tools don't bloat
        // the model-facing tool table.
        ToolVisibility::Deferred
    }

    fn render_turn_item(&self, render: &ToolCallRender<'_>) -> TurnItem {
        // Use the original (unsanitized) server/tool names from the spec rather
        // than parsing the wire name, so the harness shows readable values.
        TurnItem::McpToolCall {
            id: render.call.id.clone(),
            server: self.spec.server_name.clone(),
            tool: self.spec.tool.name.clone(),
            arguments: render.args.clone(),
            status: render.status.to_owned(),
            result: render.output.and_then(|o| serde_json::from_str(o).ok()),
            error: None,
            duration_ms: None,
        }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use slab_mcp::{McpClient, McpTool, McpToolSpec};

    use super::*;

    #[test]
    fn mcp_proxy_tool_names_are_stable_and_identifier_safe() {
        let spec = McpToolSpec {
            server_name: "team server".into(),
            tool: McpTool {
                name: "search.web/v1".into(),
                description: Some("Search the web".into()),
                input_schema: json!({"type": "object", "properties": {"query": {"type": "string"}}}),
            },
        };
        let tool = McpProxyTool::new(Arc::new(McpClient::new()), spec);

        assert_eq!(tool.name(), "mcp__team_server__search_web_v1");
        assert_eq!(tool.description(), "Search the web");
        assert_eq!(tool.parameters_schema()["properties"]["query"]["type"], "string");
    }

    #[test]
    fn mcp_proxy_tool_uses_empty_object_schema_for_null_input_schema() {
        let spec = McpToolSpec {
            server_name: "server".into(),
            tool: McpTool { name: "tool".into(), description: None, input_schema: Value::Null },
        };
        let tool = McpProxyTool::new(Arc::new(McpClient::new()), spec);

        assert_eq!(tool.description(), "Remote MCP tool proxy.");
        assert_eq!(tool.parameters_schema(), json!({"type": "object", "properties": {}}));
    }

    #[test]
    fn mcp_proxy_tool_is_deferred_and_namespaced() {
        let spec = McpToolSpec {
            server_name: "team server".into(),
            tool: McpTool { name: "search".into(), description: None, input_schema: json!({}) },
        };
        let tool = McpProxyTool::new(Arc::new(McpClient::new()), spec);
        assert_eq!(tool.visibility(), ToolVisibility::Deferred);
        assert_eq!(tool.namespace().as_str(), "mcp");
    }

    #[test]
    fn mcp_proxy_tool_renders_mcp_tool_call_from_spec() {
        // Render uses the original (unsanitized) server/tool names from the spec.
        let spec = McpToolSpec {
            server_name: "team server".into(),
            tool: McpTool {
                name: "search.web/v1".into(),
                description: None,
                input_schema: json!({}),
            },
        };
        let tool = McpProxyTool::new(Arc::new(McpClient::new()), spec);
        let call = slab_agent::port::ParsedToolCall {
            id: "c1".into(),
            name: "mcp__team_server__search_web_v1".into(),
            arguments: r#"{"q":"x"}"#.into(),
        };
        let args = json!({"q": "x"});
        let render = ToolCallRender {
            call: &call,
            args: &args,
            status: "running",
            output: None,
            workspace_root: None,
            exit_code: None,
            duration_ms: None,
        };
        match tool.render_turn_item(&render) {
            TurnItem::McpToolCall { server, tool, arguments, status, .. } => {
                assert_eq!(server, "team server");
                assert_eq!(tool, "search.web/v1");
                assert_eq!(arguments["q"], "x");
                assert_eq!(status, "running");
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn mcp_call_tool_schema_requires_server_and_tool_names() {
        let tool = McpCallTool::new(Arc::new(McpClient::new()));
        let schema = tool.parameters_schema();

        assert_eq!(tool.name(), "mcp_call");
        assert_eq!(schema["required"], json!(["server_name", "tool_name"]));
        assert_eq!(schema["properties"]["arguments"]["default"], json!({}));
    }

    #[tokio::test]
    async fn mcp_list_tools_returns_json_array() {
        let tool = McpListToolsTool::new(Arc::new(McpClient::new()));
        let ctx = ToolContext::for_thread("thread").build();
        let output = tool.execute(&ctx, &json!({})).await.expect("list tools output");

        assert_eq!(tool.name(), "mcp_list_tools");
        assert_eq!(tool.parameters_schema(), json!({"type": "object", "properties": {}}));
        assert_eq!(serde_json::from_str::<Value>(&output.content).expect("json"), json!([]));
    }
}
