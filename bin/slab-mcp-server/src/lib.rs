//! JSON-RPC handlers for the standalone Slab MCP server process.

use serde_json::{Value, json};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "slab-mcp-server";
const SERVER_INFO_TOOL: &str = "slab_server_info";
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
        "tools/list" => tools_list_result(),
        "tools/call" => return handle_tool_call(id, &message),
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

fn handle_tool_call(id: Value, message: &Value) -> Option<Value> {
    let name = message
        .get("params")
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("<unknown>");

    let result = match name {
        SERVER_INFO_TOOL => server_info_tool_result(),
        _ => return Some(error_response(id, INVALID_PARAMS, format!("tool not found: {name}"))),
    };

    Some(success_response(id, result))
}

fn tools_list_result() -> Value {
    json!({
        "tools": [{
            "name": SERVER_INFO_TOOL,
            "title": "Slab MCP Server Info",
            "description": "Return read-only metadata about this Slab MCP server process.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            },
            "outputSchema": {
                "type": "object",
                "properties": {
                    "server_name": { "type": "string" },
                    "version": { "type": "string" },
                    "protocol_version": { "type": "string" },
                    "tools": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": ["server_name", "version", "protocol_version", "tools"]
            },
            "annotations": {
                "title": "Slab MCP Server Info",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            },
            "_meta": slab_tool_meta(SERVER_INFO_TOOL, "slab:mcp:server_info:read")
        }]
    })
}

fn server_info_tool_result() -> Value {
    let structured = json!({
        "server_name": SERVER_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "protocol_version": PROTOCOL_VERSION,
        "tools": [SERVER_INFO_TOOL]
    });
    json!({
        "content": [{
            "type": "text",
            "text": structured.to_string()
        }],
        "structuredContent": structured,
        "isError": false,
        "_meta": slab_tool_meta(SERVER_INFO_TOOL, "slab:mcp:server_info:read")
    })
}

fn slab_tool_meta(tool: &str, permission: &str) -> Value {
    json!({
        "slab": {
            "source": SERVER_NAME,
            "permission": permission,
            "audit": {
                "event": "slab.mcp.tool_call",
                "tool": tool
            }
        }
    })
}

fn success_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
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
    fn tools_list_returns_server_info_tool_with_safety_metadata() {
        let response = handle_message(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        }))
        .expect("response");

        let tool = &response["result"]["tools"][0];
        assert_eq!(tool["name"], SERVER_INFO_TOOL);
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["annotations"]["destructiveHint"], false);
        assert_eq!(tool["_meta"]["slab"]["source"], SERVER_NAME);
        assert_eq!(tool["_meta"]["slab"]["permission"], "slab:mcp:server_info:read");
        assert_eq!(tool["_meta"]["slab"]["audit"]["tool"], SERVER_INFO_TOOL);
    }

    #[test]
    fn tools_call_returns_server_info_with_audit_metadata() {
        let response = handle_message(json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": SERVER_INFO_TOOL,
                "arguments": {}
            }
        }))
        .expect("response");

        assert_eq!(response["result"]["isError"], false);
        assert_eq!(response["result"]["structuredContent"]["server_name"], SERVER_NAME);
        assert_eq!(response["result"]["structuredContent"]["tools"], json!([SERVER_INFO_TOOL]));
        assert_eq!(response["result"]["_meta"]["slab"]["source"], SERVER_NAME);
        assert_eq!(response["result"]["_meta"]["slab"]["audit"]["tool"], SERVER_INFO_TOOL);
    }

    #[test]
    fn tools_call_returns_tool_not_found() {
        let response = handle_message(json!({
            "jsonrpc": "2.0",
            "id": 4,
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
            "id": 5,
            "method": "unknown"
        }))
        .expect("response");

        assert_eq!(response["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(response["error"]["message"], "method not found: unknown");
    }
}
