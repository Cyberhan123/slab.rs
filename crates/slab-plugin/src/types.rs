use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[allow(unused_imports)]
pub use slab_types::plugin::{
    PluginAgentCapabilityContribution, PluginCapabilityKind, PluginCapabilityTransport,
    PluginCapabilityTransportType, PluginCommandContribution, PluginCompatibilityManifest,
    PluginContributesManifest, PluginFilePermissions, PluginInfo, PluginIntegrityManifest,
    PluginJsManifest, PluginLanguageServerContribution, PluginLanguageServerTransport,
    PluginManifest, PluginNetworkManifest, PluginNetworkMode, PluginPermissionsManifest,
    PluginRouteContribution, PluginRuntimeManifest, PluginSettingsContribution,
    PluginSidebarContribution, PluginUiManifest, PluginWasmManifest,
};

#[derive(Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub root_dir: PathBuf,
    pub ui_entry: String,
    pub wasm_entry_path: Option<PathBuf>,
    pub js_entry_path: Option<PathBuf>,
    pub files_sha256: HashMap<String, String>,
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
