use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::backend::RuntimeBackendId;
use crate::inference::JsonOptions;
use crate::load_config::RuntimeBackendLoadSpec;

#[non_exhaustive]
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum ModelFamily {
    Llama,
    Whisper,
    Diffusion,
    Onnx,
}

#[non_exhaustive]
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    TextGeneration,
    AudioTranscription,
    ImageGeneration,
    ImageEmbedding,
    ChatGeneration,
    AudioVad,
    VideoGeneration,
}

impl Capability {
    pub const fn is_runtime_execution(self) -> bool {
        matches!(
            self,
            Self::TextGeneration
                | Self::AudioTranscription
                | Self::ImageGeneration
                | Self::ImageEmbedding
        )
    }

    pub const fn is_product_placement(self) -> bool {
        matches!(self, Self::ChatGeneration | Self::AudioVad | Self::VideoGeneration)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DriverHints {
    #[serde(default)]
    pub prefer_drivers: Vec<String>,
    #[serde(default)]
    pub avoid_drivers: Vec<String>,
    #[serde(default)]
    pub require_streaming: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModelSourceKind {
    LocalPath,
    LocalArtifacts,
    HuggingFace,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DriverLoadStyle {
    DynamicLibraryThenModel,
    ModelOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DriverDescriptor {
    pub driver_id: String,
    pub backend_id: String,
    pub family: ModelFamily,
    pub capability: Capability,
    #[serde(default)]
    pub supported_sources: Vec<ModelSourceKind>,
    #[serde(default)]
    pub supports_streaming: bool,
    pub load_style: DriverLoadStyle,
    #[serde(default)]
    pub priority: i32,
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModelSource {
    LocalPath {
        path: PathBuf,
    },
    LocalArtifacts {
        #[serde(default)]
        files: BTreeMap<String, PathBuf>,
    },
    HuggingFace {
        repo_id: String,
        #[serde(default)]
        revision: Option<String>,
        #[serde(default)]
        files: BTreeMap<String, PathBuf>,
    },
}

impl ModelSource {
    pub fn primary_path(&self) -> Option<&Path> {
        match self {
            Self::LocalPath { path } => Some(path.as_path()),
            Self::LocalArtifacts { files } | Self::HuggingFace { files, .. } => {
                files.get("model").or_else(|| files.values().next()).map(PathBuf::as_path)
            }
        }
    }

    pub fn artifact(&self, name: &str) -> Option<&Path> {
        match self {
            Self::LocalPath { path } => (name == "model").then_some(path.as_path()),
            Self::LocalArtifacts { files } | Self::HuggingFace { files, .. } => {
                files.get(name).map(PathBuf::as_path)
            }
        }
    }

    pub fn files(&self) -> BTreeMap<String, PathBuf> {
        match self {
            Self::LocalPath { path } => {
                let mut files = BTreeMap::new();
                files.insert("model".to_owned(), path.clone());
                files
            }
            Self::LocalArtifacts { files } | Self::HuggingFace { files, .. } => files.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ModelSpec {
    #[serde(default)]
    pub id: Option<String>,
    pub family: ModelFamily,
    pub capability: Capability,
    pub source: ModelSource,
    #[serde(default)]
    pub driver_hints: DriverHints,
    #[serde(default)]
    pub load_options: JsonOptions,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl ModelSpec {
    pub fn new(family: ModelFamily, capability: Capability, source: ModelSource) -> Self {
        Self {
            id: None,
            family,
            capability,
            source,
            driver_hints: DriverHints::default(),
            load_options: JsonOptions::default(),
            metadata: BTreeMap::default(),
        }
    }

    pub fn named(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn with_driver_hints(mut self, driver_hints: DriverHints) -> Self {
        self.driver_hints = driver_hints;
        self
    }

    pub fn with_load_option(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.load_options.insert(key.into(), value.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

const fn default_num_workers() -> u32 {
    1
}

/// Diffusion-specific model load options carried alongside the primary model path.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DiffusionLoadOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffusion_model_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taesd_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lora_model_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_l_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_g_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t5xxl_path: Option<PathBuf>,
    #[serde(default)]
    pub flash_attn: bool,
    #[serde(default)]
    pub vae_device: String,
    #[serde(default)]
    pub clip_device: String,
    #[serde(default)]
    pub offload_params_to_cpu: bool,
}

/// Semantic model load specification shared between server/runtime and core/runtime.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RuntimeModelLoadSpec {
    pub model_path: PathBuf,
    #[serde(default = "default_num_workers")]
    pub num_workers: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffusion: Option<DiffusionLoadOptions>,
}

impl Default for RuntimeModelLoadSpec {
    fn default() -> Self {
        Self {
            model_path: PathBuf::default(),
            num_workers: default_num_workers(),
            context_length: None,
            chat_template: None,
            diffusion: None,
        }
    }
}

/// Runtime model load command when backend routing is part of the semantic contract.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RuntimeModelLoadCommand {
    pub backend: RuntimeBackendId,
    pub spec: RuntimeBackendLoadSpec,
}

impl RuntimeModelLoadCommand {
    pub fn from_legacy(
        backend: RuntimeBackendId,
        spec: RuntimeModelLoadSpec,
    ) -> Result<Self, crate::error::SlabTypeError> {
        Ok(Self { backend, spec: RuntimeBackendLoadSpec::from_legacy(backend, spec)? })
    }

    pub fn legacy_spec(&self) -> RuntimeModelLoadSpec {
        self.spec.to_legacy_spec()
    }
}

/// Runtime-reported model status on the server/runtime boundary.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RuntimeModelStatus {
    pub backend: RuntimeBackendId,
    pub status: String,
}
