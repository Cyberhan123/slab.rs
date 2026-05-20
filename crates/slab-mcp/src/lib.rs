//! MCP protocol helpers.

pub mod client;
pub mod config;
pub mod protocol;
pub mod server;

pub use client::McpClient;
pub use config::{McpClientConfig, McpServerLauncher};
pub use protocol::{McpContent, McpToolResult, McpToolSpec};
pub use server::{McpServerConfig, McpServerToolRouter};
