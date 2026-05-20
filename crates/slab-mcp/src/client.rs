use std::{
    collections::HashMap,
    sync::{
        Arc, RwLock as StdRwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use serde_json::{Value, json};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::{Mutex, RwLock},
};
use tracing::debug;

use crate::{
    config::{McpClientConfig, McpServerLauncher},
    protocol::{JsonRpcResponse, McpContent, McpToolResult, McpToolSpec, notification, request},
};

#[derive(Debug, Error)]
pub enum McpClientError {
    #[error("MCP server `{0}` is not connected")]
    ServerNotFound(String),
    #[error("MCP server `{0}` has no stdin")]
    MissingStdin(String),
    #[error("MCP server `{0}` has no stdout")]
    MissingStdout(String),
    #[error("MCP protocol error: {0}")]
    Protocol(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Default)]
pub struct McpClient {
    servers: RwLock<HashMap<String, Arc<StdioServer>>>,
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
        let mut command = Command::new(&launcher.command);
        command.args(&launcher.args);
        for (key, value) in &launcher.env {
            command.env(key, value);
        }
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());

        let mut child = command.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpClientError::MissingStdin(launcher.name.clone()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpClientError::MissingStdout(launcher.name.clone()))?;

        let server = Arc::new(StdioServer {
            name: launcher.name.clone(),
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            next_id: AtomicU64::new(1),
        });
        server.initialize().await?;
        let tools = server.list_tools().await?;

        self.servers.write().await.insert(launcher.name.clone(), server);
        self.refresh_cached_tools().await?;
        debug!(server = launcher.name, tool_count = tools.len(), "connected MCP stdio server");
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
        let servers = self.servers.read().await;
        let server = servers
            .get(server_name)
            .ok_or_else(|| McpClientError::ServerNotFound(server_name.to_string()))?;
        server.call_tool(tool_name, arguments).await
    }

    pub async fn ping(&self, server_name: &str) -> Result<(), McpClientError> {
        let servers = self.servers.read().await;
        let server = servers
            .get(server_name)
            .ok_or_else(|| McpClientError::ServerNotFound(server_name.to_string()))?;
        server.request("ping", None).await.map(|_| ())
    }

    async fn refresh_cached_tools(&self) -> Result<(), McpClientError> {
        let servers = self.servers.read().await;
        let mut tools = Vec::new();
        for server in servers.values() {
            tools.extend(server.list_tools().await?);
        }
        if let Ok(mut cached_tools) = self.cached_tools.write() {
            *cached_tools = tools;
        }
        Ok(())
    }
}

struct StdioServer {
    name: String,
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
    next_id: AtomicU64,
}

impl StdioServer {
    async fn initialize(&self) -> Result<(), McpClientError> {
        self.request(
            "initialize",
            Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "slab", "version": env!("CARGO_PKG_VERSION") }
            })),
        )
        .await?;
        self.notify("notifications/initialized", None).await
    }

    async fn list_tools(&self) -> Result<Vec<McpToolSpec>, McpClientError> {
        let result = self.request("tools/list", None).await?;
        let raw_tools = result.get("tools").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut tools = Vec::new();
        for raw in raw_tools {
            let name = raw.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            tools.push(McpToolSpec {
                server_name: self.name.clone(),
                name,
                description: raw.get("description").and_then(Value::as_str).map(str::to_string),
                input_schema: raw.get("inputSchema").cloned().unwrap_or_else(|| json!({})),
            });
        }
        Ok(tools)
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<McpToolResult, McpClientError> {
        let result = self
            .request(
                "tools/call",
                Some(json!({
                    "name": tool_name,
                    "arguments": arguments
                })),
            )
            .await?;
        parse_tool_result(result)
    }

    async fn request(&self, method: &str, params: Option<Value>) -> Result<Value, McpClientError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let payload = serde_json::to_string(&request(id, method, params))?;
        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(payload.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        }

        let mut line = String::new();
        let bytes = {
            let mut stdout = self.stdout.lock().await;
            stdout.read_line(&mut line).await?
        };
        if bytes == 0 {
            let status = self.child.lock().await.try_wait()?;
            return Err(McpClientError::Protocol(format!(
                "MCP server `{}` closed stdout with status {status:?}",
                self.name
            )));
        }
        let response: JsonRpcResponse = serde_json::from_str(&line)?;
        if let Some(error) = response.error {
            return Err(McpClientError::Protocol(error.message));
        }
        response
            .result
            .ok_or_else(|| McpClientError::Protocol("response missing result".to_string()))
    }

    async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), McpClientError> {
        let payload = serde_json::to_string(&notification(method, params))?;
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(payload.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }
}

fn parse_tool_result(value: Value) -> Result<McpToolResult, McpClientError> {
    let content = value
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|item| {
            let mut fields = item.as_object().cloned().unwrap_or_default();
            let content_type = fields
                .remove("type")
                .and_then(|value| value.as_str().map(str::to_string))
                .unwrap_or_else(|| "text".to_string());
            McpContent { content_type, fields }
        })
        .collect();
    let is_error = value.get("isError").and_then(Value::as_bool).unwrap_or(false);
    Ok(McpToolResult { content, is_error })
}
