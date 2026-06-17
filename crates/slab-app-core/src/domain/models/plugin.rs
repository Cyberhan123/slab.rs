use serde::{Deserialize, Serialize};
use slab_types::{
    PluginCompatibilityManifest, PluginContributesManifest, PluginPermissionsManifest,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginView {
    pub id: String,
    pub name: String,
    pub version: String,
    pub valid: bool,
    pub error: Option<String>,
    pub manifest_version: u32,
    pub compatibility: Option<PluginCompatibilityManifest>,
    pub ui_entry: Option<String>,
    pub ui_url: Option<String>,
    pub has_wasm: bool,
    pub network_mode: String,
    pub allow_hosts: Vec<String>,
    pub contributions: Option<PluginContributesManifest>,
    pub permissions: Option<PluginPermissionsManifest>,
    pub source_kind: String,
    pub source_ref: Option<String>,
    pub install_root: Option<String>,
    pub installed_version: Option<String>,
    pub manifest_hash: Option<String>,
    pub enabled: bool,
    pub runtime_status: String,
    pub last_error: Option<String>,
    pub installed_at: Option<String>,
    pub updated_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub last_started_at: Option<String>,
    pub last_stopped_at: Option<String>,
    pub available_version: Option<String>,
    pub update_available: bool,
    pub removable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPluginCommand {
    pub plugin_id: String,
    pub source_id: Option<String>,
    pub version: Option<String>,
    pub package_url: Option<String>,
    pub package_sha256: Option<String>,
}
