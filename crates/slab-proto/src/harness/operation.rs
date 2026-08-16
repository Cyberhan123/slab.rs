//! `UserSubmissionOp` — the submission operations that drive an event stream.
//!
//! Each variant names a submission kind; the events it produces follow the
//! lifecycle `Op::X → Event::XStarted → Event::XDelta* → Event::XCompleted | Error`.
//! The dispatcher builds these from the JSON-RPC request params.

use std::collections::{BTreeMap, HashMap};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::harness::messages::{ApprovalPolicy, ReasoningEffort, SandboxMode};
use crate::harness::user_input::UserInput;

/// A user submission against a thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
// `UserInput` carries a Vec + BTreeMap and dwarfs the `Interrupt` unit variant.
// These are short-lived DTOs built and immediately serialized, so the size
// disparity is acceptable.
#[allow(clippy::large_enum_variant)]
pub enum UserSubmissionOp {
    /// Abort the current task without terminating background terminal
    /// processes. Produces `EventMsg::TurnAborted`.
    Interrupt,

    /// New user input that starts (or continues) a turn.
    UserInput {
        /// User input items, see [`UserInput`].
        items: Vec<UserInput>,
        /// Optional JSON Schema constraining the final assistant message.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        final_output_json_schema: Option<Value>,
        /// Optional turn-scoped Responses-API client metadata.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        responsesapi_client_metadata: Option<HashMap<String, String>>,
        /// Client-supplied context fragments keyed by an opaque source id.
        #[serde(default)]
        additional_context: BTreeMap<String, AdditionalContextEntry>,
        /// Persistent thread-settings overrides applied before the input.
        #[serde(default)]
        thread_settings: ThreadSettingsOverrides,
    },
}

impl UserSubmissionOp {
    /// Stable discriminator used to label the event stream for this submission.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Interrupt => "interrupt",
            Self::UserInput { .. } => "user_input",
        }
    }
}

/// A client-supplied context fragment attached to a submission.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalContextEntry {
    /// The context payload (raw text or structured JSON).
    pub content: Value,
    /// Optional human-readable origin label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Persistent thread-settings overrides applied before a submission's input.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSettingsOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<ApprovalPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer_instructions: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_input_op_round_trips() {
        let op = UserSubmissionOp::UserInput {
            items: vec![],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: BTreeMap::new(),
            thread_settings: ThreadSettingsOverrides::default(),
        };
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["type"], "user_input");
        assert_eq!(op.kind(), "user_input");
        let back: UserSubmissionOp = serde_json::from_value(json).unwrap();
        assert_eq!(op, back);
    }

    #[test]
    fn interrupt_op_serializes() {
        let json = serde_json::to_value(&UserSubmissionOp::Interrupt).unwrap();
        assert_eq!(json["type"], "interrupt");
    }
}
