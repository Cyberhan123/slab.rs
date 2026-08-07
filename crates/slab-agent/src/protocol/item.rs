//! `TurnItem` — the discrete artifacts that make up a turn on the wire.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One item within a [`super::turn::Turn`].
///
/// Discriminated by `type`. Each variant accepts both the camelCase and
/// PascalCase spellings (e.g. `agentMessage` / `AgentMessage`) to match the
/// public TS contract.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TurnItem {
    UserMessage {
        id: String,
        content: Vec<UserMessageContent>,
    },
    #[serde(alias = "AgentMessage")]
    AgentMessage {
        id: String,
        text: String,
    },
    #[serde(alias = "Reasoning")]
    Reasoning {
        id: String,
        summary: ReasoningText,
        content: ReasoningText,
    },
    #[serde(alias = "CommandExecution")]
    CommandExecution {
        id: String,
        command: String,
        cwd: String,
        #[serde(default, rename = "processId", skip_serializing_if = "Option::is_none")]
        process_id: Option<String>,
        status: String,
        #[serde(default, rename = "aggregatedOutput", skip_serializing_if = "Option::is_none")]
        aggregated_output: Option<String>,
        #[serde(default, rename = "exitCode", skip_serializing_if = "Option::is_none")]
        exit_code: Option<i64>,
        #[serde(default, rename = "durationMs", skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    #[serde(alias = "FileChange")]
    FileChange {
        id: String,
        changes: Vec<Value>,
        status: String,
    },
    #[serde(alias = "McpToolCall")]
    McpToolCall {
        id: String,
        server: String,
        tool: String,
        arguments: Value,
        status: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<Value>,
        #[serde(default, rename = "durationMs", skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    #[serde(alias = "WebSearch")]
    WebSearch {
        id: String,
        query: String,
    },
    #[serde(alias = "ImageView")]
    ImageView {
        id: String,
        path: String,
    },
    /// A structured execution plan authored in Plan interaction mode (by the
    /// `plan` / `update_plan` / `present_plan` tools). `plan` is the serialized
    /// `slab_agent::Plan` (snake_case fields). UI-only — its LLM-visible
    /// counterpart flows through as the tool-result text.
    #[serde(alias = "Plan")]
    Plan {
        id: String,
        plan: Value,
    },
}

impl TurnItem {
    /// The item id shared by every variant.
    pub fn id(&self) -> &str {
        match self {
            Self::UserMessage { id, .. }
            | Self::AgentMessage { id, .. }
            | Self::Reasoning { id, .. }
            | Self::CommandExecution { id, .. }
            | Self::FileChange { id, .. }
            | Self::McpToolCall { id, .. }
            | Self::WebSearch { id, .. }
            | Self::ImageView { id, .. }
            | Self::Plan { id, .. } => id,
        }
    }
}

/// A content part of a [`TurnItem::UserMessage`].
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UserMessageContent {
    Text {
        text: String,
    },
    /// Image — either a URL (`imageUrl`) or inline base64 (`base64` + `mimeType`).
    Image {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base64: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
}

/// Reasoning text — accepted as a single string or an array of strings.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ReasoningText {
    Many(Vec<String>),
    One(String),
}

impl ReasoningText {
    pub fn one(text: impl Into<String>) -> Self {
        Self::One(text.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_message_accepts_camel_and_pascal_type() {
        let camel = r#"{"type":"agentMessage","id":"i1","text":"hi"}"#;
        let pascal = r#"{"type":"AgentMessage","id":"i1","text":"hi"}"#;
        let a: TurnItem = serde_json::from_str(camel).unwrap();
        let b: TurnItem = serde_json::from_str(pascal).unwrap();
        assert_eq!(a, b);
        // Serializes back to the camelCase canonical form.
        let out = serde_json::to_value(&a).unwrap();
        assert_eq!(out["type"], "agentMessage");
    }

    #[test]
    fn reasoning_text_accepts_string_or_array() {
        let one: ReasoningText = serde_json::from_str(r#""hello""#).unwrap();
        let many: ReasoningText = serde_json::from_str(r#"["a","b"]"#).unwrap();
        assert!(matches!(one, ReasoningText::One(_)));
        assert!(matches!(many, ReasoningText::Many(_)));
    }

    #[test]
    fn command_execution_round_trips() {
        let item = TurnItem::CommandExecution {
            id: "c1".to_owned(),
            command: "ls".to_owned(),
            cwd: "/tmp".to_owned(),
            process_id: None,
            status: "completed".to_owned(),
            aggregated_output: Some("out".to_owned()),
            exit_code: Some(0),
            duration_ms: Some(12),
        };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["type"], "commandExecution");
        assert_eq!(json["cwd"], "/tmp");
        assert_eq!(json["durationMs"], 12);
    }

    #[test]
    fn plan_round_trips_and_exposes_id() {
        let item = TurnItem::Plan {
            id: "p1".to_owned(),
            plan: serde_json::json!({
                "plan_id": "plan-0",
                "summary": "ship it",
                "items": [{ "step": "do", "status": "pending" }],
                "counts": { "pending": 1, "in_progress": 0, "completed": 0, "blocked": 0 }
            }),
        };
        assert_eq!(item.id(), "p1");
        // camelCase tag; the nested plan keeps its snake_case producer shape.
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["type"], "plan");
        assert_eq!(json["plan"]["plan_id"], "plan-0");
        assert_eq!(json["plan"]["counts"]["pending"], 1);
        // Round-trips and accepts the PascalCase spelling (mirrors other variants).
        let s = serde_json::to_string(&item).unwrap();
        let back: TurnItem = serde_json::from_str(&s).unwrap();
        assert_eq!(back, item);
        let pascal: TurnItem =
            serde_json::from_str(r#"{"type":"Plan","id":"p1","plan":{}}"#).unwrap();
        assert!(matches!(pascal, TurnItem::Plan { id, .. } if id == "p1"));
    }
}
