use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolSpec {
    pub server_name: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    #[serde(default)]
    pub content: Vec<McpContent>,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

pub fn request(id: u64, method: impl Into<String>, params: Option<Value>) -> JsonRpcRequest {
    JsonRpcRequest { jsonrpc: "2.0".to_string(), id, method: method.into(), params }
}

pub fn notification(method: impl Into<String>, params: Option<Value>) -> JsonRpcNotification {
    JsonRpcNotification { jsonrpc: "2.0".to_string(), method: method.into(), params }
}
