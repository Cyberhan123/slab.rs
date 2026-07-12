//! Per-turn aggregate: the items produced during one agent turn.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::item::TurnItem;

/// Structured turn failure descriptor (mirrors the `Turn.error` shape).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnError {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_details: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub id: String,
    #[serde(default)]
    pub items: Vec<TurnItem>,
    /// Open string set: `completed` / `interrupted` / `failed` / `inProgress`
    /// (plus PascalCase aliases accepted on decode).
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<TurnError>,
}
