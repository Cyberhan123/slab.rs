use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileSearchToolCall {
    /// The type of the file search tool call. Always `file_search_call`.
    #[serde(rename = "type")]
    pub r#type: FileSearchToolCallType,
    #[serde(rename = "id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(rename = "queries", skip_serializing_if = "Option::is_none")]
    pub queries: Option<Vec<String>>,
    /// Search result hits. `null` when the search has not yet produced hits,
    /// otherwise an array of result objects (each carrying `file_id`,
    /// `filename`, `score`, `text`, `attributes`). Modeled as opaque JSON
    /// because the inner shape is provider-specific and the adapter only
    /// forwards it verbatim.
    #[serde(rename = "results", skip_serializing_if = "Option::is_none", default)]
    pub results: Option<Vec<serde_json::Value>>,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum FileSearchToolCallType {
    #[serde(rename = "file_search_call")]
    #[default]
    FileSearchCall,
}
