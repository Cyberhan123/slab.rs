use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompletionRequest {
    /// The audio file path to transcribe.
    #[serde(rename = "path")]
    #[utoipa(rename = "path")]
    pub path: String,
}
