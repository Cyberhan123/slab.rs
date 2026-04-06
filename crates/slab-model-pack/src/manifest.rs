use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slab_types::{Capability, DriverHints, ModelFamily, RuntimeBackendId};

use crate::refs::ConfigRef;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ModelPackManifest {
    pub version: u32,
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<PackModelStatus>,
    pub family: ModelFamily,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub backend_hints: DriverHints,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<PackPricing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_presets: Option<PackRuntimePresets>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<PackSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ConfigEntryRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<ConfigEntryRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapters: Vec<ConfigEntryRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<ConfigEntryRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_preset: Option<String>,
    #[serde(default)]
    pub footprint: ResourceFootprint,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackModelStatus {
    Ready,
    NotDownloaded,
    Downloading,
    Error,
}

impl PackModelStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::NotDownloaded => "not_downloaded",
            Self::Downloading => "downloading",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PackPricing {
    pub input: f64,
    pub output: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PackRuntimePresets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ConfigEntryRef {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "$config")]
    pub config_ref: ConfigRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PackSource {
    LocalPath {
        path: String,
    },
    LocalFiles {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        files: Vec<PackSourceFile>,
    },
    HuggingFace {
        repo_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        revision: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        files: Vec<PackSourceFile>,
    },
    Cloud {
        provider_id: String,
        remote_model_id: String,
    },
}

impl PackSource {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::LocalPath { .. } => "local_path",
            Self::LocalFiles { .. } => "local_files",
            Self::HuggingFace { .. } => "hugging_face",
            Self::Cloud { .. } => "cloud",
        }
    }

    pub fn files(&self) -> Vec<PackSourceFile> {
        match self {
            Self::LocalPath { path } => vec![PackSourceFile {
                id: "model".to_owned(),
                label: None,
                description: None,
                path: path.clone(),
            }],
            Self::LocalFiles { files } | Self::HuggingFace { files, .. } => files.clone(),
            Self::Cloud { .. } => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PackSourceFile {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ResourceFootprint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ram_mb: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vram_mb: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DynamicFootprint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ram_dynamic_mb: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vram_dynamic_mb: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackDocumentKind {
    Variant,
    Adapter,
    Component,
    Preset,
    BackendConfig,
}

impl PackDocumentKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Variant => "variant",
            Self::Adapter => "adapter",
            Self::Component => "component",
            Self::Preset => "preset",
            Self::BackendConfig => "backend_config",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PackDocument {
    Variant(VariantDocument),
    Adapter(AdapterDocument),
    Component(ComponentDocument),
    Preset(PresetDocument),
    BackendConfig(BackendConfigDocument),
}

impl PackDocument {
    pub const fn document_kind(&self) -> PackDocumentKind {
        match self {
            Self::Variant(_) => PackDocumentKind::Variant,
            Self::Adapter(_) => PackDocumentKind::Adapter,
            Self::Component(_) => PackDocumentKind::Component,
            Self::Preset(_) => PackDocumentKind::Preset,
            Self::BackendConfig(_) => PackDocumentKind::BackendConfig,
        }
    }

    pub fn kind(&self) -> &'static str {
        self.document_kind().as_str()
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Variant(document) => &document.id,
            Self::Adapter(document) => &document.id,
            Self::Component(document) => &document.id,
            Self::Preset(document) => &document.id,
            Self::BackendConfig(document) => &document.id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct VariantDocument {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<PackSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<RuntimeBackendId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub component_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_config: Option<ConfigRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_config: Option<ConfigRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AdapterDocument {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<PackSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub component_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ComponentDocument {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub source: PackSource,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PresetDocument {
    pub id: String,
    pub label: String,
    pub variant_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapter_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_config: Option<ConfigRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_config: Option<ConfigRef>,
    #[serde(default)]
    pub footprint: DynamicFootprint,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackendConfigScope {
    Load,
    Inference,
}

impl BackendConfigScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::Inference => "inference",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct BackendConfigDocument {
    pub id: String,
    pub label: String,
    pub backend: RuntimeBackendId,
    pub scope: BackendConfigScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub payload: Value,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}