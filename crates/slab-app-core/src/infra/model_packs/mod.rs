use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use slab_model_pack::{
    BackendConfigDocument, BackendConfigScope, ConfigEntryRef, ConfigRef, EngineTarget,
    MANIFEST_FILE_NAME, MODEL_PACK_SCHEMA_VERSION, ModelPack, ModelPackCatalogSummary,
    ModelPackError, ModelPackLoadDefaults, ModelPackManifest, ModelPackRuntimeBridge,
    PackDeployment, PackDocument, PackPricing, PackSource, PackSourceCandidate, PackSourceFile,
    PresetDocument, PresetEntryRef, VariantDocument,
};
use slab_types::{ArtifactFormat, DiffusionLoadOptions, ModelFamily, RuntimeBackendId};
use slab_utils::hash::{sha256_hex_bytes, verify_sha256_hex_expected};

use crate::domain::models::{
    CreateModelCommand, ManagedModelBackendId, ModelSpec, StoredModelConfig, UnifiedModel,
    UnifiedModelKind, validate_stored_model_config,
};
use crate::error::AppCoreError;

mod archive;
mod command;

use self::archive::{
    build_pack_bytes, collect_pack_entries, manifest_sha256_from_pack_bytes, model_pack_file_name,
    read_pack_bytes, read_pack_entry_bytes, write_bytes_file,
};
use self::command::build_model_command;

const INTERNAL_MODEL_STATE_ENTRY: &str = "internal/stored-model-config";
const GENERATED_VARIANT_ID: &str = "default-variant";
const GENERATED_VARIANT_PATH: &str = "models/variants/default.json";
const GENERATED_PRESET_ID: &str = "default";
const GENERATED_PRESET_PATH: &str = "models/presets/default.json";
const GENERATED_LOAD_CONFIG_PATH: &str = "models/configs/load.json";
const GENERATED_INFERENCE_CONFIG_ID: &str = "inference";
const GENERATED_INFERENCE_CONFIG_PATH: &str = "models/configs/inference.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedModelPackState {
    manifest_sha256: String,
    config: StoredModelConfig,
}

pub fn open_model_pack(path: &Path) -> Result<ModelPack, AppCoreError> {
    ModelPack::open(path).map_err(map_model_pack_error)
}

pub fn open_model_pack_from_bytes(bytes: &[u8]) -> Result<ModelPack, AppCoreError> {
    ModelPack::from_bytes(bytes).map_err(map_model_pack_error)
}

pub fn read_model_pack_summary(path: &Path) -> Result<ModelPackCatalogSummary, AppCoreError> {
    let pack = open_model_pack(path)?;
    let resolved = pack.resolve().map_err(map_model_pack_error)?;
    Ok(resolved.catalog_summary())
}

pub fn read_model_pack_summary_from_bytes(
    bytes: &[u8],
) -> Result<ModelPackCatalogSummary, AppCoreError> {
    let pack = open_model_pack_from_bytes(bytes)?;
    let resolved = pack.resolve().map_err(map_model_pack_error)?;
    Ok(resolved.catalog_summary())
}

pub fn list_model_pack_paths(dir: &Path) -> Result<Vec<PathBuf>, AppCoreError> {
    let mut paths = Vec::new();
    if !dir.exists() {
        return Ok(paths);
    }

    for entry in fs::read_dir(dir).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to read model pack directory '{}': {error}",
            dir.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to iterate model pack directory '{}': {error}",
                dir.display()
            ))
        })?;
        let path = entry.path();
        let is_pack = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("slab"))
            .unwrap_or(false);
        if path.is_file() && is_pack {
            paths.push(path);
        }
    }

    paths.sort();
    Ok(paths)
}

pub fn read_model_pack_runtime_bridge(path: &Path) -> Result<ModelPackRuntimeBridge, AppCoreError> {
    let pack = open_model_pack(path)?;
    let resolved = pack.resolve().map_err(map_model_pack_error)?;
    resolved.compile_default_runtime_bridge().map_err(map_model_pack_error)
}

pub fn read_model_pack_runtime_bridge_from_bytes(
    bytes: &[u8],
) -> Result<ModelPackRuntimeBridge, AppCoreError> {
    let pack = open_model_pack_from_bytes(bytes)?;
    let resolved = pack.resolve().map_err(map_model_pack_error)?;
    resolved.compile_default_runtime_bridge().map_err(map_model_pack_error)
}

pub fn build_model_command_from_pack_bytes(
    path: &Path,
    bytes: &[u8],
) -> Result<CreateModelCommand, AppCoreError> {
    let pack = open_model_pack_from_bytes(bytes)?;
    let resolved = pack.resolve().map_err(map_model_pack_error)?;
    let mut command = build_model_command(path, pack.manifest(), &resolved)?;
    if let Some(config) = read_persisted_model_config_from_pack_bytes(bytes)? {
        apply_persisted_projection_state(&mut command, &config);
    }
    Ok(command)
}

pub fn model_pack_file_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(model_pack_file_name(id))
}

pub fn ensure_model_pack_dir(path: &Path) -> Result<(), AppCoreError> {
    fs::create_dir_all(path).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to create model pack directory '{}': {error}",
            path.display()
        ))
    })
}

pub fn write_model_pack(dir: &Path, id: &str, bytes: &[u8]) -> Result<PathBuf, AppCoreError> {
    ensure_model_pack_dir(dir)?;

    let path = model_pack_file_path(dir, id);
    write_bytes_file(&path, bytes)?;
    Ok(path)
}

pub fn write_imported_model_pack(
    dir: &Path,
    model: &UnifiedModel,
    bytes: &[u8],
) -> Result<PathBuf, AppCoreError> {
    write_model_pack(dir, &model.id, bytes)
}

pub fn read_persisted_model_config_from_pack(
    path: &Path,
) -> Result<Option<StoredModelConfig>, AppCoreError> {
    read_persisted_model_config_from_pack_bytes(&read_pack_bytes(path)?)
}

pub fn write_persisted_model_pack(
    dir: &Path,
    model: &UnifiedModel,
) -> Result<PathBuf, AppCoreError> {
    let mut config: StoredModelConfig = model.clone().into();
    let path = model_pack_file_path(dir, &config.id);
    if path.exists()
        && let Some(existing) = read_persisted_model_config_from_pack(&path)?
    {
        config.materialized_artifacts = existing.materialized_artifacts;
    }
    write_persisted_model_pack_from_config(dir, &config)
}

pub fn write_persisted_model_pack_from_config(
    dir: &Path,
    config: &StoredModelConfig,
) -> Result<PathBuf, AppCoreError> {
    ensure_model_pack_dir(dir)?;

    let path = model_pack_file_path(dir, &config.id);
    let mut next_config = config.clone();
    next_config.pack_selection = None;
    let payload = if path.exists() {
        attach_persisted_state_to_pack_bytes(&read_pack_bytes(&path)?, &next_config)?
    } else {
        build_generated_model_pack_bytes(&next_config)?
    };

    write_bytes_file(&path, &payload)?;
    Ok(path)
}

pub fn delete_model_pack(dir: &Path, id: &str) -> Result<bool, AppCoreError> {
    delete_model_pack_at_path(&model_pack_file_path(dir, id))
}

pub fn delete_model_pack_at_path(path: &Path) -> Result<bool, AppCoreError> {
    if !path.exists() {
        return Ok(false);
    }

    fs::remove_file(path).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to remove model pack file '{}': {error}",
            path.display()
        ))
    })?;

    Ok(true)
}

pub fn build_model_pack_load_target(path: &Path) -> Result<ModelPackLoadTarget, AppCoreError> {
    let bridge = read_model_pack_runtime_bridge(path)?;
    let default_preset = bridge
        .model_spec
        .metadata
        .get("default_preset")
        .cloned()
        .unwrap_or_else(|| "default".to_owned());
    model_pack_load_target_from_bridge(path, &default_preset, bridge)
}

pub fn is_model_pack_path(path: &str) -> bool {
    path.trim().to_ascii_lowercase().ends_with(".slab")
}

#[derive(Debug, Clone)]
pub struct ModelPackLoadTarget {
    pub backend_id: RuntimeBackendId,
    pub model_path: String,
    pub load_defaults: ModelPackLoadDefaults,
    pub candidates: Vec<ModelPackLoadCandidate>,
}

#[derive(Debug, Clone)]
pub struct ModelPackLoadCandidate {
    pub backend_id: RuntimeBackendId,
    pub model_path: String,
    pub load_defaults: ModelPackLoadDefaults,
}

pub fn model_pack_load_target_from_bridge(
    path: &Path,
    preset_id: &str,
    bridge: ModelPackRuntimeBridge,
) -> Result<ModelPackLoadTarget, AppCoreError> {
    let mut candidates = Vec::new();
    for engine in bridge.engine_load_specs {
        let load_spec = engine.runtime_load_spec(preset_id).map_err(|error| match error {
            ModelPackError::NonMaterializedSource { .. } => AppCoreError::BadRequest(format!(
                "model pack '{}' points to a remote source and must be downloaded from the model catalog before loading",
                path.display()
            )),
            other => map_model_pack_error(other),
        })?;
        candidates.push(ModelPackLoadCandidate {
            backend_id: engine.backend,
            model_path: load_spec.model_path().to_string_lossy().into_owned(),
            load_defaults: engine.load_defaults,
        });
    }
    let primary = candidates.first().cloned().ok_or_else(|| {
        AppCoreError::BadRequest(format!(
            "model pack '{}' produced no compatible runtime engine candidates",
            path.display()
        ))
    })?;

    Ok(ModelPackLoadTarget {
        backend_id: primary.backend_id,
        model_path: primary.model_path,
        load_defaults: primary.load_defaults,
        candidates,
    })
}

fn read_persisted_model_config_from_pack_bytes(
    bytes: &[u8],
) -> Result<Option<StoredModelConfig>, AppCoreError> {
    let Some(state_bytes) = read_pack_entry_bytes(bytes, INTERNAL_MODEL_STATE_ENTRY)? else {
        return Ok(None);
    };

    let Ok(state_json) = serde_json::from_slice::<Value>(&state_bytes) else {
        return Ok(None);
    };
    let Some(state) = state_json.as_object() else {
        return Ok(None);
    };
    let Some(manifest_sha256) = state.get("manifest_sha256").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(config_json) = state.get("config") else {
        return Ok(None);
    };

    let actual_manifest_sha256 = manifest_sha256_from_pack_bytes(bytes)?;
    if verify_sha256_hex_expected(&actual_manifest_sha256, manifest_sha256).is_err() {
        tracing::warn!(
            expected_manifest_sha256 = manifest_sha256,
            actual_manifest_sha256,
            "ignoring persisted model pack config because manifest sha256 does not match"
        );
        return Ok(None);
    }

    let config =
        serde_json::from_value::<StoredModelConfig>(config_json.clone()).map_err(|error| {
            AppCoreError::BadRequest(format!("invalid persisted model config: {error}"))
        })?;
    let config = validate_stored_model_config(config).map_err(|error| {
        AppCoreError::BadRequest(format!("invalid persisted model config: {error}"))
    })?;

    Ok(Some(config))
}

fn attach_persisted_state_to_pack_bytes(
    bytes: &[u8],
    config: &StoredModelConfig,
) -> Result<Vec<u8>, AppCoreError> {
    let manifest_sha256 = manifest_sha256_from_pack_bytes(bytes)?;
    let mut entries = collect_pack_entries(bytes)?;
    entries.retain(|(path, _)| path != INTERNAL_MODEL_STATE_ENTRY);
    entries.push((
        INTERNAL_MODEL_STATE_ENTRY.to_owned(),
        build_persisted_state_bytes(&manifest_sha256, config)?,
    ));
    build_pack_bytes(entries)
}

fn apply_persisted_projection_state(
    command: &mut CreateModelCommand,
    persisted: &StoredModelConfig,
) {
    if let Some(selected_download_source) = persisted.selected_download_source.as_ref() {
        apply_selected_download_source_to_spec(&mut command.spec, selected_download_source);
        command.spec.local_path = persisted.spec.local_path.clone();
        if let Some(status) = persisted.status.clone() {
            command.status = Some(status);
        }
        return;
    }

    if same_download_source(&persisted.spec, &command.spec) {
        command.spec.local_path = persisted.spec.local_path.clone();
        if let Some(status) = persisted.status.clone() {
            command.status = Some(status);
        }
    }
}

fn apply_selected_download_source_to_spec(
    spec: &mut ModelSpec,
    selected_download_source: &crate::domain::models::SelectedModelDownloadSource,
) {
    spec.repo_id = Some(selected_download_source.repo_id.clone());
    spec.filename = Some(selected_download_source.filename.clone());
    spec.hub_provider = selected_download_source.hub_provider.clone();
}

fn same_download_source(current: &ModelSpec, next: &ModelSpec) -> bool {
    match (current.repo_id.as_deref(), next.repo_id.as_deref()) {
        (Some(_), Some(_)) => {
            current.repo_id == next.repo_id
                && current.filename == next.filename
                && current.hub_provider == next.hub_provider
        }
        (None, None) => current.local_path == next.local_path,
        _ => false,
    }
}

fn build_generated_model_pack_bytes(config: &StoredModelConfig) -> Result<Vec<u8>, AppCoreError> {
    let mut entries = build_generated_pack_entries(config)?;
    let manifest_sha256 = entries
        .iter()
        .find_map(|(path, payload)| (path == MANIFEST_FILE_NAME).then(|| sha256_hex_bytes(payload)))
        .ok_or_else(|| {
            AppCoreError::Internal("generated model pack is missing manifest.json".into())
        })?;
    entries.push((
        INTERNAL_MODEL_STATE_ENTRY.to_owned(),
        build_persisted_state_bytes(&manifest_sha256, config)?,
    ));
    build_pack_bytes(entries)
}

fn build_generated_pack_entries(
    config: &StoredModelConfig,
) -> Result<Vec<(String, Vec<u8>)>, AppCoreError> {
    let mut manifest = build_generated_manifest(config);
    let mut entries = Vec::new();

    if infer_runtime_backend_from_config(config).is_some() {
        let variant_ref = ConfigEntryRef {
            id: GENERATED_VARIANT_ID.to_owned(),
            label: "Default Variant".to_owned(),
            description: Some("Generated from catalog state".to_owned()),
            config_ref: ConfigRef::parse(format!("ref://{GENERATED_VARIANT_PATH}"))
                .map_err(map_model_pack_error)?,
        };
        let preset_ref = PresetEntryRef {
            id: GENERATED_PRESET_ID.to_owned(),
            label: "Default".to_owned(),
            description: Some("Generated from catalog state".to_owned()),
            config_ref: ConfigRef::parse(format!("ref://{GENERATED_PRESET_PATH}"))
                .map_err(map_model_pack_error)?,
        };

        let load_config_ref = build_generated_load_config(config)
            .transpose()?
            .map(|document| {
                let config_ref = ConfigRef::parse(format!("ref://{GENERATED_LOAD_CONFIG_PATH}"))
                    .map_err(map_model_pack_error)?;
                entries.push((
                    GENERATED_LOAD_CONFIG_PATH.to_owned(),
                    serialize_pretty_json(
                        &PackDocument::BackendConfig(document),
                        "generated load config",
                    )?,
                ));
                Ok::<ConfigRef, AppCoreError>(config_ref)
            })
            .transpose()?;

        let inference_config_ref = build_generated_inference_config(config)
            .transpose()?
            .map(|document| {
                let config_ref =
                    ConfigRef::parse(format!("ref://{GENERATED_INFERENCE_CONFIG_PATH}"))
                        .map_err(map_model_pack_error)?;
                entries.push((
                    GENERATED_INFERENCE_CONFIG_PATH.to_owned(),
                    serialize_pretty_json(
                        &PackDocument::BackendConfig(document),
                        "generated inference config",
                    )?,
                ));
                Ok::<ConfigRef, AppCoreError>(config_ref)
            })
            .transpose()?;

        let variant = VariantDocument {
            id: GENERATED_VARIANT_ID.to_owned(),
            label: "Default Variant".to_owned(),
            description: Some("Generated from catalog state".to_owned()),
            format: infer_artifact_format_from_config(config),
            sources: Vec::new(),
            component_ids: Vec::new(),
            load_config: load_config_ref,
            metadata: BTreeMap::new(),
        };
        let preset = PresetDocument {
            id: GENERATED_PRESET_ID.to_owned(),
            label: "Default".to_owned(),
            variant_id: GENERATED_VARIANT_ID.to_owned(),
            description: Some("Generated from catalog state".to_owned()),
            adapter_ids: Vec::new(),
            inference_config: inference_config_ref,
            footprint: Default::default(),
            metadata: BTreeMap::new(),
        };

        manifest.variants.push(variant_ref);
        manifest.presets.push(preset_ref);
        manifest.default_preset = Some(GENERATED_PRESET_ID.to_owned());

        entries.push((
            GENERATED_VARIANT_PATH.to_owned(),
            serialize_pretty_json(&PackDocument::Variant(variant), "generated variant")?,
        ));
        entries.push((
            GENERATED_PRESET_PATH.to_owned(),
            serialize_pretty_json(&PackDocument::Preset(preset), "generated preset")?,
        ));
    }

    entries.insert(
        0,
        (MANIFEST_FILE_NAME.to_owned(), serialize_pretty_json(&manifest, "generated manifest")?),
    );
    Ok(entries)
}

fn build_generated_manifest(config: &StoredModelConfig) -> ModelPackManifest {
    let family = infer_model_family(config.kind, config.backend_id);
    let mut metadata = BTreeMap::new();
    metadata.insert("generated_by".into(), "slab-app-core".into());
    let backend = infer_runtime_backend_from_config(config);

    ModelPackManifest {
        schema: None,
        schema_version: MODEL_PACK_SCHEMA_VERSION,
        deployment: PackDeployment::Local,
        id: config.id.clone(),
        label: config.display_name.clone(),
        family,
        capabilities: config.capabilities.clone(),
        context_window: config.spec.context_window,
        pricing: config
            .spec
            .pricing
            .as_ref()
            .map(|pricing| PackPricing { input: pricing.input, output: pricing.output }),
        metadata,
        engines: backend
            .map(|id| vec![EngineTarget { id, format: infer_artifact_format_from_config(config) }])
            .unwrap_or_default(),
        sources: build_pack_sources_from_config(config),
        components: Vec::new(),
        variants: Vec::new(),
        adapters: Vec::new(),
        presets: Vec::new(),
        default_preset: None,
        footprint: Default::default(),
    }
}

fn build_pack_sources_from_config(config: &StoredModelConfig) -> Vec<PackSourceCandidate> {
    if config.kind == UnifiedModelKind::Cloud {
        return Vec::new();
    }

    if let Some(local_path) =
        config.spec.local_path.as_deref().filter(|path| !is_model_pack_path(path))
    {
        return vec![PackSourceCandidate::new(PackSource::LocalPath {
            path: local_path.to_owned(),
        })];
    }

    if let (Some(repo_id), Some(filename)) =
        (config.spec.repo_id.as_deref(), config.spec.filename.as_deref())
    {
        return vec![PackSourceCandidate::new(build_remote_pack_source_from_spec(
            repo_id,
            filename,
            config.spec.hub_provider.as_deref(),
        ))];
    }

    Vec::new()
}

fn build_remote_pack_source_from_spec(
    repo_id: &str,
    filename: &str,
    hub_provider: Option<&str>,
) -> PackSource {
    let files = vec![PackSourceFile {
        id: "model".to_owned(),
        label: None,
        description: None,
        path: filename.to_owned(),
    }];

    match hub_provider
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase().replace('-', "_"))
        .as_deref()
    {
        Some("models_cat") | Some("modelscope") | Some("model_scope") => {
            PackSource::ModelScope { repo_id: repo_id.to_owned(), revision: None, files }
        }
        _ => PackSource::HuggingFace { repo_id: repo_id.to_owned(), revision: None, files },
    }
}

fn build_generated_load_config(
    _config: &StoredModelConfig,
) -> Option<Result<BackendConfigDocument, AppCoreError>> {
    None
}

fn build_generated_inference_config(
    config: &StoredModelConfig,
) -> Option<Result<BackendConfigDocument, AppCoreError>> {
    let mut payload = Map::new();
    if let Some(runtime_presets) = config.runtime_presets.as_ref() {
        if let Some(temperature) = runtime_presets.temperature {
            payload.insert("temperature".into(), Value::from(temperature));
        }
        if let Some(top_p) = runtime_presets.top_p {
            payload.insert("top_p".into(), Value::from(top_p));
        }
    }

    (!payload.is_empty()).then_some(Ok(BackendConfigDocument {
        id: Some(GENERATED_INFERENCE_CONFIG_ID.to_owned()),
        label: "Generated Inference Config".to_owned(),
        scope: BackendConfigScope::Inference,
        description: Some("Generated from catalog state".to_owned()),
        payload: Value::Object(payload),
        metadata: BTreeMap::new(),
    }))
}

fn serialize_pretty_json<T: Serialize>(value: &T, label: &str) -> Result<Vec<u8>, AppCoreError> {
    let mut payload = serde_json::to_vec_pretty(value)
        .map_err(|error| AppCoreError::Internal(format!("failed to serialize {label}: {error}")))?;
    payload.push(b'\n');
    Ok(payload)
}

fn build_persisted_state_bytes(
    manifest_sha256: &str,
    config: &StoredModelConfig,
) -> Result<Vec<u8>, AppCoreError> {
    serialize_pretty_json(
        &PersistedModelPackState {
            manifest_sha256: manifest_sha256.to_owned(),
            config: config.clone(),
        },
        "persisted model pack state",
    )
}

fn infer_runtime_backend_from_config(config: &StoredModelConfig) -> Option<RuntimeBackendId> {
    if config.kind != UnifiedModelKind::Local {
        return None;
    }

    config.backend_id.map(Into::into)
}

fn infer_artifact_format_from_config(config: &StoredModelConfig) -> ArtifactFormat {
    let candidate = config
        .spec
        .filename
        .as_deref()
        .or(config.spec.local_path.as_deref())
        .or_else(|| config.materialized_artifacts.values().next().map(String::as_str));
    if let Some(path) = candidate.map(str::trim).filter(|value| !value.is_empty()) {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".gguf") {
            return ArtifactFormat::Gguf;
        }
        if lower.ends_with(".safetensors") {
            return ArtifactFormat::Safetensors;
        }
        if lower.ends_with(".onnx") {
            return ArtifactFormat::Onnx;
        }
        if lower.ends_with(".ckpt") {
            return ArtifactFormat::Ckpt;
        }
        if lower.ends_with(".bin") {
            return ArtifactFormat::Ggml;
        }
    }

    match config.backend_id {
        Some(ManagedModelBackendId::GgmlWhisper) => ArtifactFormat::Ggml,
        Some(ManagedModelBackendId::GgmlDiffusion) => ArtifactFormat::Ckpt,
        Some(ManagedModelBackendId::CandleLlama)
        | Some(ManagedModelBackendId::CandleWhisper)
        | Some(ManagedModelBackendId::CandleDiffusion) => ArtifactFormat::Safetensors,
        Some(ManagedModelBackendId::GgmlLlama) | None => ArtifactFormat::Gguf,
    }
}

fn infer_model_family(
    kind: UnifiedModelKind,
    backend_id: Option<ManagedModelBackendId>,
) -> ModelFamily {
    let Some(backend_id) = (kind == UnifiedModelKind::Local).then_some(backend_id).flatten() else {
        return ModelFamily::Llama;
    };

    match backend_id {
        ManagedModelBackendId::GgmlWhisper | ManagedModelBackendId::CandleWhisper => {
            ModelFamily::Whisper
        }
        ManagedModelBackendId::GgmlDiffusion | ManagedModelBackendId::CandleDiffusion => {
            ModelFamily::Diffusion
        }
        ManagedModelBackendId::GgmlLlama | ManagedModelBackendId::CandleLlama => ModelFamily::Llama,
    }
}

pub fn merge_diffusion_load_defaults(
    pack: Option<DiffusionLoadOptions>,
    settings: Option<DiffusionLoadOptions>,
) -> Option<DiffusionLoadOptions> {
    pack.or(settings)
}

fn map_model_pack_error(error: ModelPackError) -> AppCoreError {
    match error {
        ModelPackError::ReadPack { .. }
        | ModelPackError::OpenArchive { .. }
        | ModelPackError::AccessArchiveEntry { .. }
        | ModelPackError::ReadArchiveEntry { .. } => AppCoreError::Internal(error.to_string()),
        _ => AppCoreError::BadRequest(error.to_string()),
    }
}

#[cfg(test)]
mod tests;
