//! JSON-RPC handlers for the standalone Slab MCP server process.

use serde_json::{Value, json};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "slab-mcp-server";
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

pub fn handle_message(message: Value) -> Option<Value> {
    let id = message.get("id").cloned();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Some(error_response(id.unwrap_or(Value::Null), INVALID_REQUEST, "invalid request"));
    };

    let id = id?;
    let result = match method {
        "initialize" => json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": env!("CARGO_PKG_VERSION"),
            }
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": [] }),
        "tools/call" => {
            let name = message
                .get("params")
                .and_then(|params| params.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("<unknown>");
            return Some(error_response(id, INVALID_PARAMS, format!("tool not found: {name}")));
        }
        _ => {
            return Some(error_response(
                id,
                METHOD_NOT_FOUND,
                format!("method not found: {method}"),
            ));
        }
    };

    Some(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
}

pub fn parse_error_response(message: impl Into<String>) -> Value {
    error_response(Value::Null, PARSE_ERROR, message)
}

fn error_response(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_returns_server_capabilities() {
        let response = handle_message(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize"
        }))
        .expect("response");

        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(response["result"]["serverInfo"]["name"], SERVER_NAME);
        assert_eq!(response["result"]["capabilities"]["tools"], json!({}));
    }

    #[test]
    fn notification_returns_no_response() {
        let response = handle_message(json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }));

        assert!(response.is_none());
    }

    #[test]
    fn tools_list_returns_empty_tools() {
        let response = handle_message(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        }))
        .expect("response");

        assert_eq!(response["result"]["tools"], json!([]));
    }

    #[test]
    fn tools_call_returns_tool_not_found() {
        let response = handle_message(json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "echo"
            }
        }))
        .expect("response");

        assert_eq!(response["error"]["code"], INVALID_PARAMS);
        assert_eq!(response["error"]["message"], "tool not found: echo");
    }

    #[test]
    fn unknown_method_returns_method_not_found() {
        let response = handle_message(json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "unknown"
        }))
        .expect("response");

        assert_eq!(response["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(response["error"]["message"], "method not found: unknown");
    }
}
