//! Server → client notification param shapes and the `ServerNotification` union.
//!
//! These param structs are the authoritative payloads: they are serialized as
//! the `params` field of a `JSONRPCNotification { method, params }`, and are
//! also wrapped by [`crate::harness::EventMsg`] variants for in-process use.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::harness::item::TurnItem;
use crate::harness::messages::Thread;

// ---- lifecycle ----

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartedParams {
    pub thread: Thread,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartedParams {
    pub thread_id: String,
    pub turn: crate::harness::messages::Turn,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnCompletedParams {
    pub thread_id: String,
    pub turn: crate::harness::messages::Turn,
}

// ---- items ----

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemStartedParams {
    pub item: TurnItem,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemCompletedParams {
    pub item: TurnItem,
    pub thread_id: String,
    pub turn_id: String,
}

// ---- deltas ----

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningTextDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub content_index: u32,
    pub delta: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningSummaryTextDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_index: Option<u32>,
    pub delta: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionOutputDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeOutputDeltaParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

// ---- approvals ----

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub command: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Operation category so the UI can render category-appropriate choices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<crate::harness::messages::OperationCategory>,
    /// Persistence scopes the client may offer (run-once / workspace / always
    /// / deny). Empty for servers that only support approve/reject.
    #[serde(default)]
    pub allowed_scopes: Vec<crate::harness::messages::ApprovalScope>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub changes: Vec<FileChangeApprovalChange>,
    #[serde(default)]
    pub allowed_scopes: Vec<crate::harness::messages::ApprovalScope>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeApprovalChange {
    pub path: String,
    /// Change kind, e.g. `add` / `edit` / `delete`.
    #[serde(rename = "type")]
    pub change_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
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
