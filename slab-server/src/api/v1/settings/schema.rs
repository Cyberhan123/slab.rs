use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::domain::models::{
    SettingView as DomainSettingView, SettingsSystemBackendView as DomainSettingsSystemBackendView,
    SettingsSystemView as DomainSettingsSystemView, UpdateSettingCommand,
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingResponse {
    pub key: String,
    pub category: crate::domain::models::SettingCategory,
    pub label: String,
    pub description: String,
    pub control: crate::domain::models::SettingControl,
    pub editable: bool,
    pub value: serde_json::Value,
    pub effective_value: serde_json::Value,
    pub default_value: serde_json::Value,
    pub search_terms: Vec<String>,
    pub validation: crate::domain::models::SettingValidation,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SystemBackendResponse {
    pub backend: String,
    pub endpoint_configured: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    pub runtime_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_setting_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configured_workers: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_workers: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingsSystemResponse {
    pub bind_address: String,
    pub transport_mode: String,
    pub swagger_enabled: bool,
    pub admin_token_enabled: bool,
    pub cors_configured: bool,
    pub session_state_dir: String,
    pub backends: Vec<SystemBackendResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct UpdateSettingRequest {
    pub value: serde_json::Value,
}

impl From<DomainSettingView> for SettingResponse {
    fn from(view: DomainSettingView) -> Self {
        Self {
            key: view.key,
            category: view.category,
            label: view.label,
            description: view.description,
            control: view.control,
            editable: view.editable,
            value: view.value,
            effective_value: view.effective_value,
            default_value: view.default_value,
            search_terms: view.search_terms,
            validation: view.validation,
        }
    }
}

impl From<DomainSettingsSystemBackendView> for SystemBackendResponse {
    fn from(view: DomainSettingsSystemBackendView) -> Self {
        Self {
            backend: view.backend,
            endpoint_configured: view.endpoint_configured,
            endpoint: view.endpoint,
            runtime_status: view.runtime_status,
            worker_setting_key: view.worker_setting_key,
            configured_workers: view.configured_workers,
            effective_workers: view.effective_workers,
        }
    }
}

impl From<DomainSettingsSystemView> for SettingsSystemResponse {
    fn from(view: DomainSettingsSystemView) -> Self {
        Self {
            bind_address: view.bind_address,
            transport_mode: view.transport_mode,
            swagger_enabled: view.swagger_enabled,
            admin_token_enabled: view.admin_token_enabled,
            cors_configured: view.cors_configured,
            session_state_dir: view.session_state_dir,
            backends: view.backends.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<UpdateSettingRequest> for UpdateSettingCommand {
    fn from(request: UpdateSettingRequest) -> Self {
        Self {
            value: request.value,
        }
    }
}
