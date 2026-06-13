use std::{
    collections::HashMap,
    sync::{Arc, RwLock as StdRwLock},
};

use futures::future::{BoxFuture, FutureExt};
use serde_json::Value;
use slab_mcp_client::{
    McpClientError as TransportError, McpTool as TransportTool, McpToolResult, StdioLaunchConfig,
    StdioMcpClient,
};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::debug;

use crate::{
    config::{McpClientConfig, McpServerLauncher},
    protocol::McpToolSpec,
};

#[derive(Debug, Error)]
pub enum McpClientError {
    #[error("MCP server `{0}` is not connected")]
    ServerNotFound(String),
    #[error("MCP server `{server_name}` failed: {source}")]
    Transport { server_name: String, source: TransportError },
}

#[derive(Default)]
pub struct McpClient {
    servers: RwLock<HashMap<String, Arc<dyn ServerConnection>>>,
    cached_tools: StdRwLock<Vec<McpToolSpec>>,
}

impl McpClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn from_config(config: McpClientConfig) -> Result<Self, McpClientError> {
        let client = Self::new();
        for launcher in config.servers {
            client.connect_stdio(launcher).await?;
        }
        Ok(client)
    }

    pub async fn connect_stdio(&self, launcher: McpServerLauncher) -> Result<(), McpClientError> {
        let server_name = launcher.name.clone();
        let server = StdioMcpClient::connect(StdioLaunchConfig {
            command: launcher.command,
            args: launcher.args,
            env: launcher.env,
            cwd: launcher.cwd,
        })
        .await
        .map_err(|source| transport_error(&server_name, source))?;

        self.servers.write().await.insert(server_name.clone(), Arc::new(server));
        self.refresh_cached_tools().await?;
        let tool_count = self
            .cached_tools()
            .await
            .into_iter()
            .filter(|tool| tool.server_name == server_name)
            .count();
        debug!(server = server_name, tool_count, "connected MCP stdio server");
        Ok(())
    }

    pub async fn cached_tools(&self) -> Vec<McpToolSpec> {
        self.cached_tools.read().map(|tools| tools.clone()).unwrap_or_default()
    }

    pub fn cached_tools_blocking(&self) -> Vec<McpToolSpec> {
        self.cached_tools.read().map(|tools| tools.clone()).unwrap_or_default()
    }

    pub async fn list_tools(&self) -> Result<Vec<McpToolSpec>, McpClientError> {
        self.refresh_cached_tools().await?;
        Ok(self.cached_tools().await)
    }

    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<McpToolResult, McpClientError> {
        let server = self.server(server_name).await?;
        server
            .call_tool(tool_name, arguments)
            .await
            .map_err(|source| transport_error(server_name, source))
    }

    pub async fn ping(&self, server_name: &str) -> Result<(), McpClientError> {
        let server = self.server(server_name).await?;
        server.ping().await.map_err(|source| transport_error(server_name, source))
    }

    async fn refresh_cached_tools(&self) -> Result<(), McpClientError> {
        let servers = self.servers.read().await;
        let mut tools = Vec::new();
        for (server_name, server) in servers.iter() {
            let remote_tools =
                server.list_tools().await.map_err(|source| transport_error(server_name, source))?;
            tools.extend(
                remote_tools
                    .into_iter()
                    .map(|tool| McpToolSpec { server_name: server_name.clone(), tool }),
            );
        }
        if let Ok(mut cached_tools) = self.cached_tools.write() {
            *cached_tools = tools;
        }
        Ok(())
    }

    async fn server(&self, server_name: &str) -> Result<Arc<dyn ServerConnection>, McpClientError> {
        let servers = self.servers.read().await;
        servers
            .get(server_name)
            .cloned()
            .ok_or_else(|| McpClientError::ServerNotFound(server_name.to_string()))
    }

    #[cfg(test)]
    async fn insert_server_for_test(&self, server_name: &str, server: Arc<dyn ServerConnection>) {
        self.servers.write().await.insert(server_name.to_string(), server);
    }
}

fn transport_error(server_name: &str, source: TransportError) -> McpClientError {
    McpClientError::Transport { server_name: server_name.to_string(), source }
}

trait ServerConnection: Send + Sync {
    fn list_tools(&self) -> BoxFuture<'_, Result<Vec<TransportTool>, TransportError>>;

    fn call_tool<'a>(
        &'a self,
        tool_name: &'a str,
        arguments: Value,
    ) -> BoxFuture<'a, Result<McpToolResult, TransportError>>;

    fn ping(&self) -> BoxFuture<'_, Result<(), TransportError>>;
}

impl ServerConnection for StdioMcpClient {
    fn list_tools(&self) -> BoxFuture<'_, Result<Vec<TransportTool>, TransportError>> {
        async move { StdioMcpClient::list_tools(self).await }.boxed()
    }

    fn call_tool<'a>(
        &'a self,
        tool_name: &'a str,
        arguments: Value,
    ) -> BoxFuture<'a, Result<McpToolResult, TransportError>> {
        async move { StdioMcpClient::call_tool(self, tool_name, arguments).await }.boxed()
    }

    fn ping(&self) -> BoxFuture<'_, Result<(), TransportError>> {
        async move { StdioMcpClient::ping(self).await }.boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    struct FakeServer {
        tools: Vec<TransportTool>,
        result: McpToolResult,
        calls: Mutex<Vec<(String, Value)>>,
    }

    impl FakeServer {
        fn new(tools: Vec<TransportTool>, result: McpToolResult) -> Self {
            Self { tools, result, calls: Mutex::new(Vec::new()) }
        }

        fn calls(&self) -> Vec<(String, Value)> {
            self.calls.lock().expect("calls").clone()
        }
    }

    impl ServerConnection for FakeServer {
        fn list_tools(&self) -> BoxFuture<'_, Result<Vec<TransportTool>, TransportError>> {
            async move { Ok(self.tools.clone()) }.boxed()
        }

        fn call_tool<'a>(
            &'a self,
            tool_name: &'a str,
            arguments: Value,
        ) -> BoxFuture<'a, Result<McpToolResult, TransportError>> {
            async move {
                self.calls.lock().expect("calls").push((tool_name.to_string(), arguments));
                Ok(self.result.clone())
            }
            .boxed()
        }

        fn ping(&self) -> BoxFuture<'_, Result<(), TransportError>> {
            async move { Ok(()) }.boxed()
        }
    }

    fn tool(name: &str) -> TransportTool {
        TransportTool {
            name: name.to_string(),
            description: Some(format!("{name} description")),
            input_schema: json!({ "type": "object" }),
        }
    }

    fn text_result(text: &str) -> McpToolResult {
        McpToolResult {
            content: vec![slab_mcp_client::McpContent {
                content_type: "text".to_string(),
                fields: serde_json::Map::from_iter([("text".to_string(), json!(text))]),
            }],
            is_error: false,
        }
    }

    #[tokio::test]
    async fn list_tools_aggregates_tools_by_server() {
        let client = McpClient::new();
        client
            .insert_server_for_test(
                "alpha",
                Arc::new(FakeServer::new(vec![tool("echo")], text_result(""))),
            )
            .await;
        client
            .insert_server_for_test(
                "beta",
                Arc::new(FakeServer::new(vec![tool("search")], text_result(""))),
            )
            .await;

        let mut tools = client.list_tools().await.expect("tools");
        tools.sort_by(|left, right| left.server_name.cmp(&right.server_name));

        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].server_name, "alpha");
        assert_eq!(tools[0].tool.name, "echo");
        assert_eq!(tools[1].server_name, "beta");
        assert_eq!(tools[1].tool.name, "search");
        assert_eq!(client.cached_tools_blocking().len(), 2);
    }

    #[tokio::test]
    async fn call_tool_routes_to_named_server() {
        let client = McpClient::new();
        let server = Arc::new(FakeServer::new(vec![tool("echo")], text_result("hello")));
        client.insert_server_for_test("alpha", server.clone()).await;

        let result = client
            .call_tool("alpha", "echo", json!({ "message": "hello" }))
            .await
            .expect("tool result");

        assert_eq!(result.content[0].fields["text"], "hello");
        assert_eq!(server.calls(), vec![("echo".to_string(), json!({ "message": "hello" }))]);
    }

    #[tokio::test]
    async fn call_tool_returns_server_not_found() {
        let client = McpClient::new();
        let error =
            client.call_tool("missing", "echo", json!({})).await.expect_err("missing server");

        assert!(matches!(error, McpClientError::ServerNotFound(name) if name == "missing"));
    }
}
