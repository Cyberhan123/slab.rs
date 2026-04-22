use std::collections::HashMap;

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub manifest_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub compatibility: PluginCompatibilityManifest,
    pub runtime: PluginRuntimeManifest,
    pub integrity: PluginIntegrityManifest,
    pub contributes: PluginContributesManifest,
    pub permissions: PluginPermissionsManifest,
}

impl<'de> Deserialize<'de> for PluginManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawPluginManifest::deserialize(deserializer)?;
        Self::try_from(raw).map_err(D::Error::custom)
    }
}

impl TryFrom<RawPluginManifest> for PluginManifest {
    type Error = String;

    fn try_from(raw: RawPluginManifest) -> Result<Self, Self::Error> {
        let runtime = match (raw.runtime, raw.ui) {
            (Some(runtime), _) => PluginRuntimeManifest { ui: runtime.ui, wasm: runtime.wasm.or(raw.wasm) },
            (None, Some(ui)) => PluginRuntimeManifest { ui, wasm: raw.wasm },
            (None, None) => return Err("missing runtime.ui or legacy ui entry".to_string()),
        };

        let mut permissions = raw.permissions.unwrap_or_default();
        if let Some(network) = raw.network {
            permissions.network = network;
        }

        Ok(Self {
            manifest_version: raw.manifest_version.unwrap_or(0),
            id: raw.id,
            name: raw.name,
            version: raw.version,
            compatibility: raw.compatibility.unwrap_or_default(),
            runtime,
            integrity: raw.integrity,
            contributes: raw.contributes.unwrap_or_default(),
            permissions,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPluginManifest {
    #[serde(default)]
    manifest_version: Option<u32>,
    id: String,
    name: String,
    version: String,
    #[serde(default)]
    compatibility: Option<PluginCompatibilityManifest>,
    #[serde(default)]
    runtime: Option<PluginRuntimeManifest>,
    #[serde(default)]
    ui: Option<PluginUiManifest>,
    #[serde(default)]
    wasm: Option<PluginWasmManifest>,
    integrity: PluginIntegrityManifest,
    #[serde(default)]
    contributes: Option<PluginContributesManifest>,
    #[serde(default)]
    permissions: Option<PluginPermissionsManifest>,
    #[serde(default)]
    network: Option<PluginNetworkManifest>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCompatibilityManifest {
    #[serde(default)]
    pub slab: Option<String>,
    #[serde(default)]
    pub plugin_api: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeManifest {
    pub ui: PluginUiManifest,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub wasm: Option<PluginWasmManifest>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginUiManifest {
    pub entry: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginWasmManifest {
    pub entry: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginIntegrityManifest {
    #[serde(rename = "filesSha256")]
    pub files_sha256: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginNetworkMode {
    Blocked,
    Allowlist,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginNetworkManifest {
    pub mode: PluginNetworkMode,
    #[serde(default)]
    pub allow_hosts: Vec<String>,
}

impl Default for PluginNetworkManifest {
    fn default() -> Self {
        Self { mode: PluginNetworkMode::Blocked, allow_hosts: Vec::new() }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPermissionsManifest {
    #[serde(default)]
    pub network: PluginNetworkManifest,
    #[serde(default)]
    pub ui: Vec<String>,
    #[serde(default)]
    pub agent: Vec<String>,
    #[serde(default)]
    pub slab_api: Vec<String>,
    #[serde(default)]
    pub files: PluginFilePermissions,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginFilePermissions {
    #[serde(default)]
    pub read: Vec<String>,
    #[serde(default)]
    pub write: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginContributesManifest {
    #[serde(default)]
    pub routes: Vec<PluginRouteContribution>,
    #[serde(default)]
    pub sidebar: Vec<PluginSidebarContribution>,
    #[serde(default)]
    pub commands: Vec<PluginCommandContribution>,
    #[serde(default)]
    pub settings: Vec<PluginSettingsContribution>,
    #[serde(default)]
    pub agent_capabilities: Vec<PluginAgentCapabilityContribution>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRouteContribution {
    pub id: String,
    pub path: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub title_key: Option<String>,
    #[serde(default)]
    pub entry: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSidebarContribution {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub label_key: Option<String>,
    #[serde(default)]
    pub route: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCommandContribution {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub label_key: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub route: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSettingsContribution {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub title_key: Option<String>,
    pub schema: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAgentCapabilityContribution {
    pub id: String,
    pub kind: PluginCapabilityKind,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub description_key: Option<String>,
    #[serde(default)]
    pub input_schema: Option<String>,
    #[serde(default)]
    pub output_schema: Option<String>,
    #[serde(default)]
    pub effects: Vec<String>,
    pub transport: PluginCapabilityTransport,
    #[serde(default)]
    pub expose_as_mcp_tool: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginCapabilityKind {
    Tool,
    Workflow,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginCapabilityTransport {
    #[serde(rename = "type")]
    pub transport_type: PluginCapabilityTransportType,
    pub function: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PluginCapabilityTransportType {
    PluginCall,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub valid: bool,
    pub error: Option<String>,
    pub manifest_version: u32,
    pub compatibility: PluginCompatibilityManifest,
    pub ui_entry: Option<String>,
    pub has_wasm: bool,
    pub network_mode: String,
    pub allow_hosts: Vec<String>,
    pub contributions: PluginContributesManifest,
    pub permissions: PluginPermissionsManifest,
}

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
