//! Server → client notification param shapes and the `ServerNotification` union.
//!
//! The notification param structs (the authoritative payloads, also wrapped by
//! [`crate::harness::EventMsg`]) now live in `slab_agent::protocol::notification`;
//! this module re-exports them and keeps the wire-envelope union
//! [`ServerNotification`], which is transport-specific. On-the-wire bytes are
//! unchanged.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Notification param payloads are owned by `slab_agent::protocol`; imported
// here (not re-exported) so the `ServerNotification` union variants below can
// reference them. Consumers use `slab_agent::protocol` directly.
use slab_agent::protocol::{
    AgentMessageDeltaParams, CommandExecutionOutputDeltaParams,
    CommandExecutionRequestApprovalParams, FileChangeOutputDeltaParams,
    FileChangeRequestApprovalParams, ItemCompletedParams, ItemStartedParams,
    ReasoningSummaryTextDeltaParams, ReasoningTextDeltaParams, ThreadStartedParams,
    TurnCompletedParams, TurnStartedParams,
};

// ---- error / account ----

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountUpdatedParams {
    pub account: Account,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountLoginCompletedParams {
    pub login_id: String,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub email: String,
    #[serde(default)]
    pub auth_methods: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription: Option<String>,
}

/// Union of every server → client notification, discriminated by `method`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "method", content = "params")]
pub enum ServerNotification {
    #[serde(rename = "thread/started")]
    ThreadStarted(ThreadStartedParams),
    #[serde(rename = "turn/started")]
    TurnStarted(TurnStartedParams),
    #[serde(rename = "turn/completed")]
    TurnCompleted(TurnCompletedParams),
    #[serde(rename = "item/started")]
    ItemStarted(ItemStartedParams),
    #[serde(rename = "item/completed")]
    ItemCompleted(ItemCompletedParams),
    #[serde(rename = "item/agentMessage/delta")]
    AgentMessageDelta(AgentMessageDeltaParams),
    #[serde(rename = "item/reasoning/textDelta")]
    ReasoningTextDelta(ReasoningTextDeltaParams),
    #[serde(rename = "item/reasoning/summaryTextDelta")]
    ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaParams),
    #[serde(rename = "item/commandExecution/outputDelta")]
    CommandExecutionOutputDelta(CommandExecutionOutputDeltaParams),
    #[serde(rename = "item/fileChange/outputDelta")]
    FileChangeOutputDelta(FileChangeOutputDeltaParams),
    #[serde(rename = "item/commandExecution/requestApproval")]
    CommandExecutionRequestApproval(CommandExecutionRequestApprovalParams),
    #[serde(rename = "item/fileChange/requestApproval")]
    FileChangeRequestApproval(FileChangeRequestApprovalParams),
    #[serde(rename = "error")]
    Error(ErrorParams),
    #[serde(rename = "account/updated")]
    AccountUpdated(AccountUpdatedParams),
    #[serde(rename = "account/loginCompleted")]
    AccountLoginCompleted(AccountLoginCompletedParams),
}

impl ServerNotification {
    /// The JSON-RPC method string for this notification.
    pub fn method(&self) -> &'static str {
        match self {
            Self::ThreadStarted(_) => crate::harness::method::THREAD_STARTED,
            Self::TurnStarted(_) => crate::harness::method::TURN_STARTED,
            Self::TurnCompleted(_) => crate::harness::method::TURN_COMPLETED,
            Self::ItemStarted(_) => crate::harness::method::ITEM_STARTED,
            Self::ItemCompleted(_) => crate::harness::method::ITEM_COMPLETED,
            Self::AgentMessageDelta(_) => crate::harness::method::ITEM_AGENT_MESSAGE_DELTA,
            Self::ReasoningTextDelta(_) => crate::harness::method::ITEM_REASONING_TEXT_DELTA,
            Self::ReasoningSummaryTextDelta(_) => {
                crate::harness::method::ITEM_REASONING_SUMMARY_TEXT_DELTA
            }
            Self::CommandExecutionOutputDelta(_) => {
                crate::harness::method::ITEM_COMMAND_EXECUTION_OUTPUT_DELTA
            }
            Self::FileChangeOutputDelta(_) => crate::harness::method::ITEM_FILE_CHANGE_OUTPUT_DELTA,
            Self::CommandExecutionRequestApproval(_) => {
                crate::harness::method::ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL
            }
            Self::FileChangeRequestApproval(_) => {
                crate::harness::method::ITEM_FILE_CHANGE_REQUEST_APPROVAL
            }
            Self::Error(_) => crate::harness::method::ERROR,
            Self::AccountUpdated(_) => crate::harness::method::ACCOUNT_UPDATED,
            Self::AccountLoginCompleted(_) => crate::harness::method::ACCOUNT_LOGIN_COMPLETED,
        }
    }

    /// The `params` payload of this notification, ready to drop into a
    /// `JSONRPCNotification { method, params }`. Serializes the whole
    /// `#[serde(tag = "method", content = "params")]` enum and extracts the
    /// `params` sub-value, so callers don't need slab-jsonrpc as a dependency.
    pub fn payload(&self) -> serde_json::Value {
        let value = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        value.get("params").cloned().unwrap_or(serde_json::Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_message_delta_notification_round_trips() {
        let n = ServerNotification::AgentMessageDelta(AgentMessageDeltaParams {
            thread_id: "t1".to_owned(),
            turn_id: "tu1".to_owned(),
            item_id: "i1".to_owned(),
            delta: "hel".to_owned(),
        });
        let json = serde_json::to_value(&n).unwrap();
        assert_eq!(json["method"], "item/agentMessage/delta");
        assert_eq!(json["params"]["itemId"], "i1");
        assert_eq!(json["params"]["delta"], "hel");
        let back: ServerNotification = serde_json::from_value(json).unwrap();
        assert_eq!(n, back);
    }

    #[test]
    fn error_notification_round_trips() {
        let n = ServerNotification::Error(ErrorParams {
            thread_id: Some("t1".to_owned()),
            turn_id: None,
            item_id: None,
            code: "turn_failed".to_owned(),
            message: "boom".to_owned(),
            data: None,
        });
        let json = serde_json::to_value(&n).unwrap();
        assert_eq!(json["method"], "error");
        assert_eq!(json["params"]["code"], "turn_failed");
    }
}
