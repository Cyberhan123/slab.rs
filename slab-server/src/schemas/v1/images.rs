use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request body for `POST /v1/images/generations`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImageGenerationRequest {
    /// The model identifier to use.
    pub model: String,
    /// Text description of the desired image.
    pub prompt: String,
    /// Number of images to generate (default `1`).
    #[serde(default = "default_n")]
    pub n: u32,
    /// Desired image size, e.g. `"512x512"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

fn default_n() -> u32 {
    1
}
