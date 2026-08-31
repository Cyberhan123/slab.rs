use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub mod host;
pub mod notifier;
pub mod router;
pub mod ws;

pub const JSONRPC_VERSION: &str = "2.0";
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;
pub const APPLICATION_ERROR: i64 = -32000;

#[derive(
    TS, Debug, Clone, PartialEq, PartialOrd, Ord, Deserialize, Serialize, Hash, Eq, JsonSchema,
)]
#[serde(untagged)]
#[ts(export)]
pub enum RequestId {
    String(String),
    Integer(i64),
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(value) => f.write_str(value),
            Self::Integer(value) => write!(f, "{value}"),
        }
    }
}

pub type Result = serde_json::Value;

/// Refers to any valid JSON-RPC object that can be decoded off the wire, or encoded to be sent.
#[derive(TS, Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
#[ts(export, rename = "JsonRpcMessage")]
pub enum JSONRPCMessage {
    Request(JSONRPCRequest),
    Notification(JSONRPCNotification),
    Response(JSONRPCResponse),
    Error(JSONRPCError),
}

/// A request that expects a response.
#[derive(TS, Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, rename = "JsonRpcRequest")]
pub struct JSONRPCRequest {
    pub id: RequestId,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    /// Optional W3C Trace Context for distributed tracing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<W3cTraceContext>,
}

/// A notification which does not expect a response.
#[derive(TS, Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, rename = "JsonRpcNotification")]
pub struct JSONRPCNotification {
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// A successful (non-error) response to a request.
#[derive(TS, Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, rename = "JsonRpcResponse")]
pub struct JSONRPCResponse {
    pub id: RequestId,
    pub result: Result,
}

/// A response to a request that indicates an error occurred.
#[derive(TS, Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, rename = "JsonRpcErrorResponse")]
pub struct JSONRPCError {
    pub error: JSONRPCErrorError,
    pub id: RequestId,
}

#[derive(TS, Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, rename = "JsonRpcErrorBody")]
pub struct JSONRPCErrorError {
    pub code: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    pub message: String,
}

#[derive(TS, Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct W3cTraceContext {
    pub traceparent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracestate: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{
        JSONRPCError, JSONRPCErrorError, JSONRPCMessage, JSONRPCNotification, JSONRPCRequest,
        JSONRPCResponse, RequestId, W3cTraceContext,
    };

    #[test]
    fn request_id_serializes_and_displays_string_and_integer_ids() {
        let string_id = RequestId::String("call-1".to_owned());
        let integer_id = RequestId::Integer(42);

        assert_eq!(string_id.to_string(), "call-1");
        assert_eq!(integer_id.to_string(), "42");
        assert_eq!(serde_json::to_value(&string_id).expect("string id json"), "call-1");
        assert_eq!(serde_json::to_value(&integer_id).expect("integer id json"), 42);
        assert_eq!(
            serde_json::from_value::<RequestId>(json!("call-1")).expect("string request id"),
            string_id
        );
        assert_eq!(
            serde_json::from_value::<RequestId>(json!(42)).expect("integer request id"),
            integer_id
        );
        assert!(RequestId::String("a".to_owned()) < RequestId::String("b".to_owned()));
    }

    #[test]
    fn request_id_rejects_unsupported_json_id_types() {
        for value in
            [Value::Null, Value::Bool(true), json!({"id": "call-1"}), json!(["call-1"]), json!(1.5)]
        {
            assert!(serde_json::from_value::<RequestId>(value).is_err());
        }
    }

    #[test]
    fn parses_jsonrpc_message_variants() {
        let request: JSONRPCMessage =
            serde_json::from_str(r#"{"id":1,"method":"ping","params":{"ok":true}}"#)
                .expect("request");
        assert!(matches!(
            request,
            JSONRPCMessage::Request(JSONRPCRequest { id: RequestId::Integer(1), .. })
        ));

        let notification: JSONRPCMessage =
            serde_json::from_str(r#"{"method":"ready"}"#).expect("notification");
        assert!(matches!(notification, JSONRPCMessage::Notification(JSONRPCNotification { .. })));

        let response: JSONRPCMessage =
            serde_json::from_str(r#"{"id":"call-1","result":{"ok":true}}"#).expect("response");
        assert!(matches!(
            response,
            JSONRPCMessage::Response(JSONRPCResponse { id: RequestId::String(_), .. })
        ));

        let error: JSONRPCMessage =
            serde_json::from_str(r#"{"id":"call-1","error":{"code":-32000,"message":"failed"}}"#)
                .expect("error");
        assert!(matches!(error, JSONRPCMessage::Error(JSONRPCError { .. })));
    }

    #[test]
    fn rejects_messages_with_invalid_request_ids() {
        for payload in [
            r#"{"id":null,"method":"ping"}"#,
            r#"{"id":true,"method":"ping"}"#,
            r#"{"id":{"bad":true},"method":"ping"}"#,
            r#"{"id":["bad"],"method":"ping"}"#,
        ] {
            assert!(serde_json::from_str::<JSONRPCMessage>(payload).is_err());
        }
    }

    #[test]
    fn serializes_request_without_absent_optional_fields() {
        let request = JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::String("call-1".to_owned()),
            method: "plugin.call".to_owned(),
            params: None,
            trace: None,
        });

        let value = serde_json::to_value(request).expect("request json");

        assert_eq!(value["id"], "call-1");
        assert_eq!(value["method"], "plugin.call");
        assert!(value.get("params").is_none());
        assert!(value.get("trace").is_none());
    }

    #[test]
    fn serializes_request_with_w3c_trace_context() {
        let request = JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::Integer(7),
            method: "plugin.call".to_owned(),
            params: Some(json!({"name": "search"})),
            trace: Some(W3cTraceContext {
                traceparent: "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_owned(),
                tracestate: Some("vendor=value".to_owned()),
            }),
        });

        let value = serde_json::to_value(request).expect("request json");

        assert_eq!(value["params"], json!({"name": "search"}));
        assert_eq!(
            value["trace"]["traceparent"],
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        );
        assert_eq!(value["trace"]["tracestate"], "vendor=value");
    }

    #[test]
    fn omits_absent_tracestate_and_error_data() {
        let trace = W3cTraceContext {
            traceparent: "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_owned(),
            tracestate: None,
        };
        let trace_value = serde_json::to_value(trace).expect("trace json");
        assert!(trace_value.get("tracestate").is_none());

        let error =
            JSONRPCErrorError { code: -32600, data: None, message: "bad request".to_owned() };
        let error_value = serde_json::to_value(error).expect("error json");
        assert!(error_value.get("data").is_none());
    }
}
