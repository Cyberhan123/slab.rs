use serde::{Deserialize, Serialize};

pub use slab_mcp_client::{McpContent, McpTool, McpToolResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolSpec {
    pub server_name: String,
    pub tool: McpTool,
}
