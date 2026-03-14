use slab_core::api::Backend;
use strum::IntoEnumIterator;

use crate::context::ModelState;
use crate::domain::models::{
    setting_definition, setting_definitions, ConfigEntryView, SettingView,
    SettingsSystemBackendView, SettingsSystemView, UpdateSettingCommand,
    DIFFUSION_NUM_WORKERS_SETTING_KEY, LLAMA_NUM_WORKERS_SETTING_KEY,
    WHISPER_NUM_WORKERS_SETTING_KEY,
};
use crate::error::ServerError;
use crate::infra::db::ConfigStore;

const DEFAULT_MODEL_NUM_WORKERS: u32 = 1;

#[derive(Clone)]
pub struct SettingsService {
    state: ModelState,
}

impl SettingsService {
    pub fn new(state: ModelState) -> Self {
        Self { state }
    }

    pub async fn list_settings(&self) -> Result<Vec<SettingView>, ServerError> {
        let mut views = Vec::with_capacity(setting_definitions().len());
        for definition in setting_definitions() {
            views.push(self.build_setting_view(definition.key).await?);
        }
        Ok(views)
    }

    pub async fn get_setting(&self, key: &str) -> Result<SettingView, ServerError> {
        self.build_setting_view(key).await
    }

    pub async fn update_setting(
        &self,
        key: &str,
        command: UpdateSettingCommand,
    ) -> Result<SettingView, ServerError> {
        let definition = setting_definition(key)
            .ok_or_else(|| ServerError::NotFound(format!("setting key '{key}' not found")))?;
        let normalized = definition.normalized_raw_from_value(&command.value)?;
        self.state
            .store()
            .set_config_entry(definition.key, Some(definition.label), &normalized)
            .await?;
        self.build_setting_view(definition.key).await
    }

    pub async fn system_info(&self) -> Result<SettingsSystemView, ServerError> {
        let config = self.state.config();
        let mut backends = Vec::new();

        for backend in Backend::iter() {
            let backend_id = backend.to_string();
            let endpoint = match backend_id.as_str() {
                "ggml.llama" => config.llama_grpc_endpoint.clone(),
                "ggml.whisper" => config.whisper_grpc_endpoint.clone(),
                "ggml.diffusion" => config.diffusion_grpc_endpoint.clone(),
                _ => None,
            };

            let worker_setting_key = workers_key_for_backend(&backend_id).map(str::to_owned);
            let configured_workers = match workers_key_for_backend(&backend_id) {
                Some(key) => resolve_configured_workers(&self.state, key).await?,
                None => None,
            };

            backends.push(SettingsSystemBackendView {
                backend: backend_id.clone(),
                endpoint_configured: endpoint
                    .as_ref()
                    .is_some_and(|value| !value.trim().is_empty()),
                endpoint,
                runtime_status: if self.state.grpc().has_backend(&backend_id) {
                    "ready".to_owned()
                } else {
                    "disabled".to_owned()
                },
                worker_setting_key,
                configured_workers,
                effective_workers: Some(configured_workers.unwrap_or(DEFAULT_MODEL_NUM_WORKERS)),
            });
        }

        Ok(SettingsSystemView {
            bind_address: config.bind_address.clone(),
            transport_mode: config.transport_mode.clone(),
            swagger_enabled: config.enable_swagger,
            admin_token_enabled: config.admin_api_token.is_some(),
            cors_configured: config
                .cors_allowed_origins
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty()),
            session_state_dir: config.session_state_dir.clone(),
            backends,
        })
    }

    async fn build_setting_view(&self, key: &str) -> Result<SettingView, ServerError> {
        let definition = setting_definition(key)
            .ok_or_else(|| ServerError::NotFound(format!("setting key '{key}' not found")))?;
        let raw = self.state.store().get_config_value(definition.key).await?;
        let value = definition.stored_value_from_raw(raw.as_deref())?;
        let effective_value = definition.effective_value_from_raw(raw.as_deref())?;

        Ok(SettingView {
            key: definition.key.to_owned(),
            category: definition.category.clone(),
            label: definition.label.to_owned(),
            description: definition.description.to_owned(),
            control: definition.control.clone(),
            editable: definition.editable,
            value,
            effective_value,
            default_value: definition.default_value(),
            search_terms: definition
                .search_terms
                .iter()
                .map(|term| (*term).to_owned())
                .collect(),
            validation: definition.validation.clone(),
        })
    }
}

pub async fn list_config_entries(state: &ModelState) -> Result<Vec<ConfigEntryView>, ServerError> {
    let entries = state.store().list_config_values().await?;
    Ok(entries
        .into_iter()
        .map(|(key, name, value)| {
            let display_name = setting_definition(&key)
                .map(|definition| definition.label.to_owned())
                .unwrap_or_else(|| if name.trim().is_empty() { key.clone() } else { name });

            ConfigEntryView {
                key,
                name: display_name,
                value,
            }
        })
        .collect())
}

pub async fn get_config_entry(
    state: &ModelState,
    key: &str,
) -> Result<ConfigEntryView, ServerError> {
    let (name, value) = state
        .store()
        .get_config_entry(key)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("config key '{key}' not found")))?;

    let display_name = setting_definition(key)
        .map(|definition| definition.label.to_owned())
        .unwrap_or_else(|| if name.trim().is_empty() { key.to_owned() } else { name });

    Ok(ConfigEntryView {
        key: key.to_owned(),
        name: display_name,
        value,
    })
}

pub async fn set_config_entry(
    state: &ModelState,
    key: &str,
    name: Option<&str>,
    value: &str,
) -> Result<ConfigEntryView, ServerError> {
    let normalized = match setting_definition(key) {
        Some(definition) => definition.normalized_raw_from_legacy_input(value)?,
        None => value.to_owned(),
    };
    let display_name = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| setting_definition(key).map(|definition| definition.label.to_owned()));

    state
        .store()
        .set_config_entry(key, display_name.as_deref(), &normalized)
        .await?;
    get_config_entry(state, key).await
}

fn workers_key_for_backend(backend_id: &str) -> Option<&'static str> {
    match backend_id {
        "ggml.llama" => Some(LLAMA_NUM_WORKERS_SETTING_KEY),
        "ggml.whisper" => Some(WHISPER_NUM_WORKERS_SETTING_KEY),
        "ggml.diffusion" => Some(DIFFUSION_NUM_WORKERS_SETTING_KEY),
        _ => None,
    }
}

async fn resolve_configured_workers(
    state: &ModelState,
    key: &str,
) -> Result<Option<u32>, ServerError> {
    let raw = state.store().get_config_value(key).await?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = trimmed.parse::<u32>().map_err(|_| {
        ServerError::BadRequest(format!("config key '{key}' must be a positive integer"))
    })?;
    if parsed < 1 {
        return Err(ServerError::BadRequest(format!(
            "config key '{key}' must be at least 1"
        )));
    }
    Ok(Some(parsed))
}
