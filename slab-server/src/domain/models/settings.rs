use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

use super::settings_jsonschema::{
    base_property_validation_schema, chat_providers_validation_schema, ensure_json_schema_is_valid,
    normalize_json_pointer, validate_settings_schema_document,
};
use crate::error::ServerError;

pub const MODEL_CACHE_DIR_PMID: &str = "runtime.model_cache_dir";
pub const LLAMA_NUM_WORKERS_PMID: &str = "runtime.llama.num_workers";
pub const WHISPER_NUM_WORKERS_PMID: &str = "runtime.whisper.num_workers";
pub const DIFFUSION_NUM_WORKERS_PMID: &str = "runtime.diffusion.num_workers";
pub const LLAMA_CONTEXT_LENGTH_PMID: &str = "runtime.llama.context_length";
pub const MODEL_AUTO_UNLOAD_ENABLED_PMID: &str = "runtime.model_auto_unload.enabled";
pub const MODEL_AUTO_UNLOAD_IDLE_MINUTES_PMID: &str = "runtime.model_auto_unload.idle_minutes";
pub const CHAT_PROVIDERS_PMID: &str = "chat.providers";
pub const DIFFUSION_MODEL_PATH_PMID: &str = "diffusion.paths.model";
pub const DIFFUSION_VAE_PATH_PMID: &str = "diffusion.paths.vae";
pub const DIFFUSION_TAESD_PATH_PMID: &str = "diffusion.paths.taesd";
pub const DIFFUSION_LORA_MODEL_DIR_PMID: &str = "diffusion.paths.lora_model_dir";
pub const DIFFUSION_CLIP_L_PATH_PMID: &str = "diffusion.paths.clip_l";
pub const DIFFUSION_CLIP_G_PATH_PMID: &str = "diffusion.paths.clip_g";
pub const DIFFUSION_T5XXL_PATH_PMID: &str = "diffusion.paths.t5xxl";
pub const DIFFUSION_FLASH_ATTN_PMID: &str = "diffusion.performance.flash_attn";
pub const DIFFUSION_KEEP_VAE_ON_CPU_PMID: &str = "diffusion.performance.keep_vae_on_cpu";
pub const DIFFUSION_KEEP_CLIP_ON_CPU_PMID: &str = "diffusion.performance.keep_clip_on_cpu";
pub const DIFFUSION_OFFLOAD_PARAMS_TO_CPU_PMID: &str =
    "diffusion.performance.offload_params_to_cpu";

// ── Setup / first-run PMIDs ──────────────────────────────────────────────────
pub const SETUP_INITIALIZED_PMID: &str = "setup.initialized";
pub const SETUP_FFMPEG_AUTO_DOWNLOAD_PMID: &str = "setup.ffmpeg.auto_download";
pub const SETUP_FFMPEG_DIR_PMID: &str = "setup.ffmpeg.dir";
pub const SETUP_BACKENDS_DIR_PMID: &str = "setup.backends.dir";
pub const SETUP_BACKENDS_LLAMA_TAG_PMID: &str = "setup.backends.llama.tag";
pub const SETUP_BACKENDS_LLAMA_ASSET_PMID: &str = "setup.backends.llama.asset";
pub const SETUP_BACKENDS_WHISPER_TAG_PMID: &str = "setup.backends.whisper.tag";
pub const SETUP_BACKENDS_WHISPER_ASSET_PMID: &str = "setup.backends.whisper.asset";
pub const SETUP_BACKENDS_DIFFUSION_TAG_PMID: &str = "setup.backends.diffusion.tag";
pub const SETUP_BACKENDS_DIFFUSION_ASSET_PMID: &str = "setup.backends.diffusion.asset";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SettingValueType {
    Boolean,
    Integer,
    String,
    Array,
    Object,
}

impl Default for SettingValueType {
    fn default() -> Self {
        Self::String
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct SettingPropertySchema {
    #[serde(rename = "type")]
    pub value_type: SettingValueType,
    #[serde(default, rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default)]
    pub default_value: Value,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub multiline: bool,
    #[serde(default)]
    pub order: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingPropertyView {
    pub pmid: String,
    pub label: String,
    #[serde(default)]
    pub description_md: String,
    pub editable: bool,
    pub schema: SettingPropertySchema,
    pub effective_value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_value: Option<Value>,
    pub is_overridden: bool,
    pub search_terms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingsSubsectionView {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description_md: String,
    pub properties: Vec<SettingPropertyView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingsSectionView {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description_md: String,
    pub subsections: Vec<SettingsSubsectionView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingsDocumentView {
    pub schema_version: u32,
    pub settings_path: String,
    pub warnings: Vec<String>,
    pub sections: Vec<SettingsSectionView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateSettingOperation {
    Set,
    Unset,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateSettingCommand {
    pub op: UpdateSettingOperation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingValidationErrorData {
    #[serde(rename = "type")]
    pub error_type: String,
    pub pmid: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CloudProviderSettingValue {
    #[serde(alias = "provider_id", alias = "providerId")]
    pub id: String,
    #[serde(default, alias = "displayName", alias = "provider_name")]
    pub name: String,
    #[serde(alias = "apiBase", alias = "base_url", alias = "baseUrl")]
    pub api_base: String,
    #[serde(default, alias = "apiKey", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, alias = "apiKeyEnv", skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    pub models: Vec<CloudProviderModelSettingValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CloudProviderModelSettingValue {
    #[serde(alias = "model", alias = "model_id", alias = "modelId")]
    pub id: String,
    #[serde(default, alias = "displayName")]
    pub display_name: String,
    #[serde(
        default,
        alias = "remoteModel",
        skip_serializing_if = "Option::is_none"
    )]
    pub remote_model: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SettingsSchema {
    schema_version: u32,
    sections: Vec<SettingsSectionDefinition>,
    property_index: BTreeMap<String, SettingDefinition>,
}

#[derive(Debug, Clone)]
pub struct SettingsSectionDefinition {
    pub id: String,
    pub title: String,
    pub description_md: String,
    pub subsections: Vec<SettingsSubsectionDefinition>,
}

#[derive(Debug, Clone)]
pub struct SettingsSubsectionDefinition {
    pub id: String,
    pub title: String,
    pub description_md: String,
    pub properties: Vec<SettingDefinition>,
}

#[derive(Debug, Clone)]
pub struct SettingDefinition {
    pub pmid: String,
    pub label: String,
    pub description_md: String,
    pub editable: bool,
    pub search_terms: Vec<String>,
    pub schema: SettingPropertySchema,
    storage_kind: SettingStorageKind,
    validation_schema: Value,
    default_validation_schema: Value,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SettingStorageKind {
    Boolean,
    Integer,
    String,
    Path,
    Array,
    Object,
    ChatProviders,
}

#[derive(Debug, Clone, Deserialize)]
struct RawSettingsSchema {
    schema_version: u32,
    sections: Vec<RawSettingsSectionDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawSettingsSectionDefinition {
    id: String,
    title: String,
    #[serde(default)]
    description_md: String,
    #[serde(default)]
    order: u32,
    #[serde(default)]
    subsections: Vec<RawSettingsSubsectionDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawSettingsSubsectionDefinition {
    id: String,
    title: String,
    #[serde(default)]
    description_md: String,
    #[serde(default)]
    order: u32,
    #[serde(default)]
    properties: Vec<RawSettingDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawSettingDefinition {
    pmid: String,
    label: String,
    #[serde(default)]
    description_md: String,
    #[serde(default = "default_true")]
    editable: bool,
    #[serde(default)]
    search_terms: Vec<String>,
    #[serde(default = "default_storage_kind")]
    storage_kind: SettingStorageKind,
    schema: SettingPropertySchema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsValuesFile {
    pub version: u32,
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
}

fn default_true() -> bool {
    true
}

fn default_storage_kind() -> SettingStorageKind {
    SettingStorageKind::String
}

pub fn embedded_settings_schema() -> Result<SettingsSchema, ServerError> {
    SettingsSchema::from_json_str(include_str!(
        "../../../../manifests/settings/settings-schema.json"
    ))
}

impl SettingsSchema {
    pub fn from_json_str(raw: &str) -> Result<Self, ServerError> {
        let raw_document: Value = serde_json::from_str(raw).map_err(|error| {
            ServerError::Internal(format!("invalid embedded settings schema: {error}"))
        })?;
        validate_settings_schema_document(&raw_document)?;
        let parsed: RawSettingsSchema = serde_json::from_value(raw_document).map_err(|error| {
            ServerError::Internal(format!("invalid embedded settings schema: {error}"))
        })?;

        if parsed.sections.is_empty() {
            return Err(ServerError::Internal(
                "embedded settings schema must contain at least one section".into(),
            ));
        }

        let mut section_ids = BTreeSet::new();
        let mut property_index = BTreeMap::new();
        let mut sections = Vec::with_capacity(parsed.sections.len());
        let mut raw_sections = parsed.sections;
        raw_sections.sort_by_key(|section| section.order);

        for raw_section in raw_sections {
            if !section_ids.insert(raw_section.id.clone()) {
                return Err(ServerError::Internal(format!(
                    "duplicate settings section id '{}'",
                    raw_section.id
                )));
            }

            let mut subsection_ids = BTreeSet::new();
            let mut subsections = Vec::with_capacity(raw_section.subsections.len());
            let mut raw_subsections = raw_section.subsections;
            raw_subsections.sort_by_key(|subsection| subsection.order);

            for raw_subsection in raw_subsections {
                if !subsection_ids.insert(raw_subsection.id.clone()) {
                    return Err(ServerError::Internal(format!(
                        "duplicate settings subsection id '{}.{}'",
                        raw_section.id, raw_subsection.id
                    )));
                }

                let mut properties = Vec::with_capacity(raw_subsection.properties.len());
                let mut raw_properties = raw_subsection.properties;
                raw_properties.sort_by_key(|property| property.schema.order);

                for raw_property in raw_properties {
                    let definition = SettingDefinition::from_raw(raw_property)?;
                    if property_index.contains_key(&definition.pmid) {
                        return Err(ServerError::Internal(format!(
                            "duplicate settings pmid '{}'",
                            definition.pmid
                        )));
                    }
                    property_index.insert(definition.pmid.clone(), definition.clone());
                    properties.push(definition);
                }

                subsections.push(SettingsSubsectionDefinition {
                    id: raw_subsection.id,
                    title: raw_subsection.title,
                    description_md: raw_subsection.description_md,
                    properties,
                });
            }

            sections.push(SettingsSectionDefinition {
                id: raw_section.id,
                title: raw_section.title,
                description_md: raw_section.description_md,
                subsections,
            });
        }

        Ok(Self {
            schema_version: parsed.schema_version,
            sections,
            property_index,
        })
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn sections(&self) -> &[SettingsSectionDefinition] {
        &self.sections
    }

    pub fn property(&self, pmid: &str) -> Option<&SettingDefinition> {
        self.property_index.get(pmid)
    }
}

impl SettingDefinition {
    fn from_raw(raw: RawSettingDefinition) -> Result<Self, ServerError> {
        let mut definition = Self {
            pmid: raw.pmid.trim().to_owned(),
            label: raw.label.trim().to_owned(),
            description_md: raw.description_md.trim().to_owned(),
            editable: raw.editable,
            search_terms: raw.search_terms,
            schema: raw.schema,
            storage_kind: raw.storage_kind,
            validation_schema: Value::Null,
            default_validation_schema: Value::Null,
        };

        if definition.pmid.is_empty() {
            return Err(ServerError::Internal(
                "settings pmid must not be empty".into(),
            ));
        }
        if definition.label.is_empty() {
            return Err(ServerError::Internal(format!(
                "settings '{}' must define a label",
                definition.pmid
            )));
        }

        definition.validate_storage_shape()?;
        let (validation_schema, default_validation_schema) =
            definition.build_validation_schemas()?;
        definition.validation_schema = validation_schema;
        definition.default_validation_schema = default_validation_schema;
        definition.schema.default_value = definition.canonicalize_default_value()?;

        Ok(definition)
    }

    pub fn default_value(&self) -> &Value {
        &self.schema.default_value
    }

    pub fn build_view(&self, override_value: Option<&Value>) -> SettingPropertyView {
        let effective_value = override_value
            .cloned()
            .unwrap_or_else(|| self.schema.default_value.clone());

        SettingPropertyView {
            pmid: self.pmid.clone(),
            label: self.label.clone(),
            description_md: self.description_md.clone(),
            editable: self.editable,
            schema: self.schema.clone(),
            effective_value,
            override_value: override_value.cloned(),
            is_overridden: override_value.is_some(),
            search_terms: self.search_terms.clone(),
        }
    }

    pub fn canonicalize_update_command(
        &self,
        command: &UpdateSettingCommand,
    ) -> Result<Option<Value>, ServerError> {
        match command.op {
            UpdateSettingOperation::Unset => Ok(None),
            UpdateSettingOperation::Set => {
                let value = command.value.as_ref().ok_or_else(|| {
                    self.validation_error("/", "value is required when op is 'set'")
                })?;
                Ok(Some(self.canonicalize_runtime_value(value)?))
            }
        }
    }

    pub fn canonicalize_loaded_override(
        &self,
        value: &Value,
    ) -> Result<Option<Value>, ServerError> {
        let canonical = self.canonicalize_runtime_value(value)?;
        if canonical == *self.default_value() {
            Ok(None)
        } else {
            Ok(Some(canonical))
        }
    }

    fn validate_storage_shape(&self) -> Result<(), ServerError> {
        let expected_type = match self.storage_kind {
            SettingStorageKind::Boolean => SettingValueType::Boolean,
            SettingStorageKind::Integer => SettingValueType::Integer,
            SettingStorageKind::String | SettingStorageKind::Path => SettingValueType::String,
            SettingStorageKind::Array => SettingValueType::Array,
            SettingStorageKind::Object => SettingValueType::Object,
            SettingStorageKind::ChatProviders => SettingValueType::Array,
        };

        if self.schema.value_type != expected_type {
            return Err(ServerError::Internal(format!(
                "settings '{}' uses storage kind '{:?}' but schema type '{:?}'",
                self.pmid, self.storage_kind, self.schema.value_type
            )));
        }

        if self.schema.enum_values.is_some() && self.schema.value_type != SettingValueType::String {
            return Err(ServerError::Internal(format!(
                "settings '{}' only supports enum values for string properties",
                self.pmid
            )));
        }

        Ok(())
    }

    fn canonicalize_default_value(&self) -> Result<Value, ServerError> {
        if self.schema.default_value.is_null() {
            return match self.schema.value_type {
                SettingValueType::Integer => Ok(Value::Null),
                _ => Err(ServerError::Internal(format!(
                    "settings '{}' uses null default for a non-nullable property",
                    self.pmid
                ))),
            };
        }

        self.canonicalize_value(&self.schema.default_value, true)
            .map_err(|error| match error {
                ServerError::BadRequest(message) => ServerError::Internal(format!(
                    "settings '{}' has invalid default value: {message}",
                    self.pmid
                )),
                ServerError::BadRequestData { message, .. } => ServerError::Internal(format!(
                    "settings '{}' has invalid default value: {message}",
                    self.pmid
                )),
                other => other,
            })
    }

    fn canonicalize_runtime_value(&self, value: &Value) -> Result<Value, ServerError> {
        self.canonicalize_value(value, false)
    }

    fn canonicalize_value(
        &self,
        value: &Value,
        allow_null_default: bool,
    ) -> Result<Value, ServerError> {
        let schema = if allow_null_default {
            &self.default_validation_schema
        } else {
            &self.validation_schema
        };

        match self.storage_kind {
            SettingStorageKind::Boolean => {
                let canonical = canonicalize_bool_value(value)
                    .map_err(|message| self.validation_error("/", message))?;
                self.validate_json_value(schema, &canonical)?;
                Ok(canonical)
            }
            SettingStorageKind::Integer => {
                if allow_null_default && value.is_null() {
                    self.validate_json_value(schema, value)?;
                    return Ok(Value::Null);
                }
                let canonical = canonicalize_integer_value(value)
                    .map_err(|message| self.validation_error("/", message))?;
                self.validate_json_value(schema, &canonical)?;
                Ok(canonical)
            }
            SettingStorageKind::String | SettingStorageKind::Path => {
                let canonical = canonicalize_string_value(value)
                    .map_err(|message| self.validation_error("/", message))?;
                self.validate_json_value(schema, &canonical)?;
                Ok(canonical)
            }
            SettingStorageKind::Array | SettingStorageKind::Object => {
                self.validate_json_value(schema, value)?;
                Ok(value.clone())
            }
            SettingStorageKind::ChatProviders => {
                let providers = canonicalize_chat_providers_from_value(value)
                    .map_err(|message| self.validation_error("/", message))?;
                let canonical = serde_json::to_value(providers).map_err(|error| {
                    ServerError::Internal(format!("serialize settings value: {error}"))
                })?;
                self.validate_json_value(schema, &canonical)?;
                Ok(canonical)
            }
        }
    }

    fn build_validation_schemas(&self) -> Result<(Value, Value), ServerError> {
        let validation_schema = self.build_validation_schema(false);
        ensure_json_schema_is_valid(
            &validation_schema,
            &format!("setting '{}' runtime schema", self.pmid),
        )?;

        let default_validation_schema = self.build_validation_schema(true);
        ensure_json_schema_is_valid(
            &default_validation_schema,
            &format!("setting '{}' default schema", self.pmid),
        )?;

        Ok((validation_schema, default_validation_schema))
    }

    fn build_validation_schema(&self, allow_null_default: bool) -> Value {
        match self.storage_kind {
            SettingStorageKind::ChatProviders => chat_providers_validation_schema(),
            _ => {
                let mut schema = base_property_validation_schema(
                    self.schema.value_type,
                    allow_null_default && self.storage_kind == SettingStorageKind::Integer,
                );

                if let Some(enum_values) = &self.schema.enum_values {
                    schema.insert("enum".to_owned(), json!(enum_values));
                }
                if let Some(minimum) = self.schema.minimum {
                    schema.insert("minimum".to_owned(), json!(minimum));
                }
                if let Some(maximum) = self.schema.maximum {
                    schema.insert("maximum".to_owned(), json!(maximum));
                }
                if let Some(pattern) = &self.schema.pattern {
                    schema.insert("pattern".to_owned(), json!(pattern));
                }

                Value::Object(schema)
            }
        }
    }

    fn validate_json_value(&self, schema: &Value, value: &Value) -> Result<(), ServerError> {
        let validator = jsonschema::validator_for(schema).map_err(|error| {
            ServerError::Internal(format!(
                "failed to compile validation schema for '{}': {error}",
                self.pmid
            ))
        })?;

        if let Some(error) = validator.iter_errors(value).next() {
            return Err(self.validation_error(
                &normalize_json_pointer(error.instance_path().to_string()),
                error.to_string(),
            ));
        }

        Ok(())
    }

    fn validation_error(&self, path: &str, message: impl Into<String>) -> ServerError {
        let message = message.into();
        ServerError::BadRequestData {
            message: message.clone(),
            data: json!(SettingValidationErrorData {
                error_type: "setting_validation".to_owned(),
                pmid: self.pmid.clone(),
                path: path.to_owned(),
                message,
            }),
        }
    }
}

fn canonicalize_bool_value(value: &Value) -> Result<Value, &'static str> {
    match value {
        Value::Bool(parsed) => Ok(Value::Bool(*parsed)),
        _ => Err("value must be a boolean"),
    }
}

fn canonicalize_integer_value(value: &Value) -> Result<Value, &'static str> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .map(|parsed| json!(parsed))
            .ok_or("value must be an integer"),
        _ => Err("value must be an integer"),
    }
}

fn canonicalize_string_value(value: &Value) -> Result<Value, &'static str> {
    match value {
        Value::String(raw) => Ok(Value::String(raw.trim().to_owned())),
        _ => Err("value must be a string"),
    }
}

fn normalize_optional_text(raw: Option<&str>) -> Option<String> {
    raw.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

pub fn canonicalize_chat_providers_from_value(
    value: &Value,
) -> Result<Vec<CloudProviderSettingValue>, String> {
    if value.is_null() {
        return Ok(Vec::new());
    }

    let providers: Vec<CloudProviderSettingValue> = serde_json::from_value(value.clone())
        .map_err(|error| format!("value has invalid provider payload: {error}"))?;
    canonicalize_chat_providers(providers)
}

fn canonicalize_chat_providers(
    providers: Vec<CloudProviderSettingValue>,
) -> Result<Vec<CloudProviderSettingValue>, String> {
    let mut out = Vec::with_capacity(providers.len());
    let mut provider_ids = BTreeSet::new();

    for provider in providers {
        let canonical = canonicalize_chat_provider(provider)?;
        if !provider_ids.insert(canonical.id.clone()) {
            return Err(format!("duplicate cloud provider id '{}'", canonical.id));
        }
        out.push(canonical);
    }

    Ok(out)
}

fn canonicalize_chat_provider(
    mut provider: CloudProviderSettingValue,
) -> Result<CloudProviderSettingValue, String> {
    provider.id = provider.id.trim().to_owned();
    provider.name = provider.name.trim().to_owned();
    provider.api_base = provider.api_base.trim().trim_end_matches('/').to_owned();
    provider.api_key = normalize_optional_text(provider.api_key.as_deref());
    provider.api_key_env = normalize_optional_text(provider.api_key_env.as_deref());

    if provider.id.is_empty() {
        return Err("cloud provider id must not be empty".into());
    }
    if provider.name.is_empty() {
        provider.name = provider.id.clone();
    }
    if provider.api_base.is_empty() {
        return Err(format!(
            "cloud provider '{}' has empty api_base",
            provider.id
        ));
    }
    if provider.models.is_empty() {
        return Err(format!(
            "cloud provider '{}' must define at least one model",
            provider.id
        ));
    }

    let mut model_ids = BTreeSet::new();
    for model in &mut provider.models {
        model.id = model.id.trim().to_owned();
        model.display_name = model.display_name.trim().to_owned();
        model.remote_model = normalize_optional_text(model.remote_model.as_deref());

        if model.id.is_empty() {
            return Err(format!(
                "cloud provider '{}' contains model with empty id",
                provider.id
            ));
        }
        if model.display_name.is_empty() {
            model.display_name = model.id.clone();
        }
        if !model_ids.insert(model.id.clone()) {
            return Err(format!(
                "cloud provider '{}' contains duplicate model id '{}'",
                provider.id, model.id
            ));
        }
    }

    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_rejects_duplicate_pmids() {
        let raw = r#"{
          "schema_version": 1,
          "sections": [
            {
              "id": "runtime",
              "title": "Runtime",
              "subsections": [
                {
                  "id": "general",
                  "title": "General",
                  "properties": [
                    {
                      "pmid": "runtime.foo",
                      "label": "Foo",
                      "storage_kind": "string",
                      "schema": { "type": "string", "default_value": "" }
                    },
                    {
                      "pmid": "runtime.foo",
                      "label": "Foo 2",
                      "storage_kind": "string",
                      "schema": { "type": "string", "default_value": "" }
                    }
                  ]
                }
              ]
            }
          ]
        }"#;

        let error = SettingsSchema::from_json_str(raw).expect_err("duplicate pmid");
        assert!(error.to_string().contains("duplicate settings pmid"));
    }

    #[test]
    fn integer_default_can_be_null() {
        let schema = embedded_settings_schema().expect("schema");
        let definition = schema
            .property(LLAMA_CONTEXT_LENGTH_PMID)
            .expect("context length");

        assert!(definition.default_value().is_null());
    }

    #[test]
    fn schema_rejects_invalid_default_value_shape() {
        let raw = r#"{
          "schema_version": 1,
          "sections": [
            {
              "id": "runtime",
              "title": "Runtime",
              "subsections": [
                {
                  "id": "general",
                  "title": "General",
                  "properties": [
                    {
                      "pmid": "runtime.flag",
                      "label": "Flag",
                      "storage_kind": "boolean",
                      "schema": { "type": "boolean", "default_value": "nope" }
                    }
                  ]
                }
              ]
            }
          ]
        }"#;

        let error = SettingsSchema::from_json_str(raw).expect_err("invalid default");
        assert!(error.to_string().contains("invalid default value"));
    }

    #[test]
    fn chat_provider_payload_is_canonicalized() {
        let providers = canonicalize_chat_providers_from_value(&json!([
            {
                "id": " openai-main ",
                "name": "",
                "api_base": "https://api.openai.com/v1/",
                "models": [{ "id": "gpt-4.1-mini", "display_name": "" }]
            }
        ]))
        .expect("providers");

        assert_eq!(providers[0].id, "openai-main");
        assert_eq!(providers[0].name, "openai-main");
        assert_eq!(providers[0].api_base, "https://api.openai.com/v1");
        assert_eq!(providers[0].models[0].display_name, "gpt-4.1-mini");
    }
}
