use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub ui: PluginUiManifest,
    pub wasm: PluginWasmManifest,
    pub integrity: PluginIntegrityManifest,
    pub network: PluginNetworkManifest,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub valid: bool,
    pub error: Option<String>,
    pub ui_entry: Option<String>,
    pub network_mode: String,
    pub allow_hosts: Vec<String>,
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
