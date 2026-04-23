use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginView {
    pub id: String,
    pub name: String,
    pub version: String,
    pub valid: bool,
    pub error: Option<String>,
    pub manifest_version: u32,
    pub compatibility: Value,
    pub ui_entry: Option<String>,
    pub has_wasm: bool,
    pub network_mode: String,
    pub allow_hosts: Vec<String>,
    pub contributions: Value,
    pub permissions: Value,
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
pub struct PluginMarketView {
    pub source_id: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub package_url: String,
    pub package_sha256: Option<String>,
    pub homepage: Option<String>,
    pub tags: Vec<String>,
    pub installed_version: Option<String>,
    pub enabled: bool,
    pub update_available: bool,
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
