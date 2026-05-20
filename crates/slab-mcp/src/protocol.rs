use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use slab_mcp_client::{McpContent, McpToolResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolSpec {
    pub server_name: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Value,
}
