use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};

use serde_json::{Value, json};
use slab_jsonrpc::{
    notification_with_optional_params, parse_message, request_with_optional_params,
};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::Mutex,
};

use crate::protocol::{McpContent, McpTool, McpToolResult};

#[derive(Debug, Clone, Default)]
pub struct StdioLaunchConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

#[derive(Debug, Error)]
pub enum McpClientError {
    #[error("MCP server process has no stdin")]
    MissingStdin,
    #[error("MCP server process has no stdout")]
    MissingStdout,
    #[error("MCP protocol error: {0}")]
    Protocol(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct StdioMcpClient {
    _child: Mutex<Child>,
    connection: JsonRpcConnection<tokio::process::ChildStdout, tokio::process::ChildStdin>,
}

impl StdioMcpClient {
    pub async fn connect(config: StdioLaunchConfig) -> Result<Self, McpClientError> {
        let mut command = Command::new(&config.command);
        command.args(&config.args);
        for (key, value) in &config.env {
            command.env(key, value);
        }
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());

        let mut child = command.spawn()?;
        let stdin = child.stdin.take().ok_or(McpClientError::MissingStdin)?;
        let stdout = child.stdout.take().ok_or(McpClientError::MissingStdout)?;
        let client =
            Self { _child: Mutex::new(child), connection: JsonRpcConnection::new(stdout, stdin) };
        client.initialize().await?;
        Ok(client)
    }

    pub async fn initialize(&self) -> Result<(), McpClientError> {
        self.connection.initialize().await
    }

    pub async fn ping(&self) -> Result<(), McpClientError> {
        self.connection.ping().await
    }

    pub async fn list_tools(&self) -> Result<Vec<McpTool>, McpClientError> {
        self.connection.list_tools().await
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<McpToolResult, McpClientError> {
        self.connection.call_tool(tool_name, arguments).await
    }

    pub async fn request(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, McpClientError> {
        self.connection.request(method, params).await
    }

    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), McpClientError> {
        self.connection.notify(method, params).await
    }
}

struct JsonRpcConnection<R, W> {
    reader: Mutex<BufReader<R>>,
    writer: Mutex<W>,
    next_id: AtomicU64,
}

impl<R, W> JsonRpcConnection<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    fn new(reader: R, writer: W) -> Self {
        Self {
            reader: Mutex::new(BufReader::new(reader)),
            writer: Mutex::new(writer),
            next_id: AtomicU64::new(1),
        }
    }

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

    async fn ping(&self) -> Result<(), McpClientError> {
        self.request("ping", None).await.map(|_| ())
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>, McpClientError> {
        let result = self.request("tools/list", None).await?;
        let raw_tools = result.get("tools").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut tools = Vec::new();
        for raw in raw_tools {
            let name = raw.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            tools.push(McpTool {
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
        let payload =
            serde_json::to_string(&request_with_optional_params(Value::from(id), method, params))?;
        {
            let mut writer = self.writer.lock().await;
            writer.write_all(payload.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }

        let mut line = String::new();
        let bytes = {
            let mut reader = self.reader.lock().await;
            reader.read_line(&mut line).await?
        };
        if bytes == 0 {
            return Err(McpClientError::Protocol(
                "MCP connection closed before response".to_string(),
            ));
        }
        let response = parse_message(&line)?;
        if let Some(error) = response.error {
            return Err(McpClientError::Protocol(error.message));
        }
        response
            .result
            .ok_or_else(|| McpClientError::Protocol("response missing result".to_string()))
    }

    async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), McpClientError> {
        let payload = serde_json::to_string(&notification_with_optional_params(method, params))?;
        let mut writer = self.writer.lock().await;
        writer.write_all(payload.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    fn connection_pair() -> (
        JsonRpcConnection<tokio::io::DuplexStream, tokio::io::DuplexStream>,
        tokio::io::DuplexStream,
        tokio::io::DuplexStream,
    ) {
        let (client_read, server_write) = duplex(4096);
        let (server_read, client_write) = duplex(4096);
        (JsonRpcConnection::new(client_read, client_write), server_read, server_write)
    }

    #[tokio::test]
    async fn initialize_sends_request_and_initialized_notification() {
        let (connection, server_read, mut server_write) = connection_pair();
        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(server_read);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("initialize request");
            let request: Value = serde_json::from_str(&line).expect("initialize json");
            assert_eq!(request["method"], "initialize");
            assert_eq!(request["params"]["clientInfo"]["name"], "slab");
            server_write
                .write_all(
                    json!({
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "result": {}
                    })
                    .to_string()
                    .as_bytes(),
                )
                .await
                .expect("write initialize response");
            server_write.write_all(b"\n").await.expect("write newline");

            line.clear();
            reader.read_line(&mut line).await.expect("initialized notification");
            let notification: Value = serde_json::from_str(&line).expect("notification json");
            assert_eq!(notification["method"], "notifications/initialized");
            assert!(notification.get("id").is_none());
        });

        connection.initialize().await.expect("initialize succeeds");
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn list_tools_parses_remote_tools() {
        let (connection, server_read, mut server_write) = connection_pair();
        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(server_read);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("tools/list request");
            let request: Value = serde_json::from_str(&line).expect("request json");
            assert_eq!(request["method"], "tools/list");
            let response = json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "result": {
                    "tools": [{
                        "name": "echo",
                        "description": "Echo input",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "message": { "type": "string" }
                            }
                        }
                    }, {
                        "description": "missing name"
                    }]
                }
            });
            server_write.write_all(response.to_string().as_bytes()).await.expect("write response");
            server_write.write_all(b"\n").await.expect("write newline");
        });

        let tools = connection.list_tools().await.expect("tools");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
        assert_eq!(tools[0].input_schema["properties"]["message"]["type"], "string");
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn call_tool_parses_tool_result() {
        let (connection, server_read, mut server_write) = connection_pair();
        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(server_read);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("tools/call request");
            let request: Value = serde_json::from_str(&line).expect("request json");
            assert_eq!(request["method"], "tools/call");
            assert_eq!(request["params"]["name"], "echo");
            assert_eq!(request["params"]["arguments"]["message"], "hello");
            let response = json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "result": {
                    "content": [{
                        "type": "text",
                        "text": "hello"
                    }],
                    "isError": false
                }
            });
            server_write.write_all(response.to_string().as_bytes()).await.expect("write response");
            server_write.write_all(b"\n").await.expect("write newline");
        });

        let result =
            connection.call_tool("echo", json!({ "message": "hello" })).await.expect("tool result");
        assert!(!result.is_error);
        assert_eq!(result.content[0].content_type, "text");
        assert_eq!(result.content[0].fields["text"], "hello");
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn json_rpc_error_maps_to_protocol_error() {
        let (connection, server_read, mut server_write) = connection_pair();
        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(server_read);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("request");
            let request: Value = serde_json::from_str(&line).expect("request json");
            let response = json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "error": {
                    "code": -32601,
                    "message": "method not found"
                }
            });
            server_write.write_all(response.to_string().as_bytes()).await.expect("write response");
            server_write.write_all(b"\n").await.expect("write newline");
        });

        let error = connection.ping().await.expect_err("protocol error");
        assert_eq!(error.to_string(), "MCP protocol error: method not found");
        server.await.expect("server task");
    }
}
