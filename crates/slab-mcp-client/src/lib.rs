//! Single-connection MCP client transport.

mod protocol;
mod stdio;

pub use protocol::{McpContent, McpTool, McpToolResult};
pub use stdio::{McpClientError, StdioLaunchConfig, StdioMcpClient};
