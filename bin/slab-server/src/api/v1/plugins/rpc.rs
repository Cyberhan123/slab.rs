use serde_json::Value;
use slab_app_core::domain::services::PluginService;
use slab_jsonrpc::{
    APPLICATION_ERROR, INVALID_REQUEST, JSONRPC_VERSION, JSONRPCError, JSONRPCErrorError,
    JSONRPCMessage, JSONRPCRequest, JSONRPCResponse, METHOD_NOT_FOUND, PARSE_ERROR, RequestId,
};

pub(super) async fn handle_payload(service: &PluginService, payload: &str) -> String {
    let request = match parse_request_payload(payload) {
        Ok(request) => request,
        Err(response) => return response,
    };

    let Some((plugin_id, function_name)) = parse_method(&request.method) else {
        return serialize_error_response(
            request.id,
            METHOD_NOT_FOUND,
            "method must use `plugin_id.function_name`",
        );
    };

    match service
        .dispatch_rpc(plugin_id, function_name, request.params.unwrap_or(Value::Null))
        .await
    {
        Ok(result) => serialize_success_response(request.id, result),
        Err(error) => serialize_error_response(request.id, APPLICATION_ERROR, error.to_string()),
    }
}

fn parse_request_payload(payload: &str) -> Result<JSONRPCRequest, String> {
    let mut value = serde_json::from_str::<Value>(payload).map_err(|error| {
        serialize_error_response(
            fallback_error_id(),
            PARSE_ERROR,
            format!("invalid json-rpc payload: {error}"),
        )
    })?;

    if value.get("jsonrpc").and_then(Value::as_str) != Some(JSONRPC_VERSION) {
        let id = parse_wire_id(&value).unwrap_or_else(fallback_error_id);
        return Err(serialize_error_response(id, INVALID_REQUEST, "jsonrpc must be `2.0`"));
    }

    if let Value::Object(object) = &mut value {
        object.remove("jsonrpc");
    }

    match serde_json::from_value::<JSONRPCMessage>(value) {
        Ok(JSONRPCMessage::Request(request)) => Ok(request),
        Ok(JSONRPCMessage::Notification(_)) => Err(serialize_error_response(
            fallback_error_id(),
            INVALID_REQUEST,
            "request missing id",
        )),
        Ok(JSONRPCMessage::Response(_) | JSONRPCMessage::Error(_)) => {
            Err(serialize_error_response(
                fallback_error_id(),
                INVALID_REQUEST,
                "request missing method",
            ))
        }
        Err(error) => Err(serialize_error_response(
            fallback_error_id(),
            INVALID_REQUEST,
            format!("invalid json-rpc request: {error}"),
        )),
    }
}

fn serialize_success_response(id: RequestId, result: Value) -> String {
    serialize_wire_message(&JSONRPCMessage::Response(JSONRPCResponse { id, result }))
}

fn serialize_error_response(id: RequestId, code: i64, message: impl Into<String>) -> String {
    serialize_wire_message(&JSONRPCMessage::Error(JSONRPCError {
        error: JSONRPCErrorError { code, data: None, message: message.into() },
        id,
    }))
}

fn serialize_wire_message(message: &JSONRPCMessage) -> String {
    let mut value = serde_json::to_value(message).unwrap_or_else(|error| {
        serde_json::json!({
            "id": "serialize-error",
            "error": {
                "code": APPLICATION_ERROR,
                "message": format!("failed to serialize json-rpc response: {error}"),
            },
        })
    });
    if let Value::Object(object) = &mut value {
        object.insert("jsonrpc".to_owned(), Value::String(JSONRPC_VERSION.to_owned()));
    }
    serde_json::to_string(&value).unwrap_or_else(|error| {
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":\"serialize-error\",\"error\":{{\"code\":-32000,\"message\":\"failed to serialize json-rpc response: {error}\"}}}}"
        )
    })
}

fn parse_wire_id(value: &Value) -> Option<RequestId> {
    value.get("id").cloned().and_then(|id| serde_json::from_value(id).ok())
}

fn fallback_error_id() -> RequestId {
    RequestId::String("request-error".to_owned())
}

fn parse_method(method: &str) -> Option<(&str, &str)> {
    let (plugin_id, function_name) = method.split_once('.')?;
    if plugin_id.trim().is_empty() || function_name.trim().is_empty() {
        return None;
    }
    Some((plugin_id, function_name))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use slab_jsonrpc::{INVALID_REQUEST, JSONRPC_VERSION, RequestId};

    use super::{
        fallback_error_id, parse_method, parse_request_payload, serialize_error_response,
        serialize_success_response,
    };

    #[test]
    fn parses_plugin_rpc_method_shape() {
        assert_eq!(parse_method("plugin-a.run"), Some(("plugin-a", "run")));
        assert_eq!(parse_method("plugin-a."), None);
        assert_eq!(parse_method(".run"), None);
        assert_eq!(parse_method("plugin-a"), None);
    }

    #[test]
    fn parses_typed_request_payload() {
        let request = parse_request_payload(
            r#"{"jsonrpc":"2.0","id":"call-1","method":"plugin.run","params":{"ok":true}}"#,
        )
        .expect("request");

        assert_eq!(request.id, RequestId::String("call-1".to_owned()));
        assert_eq!(request.method, "plugin.run");
        assert_eq!(request.params, Some(json!({"ok": true})));
    }

    #[test]
    fn rejects_invalid_version_with_request_id() {
        let response =
            parse_request_payload(r#"{"jsonrpc":"1.0","id":"call-1","method":"plugin.run"}"#)
                .expect_err("invalid version");
        let value = serde_json::from_str::<serde_json::Value>(&response).expect("response json");

        assert_eq!(value["jsonrpc"], JSONRPC_VERSION);
        assert_eq!(value["id"], "call-1");
        assert_eq!(value["error"]["code"], INVALID_REQUEST);
    }

    #[test]
    fn serializes_typed_success_and_error_responses() {
        let success = serialize_success_response(RequestId::Integer(7), json!({"ok": true}));
        let value = serde_json::from_str::<serde_json::Value>(&success).expect("success json");
        assert_eq!(value["jsonrpc"], JSONRPC_VERSION);
        assert_eq!(value["id"], 7);
        assert_eq!(value["result"], json!({"ok": true}));

        let error = serialize_error_response(fallback_error_id(), INVALID_REQUEST, "bad request");
        let value = serde_json::from_str::<serde_json::Value>(&error).expect("error json");
        assert_eq!(value["jsonrpc"], JSONRPC_VERSION);
        assert_eq!(value["id"], "request-error");
        assert_eq!(value["error"]["message"], "bad request");
    }
}
