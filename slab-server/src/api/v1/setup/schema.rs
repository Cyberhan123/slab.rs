use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::models::{CompleteSetupCommand, ComponentStatus, EnvironmentStatus};

/// Response body for `GET /v1/setup/status`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SetupStatusResponse {
    /// Whether the one-time setup wizard has been completed.
    pub initialized: bool,
    /// FFmpeg binary availability.
    pub ffmpeg: ComponentStatusResponse,
    /// AI backend library availability (one entry per backend).
    pub backends: Vec<ComponentStatusResponse>,
}

/// Availability information for a single environment component.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ComponentStatusResponse {
    pub name: String,
    pub installed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Request body for `POST /v1/setup/complete`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteSetupRequest {
    /// Pass `true` to mark setup as done, `false` to reset it.
    #[serde(default = "default_true")]
    pub initialized: bool,
}

fn default_true() -> bool {
    true
}

// ── conversions ───────────────────────────────────────────────────────────────

impl From<ComponentStatus> for ComponentStatusResponse {
    fn from(s: ComponentStatus) -> Self {
        Self {
            name: s.name,
            installed: s.installed,
            version: s.version,
        }
    }
}

impl From<EnvironmentStatus> for SetupStatusResponse {
    fn from(s: EnvironmentStatus) -> Self {
        Self {
            initialized: s.initialized,
            ffmpeg: s.ffmpeg.into(),
            backends: s.backends.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<CompleteSetupRequest> for CompleteSetupCommand {
    fn from(r: CompleteSetupRequest) -> Self {
        Self {
            initialized: r.initialized,
        }
    }
}
