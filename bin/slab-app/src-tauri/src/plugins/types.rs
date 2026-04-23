use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[allow(unused_imports)]
pub use slab_types::plugin::{
    PluginAgentCapabilityContribution, PluginCapabilityKind, PluginCapabilityTransport,
    PluginCapabilityTransportType, PluginCommandContribution, PluginCompatibilityManifest,
    PluginContributesManifest, PluginFilePermissions, PluginInfo, PluginIntegrityManifest,
    PluginManifest, PluginNetworkManifest, PluginNetworkMode, PluginPermissionsManifest,
    PluginRouteContribution, PluginRuntimeManifest, PluginSettingsContribution,
    PluginSidebarContribution, PluginUiManifest, PluginWasmManifest,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginViewBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMountViewRequest {
    pub plugin_id: String,
    pub bounds: PluginViewBounds,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginUpdateViewBoundsRequest {
    pub plugin_id: String,
    pub bounds: PluginViewBounds,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginUnmountViewRequest {
    pub plugin_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMountViewResponse {
    pub plugin_id: String,
    pub webview_label: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCallRequest {
    pub plugin_id: String,
    pub function: String,
    #[serde(default)]
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCallResponse {
    pub output_text: String,
    pub output_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPickFileResponse {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginApiRequest {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginApiResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginThemeMode {
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginThemeSnapshot {
    pub mode: PluginThemeMode,
    #[serde(default)]
    pub tokens: HashMap<String, String>,
    #[serde(default)]
    pub updated_at: Option<u64>,
}

impl Default for PluginThemeSnapshot {
    fn default() -> Self {
        Self { mode: PluginThemeMode::Light, tokens: HashMap::new(), updated_at: None }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEmitRequest {
    pub topic: String,
    #[serde(default)]
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEventPayload {
    pub plugin_id: String,
    pub topic: String,
    pub data: Value,
    pub ts: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_legacy_manifest() {
        let manifest = serde_json::from_value::<PluginManifest>(serde_json::json!({
            "id": "legacy-plugin",
            "name": "Legacy Plugin",
            "version": "0.1.0",
            "ui": { "entry": "ui/index.html" },
            "integrity": { "filesSha256": { "ui/index.html": "0".repeat(64) } },
            "network": { "mode": "blocked", "allowHosts": [] }
        }))
        .expect("legacy manifest should deserialize");

        assert_eq!(manifest.manifest_version, 0);
        assert_eq!(manifest.runtime.ui.entry, "ui/index.html");
        assert_eq!(manifest.permissions.network.mode, PluginNetworkMode::Blocked);
    }

    #[test]
    fn deserializes_v1_manifest() {
        let manifest = serde_json::from_value::<PluginManifest>(serde_json::json!({
            "manifestVersion": 1,
            "id": "plugin-v1",
            "name": "Plugin V1",
            "version": "0.1.0",
            "runtime": { "ui": { "entry": "ui/index.html" } },
            "integrity": { "filesSha256": { "ui/index.html": "0".repeat(64) } },
            "permissions": {
                "network": { "mode": "blocked", "allowHosts": [] },
                "ui": ["route:create"]
            },
            "contributes": {
                "routes": [{ "id": "plugin.page", "path": "/plugins/plugin-v1" }]
            }
        }))
        .expect("v1 manifest should deserialize");

        assert_eq!(manifest.manifest_version, 1);
        assert_eq!(manifest.permissions.ui, vec!["route:create"]);
        assert_eq!(manifest.contributes.routes.len(), 1);
    }
}
