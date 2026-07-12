//! Error / warning payloads for the harness protocol.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A terminal error while executing a submission. Carried by
/// [`super::EventMsg::Error`].
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ErrorEvent {
    /// Stable machine-readable code (e.g. `"turn_failed"`, `"stream_lagged"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Human-readable error message.
    pub message: String,
    /// Optional opaque structured detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl ErrorEvent {
    pub fn new(message: impl Into<String>) -> Self {
        Self { code: None, message: message.into(), data: None }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

/// A non-terminal warning: the turn continued but the user should be notified.
/// Carried by [`super::EventMsg::Warning`].
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct WarningEvent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_event_round_trips_and_omits_optionals() {
        let event = ErrorEvent::new("boom").with_code("turn_failed");
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["code"], "turn_failed");
        assert_eq!(json["message"], "boom");
        assert!(json.get("data").is_none());
        let back: ErrorEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event, back);
    }
}
