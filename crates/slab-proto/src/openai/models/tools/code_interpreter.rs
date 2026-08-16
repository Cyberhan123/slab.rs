/*
 * OpenAI API - Merged type definitions
 */

use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodeInterpreterToolCall {
    /// The type of the code interpreter tool call. Always `code_interpreter_call`.
    #[serde(rename = "type")]
    pub r#type: CodeInterpreterToolCallType,
    /// The unique ID of the code interpreter tool call.
    #[serde(rename = "id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The status of the code interpreter tool call (e.g. `completed`).
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The Python code the model executed.
    #[serde(rename = "code", skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The container ID the code ran in.
    #[serde(rename = "container_id", skip_serializing_if = "Option::is_none")]
    pub container_id: Option<String>,
    /// The outputs produced by the code (e.g. `{type:"logs", logs}`). Opaque JSON
    /// because the slab-proto crate does not model every output variant.
    #[serde(rename = "outputs", skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<serde_json::Value>>,
}

impl CodeInterpreterToolCall {
    pub fn new(r#type: CodeInterpreterToolCallType) -> CodeInterpreterToolCall {
        CodeInterpreterToolCall {
            r#type,
            id: None,
            status: None,
            code: None,
            container_id: None,
            outputs: None,
        }
    }
}

/// The type of the code interpreter tool call. Always `code_interpreter_call`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum CodeInterpreterToolCallType {
    #[serde(rename = "code_interpreter_call")]
    #[default]
    CodeInterpreterCall,
}
