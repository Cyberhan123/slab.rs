use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::error::ServerError;

pub const MODEL_CACHE_DIR_SETTING_KEY: &str = "model_cache_dir";
pub const LLAMA_NUM_WORKERS_SETTING_KEY: &str = "llama_num_workers";
pub const WHISPER_NUM_WORKERS_SETTING_KEY: &str = "whisper_num_workers";
pub const DIFFUSION_NUM_WORKERS_SETTING_KEY: &str = "diffusion_num_workers";
pub const LLAMA_CONTEXT_LENGTH_SETTING_KEY: &str = "llama_context_length";
pub const MODEL_AUTO_UNLOAD_ENABLED_SETTING_KEY: &str = "model_auto_unload_enabled";
pub const MODEL_AUTO_UNLOAD_IDLE_MINUTES_SETTING_KEY: &str = "model_auto_unload_idle_minutes";
pub const CHAT_MODEL_PROVIDERS_SETTING_KEY: &str = "chat_model_providers";
pub const DIFFUSION_MODEL_PATH_SETTING_KEY: &str = "diffusion_model_path";
pub const DIFFUSION_VAE_PATH_SETTING_KEY: &str = "diffusion_vae_path";
pub const DIFFUSION_TAESD_PATH_SETTING_KEY: &str = "diffusion_taesd_path";
pub const DIFFUSION_LORA_MODEL_DIR_SETTING_KEY: &str = "diffusion_lora_model_dir";
pub const DIFFUSION_CLIP_L_PATH_SETTING_KEY: &str = "diffusion_clip_l_path";
pub const DIFFUSION_CLIP_G_PATH_SETTING_KEY: &str = "diffusion_clip_g_path";
pub const DIFFUSION_T5XXL_PATH_SETTING_KEY: &str = "diffusion_t5xxl_path";
pub const DIFFUSION_FLASH_ATTN_SETTING_KEY: &str = "diffusion_flash_attn";
pub const DIFFUSION_KEEP_VAE_ON_CPU_SETTING_KEY: &str = "diffusion_keep_vae_on_cpu";
pub const DIFFUSION_KEEP_CLIP_ON_CPU_SETTING_KEY: &str = "diffusion_keep_clip_on_cpu";
pub const DIFFUSION_OFFLOAD_PARAMS_SETTING_KEY: &str = "diffusion_offload_params_to_cpu";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SettingCategory {
    Runtime,
    ChatProviders,
    Diffusion,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SettingControl {
    Toggle,
    Number,
    Text,
    Path,
    Json,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, Default)]
pub struct SettingValidation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<i64>,
    #[serde(default)]
    pub multiline: bool,
    #[serde(default)]
    pub allow_empty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingView {
    pub key: String,
    pub category: SettingCategory,
    pub label: String,
    pub description: String,
    pub control: SettingControl,
    pub editable: bool,
    pub value: Value,
    pub effective_value: Value,
    pub default_value: Value,
    pub search_terms: Vec<String>,
    pub validation: SettingValidation,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingsSystemBackendView {
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
pub struct SettingsSystemView {
    pub bind_address: String,
    pub transport_mode: String,
    pub swagger_enabled: bool,
    pub admin_token_enabled: bool,
    pub cors_configured: bool,
    pub session_state_dir: String,
    pub backends: Vec<SettingsSystemBackendView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateSettingCommand {
    pub value: Value,
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
    #[serde(default, alias = "remoteModel", skip_serializing_if = "Option::is_none")]
    pub remote_model: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct SettingDefinition {
    pub key: &'static str,
    pub category: SettingCategory,
    pub label: &'static str,
    pub description: &'static str,
    pub control: SettingControl,
    pub editable: bool,
    pub validation: SettingValidation,
    pub search_terms: &'static [&'static str],
    value_kind: SettingValueKind,
    default_kind: SettingDefaultKind,
}

#[derive(Debug, Clone, Copy)]
enum SettingValueKind {
    Bool,
    Integer {
        min: Option<i64>,
        max: Option<i64>,
        empty_is_null: bool,
    },
    Path,
    ChatProviders,
}

#[derive(Debug, Clone, Copy)]
enum SettingDefaultKind {
    Bool(bool),
    Integer(Option<i64>),
    Text(&'static str),
    ChatProviders,
}

const SETTINGS_REGISTRY: [SettingDefinition; 19] = [
    SettingDefinition {
        key: MODEL_CACHE_DIR_SETTING_KEY,
        category: SettingCategory::Runtime,
        label: "Model Cache Directory",
        description: "Directory used for model downloads. Leave empty to use hf-hub defaults.",
        control: SettingControl::Path,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: false,
            allow_empty: true,
        },
        search_terms: &["cache", "model", "download", "storage"],
        value_kind: SettingValueKind::Path,
        default_kind: SettingDefaultKind::Text(""),
    },
    SettingDefinition {
        key: LLAMA_NUM_WORKERS_SETTING_KEY,
        category: SettingCategory::Runtime,
        label: "Llama Workers",
        description: "Worker count for ggml.llama runtime jobs.",
        control: SettingControl::Number,
        editable: true,
        validation: SettingValidation {
            min: Some(1),
            max: None,
            step: Some(1),
            multiline: false,
            allow_empty: true,
        },
        search_terms: &["llama", "workers", "runtime", "parallel"],
        value_kind: SettingValueKind::Integer {
            min: Some(1),
            max: None,
            empty_is_null: true,
        },
        default_kind: SettingDefaultKind::Integer(Some(1)),
    },
    SettingDefinition {
        key: WHISPER_NUM_WORKERS_SETTING_KEY,
        category: SettingCategory::Runtime,
        label: "Whisper Workers",
        description: "Worker count for ggml.whisper runtime jobs.",
        control: SettingControl::Number,
        editable: true,
        validation: SettingValidation {
            min: Some(1),
            max: None,
            step: Some(1),
            multiline: false,
            allow_empty: true,
        },
        search_terms: &["whisper", "workers", "audio", "runtime"],
        value_kind: SettingValueKind::Integer {
            min: Some(1),
            max: None,
            empty_is_null: true,
        },
        default_kind: SettingDefaultKind::Integer(Some(1)),
    },
    SettingDefinition {
        key: DIFFUSION_NUM_WORKERS_SETTING_KEY,
        category: SettingCategory::Runtime,
        label: "Diffusion Workers",
        description: "Worker count for ggml.diffusion runtime jobs.",
        control: SettingControl::Number,
        editable: true,
        validation: SettingValidation {
            min: Some(1),
            max: None,
            step: Some(1),
            multiline: false,
            allow_empty: true,
        },
        search_terms: &["diffusion", "workers", "image", "runtime"],
        value_kind: SettingValueKind::Integer {
            min: Some(1),
            max: None,
            empty_is_null: true,
        },
        default_kind: SettingDefaultKind::Integer(Some(1)),
    },
    SettingDefinition {
        key: LLAMA_CONTEXT_LENGTH_SETTING_KEY,
        category: SettingCategory::Runtime,
        label: "Llama Context Length",
        description: "Override llama context length. Leave empty to use backend default.",
        control: SettingControl::Number,
        editable: true,
        validation: SettingValidation {
            min: Some(1),
            max: None,
            step: Some(1),
            multiline: false,
            allow_empty: true,
        },
        search_terms: &["llama", "context", "tokens", "runtime"],
        value_kind: SettingValueKind::Integer {
            min: Some(1),
            max: None,
            empty_is_null: true,
        },
        default_kind: SettingDefaultKind::Integer(None),
    },
    SettingDefinition {
        key: MODEL_AUTO_UNLOAD_ENABLED_SETTING_KEY,
        category: SettingCategory::Runtime,
        label: "Model Auto Unload Enabled",
        description: "Unload idle models automatically to free memory.",
        control: SettingControl::Toggle,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: false,
            allow_empty: false,
        },
        search_terms: &["auto unload", "idle", "memory", "runtime"],
        value_kind: SettingValueKind::Bool,
        default_kind: SettingDefaultKind::Bool(false),
    },
    SettingDefinition {
        key: MODEL_AUTO_UNLOAD_IDLE_MINUTES_SETTING_KEY,
        category: SettingCategory::Runtime,
        label: "Model Auto Unload Idle Minutes",
        description: "Idle timeout in minutes before an auto unload runs.",
        control: SettingControl::Number,
        editable: true,
        validation: SettingValidation {
            min: Some(1),
            max: None,
            step: Some(1),
            multiline: false,
            allow_empty: false,
        },
        search_terms: &["auto unload", "idle", "minutes", "runtime"],
        value_kind: SettingValueKind::Integer {
            min: Some(1),
            max: None,
            empty_is_null: false,
        },
        default_kind: SettingDefaultKind::Integer(Some(10)),
    },
    SettingDefinition {
        key: CHAT_MODEL_PROVIDERS_SETTING_KEY,
        category: SettingCategory::ChatProviders,
        label: "Chat Model Providers",
        description: "OpenAI-compatible cloud providers used by chat model selection.",
        control: SettingControl::Json,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: true,
            allow_empty: true,
        },
        search_terms: &["chat", "cloud", "provider", "api", "openai"],
        value_kind: SettingValueKind::ChatProviders,
        default_kind: SettingDefaultKind::ChatProviders,
    },
    SettingDefinition {
        key: DIFFUSION_MODEL_PATH_SETTING_KEY,
        category: SettingCategory::Diffusion,
        label: "Diffusion Model Path",
        description: "Default diffusion model path used during model load operations.",
        control: SettingControl::Path,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: false,
            allow_empty: true,
        },
        search_terms: &["diffusion", "model", "path"],
        value_kind: SettingValueKind::Path,
        default_kind: SettingDefaultKind::Text(""),
    },
    SettingDefinition {
        key: DIFFUSION_VAE_PATH_SETTING_KEY,
        category: SettingCategory::Diffusion,
        label: "Diffusion VAE Path",
        description: "Optional external VAE model path.",
        control: SettingControl::Path,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: false,
            allow_empty: true,
        },
        search_terms: &["diffusion", "vae", "path"],
        value_kind: SettingValueKind::Path,
        default_kind: SettingDefaultKind::Text(""),
    },
    SettingDefinition {
        key: DIFFUSION_TAESD_PATH_SETTING_KEY,
        category: SettingCategory::Diffusion,
        label: "Diffusion TAESD Path",
        description: "Tiny autoencoder path for faster decode.",
        control: SettingControl::Path,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: false,
            allow_empty: true,
        },
        search_terms: &["diffusion", "taesd", "path"],
        value_kind: SettingValueKind::Path,
        default_kind: SettingDefaultKind::Text(""),
    },
    SettingDefinition {
        key: DIFFUSION_LORA_MODEL_DIR_SETTING_KEY,
        category: SettingCategory::Diffusion,
        label: "Diffusion LoRA Model Directory",
        description: "Directory containing LoRA safetensors files.",
        control: SettingControl::Path,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: false,
            allow_empty: true,
        },
        search_terms: &["diffusion", "lora", "directory", "path"],
        value_kind: SettingValueKind::Path,
        default_kind: SettingDefaultKind::Text(""),
    },
    SettingDefinition {
        key: DIFFUSION_CLIP_L_PATH_SETTING_KEY,
        category: SettingCategory::Diffusion,
        label: "Diffusion CLIP-L Path",
        description: "Optional CLIP-L path.",
        control: SettingControl::Path,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: false,
            allow_empty: true,
        },
        search_terms: &["diffusion", "clip", "path", "clip-l"],
        value_kind: SettingValueKind::Path,
        default_kind: SettingDefaultKind::Text(""),
    },
    SettingDefinition {
        key: DIFFUSION_CLIP_G_PATH_SETTING_KEY,
        category: SettingCategory::Diffusion,
        label: "Diffusion CLIP-G Path",
        description: "Optional CLIP-G path.",
        control: SettingControl::Path,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: false,
            allow_empty: true,
        },
        search_terms: &["diffusion", "clip", "path", "clip-g"],
        value_kind: SettingValueKind::Path,
        default_kind: SettingDefaultKind::Text(""),
    },
    SettingDefinition {
        key: DIFFUSION_T5XXL_PATH_SETTING_KEY,
        category: SettingCategory::Diffusion,
        label: "Diffusion T5XXL Path",
        description: "Optional T5XXL encoder path.",
        control: SettingControl::Path,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: false,
            allow_empty: true,
        },
        search_terms: &["diffusion", "t5xxl", "path", "encoder"],
        value_kind: SettingValueKind::Path,
        default_kind: SettingDefaultKind::Text(""),
    },
    SettingDefinition {
        key: DIFFUSION_FLASH_ATTN_SETTING_KEY,
        category: SettingCategory::Diffusion,
        label: "Diffusion Flash Attention",
        description: "Enable flash attention when supported by the backend.",
        control: SettingControl::Toggle,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: false,
            allow_empty: false,
        },
        search_terms: &["diffusion", "flash attention", "performance"],
        value_kind: SettingValueKind::Bool,
        default_kind: SettingDefaultKind::Bool(false),
    },
    SettingDefinition {
        key: DIFFUSION_KEEP_VAE_ON_CPU_SETTING_KEY,
        category: SettingCategory::Diffusion,
        label: "Diffusion Keep VAE On CPU",
        description: "Reduce VRAM usage by keeping VAE on CPU.",
        control: SettingControl::Toggle,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: false,
            allow_empty: false,
        },
        search_terms: &["diffusion", "vae", "cpu", "memory"],
        value_kind: SettingValueKind::Bool,
        default_kind: SettingDefaultKind::Bool(false),
    },
    SettingDefinition {
        key: DIFFUSION_KEEP_CLIP_ON_CPU_SETTING_KEY,
        category: SettingCategory::Diffusion,
        label: "Diffusion Keep CLIP On CPU",
        description: "Reduce VRAM usage by keeping CLIP on CPU.",
        control: SettingControl::Toggle,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: false,
            allow_empty: false,
        },
        search_terms: &["diffusion", "clip", "cpu", "memory"],
        value_kind: SettingValueKind::Bool,
        default_kind: SettingDefaultKind::Bool(false),
    },
    SettingDefinition {
        key: DIFFUSION_OFFLOAD_PARAMS_SETTING_KEY,
        category: SettingCategory::Diffusion,
        label: "Diffusion Offload Params To CPU",
        description: "Reduce VRAM usage by offloading parameters to CPU memory.",
        control: SettingControl::Toggle,
        editable: true,
        validation: SettingValidation {
            min: None,
            max: None,
            step: None,
            multiline: false,
            allow_empty: false,
        },
        search_terms: &["diffusion", "offload", "cpu", "memory"],
        value_kind: SettingValueKind::Bool,
        default_kind: SettingDefaultKind::Bool(false),
    },
];

pub fn setting_definitions() -> &'static [SettingDefinition] {
    &SETTINGS_REGISTRY
}

pub fn setting_definition(key: &str) -> Option<&'static SettingDefinition> {
    SETTINGS_REGISTRY.iter().find(|definition| definition.key == key)
}

impl SettingDefinition {
    pub fn default_value(&self) -> Value {
        match self.default_kind {
            SettingDefaultKind::Bool(value) => Value::Bool(value),
            SettingDefaultKind::Integer(Some(value)) => json!(value),
            SettingDefaultKind::Integer(None) => Value::Null,
            SettingDefaultKind::Text(value) => Value::String(value.to_owned()),
            SettingDefaultKind::ChatProviders => Value::Array(Vec::new()),
        }
    }

    pub fn stored_value_from_raw(&self, raw: Option<&str>) -> Result<Value, ServerError> {
        match self.value_kind {
            SettingValueKind::Bool => match normalize_optional_text(raw) {
                Some(value) => Ok(Value::Bool(parse_bool_like(&value)?)),
                None => Ok(Value::Null),
            },
            SettingValueKind::Integer {
                min,
                max,
                empty_is_null,
            } => match normalize_optional_text(raw) {
                Some(value) => Ok(json!(parse_integer_like(self.key, &value, min, max)?)),
                None if empty_is_null => Ok(Value::Null),
                None => Ok(self.default_value()),
            },
            SettingValueKind::Path => Ok(Value::String(
                raw.map(str::trim).unwrap_or_default().to_owned(),
            )),
            SettingValueKind::ChatProviders => {
                let providers = parse_chat_providers_from_raw(raw)?;
                serde_json::to_value(providers).map_err(|error| {
                    ServerError::Internal(format!("serialize settings value: {error}"))
                })
            }
        }
    }

    pub fn effective_value_from_raw(&self, raw: Option<&str>) -> Result<Value, ServerError> {
        let value = self.stored_value_from_raw(raw)?;
        if value.is_null() {
            Ok(self.default_value())
        } else {
            Ok(value)
        }
    }

    pub fn normalized_raw_from_value(&self, value: &Value) -> Result<String, ServerError> {
        match self.value_kind {
            SettingValueKind::Bool => parse_bool_from_value(value).map(|parsed| parsed.to_string()),
            SettingValueKind::Integer {
                min,
                max,
                empty_is_null,
            } => parse_integer_from_value(self.key, value, min, max, empty_is_null)
                .map(|parsed| parsed.map(|number| number.to_string()).unwrap_or_default()),
            SettingValueKind::Path => parse_string_from_value(value, false),
            SettingValueKind::ChatProviders => normalize_chat_providers_from_value(value),
        }
    }

    pub fn normalized_raw_from_legacy_input(&self, raw: &str) -> Result<String, ServerError> {
        match self.value_kind {
            SettingValueKind::Bool => parse_bool_like(raw).map(|parsed| parsed.to_string()),
            SettingValueKind::Integer {
                min,
                max,
                empty_is_null,
            } => {
                let normalized = normalize_optional_text(Some(raw));
                match normalized {
                    Some(value) => {
                        parse_integer_like(self.key, &value, min, max).map(|parsed| parsed.to_string())
                    }
                    None if empty_is_null => Ok(String::new()),
                    None => Ok(self.default_value().to_string()),
                }
            }
            SettingValueKind::Path => Ok(raw.trim().to_owned()),
            SettingValueKind::ChatProviders => {
                let providers = parse_chat_providers_from_raw(Some(raw))?;
                serde_json::to_string(&providers).map_err(|error| {
                    ServerError::Internal(format!("serialize settings value: {error}"))
                })
            }
        }
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

fn parse_bool_like(raw: &str) -> Result<bool, ServerError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(ServerError::BadRequest(format!(
            "setting value '{raw}' is not a valid boolean"
        ))),
    }
}

fn parse_bool_from_value(value: &Value) -> Result<bool, ServerError> {
    match value {
        Value::Bool(value) => Ok(*value),
        Value::String(value) => parse_bool_like(value),
        _ => Err(ServerError::BadRequest(
            "setting value must be a boolean".into(),
        )),
    }
}

fn parse_integer_like(
    key: &str,
    raw: &str,
    min: Option<i64>,
    max: Option<i64>,
) -> Result<i64, ServerError> {
    let parsed = raw.parse::<i64>().map_err(|_| {
        ServerError::BadRequest(format!("setting '{key}' must be an integer value"))
    })?;
    validate_integer_bounds(key, parsed, min, max)?;
    Ok(parsed)
}

fn parse_integer_from_value(
    key: &str,
    value: &Value,
    min: Option<i64>,
    max: Option<i64>,
    empty_is_null: bool,
) -> Result<Option<i64>, ServerError> {
    match value {
        Value::Null if empty_is_null => Ok(None),
        Value::Number(number) => {
            let parsed = number.as_i64().ok_or_else(|| {
                ServerError::BadRequest(format!("setting '{key}' must be an integer value"))
            })?;
            validate_integer_bounds(key, parsed, min, max)?;
            Ok(Some(parsed))
        }
        Value::String(raw) => {
            let normalized = normalize_optional_text(Some(raw));
            match normalized {
                Some(raw) => parse_integer_like(key, &raw, min, max).map(Some),
                None if empty_is_null => Ok(None),
                None => Err(ServerError::BadRequest(format!(
                    "setting '{key}' must not be empty"
                ))),
            }
        }
        _ => Err(ServerError::BadRequest(format!(
            "setting '{key}' must be an integer value"
        ))),
    }
}

fn validate_integer_bounds(
    key: &str,
    value: i64,
    min: Option<i64>,
    max: Option<i64>,
) -> Result<(), ServerError> {
    if let Some(min) = min {
        if value < min {
            return Err(ServerError::BadRequest(format!(
                "setting '{key}' must be at least {min}"
            )));
        }
    }
    if let Some(max) = max {
        if value > max {
            return Err(ServerError::BadRequest(format!(
                "setting '{key}' must be at most {max}"
            )));
        }
    }
    Ok(())
}

fn parse_string_from_value(value: &Value, empty_is_null: bool) -> Result<String, ServerError> {
    match value {
        Value::Null if empty_is_null => Ok(String::new()),
        Value::String(value) => Ok(value.trim().to_owned()),
        _ => Err(ServerError::BadRequest(
            "setting value must be a string".into(),
        )),
    }
}

fn parse_chat_providers_from_raw(
    raw: Option<&str>,
) -> Result<Vec<CloudProviderSettingValue>, ServerError> {
    let normalized = normalize_optional_text(raw);
    let Some(raw) = normalized else {
        return Ok(Vec::new());
    };

    let providers: Vec<CloudProviderSettingValue> =
        serde_json::from_str(&raw).map_err(|error| {
            ServerError::BadRequest(format!(
                "setting '{CHAT_MODEL_PROVIDERS_SETTING_KEY}' contains invalid JSON: {error}"
            ))
        })?;
    canonicalize_chat_providers(providers)
}

fn normalize_chat_providers_from_value(value: &Value) -> Result<String, ServerError> {
    if value.is_null() {
        return Ok("[]".to_owned());
    }

    let providers: Vec<CloudProviderSettingValue> = serde_json::from_value(value.clone())
        .map_err(|error| {
            ServerError::BadRequest(format!(
                "setting '{CHAT_MODEL_PROVIDERS_SETTING_KEY}' has invalid provider payload: {error}"
            ))
        })?;
    let canonical = canonicalize_chat_providers(providers)?;
    serde_json::to_string(&canonical)
        .map_err(|error| ServerError::Internal(format!("serialize chat providers setting: {error}")))
}

fn canonicalize_chat_providers(
    providers: Vec<CloudProviderSettingValue>,
) -> Result<Vec<CloudProviderSettingValue>, ServerError> {
    let mut out = Vec::with_capacity(providers.len());
    let mut provider_ids = std::collections::HashSet::new();

    for provider in providers {
        let canonical = canonicalize_chat_provider(provider)?;
        if !provider_ids.insert(canonical.id.clone()) {
            return Err(ServerError::BadRequest(format!(
                "duplicate cloud provider id '{}'",
                canonical.id
            )));
        }
        out.push(canonical);
    }

    Ok(out)
}

fn canonicalize_chat_provider(
    mut provider: CloudProviderSettingValue,
) -> Result<CloudProviderSettingValue, ServerError> {
    provider.id = provider.id.trim().to_owned();
    provider.name = provider.name.trim().to_owned();
    provider.api_base = provider.api_base.trim().trim_end_matches('/').to_owned();
    provider.api_key = normalize_optional_text(provider.api_key.as_deref());
    provider.api_key_env = normalize_optional_text(provider.api_key_env.as_deref());

    if provider.id.is_empty() {
        return Err(ServerError::BadRequest(
            "cloud provider id must not be empty".into(),
        ));
    }
    if provider.name.is_empty() {
        provider.name = provider.id.clone();
    }
    if provider.api_base.is_empty() {
        return Err(ServerError::BadRequest(format!(
            "cloud provider '{}' has empty api_base",
            provider.id
        )));
    }
    if provider.models.is_empty() {
        return Err(ServerError::BadRequest(format!(
            "cloud provider '{}' must define at least one model",
            provider.id
        )));
    }

    let mut model_ids = std::collections::HashSet::new();
    for model in &mut provider.models {
        model.id = model.id.trim().to_owned();
        model.display_name = model.display_name.trim().to_owned();
        model.remote_model = normalize_optional_text(model.remote_model.as_deref());

        if model.id.is_empty() {
            return Err(ServerError::BadRequest(format!(
                "cloud provider '{}' contains model with empty id",
                provider.id
            )));
        }
        if model.display_name.is_empty() {
            model.display_name = model.id.clone();
        }
        if !model_ids.insert(model.id.clone()) {
            return Err(ServerError::BadRequest(format!(
                "cloud provider '{}' contains duplicate model id '{}'",
                provider.id, model.id
            )));
        }
    }

    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workers_use_default_when_empty() {
        let definition = setting_definition(LLAMA_NUM_WORKERS_SETTING_KEY).expect("definition");
        assert_eq!(definition.stored_value_from_raw(Some("")).unwrap(), Value::Null);
        assert_eq!(definition.effective_value_from_raw(Some("")).unwrap(), json!(1));
    }

    #[test]
    fn invalid_integer_is_rejected() {
        let definition = setting_definition(MODEL_AUTO_UNLOAD_IDLE_MINUTES_SETTING_KEY)
            .expect("definition");
        let error = definition
            .normalized_raw_from_value(&json!(0))
            .expect_err("validation error");
        assert!(matches!(error, ServerError::BadRequest(_)));
    }

    #[test]
    fn chat_provider_payload_is_canonicalized() {
        let definition = setting_definition(CHAT_MODEL_PROVIDERS_SETTING_KEY).expect("definition");
        let raw = definition
            .normalized_raw_from_value(&json!([
                {
                    "id": " openai-main ",
                    "name": "",
                    "api_base": "https://api.openai.com/v1/",
                    "models": [{ "id": "gpt-4.1-mini", "display_name": "" }]
                }
            ]))
            .expect("normalized");
        assert_eq!(
            raw,
            r#"[{"id":"openai-main","name":"openai-main","api_base":"https://api.openai.com/v1","models":[{"id":"gpt-4.1-mini","display_name":"gpt-4.1-mini"}]}]"#
        );
    }
}
