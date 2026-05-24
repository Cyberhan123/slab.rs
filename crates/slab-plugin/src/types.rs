use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
pub use slab_types::{PluginApiRequest, PluginApiResponse, PluginEventPayload};

#[allow(unused_imports)]
pub use slab_types::plugin::{
    PluginAgentCapabilityContribution, PluginCapabilityKind, PluginCapabilityTransport,
    PluginCapabilityTransportType, PluginCommandContribution, PluginCompatibilityManifest,
    PluginContributesManifest, PluginFilePermissions, PluginInfo, PluginIntegrityManifest,
    PluginJsManifest, PluginLanguageServerContribution, PluginLanguageServerTransport,
    PluginManifest, PluginNetworkManifest, PluginNetworkMode, PluginPermissionsManifest,
    PluginPythonManifest, PluginRouteContribution, PluginRuntimeManifest,
    PluginSettingsContribution, PluginSidebarContribution, PluginUiManifest, PluginWasmManifest,
};

#[derive(Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub root_dir: PathBuf,
    pub ui_entry: String,
    pub wasm_entry_path: Option<PathBuf>,
    pub js_entry_path: Option<PathBuf>,
    pub python_entry_path: Option<PathBuf>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEmitRequest {
    pub topic: String,
    #[serde(default)]
    pub data: Value,
}
