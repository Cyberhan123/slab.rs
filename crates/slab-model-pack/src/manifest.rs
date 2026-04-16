use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slab_types::{Capability, DriverHints, ModelFamily};

use crate::refs::ConfigRef;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ModelPackManifest {
    pub version: u32,
    pub id: String,
    pub label: String,
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
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        alias = "source",
        deserialize_with = "deserialize_pack_source_candidates"
    )]
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
pub struct PresetEntryRef {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "variant")]
    pub variant_id: Option<String>,
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
    ModelScope {
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
            Self::ModelScope { .. } => "model_scope",
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
            Self::LocalFiles { files }
            | Self::HuggingFace { files, .. }
            | Self::ModelScope { files, .. } => files.clone(),
            Self::Cloud { .. } => Vec::new(),
        }
    }

    pub fn with_files(&self, files: Vec<PackSourceFile>) -> Self {
        match self {
            Self::LocalPath { .. } | Self::Cloud { .. } => self.clone(),
            Self::LocalFiles { .. } => Self::LocalFiles { files },
            Self::HuggingFace { repo_id, revision, .. } => Self::HuggingFace {
                repo_id: repo_id.clone(),
                revision: revision.clone(),
                files,
            },
            Self::ModelScope { repo_id, revision, .. } => Self::ModelScope {
                repo_id: repo_id.clone(),
                revision: revision.clone(),
                files,
            },
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
            Self::LocalPath { .. } | Self::LocalFiles { .. } | Self::Cloud { .. } => None,
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

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, Deserialize)]
enum LegacyRemoteSourceKind {
    #[serde(rename = "hugging_face", alias = "hf_hub", alias = "huggingface")]
    HuggingFace,
    #[serde(rename = "model_scope", alias = "models_cat", alias = "modelscope")]
    ModelScope,
}

#[derive(Debug, Clone, Deserialize)]
struct FlatPackSourceCandidateWire {
    #[serde(flatten)]
    source: PackSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    priority: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hub_provider: Option<LegacyRemoteSourceKind>,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyPackSourceCandidateWire {
    source: PackSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    priority: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hub_provider: Option<LegacyRemoteSourceKind>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PackSourceCandidateWire {
    SourceOnly(PackSource),
    Flat(FlatPackSourceCandidateWire),
    Legacy(LegacyPackSourceCandidateWire),
}

impl PackSourceCandidateWire {
    fn into_candidate(self) -> PackSourceCandidate {
        match self {
            Self::SourceOnly(source) => PackSourceCandidate::new(source),
            Self::Flat(wire) => PackSourceCandidate {
                source: apply_legacy_remote_source_kind(wire.source, wire.hub_provider),
                priority: wire.priority,
            },
            Self::Legacy(wire) => PackSourceCandidate {
                source: apply_legacy_remote_source_kind(wire.source, wire.hub_provider),
                priority: wire.priority,
            },
        }
    }
}

impl<'de> Deserialize<'de> for PackSourceCandidate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        PackSourceCandidateWire::deserialize(deserializer).map(PackSourceCandidateWire::into_candidate)
    }
}

fn apply_legacy_remote_source_kind(
    source: PackSource,
    legacy_kind: Option<LegacyRemoteSourceKind>,
) -> PackSource {
    match legacy_kind {
        None => source,
        Some(LegacyRemoteSourceKind::HuggingFace) => match source {
            PackSource::ModelScope { repo_id, revision, files } => {
                PackSource::HuggingFace { repo_id, revision, files }
            }
            other => other,
        },
        Some(LegacyRemoteSourceKind::ModelScope) => match source {
            PackSource::HuggingFace { repo_id, revision, files } => {
                PackSource::ModelScope { repo_id, revision, files }
            }
            other => other,
        },
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PackSourceCandidatesRepr {
    SingleSource(PackSource),
    SingleCandidate(PackSourceCandidate),
    CandidateList(Vec<PackSourceCandidate>),
}

fn deserialize_pack_source_candidates<'de, D>(
    deserializer: D,
) -> Result<Vec<PackSourceCandidate>, D::Error>
where
    D: Deserializer<'de>,
{
    let repr = Option::<PackSourceCandidatesRepr>::deserialize(deserializer)?;
    Ok(match repr {
        None => Vec::new(),
        Some(PackSourceCandidatesRepr::SingleSource(source)) => {
            vec![PackSourceCandidate::new(source)]
        }
        Some(PackSourceCandidatesRepr::SingleCandidate(candidate)) => vec![candidate],
        Some(PackSourceCandidatesRepr::CandidateList(candidates)) => candidates,
    })
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
#[serde(deny_unknown_fields)]
pub struct VariantDocument {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        alias = "source",
        deserialize_with = "deserialize_pack_source_candidates"
    )]
    pub sources: Vec<PackSourceCandidate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub component_ids: Vec<String>,
    #[serde(rename = "$load_config", default, skip_serializing_if = "Option::is_none")]
    pub load_config: Option<ConfigRef>,
    #[serde(rename = "$inference_config", default, skip_serializing_if = "Option::is_none")]
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
#[serde(deny_unknown_fields)]
pub struct PresetDocument {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapter_ids: Vec<String>,
    #[serde(rename = "$load_config", default, skip_serializing_if = "Option::is_none")]
    pub load_config: Option<ConfigRef>,
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
    pub id: String,
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

    #[test]
    fn deserializes_legacy_nested_source_candidates() {
        let candidate: PackSourceCandidate = serde_json::from_value(json!({
            "source": {
                "kind": "hugging_face",
                "repo_id": "Qwen/Qwen2.5-7B-Instruct-GGUF",
                "files": [
                    {
                        "id": "model",
                        "path": "Qwen2.5-7B-Instruct-Q4_K_M.gguf"
                    }
                ]
            },
            "hub_provider": "model_scope",
            "priority": 5
        }))
        .expect("legacy candidate should deserialize");

        assert_eq!(
            candidate,
            PackSourceCandidate {
                source: PackSource::ModelScope {
                    repo_id: "Qwen/Qwen2.5-7B-Instruct-GGUF".into(),
                    revision: None,
                    files: vec![PackSourceFile {
                        id: "model".into(),
                        label: None,
                        description: None,
                        path: "Qwen2.5-7B-Instruct-Q4_K_M.gguf".into(),
                    }],
                },
                priority: Some(5),
            }
        );
    }
}
