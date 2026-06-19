use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod host;

pub const VERSION: &str = "2.0";
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INTERNAL_ERROR: i64 = -32603;
pub const APPLICATION_ERROR: i64 = -32000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), data: None }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct IncomingMessage {
    #[serde(default)]
    pub jsonrpc: Option<String>,
    #[serde(default)]
    pub id: Option<Value>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub params: Value,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<RpcError>,
}

impl IncomingMessage {
    pub fn has_valid_version(&self) -> bool {
        self.jsonrpc.as_deref() == Some(VERSION)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpcRequest<'a> {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub method: &'a str,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpcNotification<'a> {
    pub jsonrpc: &'static str,
    pub method: &'a str,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpcOptionalRequest<'a> {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpcOptionalNotification<'a> {
    pub jsonrpc: &'static str,
    pub method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpcResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

pub fn request(id: Value, method: &str, params: Value) -> RpcRequest<'_> {
    RpcRequest { jsonrpc: VERSION, id, method, params }
}

pub fn request_with_optional_params(
    id: Value,
    method: &str,
    params: Option<Value>,
) -> RpcOptionalRequest<'_> {
    RpcOptionalRequest { jsonrpc: VERSION, id, method, params }
}

pub fn notification(method: &str, params: Value) -> RpcNotification<'_> {
    RpcNotification { jsonrpc: VERSION, method, params }
}

pub fn notification_with_optional_params(
    method: &str,
    params: Option<Value>,
) -> RpcOptionalNotification<'_> {
    RpcOptionalNotification { jsonrpc: VERSION, method, params }
}

pub fn success_response(id: Value, result: Value) -> RpcResponse {
    RpcResponse { jsonrpc: VERSION, id, result: Some(result), error: None }
}

pub fn error_response(id: Value, code: i64, message: impl Into<String>) -> RpcResponse {
    RpcResponse { jsonrpc: VERSION, id, result: None, error: Some(RpcError::new(code, message)) }
}

pub fn application_error_response(id: Value, message: impl Into<String>) -> RpcResponse {
    error_response(id, APPLICATION_ERROR, message)
}

pub fn parse_message(line: &str) -> Result<IncomingMessage, serde_json::Error> {
    serde_json::from_str(line)
}

pub fn serialize_response(response: &RpcResponse) -> String {
    serde_json::to_string(response).unwrap_or_else(|error| {
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{{\"code\":-32603,\"message\":\"failed to serialize response: {error}\"}}}}"
        )
    })
}

pub fn id_key(id: &Value) -> String {
    match id {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{
        APPLICATION_ERROR, IncomingMessage, RpcError, application_error_response, id_key,
        notification, notification_with_optional_params, parse_message, request,
        request_with_optional_params, serialize_response, success_response,
    };

    #[test]
    fn parses_single_request_envelope() {
        let message =
            parse_message(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#).expect("message");

        assert!(message.has_valid_version());
        assert_eq!(message.id, Some(Value::from(1)));
        assert_eq!(message.method.as_deref(), Some("ping"));
    }

    #[test]
    fn serializes_optional_request_without_params_when_absent() {
        let request = request_with_optional_params(Value::from("call-1"), "plugin.call", None);
        let value = serde_json::to_value(request).expect("request json");

        assert_eq!(value["jsonrpc"], "2.0");
        assert!(value.get("params").is_none());
    }

    #[test]
    fn serializes_requests_and_notifications_with_params_when_present() {
        let request = request(Value::from(3), "plugin.call", json!({"name": "search"}));
        let request_value = serde_json::to_value(request).expect("request json");
        assert_eq!(request_value["jsonrpc"], "2.0");
        assert_eq!(request_value["id"], 3);
        assert_eq!(request_value["method"], "plugin.call");
        assert_eq!(request_value["params"], json!({"name": "search"}));

        let notification = notification("plugin.ready", json!({"ok": true}));
        let notification_value = serde_json::to_value(notification).expect("notification json");
        assert_eq!(notification_value["jsonrpc"], "2.0");
        assert!(notification_value.get("id").is_none());
        assert_eq!(notification_value["params"], json!({"ok": true}));
    }

    #[test]
    fn serializes_optional_notifications_without_absent_params() {
        let notification = notification_with_optional_params("plugin.ready", None);
        let value = serde_json::to_value(notification).expect("notification json");

        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["method"], "plugin.ready");
        assert!(value.get("id").is_none());
        assert!(value.get("params").is_none());
    }

    #[test]
    fn serializes_success_and_error_responses() {
        let success = serialize_response(&success_response(Value::from(7), json!({"ok": true})));
        assert_eq!(success, r#"{"jsonrpc":"2.0","id":7,"result":{"ok":true}}"#);

        let error = application_error_response(Value::from("call-1"), "failed");
        assert_eq!(error.error.as_ref().expect("error").code, APPLICATION_ERROR);
    }

    #[test]
    fn omits_error_data_when_absent() {
        let error = RpcError::new(-32600, "bad request");
        let value = serde_json::to_value(error).expect("error json");
        assert!(value.get("data").is_none());
    }

    #[test]
    fn preserves_error_data_and_response_envelopes_when_parsing() {
        let error = RpcError {
            code: -32600,
            message: "bad request".to_string(),
            data: Some(json!({"path": "params.name"})),
        };
        let value = serde_json::to_value(error).expect("error json");
        assert_eq!(value["data"], json!({"path": "params.name"}));

        let message = parse_message(
            r#"{"jsonrpc":"2.0","id":"call-1","error":{"code":-32000,"message":"failed","data":{"retry":false}}}"#,
        )
        .expect("response message");

        assert!(message.has_valid_version());
        assert_eq!(message.id, Some(Value::from("call-1")));
        assert!(message.method.is_none());
        let error = message.error.expect("response error");
        assert_eq!(error.code, APPLICATION_ERROR);
        assert_eq!(error.data, Some(json!({"retry": false})));
    }

    #[test]
    fn normalizes_id_keys() {
        assert_eq!(id_key(&Value::from("abc")), "abc");
        assert_eq!(id_key(&Value::from(42)), "42");
        assert_eq!(id_key(&Value::Null), "null");
        assert_eq!(id_key(&Value::Bool(true)), "true");
    }

    #[test]
    fn invalid_version_is_detected_without_rejecting_parse() {
        let message: IncomingMessage =
            parse_message(r#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#).expect("message");

        assert!(!message.has_valid_version());
    }
}
