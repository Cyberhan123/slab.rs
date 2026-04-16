use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Status of a single environment component (FFmpeg, a backend library, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ComponentStatus {
    /// Human-readable component name.
    pub name: String,
    /// `true` when the component is installed and ready to use.
    pub installed: bool,
    /// Version string, if detectable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Full environment status returned by `GET /v1/setup/status`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EnvironmentStatus {
    /// Whether the one-time setup wizard has been completed.
    pub initialized: bool,
    /// Whether the packaged runtime payload is already present under `resources/libs`.
    pub runtime_payload_installed: bool,
    /// Status of the FFmpeg binary.
    pub ffmpeg: ComponentStatus,
    /// Status of each AI backend library.
    pub backends: Vec<ComponentStatus>,
}

/// Command used to mark setup as complete via `POST /v1/setup/complete`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteSetupCommand {
    /// Set to `true` to mark setup as initialized.
    #[serde(default = "default_true")]
    pub initialized: bool,
}

fn default_true() -> bool {
    true
}
