use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::SlabTypeError;

/// Shared runtime transport modes supported by the supervisor and gateway.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTransportMode {
    #[default]
    Http,
    Ipc,
}

impl RuntimeTransportMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Ipc => "ipc",
        }
    }
}

impl FromStr for RuntimeTransportMode {
    type Err = SlabTypeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "http" | "both" => Ok(Self::Http),
            "ipc" => Ok(Self::Ipc),
            other => Err(SlabTypeError::Parse(format!(
                "invalid runtime transport '{other}'; expected 'http' or 'ipc'"
            ))),
        }
    }
}

/// Shared launch settings used to build host-specific runtime supervisor plans.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct LaunchConfig {
    pub transport: RuntimeTransportMode,
    pub queue_capacity: u32,
    pub backend_capacity: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_ipc_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_log_dir: Option<String>,
    pub backends: LaunchBackendsConfig,
    pub profiles: LaunchProfilesConfig,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            transport: RuntimeTransportMode::Http,
            queue_capacity: 64,
            backend_capacity: 4,
            runtime_ipc_dir: None,
            runtime_log_dir: None,
            backends: LaunchBackendsConfig::default(),
            profiles: LaunchProfilesConfig::default(),
        }
    }
}

/// Per-backend enablement flags used by both server and desktop launch profiles.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct LaunchBackendsConfig {
    pub llama: LaunchBackendConfig,
    pub whisper: LaunchBackendConfig,
    pub diffusion: LaunchBackendConfig,
}

impl Default for LaunchBackendsConfig {
    fn default() -> Self {
        Self {
            llama: LaunchBackendConfig::default(),
            whisper: LaunchBackendConfig::default(),
            diffusion: LaunchBackendConfig::default(),
        }
    }
}

/// Launch settings for a single runtime backend child.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct LaunchBackendConfig {
    pub enabled: bool,
}

impl Default for LaunchBackendConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Profile-specific launch settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct LaunchProfilesConfig {
    pub server: ServerLaunchProfileConfig,
    pub desktop: DesktopLaunchProfileConfig,
}

impl Default for LaunchProfilesConfig {
    fn default() -> Self {
        Self {
            server: ServerLaunchProfileConfig::default(),
            desktop: DesktopLaunchProfileConfig::default(),
        }
    }
}

/// Host-specific launch settings for `slab-server`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ServerLaunchProfileConfig {
    pub gateway_bind: String,
    pub runtime_bind_host: String,
    pub runtime_bind_base_port: u32,
}

impl Default for ServerLaunchProfileConfig {
    fn default() -> Self {
        Self {
            gateway_bind: "127.0.0.1:3000".to_owned(),
            runtime_bind_host: "127.0.0.1".to_owned(),
            runtime_bind_base_port: 3001,
        }
    }
}

/// Host-specific launch settings for the Tauri desktop host.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DesktopLaunchProfileConfig {
    pub runtime_bind_host: String,
    pub runtime_bind_base_port: u32,
}

impl Default for DesktopLaunchProfileConfig {
    fn default() -> Self {
        Self { runtime_bind_host: "127.0.0.1".to_owned(), runtime_bind_base_port: 50051 }
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeTransportMode;
    use std::str::FromStr;

    #[test]
    fn parses_runtime_transport_aliases() {
        assert_eq!(RuntimeTransportMode::from_str("http").unwrap(), RuntimeTransportMode::Http);
        assert_eq!(RuntimeTransportMode::from_str("both").unwrap(), RuntimeTransportMode::Http);
        assert_eq!(RuntimeTransportMode::from_str("ipc").unwrap(), RuntimeTransportMode::Ipc);
    }
}
