use std::collections::BTreeMap;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slab_types::{ArtifactFormat, Capability, ModelFamily, RuntimeBackendId};

use crate::refs::ConfigRef;

pub const MODEL_PACK_SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModelPackManifest {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub schema_version: u32,
    pub deployment: PackDeployment,
    pub id: String,
    pub label: String,
    pub family: ModelFamily,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<PackPricing>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub engines: Vec<EngineTarget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<PackSourceCandidate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ConfigEntryRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<ConfigEntryRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapters: Vec<ConfigEntryRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<PresetEntryRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_preset: Option<String>,
    #[serde(default)]
    pub footprint: ResourceFootprint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud: Option<CloudModelTarget>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackDeployment {
    Local,
    Cloud,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
pub struct EngineTarget {
    #[serde(
        serialize_with = "runtime_backend_id_canonical::serialize",
        deserialize_with = "runtime_backend_id_canonical::deserialize"
    )]
    pub id: RuntimeBackendId,
    pub format: ArtifactFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PackPricing {
    pub input: f64,
    pub output: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CloudModelTarget {
    pub provider_id: String,
    pub remote_model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_api_base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<CloudCredentials>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CloudCredentials {
    pub secret_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConfigEntryRef {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "$ref")]
    pub config_ref: ConfigRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresetEntryRef {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "$ref")]
    pub config_ref: ConfigRef,
}

#[non_exhaustive]
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
    ModelScope {
        repo_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        revision: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        files: Vec<PackSourceFile>,
    },
}

impl PackSource {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::LocalPath { .. } => "local_path",
            Self::LocalFiles { .. } => "local_files",
            Self::HuggingFace { .. } => "hugging_face",
            Self::ModelScope { .. } => "model_scope",
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
            Self::LocalFiles { files }
            | Self::HuggingFace { files, .. }
            | Self::ModelScope { files, .. } => files.clone(),
        }
    }

    pub fn with_files(&self, files: Vec<PackSourceFile>) -> Self {
        match self {
            Self::LocalPath { .. } => self.clone(),
            Self::LocalFiles { .. } => Self::LocalFiles { files },
            Self::HuggingFace { repo_id, revision, .. } => {
                Self::HuggingFace { repo_id: repo_id.clone(), revision: revision.clone(), files }
            }
            Self::ModelScope { repo_id, revision, .. } => {
                Self::ModelScope { repo_id: repo_id.clone(), revision: revision.clone(), files }
            }
        }
    }

    pub fn remote_repository(&self) -> Option<PackRemoteRepository<'_>> {
        match self {
            Self::HuggingFace { repo_id, revision, files } => Some(PackRemoteRepository {
                source_kind: "hugging_face",
                hub_provider: "hf_hub",
                repo_id,
                revision: revision.as_deref(),
                files,
            }),
            Self::ModelScope { repo_id, revision, files } => Some(PackRemoteRepository {
                source_kind: "model_scope",
                hub_provider: "models_cat",
                repo_id,
                revision: revision.as_deref(),
                files,
            }),
            Self::LocalPath { .. } | Self::LocalFiles { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackRemoteRepository<'a> {
    pub source_kind: &'static str,
    pub hub_provider: &'static str,
    pub repo_id: &'a str,
    pub revision: Option<&'a str>,
    pub files: &'a [PackSourceFile],
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PackSourceCandidate {
    #[serde(flatten)]
    pub source: PackSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
}

impl PackSourceCandidate {
    pub const fn new(source: PackSource) -> Self {
        Self { source, priority: None }
    }

    pub const fn kind(&self) -> &'static str {
        self.source.kind()
    }

    pub fn files(&self) -> Vec<PackSourceFile> {
        self.source.files()
    }

    pub fn with_source(&self, source: PackSource) -> Self {
        Self { source, priority: self.priority }
    }
}

mod runtime_backend_id_canonical {
    use super::*;

    pub fn serialize<S>(value: &RuntimeBackendId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(value.canonical_id())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<RuntimeBackendId, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        RuntimeBackendId::from_str(&value).map_err(<D::Error as serde::de::Error>::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
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
            Self::BackendConfig(document) => document.id.as_deref().unwrap_or(""),
        }
    }

    pub fn declared_id(&self) -> Option<&str> {
        match self {
            Self::Variant(document) => Some(&document.id),
            Self::Adapter(document) => Some(&document.id),
            Self::Component(document) => Some(&document.id),
            Self::Preset(document) => Some(&document.id),
            Self::BackendConfig(document) => document.id.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VariantDocument {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub format: ArtifactFormat,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<PackSourceCandidate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub component_ids: Vec<String>,
    #[serde(rename = "$load_config", default, skip_serializing_if = "Option::is_none")]
    pub load_config: Option<ConfigRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct PresetDocument {
    pub id: String,
    pub label: String,
    pub variant_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapter_ids: Vec<String>,
    #[serde(rename = "$inference_config", default, skip_serializing_if = "Option::is_none")]
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
#[serde(deny_unknown_fields)]
pub struct BackendConfigDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub label: String,
    pub scope: BackendConfigScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub payload: Value,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{PackSource, PackSourceCandidate, PackSourceFile};

    #[test]
    fn serializes_source_candidates_without_nested_source_wrapper() {
        let candidate = PackSourceCandidate {
            source: PackSource::HuggingFace {
                repo_id: "bartowski/Qwen2.5-0.5B-Instruct-GGUF".into(),
                revision: None,
                files: vec![PackSourceFile {
                    id: "model".into(),
                    label: None,
                    description: None,
                    path: "Qwen2.5-0.5B-Instruct-Q4_K_M.gguf".into(),
                }],
            },
            priority: Some(0),
        };

        assert_eq!(
            serde_json::to_value(&candidate).expect("candidate should serialize"),
            json!({
                "kind": "hugging_face",
                "repo_id": "bartowski/Qwen2.5-0.5B-Instruct-GGUF",
                "files": [
                    {
                        "id": "model",
                        "path": "Qwen2.5-0.5B-Instruct-Q4_K_M.gguf"
                    }
                ],
                "priority": 0
            })
        );
    }
}
