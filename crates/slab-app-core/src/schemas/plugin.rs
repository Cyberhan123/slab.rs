use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::domain::models::{InstallPluginCommand, PluginView};

#[derive(Debug, Clone, Deserialize, IntoParams, Validate, ToSchema)]
pub struct PluginPath {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "plugin id must not be empty"
    ))]
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstallPluginRequest {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "plugin id must not be empty"
    ))]
    pub plugin_id: String,
    pub source_id: Option<String>,
    pub version: Option<String>,
    pub package_url: Option<String>,
    pub package_sha256: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StopPluginRequest {
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginResponse {
    pub id: String,
    pub name: String,
    pub version: String,
    pub valid: bool,
    pub error: Option<String>,
    pub manifest_version: u32,
    #[schema(value_type = Object)]
    pub compatibility: Value,
    pub ui_entry: Option<String>,
    pub has_wasm: bool,
    pub network_mode: String,
    pub allow_hosts: Vec<String>,
    #[schema(value_type = Object)]
    pub contributions: Value,
    #[schema(value_type = Object)]
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeletePluginResponse {
    pub id: String,
    pub deleted: bool,
}

impl From<InstallPluginRequest> for InstallPluginCommand {
    fn from(value: InstallPluginRequest) -> Self {
        Self {
            plugin_id: value.plugin_id,
            source_id: value.source_id,
            version: value.version,
            package_url: value.package_url,
            package_sha256: value.package_sha256,
        }
    }
}

impl From<PluginView> for PluginResponse {
    fn from(value: PluginView) -> Self {
        Self {
            id: value.id,
            name: value.name,
            version: value.version,
            valid: value.valid,
            error: value.error,
            manifest_version: value.manifest_version,
            compatibility: value.compatibility,
            ui_entry: value.ui_entry,
            has_wasm: value.has_wasm,
            network_mode: value.network_mode,
            allow_hosts: value.allow_hosts,
            contributions: value.contributions,
            permissions: value.permissions,
            source_kind: value.source_kind,
            source_ref: value.source_ref,
            install_root: value.install_root,
            installed_version: value.installed_version,
            manifest_hash: value.manifest_hash,
            enabled: value.enabled,
            runtime_status: value.runtime_status,
            last_error: value.last_error,
            installed_at: value.installed_at,
            updated_at: value.updated_at,
            last_seen_at: value.last_seen_at,
            last_started_at: value.last_started_at,
            last_stopped_at: value.last_stopped_at,
            available_version: value.available_version,
            update_available: value.update_available,
            removable: value.removable,
        }
    }
}

