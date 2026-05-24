use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::plugin::PluginPermissionsManifest;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeCallRequest {
    pub call_id: String,
    pub plugin_id: String,
    pub root_dir: String,
    pub entry: String,
    pub export_name: String,
    #[serde(default)]
    pub params: Value,
    pub permissions: PluginPermissionsManifest,
    #[serde(default)]
    pub file_grants: Vec<PluginRuntimeFileGrant>,
    #[serde(default)]
    pub blocked_fetch_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeCallResponse {
    #[serde(default)]
    pub result: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeFileGrant {
    pub label: String,
    pub path: String,
    pub access: PluginRuntimeFileAccess,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum PluginRuntimeFileAccess {
    Read,
    Write,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginApiResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeApiHostRequest {
    pub call_id: String,
    pub plugin_id: String,
    pub request: PluginApiRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeUiEmitRequest {
    pub call_id: String,
    pub plugin_id: String,
    pub topic: String,
    #[serde(default)]
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginEventPayload {
    pub plugin_id: String,
    pub topic: String,
    pub data: Value,
    pub ts: u64,
}

pub fn authorize_plugin_slab_api_request(
    allowed: &[String],
    request: &PluginApiRequest,
) -> Result<(), String> {
    let Some(required_permission) =
        required_plugin_slab_api_permission(&request.method, &request.path)
    else {
        return Err(format!(
            "plugin API request {} {} is not part of the allowed plugin API surface",
            request.method, request.path
        ));
    };

    if allowed.iter().any(|permission| permission == required_permission) {
        return Ok(());
    }

    Err(format!(
        "plugin API request {} {} requires permissions.slabApi `{required_permission}`",
        request.method, request.path
    ))
}

pub fn required_plugin_slab_api_permission(method: &str, path: &str) -> Option<&'static str> {
    let method = method.to_ascii_uppercase();
    let path = path.split('?').next().unwrap_or(path);

    match method.as_str() {
        "GET" if plugin_path_matches(path, "/v1/models") => Some("models:read"),
        "POST" if path == "/v1/models/load" => Some("models:load"),
        "POST" if path == "/v1/ffmpeg/convert" => Some("ffmpeg:convert"),
        "POST" if path == "/v1/audio/transcriptions" => Some("audio:transcribe"),
        "POST" if path == "/v1/subtitles/render" => Some("subtitle:render"),
        "POST" if path == "/v1/chat/completions" => Some("chat:complete"),
        "GET" if plugin_path_matches(path, "/v1/tasks") => Some("tasks:read"),
        "POST" if path.starts_with("/v1/tasks/") && path.ends_with("/cancel") => {
            Some("tasks:cancel")
        }
        _ => None,
    }
}

fn plugin_path_matches(path: &str, base: &str) -> bool {
    path == base || path.starts_with(&format!("{base}/"))
}
