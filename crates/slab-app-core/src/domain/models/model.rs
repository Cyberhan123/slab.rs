use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use slab_types::{Capability, RuntimeBackendId};

// ---------------------------------------------------------------------------
// Status enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedModelStatus {
    Ready,
    NotDownloaded,
    Downloading,
    Error,
}

impl UnifiedModelStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::NotDownloaded => "not_downloaded",
            Self::Downloading => "downloading",
            Self::Error => "error",
        }
    }
}

impl FromStr for UnifiedModelStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ready" => Ok(Self::Ready),
            "not_downloaded" => Ok(Self::NotDownloaded),
            "downloading" => Ok(Self::Downloading),
            "error" => Ok(Self::Error),
            other => Err(format!("unknown model status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedModelKind {
    Local,
    Cloud,
}

impl UnifiedModelKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Cloud => "cloud",
        }
    }
}

impl FromStr for UnifiedModelKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "local" => Ok(Self::Local),
            "cloud" => Ok(Self::Cloud),
            other => Err(format!("unknown model kind: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ManagedModelBackendId {
    GgmlLlama,
    GgmlWhisper,
    GgmlDiffusion,
}

impl ManagedModelBackendId {
    pub const ALL: [Self; 3] = [Self::GgmlLlama, Self::GgmlWhisper, Self::GgmlDiffusion];

    pub const fn as_runtime_backend_id(self) -> RuntimeBackendId {
        match self {
            Self::GgmlLlama => RuntimeBackendId::GgmlLlama,
            Self::GgmlWhisper => RuntimeBackendId::GgmlWhisper,
            Self::GgmlDiffusion => RuntimeBackendId::GgmlDiffusion,
        }
    }

    pub const fn canonical_id(self) -> &'static str {
        self.as_runtime_backend_id().canonical_id()
    }
}

impl fmt::Display for ManagedModelBackendId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.canonical_id())
    }
}

impl Serialize for ManagedModelBackendId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.canonical_id())
    }
}

impl<'de> Deserialize<'de> for ManagedModelBackendId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(serde::de::Error::custom)
    }
}

impl From<ManagedModelBackendId> for RuntimeBackendId {
    fn from(value: ManagedModelBackendId) -> Self {
        value.as_runtime_backend_id()
    }
}

impl TryFrom<RuntimeBackendId> for ManagedModelBackendId {
    type Error = String;

    fn try_from(value: RuntimeBackendId) -> Result<Self, Self::Error> {
        match value {
            RuntimeBackendId::GgmlLlama => Ok(Self::GgmlLlama),
            RuntimeBackendId::GgmlWhisper => Ok(Self::GgmlWhisper),
            RuntimeBackendId::GgmlDiffusion => Ok(Self::GgmlDiffusion),
            other => Err(format!("unsupported managed model backend id: {}", other.canonical_id())),
        }
    }
}

impl FromStr for ManagedModelBackendId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let backend = RuntimeBackendId::from_str(value)
            .map_err(|_| format!("unknown managed model backend id: {}", value.trim()))?;
        Self::try_from(backend)
    }
}

// ---------------------------------------------------------------------------
// Spec and runtime presets (shared with JSON schema)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelSpec {
    /// Cloud provider id from the settings document `providers.registry` list
    /// (e.g. `"openai-main"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    /// Remote model identifier used by cloud providers (e.g. `"gpt-4o"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_model_id: Option<String>,
    /// Optional pricing info for cost tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pricing: Option<Pricing>,
    /// HuggingFace repo ID for local models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_id: Option<String>,
    /// Optional explicit hub provider override for local model downloads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hub_provider: Option<String>,
    /// Filename within the HF repo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// Absolute path to the downloaded model file (populated after download).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_path: Option<String>,
    /// Maximum context window size in tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    /// Optional chat prompt template name for local chat rendering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pricing {
    pub input: f64,
    pub output: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimePresets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModelPackSelection {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub preset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub variant_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Unified domain model view
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UnifiedModel {
    pub id: String,
    pub display_name: String,
    pub kind: UnifiedModelKind,
    pub backend_id: Option<ManagedModelBackendId>,
    pub capabilities: Vec<Capability>,
    pub status: UnifiedModelStatus,
    pub spec: ModelSpec,
    pub runtime_presets: Option<RuntimePresets>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CreateModelCommand {
    pub id: Option<String>,
    pub display_name: String,
    pub kind: UnifiedModelKind,
    pub backend_id: Option<ManagedModelBackendId>,
    pub capabilities: Option<Vec<Capability>>,
    /// If `None`, the status is inferred from the model kind.
    pub status: Option<UnifiedModelStatus>,
    pub spec: ModelSpec,
    pub runtime_presets: Option<RuntimePresets>,
}

pub const CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION: u32 = 2;
pub const CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION: u32 = 3;

const fn current_stored_model_config_schema_version() -> u32 {
    CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION
}

const fn current_stored_model_config_policy_version() -> u32 {
    CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredModelConfig {
    #[serde(default = "current_stored_model_config_schema_version")]
    pub schema_version: u32,
    #[serde(default = "current_stored_model_config_policy_version")]
    pub policy_version: u32,
    pub id: String,
    pub display_name: String,
    pub kind: UnifiedModelKind,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub backend_id: Option<ManagedModelBackendId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<Capability>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub status: Option<UnifiedModelStatus>,
    #[serde(default)]
    pub spec: ModelSpec,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub runtime_presets: Option<RuntimePresets>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub materialized_artifacts: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pack_selection: Option<ModelPackSelection>,
}

pub fn upgrade_stored_model_config(config: StoredModelConfig) -> Result<StoredModelConfig, String> {
    let config = upgrade_stored_model_config_schema(config)?;
    let config = upgrade_stored_model_config_policy(config)?;
    Ok(normalize_stored_model_capabilities(config))
}

fn upgrade_stored_model_config_schema(
    config: StoredModelConfig,
) -> Result<StoredModelConfig, String> {
    if config.schema_version == 0 {
        return Err("stored model config schema_version must be at least 1".to_owned());
    }

    if config.schema_version > CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION {
        return Err(format!(
            "unsupported stored model config schema_version: {}",
            config.schema_version
        ));
    }

    if config.schema_version < CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION {
        return Err(format!(
            "missing stored model config schema upgrader for version {}",
            config.schema_version
        ));
    }

    Ok(config)
}

fn upgrade_stored_model_config_policy(
    config: StoredModelConfig,
) -> Result<StoredModelConfig, String> {
    if config.policy_version == 0 {
        return Err("stored model config policy_version must be at least 1".to_owned());
    }

    if config.policy_version > CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION {
        return Err(format!(
            "unsupported stored model config policy_version: {}",
            config.policy_version
        ));
    }

    if config.policy_version < CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION {
        let mut upgraded = config;
        upgraded.policy_version = CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION;
        return Ok(upgraded);
    }

    Ok(config)
}

#[derive(Debug, Clone)]
pub struct UpdateModelCommand {
    pub display_name: Option<String>,
    pub kind: Option<UnifiedModelKind>,
    pub backend_id: Option<ManagedModelBackendId>,
    pub capabilities: Option<Vec<Capability>>,
    pub status: Option<UnifiedModelStatus>,
    pub spec: Option<ModelSpec>,
    pub runtime_presets: Option<RuntimePresets>,
}

#[derive(Debug, Clone)]
pub struct ModelEnhancementPresetOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub variant_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModelEnhancementVariantOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub repo_id: Option<String>,
    pub filename: Option<String>,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModelEnhancementView {
    pub model: UnifiedModel,
    pub default_preset_id: Option<String>,
    pub selected_preset_id: Option<String>,
    pub selected_variant_id: Option<String>,
    pub presets: Vec<ModelEnhancementPresetOption>,
    pub variants: Vec<ModelEnhancementVariantOption>,
    pub resolved_spec: ModelSpec,
    pub resolved_runtime_presets: Option<RuntimePresets>,
}

#[derive(Debug, Clone)]
pub struct UpdateModelEnhancementCommand {
    pub display_name: String,
    pub selected_preset_id: Option<String>,
    pub selected_variant_id: Option<String>,
    pub context_window: Option<u32>,
    pub chat_template: Option<String>,
    pub runtime_presets: Option<RuntimePresets>,
}

#[derive(Debug, Clone)]
pub struct ModelConfigPresetOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub variant_id: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct ModelConfigVariantOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub repo_id: Option<String>,
    pub filename: Option<String>,
    pub local_path: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct ModelConfigSelectionView {
    pub default_preset_id: Option<String>,
    pub default_variant_id: Option<String>,
    pub selected_preset_id: Option<String>,
    pub selected_variant_id: Option<String>,
    pub effective_preset_id: Option<String>,
    pub effective_variant_id: Option<String>,
    pub presets: Vec<ModelConfigPresetOption>,
    pub variants: Vec<ModelConfigVariantOption>,
}

#[derive(Debug, Clone)]
pub struct ModelConfigSourceArtifact {
    pub id: String,
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct ModelConfigSourceSummary {
    pub source_kind: String,
    pub repo_id: Option<String>,
    pub filename: Option<String>,
    pub local_path: Option<String>,
    pub artifacts: Vec<ModelConfigSourceArtifact>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelConfigFieldScope {
    Summary,
    Source,
    Load,
    Inference,
    Advanced,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelConfigValueType {
    String,
    Integer,
    Number,
    Boolean,
    Path,
    Json,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelConfigOrigin {
    PackManifest,
    SelectedPreset,
    SelectedVariant,
    SelectedBackendConfig,
    PmidFallback,
    Derived,
}

#[derive(Debug, Clone)]
pub struct ModelConfigFieldView {
    pub path: String,
    pub scope: ModelConfigFieldScope,
    pub label: String,
    pub description_md: Option<String>,
    pub value_type: ModelConfigValueType,
    pub effective_value: Value,
    pub origin: ModelConfigOrigin,
    pub editable: bool,
    pub locked: bool,
    pub json_schema: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct ModelConfigSectionView {
    pub id: String,
    pub label: String,
    pub description_md: Option<String>,
    pub fields: Vec<ModelConfigFieldView>,
}

#[derive(Debug, Clone)]
pub struct ModelConfigDocument {
    pub model_summary: UnifiedModel,
    pub selection: ModelConfigSelectionView,
    pub sections: Vec<ModelConfigSectionView>,
    pub source_summary: ModelConfigSourceSummary,
    pub resolved_load_spec: Value,
    pub resolved_inference_spec: Value,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateModelConfigSelectionCommand {
    pub selected_preset_id: Option<String>,
    pub selected_variant_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ListModelsFilter {
    pub capability: Option<Capability>,
}

#[derive(Debug, Clone)]
pub struct ModelLoadCommand {
    pub model_id: Option<String>,
    pub backend_id: Option<String>,
    pub model_path: Option<String>,
    pub num_workers: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ModelStatus {
    pub backend: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct DeletedModelView {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct AvailableModelsQuery {
    pub repo_id: String,
}

#[derive(Debug, Clone)]
pub struct AvailableModelsView {
    pub repo_id: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DownloadModelCommand {
    pub model_id: String,
}

impl From<StoredModelConfig> for CreateModelCommand {
    fn from(config: StoredModelConfig) -> Self {
        Self {
            id: Some(config.id),
            display_name: config.display_name,
            kind: config.kind,
            backend_id: config.backend_id,
            capabilities: Some(config.capabilities),
            status: config.status,
            spec: config.spec,
            runtime_presets: config.runtime_presets,
        }
    }
}

impl From<UnifiedModel> for StoredModelConfig {
    fn from(model: UnifiedModel) -> Self {
        Self {
            schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
            policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
            id: model.id,
            display_name: model.display_name,
            kind: model.kind,
            backend_id: model.backend_id,
            capabilities: model.capabilities,
            status: Some(model.status),
            spec: model.spec,
            runtime_presets: model.runtime_presets,
            materialized_artifacts: BTreeMap::new(),
            pack_selection: None,
        }
    }
}

pub fn default_model_capabilities(
    kind: UnifiedModelKind,
    backend_id: Option<ManagedModelBackendId>,
    display_name: &str,
    spec: &ModelSpec,
) -> Vec<Capability> {
    match (kind, backend_id) {
        (UnifiedModelKind::Cloud, _) => {
            vec![Capability::TextGeneration, Capability::ChatGeneration]
        }
        (UnifiedModelKind::Local, Some(ManagedModelBackendId::GgmlWhisper))
            if looks_like_vad_model(display_name, spec) =>
        {
            vec![Capability::AudioVad]
        }
        (UnifiedModelKind::Local, Some(ManagedModelBackendId::GgmlWhisper)) => {
            vec![Capability::AudioTranscription]
        }
        (UnifiedModelKind::Local, Some(ManagedModelBackendId::GgmlDiffusion)) => {
            vec![Capability::ImageGeneration, Capability::VideoGeneration]
        }
        _ => vec![Capability::TextGeneration, Capability::ChatGeneration],
    }
}

pub fn normalize_model_capabilities(
    kind: UnifiedModelKind,
    backend_id: Option<ManagedModelBackendId>,
    display_name: &str,
    spec: &ModelSpec,
    capabilities: Option<Vec<Capability>>,
) -> Vec<Capability> {
    let mut normalized = capabilities.unwrap_or_default();
    normalized.retain(|capability| {
        capability.is_runtime_execution() || capability.is_product_placement()
    });
    dedupe_capabilities(&mut normalized);

    if normalized.is_empty() {
        return default_model_capabilities(kind, backend_id, display_name, spec);
    }

    if normalized.contains(&Capability::ChatGeneration)
        && !normalized.contains(&Capability::TextGeneration)
    {
        normalized.insert(0, Capability::TextGeneration);
    }

    dedupe_capabilities(&mut normalized);
    normalized
}

fn normalize_stored_model_capabilities(mut config: StoredModelConfig) -> StoredModelConfig {
    config.capabilities = normalize_model_capabilities(
        config.kind,
        config.backend_id,
        &config.display_name,
        &config.spec,
        Some(config.capabilities),
    );
    config
}

fn dedupe_capabilities(capabilities: &mut Vec<Capability>) {
    let mut deduped = Vec::with_capacity(capabilities.len());
    for capability in capabilities.drain(..) {
        if !deduped.contains(&capability) {
            deduped.push(capability);
        }
    }
    *capabilities = deduped;
}

fn looks_like_vad_model(display_name: &str, spec: &ModelSpec) -> bool {
    let haystack = format!(
        "{} {} {}",
        display_name,
        spec.repo_id.as_deref().unwrap_or_default(),
        spec.filename.as_deref().unwrap_or_default()
    )
    .to_ascii_lowercase();

    haystack.contains(" silero")
        || haystack.contains("silero ")
        || haystack.contains("-vad")
        || haystack.contains("_vad")
        || haystack.contains(" vad")
        || haystack.contains("vad ")
        || haystack.ends_with("vad")
}

#[cfg(test)]
mod tests {
    use super::{
        CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
        ManagedModelBackendId, ModelSpec, RuntimePresets, StoredModelConfig, UnifiedModel,
        UnifiedModelKind, UnifiedModelStatus, default_model_capabilities,
        upgrade_stored_model_config,
    };
    use chrono::Utc;
    use serde_json::json;
    use slab_types::Capability;
    use std::collections::BTreeMap;

    #[test]
    fn legacy_stored_model_config_defaults_versions_during_deserialization() {
        let config: StoredModelConfig = serde_json::from_value(json!({
            "id": "cloud-model",
            "display_name": "Cloud Model",
            "kind": "cloud",
            "status": "ready",
            "spec": {
                "provider_id": "openai-main",
                "remote_model_id": "gpt-4.1-mini"
            }
        }))
        .expect("deserialize legacy config");

        assert_eq!(config.schema_version, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION);
        assert_eq!(config.policy_version, CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION);
        assert!(config.capabilities.is_empty());
    }

    #[test]
    fn upgrading_legacy_stored_model_config_restores_default_capabilities() {
        let config = upgrade_stored_model_config(
            serde_json::from_value(json!({
                "id": "cloud-model",
                "display_name": "Cloud Model",
                "kind": "cloud",
                "status": "ready",
                "spec": {
                    "provider_id": "openai-main",
                    "remote_model_id": "gpt-4.1-mini"
                }
            }))
            .expect("deserialize legacy config"),
        )
        .expect("upgrade legacy config");

        assert_eq!(config.schema_version, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION);
        assert_eq!(config.policy_version, CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION);
        assert_eq!(
            config.capabilities,
            vec![Capability::TextGeneration, Capability::ChatGeneration]
        );
    }

    #[test]
    fn unified_model_conversion_uses_current_config_versions() {
        let config: StoredModelConfig = UnifiedModel {
            id: "local-qwen".to_owned(),
            display_name: "Local Qwen".to_owned(),
            kind: UnifiedModelKind::Local,
            backend_id: Some(ManagedModelBackendId::GgmlLlama),
            capabilities: vec![Capability::TextGeneration, Capability::ChatGeneration],
            status: UnifiedModelStatus::NotDownloaded,
            spec: ModelSpec {
                repo_id: Some("bartowski/Qwen2.5-7B-Instruct-GGUF".to_owned()),
                filename: Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf".to_owned()),
                context_window: Some(8192),
                ..ModelSpec::default()
            },
            runtime_presets: Some(RuntimePresets { temperature: Some(0.7), top_p: Some(0.95) }),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
        .into();

        assert_eq!(config.schema_version, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION);
        assert_eq!(config.policy_version, CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION);
    }

    #[test]
    fn future_schema_versions_are_rejected() {
        let error = upgrade_stored_model_config(StoredModelConfig {
            schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION + 1,
            policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
            id: "cloud-model".to_owned(),
            display_name: "Cloud Model".to_owned(),
            kind: UnifiedModelKind::Cloud,
            backend_id: None,
            capabilities: vec![Capability::TextGeneration, Capability::ChatGeneration],
            status: Some(UnifiedModelStatus::Ready),
            spec: ModelSpec {
                provider_id: Some("openai-main".to_owned()),
                remote_model_id: Some("gpt-4.1-mini".to_owned()),
                ..ModelSpec::default()
            },
            runtime_presets: None,
            materialized_artifacts: BTreeMap::new(),
            pack_selection: None,
        })
        .expect_err("future schema version should fail");

        assert!(error.contains("unsupported stored model config schema_version"));
    }

    #[test]
    fn future_policy_versions_are_rejected() {
        let error = upgrade_stored_model_config(StoredModelConfig {
            schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
            policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION + 1,
            id: "cloud-model".to_owned(),
            display_name: "Cloud Model".to_owned(),
            kind: UnifiedModelKind::Cloud,
            backend_id: None,
            capabilities: vec![Capability::TextGeneration, Capability::ChatGeneration],
            status: Some(UnifiedModelStatus::Ready),
            spec: ModelSpec {
                provider_id: Some("openai-main".to_owned()),
                remote_model_id: Some("gpt-4.1-mini".to_owned()),
                ..ModelSpec::default()
            },
            runtime_presets: None,
            materialized_artifacts: BTreeMap::new(),
            pack_selection: None,
        })
        .expect_err("future policy version should fail");

        assert!(error.contains("unsupported stored model config policy_version"));
    }

    #[test]
    fn default_capabilities_distinguish_chat_vad_and_video_models() {
        let chat_caps = default_model_capabilities(
            UnifiedModelKind::Local,
            Some(ManagedModelBackendId::GgmlLlama),
            "Local Chat",
            &ModelSpec::default(),
        );
        assert_eq!(chat_caps, vec![Capability::TextGeneration, Capability::ChatGeneration]);

        let vad_caps = default_model_capabilities(
            UnifiedModelKind::Local,
            Some(ManagedModelBackendId::GgmlWhisper),
            "Silero VAD",
            &ModelSpec { filename: Some("silero_vad.bin".into()), ..ModelSpec::default() },
        );
        assert_eq!(vad_caps, vec![Capability::AudioVad]);

        let video_caps = default_model_capabilities(
            UnifiedModelKind::Local,
            Some(ManagedModelBackendId::GgmlDiffusion),
            "Diffusion",
            &ModelSpec::default(),
        );
        assert_eq!(video_caps, vec![Capability::ImageGeneration, Capability::VideoGeneration]);
    }
}
