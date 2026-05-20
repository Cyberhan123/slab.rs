//! MCP server management and tool aggregation helpers.

pub mod client;
pub mod config;
pub mod protocol;

pub use client::McpClient;
pub use config::{McpClientConfig, McpServerLauncher};
pub use protocol::{McpContent, McpToolResult, McpToolSpec};
