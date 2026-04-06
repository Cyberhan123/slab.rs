use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use utoipa::ToSchema;

use super::settings_jsonschema::{
    base_property_validation_schema, chat_providers_validation_schema, ensure_json_schema_is_valid,
    normalize_json_pointer, validate_settings_schema_document,
};
use crate::error::AppCoreError;

// ── Setup / first-run PMIDs ──────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SettingValueType {
    Boolean,
    Integer,
    #[default]
    String,
    Array,
    Object,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<Value>,
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

use slab_types::settings::{CloudProviderConfig, PMID};

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

fn legacy_settings_schema() -> RawSettingsSchema {
    RawSettingsSchema {
        schema_version: 1,
        sections: vec![
            legacy_section(
                "setup",
                "Setup & Downloads",
                "First-time setup settings: whether initialization is complete, and configurable download locations and versions for FFmpeg and AI backends.",
                5,
                vec![
                    legacy_subsection(
                        "general",
                        "General",
                        "Global setup state.",
                        10,
                        vec![legacy_bool_property(
                            PMID.setup.initialized().into_string(),
                            "Setup Initialized",
                            false,
                            10,
                        )],
                    ),
                    legacy_subsection(
                        "ffmpeg",
                        "FFmpeg",
                        "Settings for automatic FFmpeg download.",
                        20,
                        vec![
                            legacy_bool_property(
                                PMID.setup.ffmpeg.auto_download().into_string(),
                                "FFmpeg Auto-Download",
                                true,
                                10,
                            ),
                            legacy_path_property(
                                PMID.setup.ffmpeg.dir().into_string(),
                                "FFmpeg Install Directory",
                                20,
                            ),
                        ],
                    ),
                    legacy_subsection(
                        "backends",
                        "AI Backend Libraries",
                        "Settings for automatic download of GGML backend libraries and bundled Candle/ONNX backend packages.",
                        30,
                        vec![
                            legacy_path_property(
                                PMID.setup.backends.dir().into_string(),
                                "Backend Library Directory",
                                10,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.ggml_llama.tag().into_string(),
                                "GGML Llama Release Tag",
                                "",
                                20,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.ggml_llama.asset().into_string(),
                                "GGML Llama Asset Name",
                                "",
                                30,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.ggml_whisper.tag().into_string(),
                                "GGML Whisper Release Tag",
                                "",
                                40,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.ggml_whisper.asset().into_string(),
                                "GGML Whisper Asset Name",
                                "",
                                50,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.ggml_diffusion.tag().into_string(),
                                "GGML Diffusion Release Tag",
                                "",
                                60,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.ggml_diffusion.asset().into_string(),
                                "GGML Diffusion Asset Name",
                                "",
                                70,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.candle_llama.tag().into_string(),
                                "Candle Llama Release Tag",
                                "",
                                80,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.candle_llama.asset().into_string(),
                                "Candle Llama Asset Name",
                                "",
                                90,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.candle_whisper.tag().into_string(),
                                "Candle Whisper Release Tag",
                                "",
                                100,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.candle_whisper.asset().into_string(),
                                "Candle Whisper Asset Name",
                                "",
                                110,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.candle_diffusion.tag().into_string(),
                                "Candle Diffusion Release Tag",
                                "",
                                120,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.candle_diffusion.asset().into_string(),
                                "Candle Diffusion Asset Name",
                                "",
                                130,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.onnx.tag().into_string(),
                                "ONNX Release Tag",
                                "",
                                140,
                            ),
                            legacy_string_property(
                                PMID.setup.backends.onnx.asset().into_string(),
                                "ONNX Asset Name",
                                "",
                                150,
                            ),
                        ],
                    ),
                ],
            ),
            legacy_section(
                "runtime",
                "Runtime",
                "Core runtime behavior, worker sizing, and automatic unload settings.",
                10,
                vec![
                    legacy_subsection(
                        "general",
                        "General",
                        "Global runtime paths and cache directories.",
                        10,
                        vec![legacy_path_property(
                            PMID.runtime.model_cache_dir().into_string(),
                            "Model Cache Directory",
                            10,
                        )],
                    ),
                    legacy_subsection(
                        "llama",
                        "Llama",
                        "Settings for the ggml.llama runtime.",
                        20,
                        vec![
                            legacy_integer_property(
                                PMID.runtime.llama.num_workers().into_string(),
                                "Llama Workers",
                                json!(1),
                                Some(1),
                                None,
                                10,
                            ),
                            legacy_integer_property(
                                PMID.runtime.llama.context_length().into_string(),
                                "Llama Context Length",
                                Value::Null,
                                Some(1),
                                None,
                                20,
                            ),
                        ],
                    ),
                    legacy_subsection(
                        "whisper",
                        "Whisper",
                        "Settings for the ggml.whisper runtime.",
                        30,
                        vec![legacy_integer_property(
                            PMID.runtime.whisper.num_workers().into_string(),
                            "Whisper Workers",
                            json!(1),
                            Some(1),
                            None,
                            10,
                        )],
                    ),
                    legacy_subsection(
                        "diffusion_runtime",
                        "Diffusion Runtime",
                        "Worker sizing for the ggml.diffusion runtime. Values above 1 are clamped to 1 because diffusion model state must stay on a single effective worker.",
                        40,
                        vec![legacy_integer_property(
                            PMID.runtime.diffusion.num_workers().into_string(),
                            "Diffusion Workers",
                            json!(1),
                            Some(1),
                            Some(1),
                            10,
                        )],
                    ),
                    legacy_subsection(
                        "model_auto_unload",
                        "Model Auto Unload",
                        "Unload idle models automatically to reclaim memory.",
                        50,
                        vec![
                            legacy_bool_property(
                                PMID.runtime.model_auto_unload.enabled().into_string(),
                                "Model Auto Unload Enabled",
                                false,
                                10,
                            ),
                            legacy_integer_property(
                                PMID.runtime.model_auto_unload.idle_minutes().into_string(),
                                "Model Auto Unload Idle Minutes",
                                json!(10),
                                Some(1),
                                None,
                                20,
                            ),
                        ],
                    ),
                ],
            ),
            legacy_section(
                "launch",
                "Launch",
                "Shared supervisor launch settings used by both slab-server and the desktop host.",
                15,
                vec![
                    legacy_subsection(
                        "general",
                        "General",
                        "Shared runtime transport and capacity settings. Backend library directories continue to come from setup.backends.dir.",
                        10,
                        vec![
                            legacy_enum_string_property(
                                PMID.launch.transport().into_string(),
                                "Runtime Transport",
                                "http",
                                vec!["http".to_owned(), "ipc".to_owned()],
                                10,
                            ),
                            legacy_integer_property(
                                PMID.launch.queue_capacity().into_string(),
                                "Runtime Queue Capacity",
                                json!(64),
                                Some(1),
                                None,
                                20,
                            ),
                            legacy_integer_property(
                                PMID.launch.backend_capacity().into_string(),
                                "Runtime Backend Capacity",
                                json!(4),
                                Some(1),
                                None,
                                30,
                            ),
                            legacy_path_property(
                                PMID.launch.runtime_ipc_dir().into_string(),
                                "Runtime IPC Directory",
                                40,
                            ),
                            legacy_path_property(
                                PMID.launch.runtime_log_dir().into_string(),
                                "Runtime Log Directory",
                                50,
                            ),
                        ],
                    ),
                    legacy_subsection(
                        "backends",
                        "Backends",
                        "Enable or disable runtime child processes per backend.",
                        20,
                        vec![
                            legacy_bool_property(
                                PMID.launch.backends.llama.enabled().into_string(),
                                "Launch Llama Runtime",
                                true,
                                10,
                            ),
                            legacy_bool_property(
                                PMID.launch.backends.whisper.enabled().into_string(),
                                "Launch Whisper Runtime",
                                true,
                                20,
                            ),
                            legacy_bool_property(
                                PMID.launch.backends.diffusion.enabled().into_string(),
                                "Launch Diffusion Runtime",
                                true,
                                30,
                            ),
                        ],
                    ),
                    legacy_subsection(
                        "server_profile",
                        "Server Profile",
                        "Host-specific launch settings used by slab-server.",
                        30,
                        vec![
                            legacy_string_property(
                                PMID.launch.profiles.server.gateway_bind().into_string(),
                                "Server Gateway Bind",
                                "127.0.0.1:3000",
                                10,
                            ),
                            legacy_string_property(
                                PMID.launch.profiles.server.runtime_bind_host().into_string(),
                                "Server Runtime Bind Host",
                                "127.0.0.1",
                                20,
                            ),
                            legacy_integer_property(
                                PMID.launch.profiles.server.runtime_bind_base_port().into_string(),
                                "Server Runtime Base Port",
                                json!(3001),
                                Some(1),
                                Some(65535),
                                30,
                            ),
                        ],
                    ),
                    legacy_subsection(
                        "desktop_profile",
                        "Desktop Profile",
                        "Host-specific launch settings used by the Tauri desktop app.",
                        40,
                        vec![
                            legacy_string_property(
                                PMID.launch.profiles.desktop.runtime_bind_host().into_string(),
                                "Desktop Runtime Bind Host",
                                "127.0.0.1",
                                10,
                            ),
                            legacy_integer_property(
                                PMID.launch.profiles.desktop.runtime_bind_base_port().into_string(),
                                "Desktop Runtime Base Port",
                                json!(50051),
                                Some(1),
                                Some(65535),
                                20,
                            ),
                        ],
                    ),
                ],
            ),
            legacy_section(
                "cloud",
                "Cloud Providers",
                "Global cloud provider configuration used by cloud model entries.",
                20,
                vec![legacy_subsection(
                    "providers",
                    "Cloud Providers",
                    "OpenAI-compatible cloud provider configuration stored as JSON.",
                    10,
                    vec![legacy_chat_providers_property(PMID.chat.providers().into_string(), 10)],
                )],
            ),
            legacy_section(
                "diffusion",
                "Diffusion",
                "Diffusion model paths and runtime performance toggles.",
                30,
                vec![
                    legacy_subsection(
                        "paths",
                        "Paths",
                        "Optional model paths passed to the diffusion backend.",
                        10,
                        vec![
                            legacy_path_property(
                                PMID.diffusion.paths.model().into_string(),
                                "Diffusion Model Path",
                                10,
                            ),
                            legacy_path_property(
                                PMID.diffusion.paths.vae().into_string(),
                                "Diffusion VAE Path",
                                20,
                            ),
                            legacy_path_property(
                                PMID.diffusion.paths.taesd().into_string(),
                                "Diffusion TAESD Path",
                                30,
                            ),
                            legacy_path_property(
                                PMID.diffusion.paths.lora_model_dir().into_string(),
                                "Diffusion LoRA Model Directory",
                                40,
                            ),
                            legacy_path_property(
                                PMID.diffusion.paths.clip_l().into_string(),
                                "Diffusion CLIP-L Path",
                                50,
                            ),
                            legacy_path_property(
                                PMID.diffusion.paths.clip_g().into_string(),
                                "Diffusion CLIP-G Path",
                                60,
                            ),
                            legacy_path_property(
                                PMID.diffusion.paths.t5xxl().into_string(),
                                "Diffusion T5XXL Path",
                                70,
                            ),
                        ],
                    ),
                    legacy_subsection(
                        "performance",
                        "Performance",
                        "Flags that trade memory usage for performance.",
                        20,
                        vec![
                            legacy_bool_property(
                                PMID.diffusion.performance.flash_attn().into_string(),
                                "Diffusion Flash Attention",
                                false,
                                10,
                            ),
                            legacy_string_property(
                                PMID.diffusion.performance.vae_device().into_string(),
                                "Diffusion VAE Device",
                                "",
                                20,
                            ),
                            legacy_string_property(
                                PMID.diffusion.performance.clip_device().into_string(),
                                "Diffusion CLIP Device",
                                "",
                                30,
                            ),
                            legacy_bool_property(
                                PMID.diffusion.performance.offload_params_to_cpu().into_string(),
                                "Diffusion Offload Params To CPU",
                                false,
                                40,
                            ),
                        ],
                    ),
                ],
            ),
        ],
    }
}

fn legacy_section(
    id: &str,
    title: &str,
    description_md: &str,
    order: u32,
    subsections: Vec<RawSettingsSubsectionDefinition>,
) -> RawSettingsSectionDefinition {
    RawSettingsSectionDefinition {
        id: id.to_owned(),
        title: title.to_owned(),
        description_md: description_md.to_owned(),
        order,
        subsections,
    }
}

fn legacy_subsection(
    id: &str,
    title: &str,
    description_md: &str,
    order: u32,
    properties: Vec<RawSettingDefinition>,
) -> RawSettingsSubsectionDefinition {
    RawSettingsSubsectionDefinition {
        id: id.to_owned(),
        title: title.to_owned(),
        description_md: description_md.to_owned(),
        order,
        properties,
    }
}

fn legacy_bool_property(
    pmid: String,
    label: &str,
    default_value: bool,
    order: u32,
) -> RawSettingDefinition {
    legacy_property(
        pmid,
        label,
        SettingStorageKind::Boolean,
        SettingPropertySchema {
            value_type: SettingValueType::Boolean,
            default_value: Value::Bool(default_value),
            order,
            ..Default::default()
        },
    )
}

fn legacy_string_property(
    pmid: String,
    label: &str,
    default_value: &str,
    order: u32,
) -> RawSettingDefinition {
    legacy_property(
        pmid,
        label,
        SettingStorageKind::String,
        SettingPropertySchema {
            value_type: SettingValueType::String,
            default_value: json!(default_value),
            order,
            ..Default::default()
        },
    )
}

fn legacy_path_property(pmid: String, label: &str, order: u32) -> RawSettingDefinition {
    legacy_property(
        pmid,
        label,
        SettingStorageKind::Path,
        SettingPropertySchema {
            value_type: SettingValueType::String,
            default_value: json!(""),
            order,
            ..Default::default()
        },
    )
}

fn legacy_integer_property(
    pmid: String,
    label: &str,
    default_value: Value,
    minimum: Option<i64>,
    maximum: Option<i64>,
    order: u32,
) -> RawSettingDefinition {
    legacy_property(
        pmid,
        label,
        SettingStorageKind::Integer,
        SettingPropertySchema {
            value_type: SettingValueType::Integer,
            minimum,
            maximum,
            default_value,
            order,
            ..Default::default()
        },
    )
}

fn legacy_enum_string_property(
    pmid: String,
    label: &str,
    default_value: &str,
    enum_values: Vec<String>,
    order: u32,
) -> RawSettingDefinition {
    legacy_property(
        pmid,
        label,
        SettingStorageKind::String,
        SettingPropertySchema {
            value_type: SettingValueType::String,
            enum_values: Some(enum_values),
            default_value: json!(default_value),
            order,
            ..Default::default()
        },
    )
}

fn legacy_chat_providers_property(pmid: String, order: u32) -> RawSettingDefinition {
    legacy_property(
        pmid,
        "Cloud Providers",
        SettingStorageKind::ChatProviders,
        SettingPropertySchema {
            value_type: SettingValueType::Array,
            json_schema: Some(legacy_chat_providers_json_schema()),
            default_value: json!([]),
            multiline: true,
            order,
            ..Default::default()
        },
    )
}

fn legacy_property(
    pmid: String,
    label: &str,
    storage_kind: SettingStorageKind,
    schema: SettingPropertySchema,
) -> RawSettingDefinition {
    let search_terms = pmid
        .split('.')
        .flat_map(|segment| segment.split('_'))
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_owned())
        .collect();

    RawSettingDefinition {
        pmid,
        label: label.to_owned(),
        description_md: String::new(),
        editable: true,
        search_terms,
        storage_kind,
        schema,
    }
}

fn legacy_chat_providers_json_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "array",
        "title": "Cloud Providers",
        "description": "Configure OpenAI-compatible providers referenced by cloud model entries.",
        "items": {
            "type": "object",
            "title": "Provider",
            "additionalProperties": false,
            "required": ["id", "name", "api_base"],
            "properties": {
                "id": {
                    "type": "string",
                    "title": "Provider ID",
                    "minLength": 1,
                    "examples": ["openai-main"]
                },
                "name": {
                    "type": "string",
                    "title": "Display Name",
                    "minLength": 1,
                    "examples": ["OpenAI"]
                },
                "api_base": {
                    "type": "string",
                    "title": "API Base URL",
                    "minLength": 1,
                    "examples": ["https://api.openai.com/v1"]
                },
                "api_key": {
                    "type": ["string", "null"],
                    "title": "Literal API Key",
                    "writeOnly": true,
                    "examples": ["sk-live-..."]
                },
                "api_key_env": {
                    "type": ["string", "null"],
                    "title": "API Key Env Var",
                    "examples": ["OPENAI_API_KEY"]
                }
            }
        }
    })
}

pub fn embedded_settings_schema() -> Result<SettingsSchema, AppCoreError> {
    SettingsSchema::from_raw(legacy_settings_schema())
}

impl SettingsSchema {
    pub fn from_json_str(raw: &str) -> Result<Self, AppCoreError> {
        let raw_document: Value = serde_json::from_str(raw).map_err(|error| {
            AppCoreError::Internal(format!("invalid embedded settings schema: {error}"))
        })?;
        validate_settings_schema_document(&raw_document)?;
        let parsed: RawSettingsSchema = serde_json::from_value(raw_document).map_err(|error| {
            AppCoreError::Internal(format!("invalid embedded settings schema: {error}"))
        })?;

        Self::from_raw(parsed)
    }

    fn from_raw(parsed: RawSettingsSchema) -> Result<Self, AppCoreError> {
        if parsed.sections.is_empty() {
            return Err(AppCoreError::Internal(
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
                return Err(AppCoreError::Internal(format!(
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
                    return Err(AppCoreError::Internal(format!(
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
                        return Err(AppCoreError::Internal(format!(
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

        Ok(Self { schema_version: parsed.schema_version, sections, property_index })
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
    fn from_raw(raw: RawSettingDefinition) -> Result<Self, AppCoreError> {
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
            return Err(AppCoreError::Internal("settings pmid must not be empty".into()));
        }
        if definition.label.is_empty() {
            return Err(AppCoreError::Internal(format!(
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
        let effective_value =
            override_value.cloned().unwrap_or_else(|| self.schema.default_value.clone());

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
    ) -> Result<Option<Value>, AppCoreError> {
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
    ) -> Result<Option<Value>, AppCoreError> {
        let canonical = self.canonicalize_runtime_value(value)?;
        if canonical == *self.default_value() { Ok(None) } else { Ok(Some(canonical)) }
    }

    fn validate_storage_shape(&self) -> Result<(), AppCoreError> {
        let expected_type = match self.storage_kind {
            SettingStorageKind::Boolean => SettingValueType::Boolean,
            SettingStorageKind::Integer => SettingValueType::Integer,
            SettingStorageKind::String | SettingStorageKind::Path => SettingValueType::String,
            SettingStorageKind::Array => SettingValueType::Array,
            SettingStorageKind::Object => SettingValueType::Object,
            SettingStorageKind::ChatProviders => SettingValueType::Array,
        };

        if self.schema.value_type != expected_type {
            return Err(AppCoreError::Internal(format!(
                "settings '{}' uses storage kind '{:?}' but schema type '{:?}'",
                self.pmid, self.storage_kind, self.schema.value_type
            )));
        }

        if self.schema.enum_values.is_some() && self.schema.value_type != SettingValueType::String {
            return Err(AppCoreError::Internal(format!(
                "settings '{}' only supports enum values for string properties",
                self.pmid
            )));
        }

        Ok(())
    }

    fn canonicalize_default_value(&self) -> Result<Value, AppCoreError> {
        if self.schema.default_value.is_null() {
            return match self.schema.value_type {
                SettingValueType::Integer => Ok(Value::Null),
                _ => Err(AppCoreError::Internal(format!(
                    "settings '{}' uses null default for a non-nullable property",
                    self.pmid
                ))),
            };
        }

        self.canonicalize_value(&self.schema.default_value, true).map_err(|error| match error {
            AppCoreError::BadRequest(message) => AppCoreError::Internal(format!(
                "settings '{}' has invalid default value: {message}",
                self.pmid
            )),
            AppCoreError::BadRequestData { message, .. } => AppCoreError::Internal(format!(
                "settings '{}' has invalid default value: {message}",
                self.pmid
            )),
            other => other,
        })
    }

    fn canonicalize_runtime_value(&self, value: &Value) -> Result<Value, AppCoreError> {
        self.canonicalize_value(value, false)
    }

    fn canonicalize_value(
        &self,
        value: &Value,
        allow_null_default: bool,
    ) -> Result<Value, AppCoreError> {
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
                    AppCoreError::Internal(format!("serialize settings value: {error}"))
                })?;
                self.validate_json_value(schema, &canonical)?;
                Ok(canonical)
            }
        }
    }

    fn build_validation_schemas(&self) -> Result<(Value, Value), AppCoreError> {
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
        if let Some(json_schema) = &self.schema.json_schema {
            return json_schema.clone();
        }

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

    fn validate_json_value(&self, schema: &Value, value: &Value) -> Result<(), AppCoreError> {
        let validator = jsonschema::validator_for(schema).map_err(|error| {
            AppCoreError::Internal(format!(
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

    fn validation_error(&self, path: &str, message: impl Into<String>) -> AppCoreError {
        let message = message.into();
        AppCoreError::BadRequestData {
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
        Value::Number(number) => {
            number.as_i64().map(|parsed| json!(parsed)).ok_or("value must be an integer")
        }
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
        if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
    })
}

pub fn canonicalize_chat_providers_from_value(
    value: &Value,
) -> Result<Vec<CloudProviderConfig>, String> {
    if value.is_null() {
        return Ok(Vec::new());
    }

    let providers: Vec<CloudProviderConfig> = serde_json::from_value(value.clone())
        .map_err(|error| format!("value has invalid provider payload: {error}"))?;
    canonicalize_chat_providers(providers)
}

fn canonicalize_chat_providers(
    providers: Vec<CloudProviderConfig>,
) -> Result<Vec<CloudProviderConfig>, String> {
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
    mut provider: CloudProviderConfig,
) -> Result<CloudProviderConfig, String> {
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
        return Err(format!("cloud provider '{}' has empty api_base", provider.id));
    }

    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::PMID;

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
        let definition =
            schema.property(PMID.runtime.llama.context_length().as_str()).expect("context length");

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
        assert_eq!(providers[0].api_key, None);
    }

    #[test]
    fn chat_provider_payload_without_models_is_canonicalized() {
        let providers = canonicalize_chat_providers_from_value(&json!([
            {
                "id": " openai-main ",
                "name": "",
                "api_base": "https://api.openai.com/v1/",
                "api_key_env": " OPENAI_API_KEY "
            }
        ]))
        .expect("providers");

        assert_eq!(providers[0].id, "openai-main");
        assert_eq!(providers[0].name, "openai-main");
        assert_eq!(providers[0].api_base, "https://api.openai.com/v1");
        assert_eq!(providers[0].api_key_env.as_deref(), Some("OPENAI_API_KEY"));
        assert_eq!(providers[0].api_key, None);
    }

    #[test]
    fn embedded_chat_provider_setting_exposes_structured_json_schema() {
        let schema = embedded_settings_schema().expect("schema");
        let definition = schema.property(PMID.chat.providers().as_str()).expect("cloud providers");

        let json_schema = definition.schema.json_schema.as_ref().expect("structured json schema");
        let provider_items =
            json_schema.get("items").and_then(Value::as_object).expect("provider items");
        let provider_properties = provider_items
            .get("properties")
            .and_then(Value::as_object)
            .expect("provider properties");

        assert!(provider_properties.contains_key("api_base"));
        assert!(provider_properties.contains_key("api_key_env"));
        assert!(!provider_properties.contains_key("models"));
    }

    #[test]
    fn schema_rejects_invalid_custom_json_schema() {
        let raw = r#"{
          "schema_version": 1,
          "sections": [
            {
              "id": "cloud",
              "title": "Cloud Providers",
              "subsections": [
                {
                  "id": "providers",
                  "title": "Cloud Providers",
                  "properties": [
                    {
                      "pmid": "chat.providers",
                      "label": "Cloud Providers",
                      "storage_kind": "array",
                      "schema": {
                        "type": "array",
                        "default_value": [],
                        "json_schema": { "type": 42 }
                      }
                    }
                  ]
                }
              ]
            }
          ]
        }"#;

        let error = SettingsSchema::from_json_str(raw).expect_err("invalid custom json schema");
        assert!(error.to_string().contains("runtime schema"));
    }
}
