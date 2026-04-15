use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use slab_model_pack::{
    BackendConfigDocument, BackendConfigScope, ConfigEntryRef, ConfigRef, MANIFEST_FILE_NAME,
    ModelPack, ModelPackCatalogSummary, ModelPackError, ModelPackLoadDefaults, ModelPackManifest,
    ModelPackRuntimeBridge, PACK_EXTENSION, PackDocument, PackModelStatus, PackPricing,
    PackRuntimePresets, PackSource, PackSourceCandidate, PackSourceFile, PresetDocument,
    PresetEntryRef, VariantDocument,
};
use slab_types::{DiffusionLoadOptions, DriverHints, ModelFamily, RuntimeBackendId};
use uuid::Uuid;
use zip::CompressionMethod;
use zip::ZipArchive;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::domain::models::{
    CreateModelCommand, ManagedModelBackendId, ModelSpec, Pricing, RuntimePresets,
    StoredModelConfig, UnifiedModel, UnifiedModelKind, UnifiedModelStatus,
    upgrade_stored_model_config,
};
use crate::error::AppCoreError;

const INTERNAL_MODEL_STATE_ENTRY: &str = "internal/stored-model-config";
const GENERATED_VARIANT_ID: &str = "default-variant";
const GENERATED_VARIANT_PATH: &str = "models/variants/default.json";
const GENERATED_PRESET_ID: &str = "default";
const GENERATED_PRESET_PATH: &str = "models/presets/default.json";
const GENERATED_LOAD_CONFIG_ID: &str = "load";
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

pub fn build_model_command_from_pack(path: &Path) -> Result<CreateModelCommand, AppCoreError> {
    let bytes = read_pack_bytes(path)?;
    build_model_command_from_pack_bytes(path, &bytes)
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

fn build_model_command(
    path: &Path,
    manifest: &ModelPackManifest,
    resolved: &slab_model_pack::ResolvedModelPack,
) -> Result<CreateModelCommand, AppCoreError> {
    match manifest.sources.first().map(|candidate| &candidate.source) {
        Some(PackSource::Cloud { provider_id, remote_model_id }) => {
            build_cloud_model_command(manifest, provider_id, remote_model_id)
        }
        _ => build_local_model_command(path, manifest, resolved),
    }
}

pub fn build_model_pack_load_target(path: &Path) -> Result<ModelPackLoadTarget, AppCoreError> {
    let bridge = read_model_pack_runtime_bridge(path)?;
    let default_preset =
        bridge.model_spec.metadata.get("default_preset").map(String::as_str).unwrap_or("default");
    let load_spec = bridge
        .runtime_load_spec(default_preset)
        .map_err(|error| match error {
            ModelPackError::NonMaterializedSource { .. } => AppCoreError::BadRequest(format!(
                "model pack '{}' points to a remote source and must be downloaded from the model catalog before loading",
                path.display()
            )),
            other => map_model_pack_error(other),
        })?;

    Ok(ModelPackLoadTarget {
        backend_id: bridge.backend,
        model_path: load_spec.model_path.to_string_lossy().into_owned(),
        load_defaults: bridge.load_defaults,
    })
}

pub fn is_model_pack_path(path: &str) -> bool {
    path.trim().to_ascii_lowercase().ends_with(".slab")
}

#[derive(Debug, Clone)]
pub struct ModelPackLoadTarget {
    pub backend_id: RuntimeBackendId,
    pub model_path: String,
    pub load_defaults: ModelPackLoadDefaults,
}

fn default_status_for_runtime_bridge(bridge: &ModelPackRuntimeBridge) -> UnifiedModelStatus {
    match bridge.model_spec.source {
        slab_types::ModelSource::HuggingFace { .. } => UnifiedModelStatus::NotDownloaded,
        _ => UnifiedModelStatus::Ready,
    }
}

fn build_runtime_presets(options: &slab_types::JsonOptions) -> Option<RuntimePresets> {
    let temperature = options.get("temperature").and_then(value_to_f32);
    let top_p = options.get("top_p").and_then(value_to_f32);

    (temperature.is_some() || top_p.is_some()).then_some(RuntimePresets { temperature, top_p })
}

fn value_to_f32(value: &serde_json::Value) -> Option<f32> {
    value.as_f64().map(|value| value as f32)
}

fn read_pack_bytes(path: &Path) -> Result<Vec<u8>, AppCoreError> {
    fs::read(path).map_err(|error| {
        AppCoreError::Internal(format!("failed to read model pack '{}': {error}", path.display()))
    })
}

fn model_pack_file_name(id: &str) -> String {
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(id.as_bytes());
    format!("{encoded}.{PACK_EXTENSION}")
}

fn write_bytes_file(path: &Path, payload: &[u8]) -> Result<(), AppCoreError> {
    let parent = path.parent().ok_or_else(|| {
        AppCoreError::Internal(format!(
            "model pack path '{}' has no parent directory",
            path.display()
        ))
    })?;
    ensure_model_pack_dir(parent)?;

    let file_name = path.file_name().and_then(|value| value.to_str()).ok_or_else(|| {
        AppCoreError::Internal(format!(
            "model pack path '{}' has an invalid file name",
            path.display()
        ))
    })?;
    let temp_path = parent.join(format!(".{}.tmp-{}", file_name, Uuid::new_v4()));

    let write_result = (|| -> Result<(), AppCoreError> {
        let mut temp_file =
            OpenOptions::new().create_new(true).write(true).open(&temp_path).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to create temp model pack file '{}': {error}",
                    temp_path.display()
                ))
            })?;

        temp_file.write_all(payload).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to write temp model pack file '{}': {error}",
                temp_path.display()
            ))
        })?;
        temp_file.flush().map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to flush temp model pack file '{}': {error}",
                temp_path.display()
            ))
        })?;
        temp_file.sync_all().map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to sync temp model pack file '{}': {error}",
                temp_path.display()
            ))
        })?;
        drop(temp_file);

        if path.exists() {
            fs::remove_file(path).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to replace existing model pack file '{}': {error}",
                    path.display()
                ))
            })?;
        }

        fs::rename(&temp_path, path).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to finalize model pack file '{}': {error}",
                path.display()
            ))
        })?;

        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result
}

fn read_persisted_model_config_from_pack_bytes(
    bytes: &[u8],
) -> Result<Option<StoredModelConfig>, AppCoreError> {
    let Some(state_bytes) = read_pack_entry_bytes(bytes, INTERNAL_MODEL_STATE_ENTRY)? else {
        return Ok(None);
    };

    let Ok(state) = serde_json::from_slice::<PersistedModelPackState>(&state_bytes) else {
        return Ok(None);
    };

    let manifest_sha256 = manifest_sha256_from_pack_bytes(bytes)?;
    if state.manifest_sha256 != manifest_sha256 {
        return Ok(None);
    }

    let config = upgrade_stored_model_config(state.config).map_err(|error| {
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
        .find_map(|(path, payload)| (path == MANIFEST_FILE_NAME).then(|| hash_bytes_hex(payload)))
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
            variant_id: None,
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
            sources: Vec::new(),
            component_ids: Vec::new(),
            load_config: load_config_ref,
            inference_config: inference_config_ref,
            metadata: BTreeMap::new(),
        };
        let preset = PresetDocument {
            id: GENERATED_PRESET_ID.to_owned(),
            label: "Default".to_owned(),
            variant_id: Some(GENERATED_VARIANT_ID.to_owned()),
            description: Some("Generated from catalog state".to_owned()),
            adapter_ids: Vec::new(),
            load_config: None,
            inference_config: None,
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

    ModelPackManifest {
        version: 1,
        id: config.id.clone(),
        label: config.display_name.clone(),
        status: config.status.clone().map(pack_status_from_unified),
        family,
        capabilities: config.capabilities.clone(),
        backend_hints: build_generated_backend_hints(config.backend_id),
        context_window: config.spec.context_window,
        pricing: config
            .spec
            .pricing
            .as_ref()
            .map(|pricing| PackPricing { input: pricing.input, output: pricing.output }),
        runtime_presets: config.runtime_presets.as_ref().and_then(pack_runtime_presets_from_model),
        metadata,
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
        if let (Some(provider_id), Some(remote_model_id)) =
            (config.spec.provider_id.as_deref(), config.spec.remote_model_id.as_deref())
        {
            return vec![PackSourceCandidate::new(PackSource::Cloud {
                provider_id: provider_id.to_owned(),
                remote_model_id: remote_model_id.to_owned(),
            })];
        }

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
        return vec![PackSourceCandidate {
            source: PackSource::HuggingFace {
                repo_id: repo_id.to_owned(),
                revision: None,
                files: vec![PackSourceFile {
                    id: "model".to_owned(),
                    label: None,
                    description: None,
                    path: filename.to_owned(),
                }],
            },
            hub_provider: pack_hub_provider_from_spec(config.spec.hub_provider.as_deref()),
            priority: None,
        }];
    }

    Vec::new()
}

fn pack_hub_provider_from_spec(
    hub_provider: Option<&str>,
) -> Option<slab_model_pack::PackHubProvider> {
    match hub_provider
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase().replace('-', "_"))
        .as_deref()
    {
        Some("hf") | Some("hf_hub") | Some("huggingface") | Some("hugging_face") => {
            Some(slab_model_pack::PackHubProvider::HuggingFace)
        }
        Some("models_cat") | Some("modelscope") | Some("model_scope") => {
            Some(slab_model_pack::PackHubProvider::ModelScope)
        }
        _ => None,
    }
}

fn build_generated_backend_hints(backend: Option<ManagedModelBackendId>) -> DriverHints {
    let Some(backend) = backend else {
        return DriverHints::default();
    };

    DriverHints {
        prefer_drivers: vec![backend.canonical_id().to_owned()],
        avoid_drivers: Vec::new(),
        require_streaming: false,
    }
}

fn build_generated_load_config(
    config: &StoredModelConfig,
) -> Option<Result<BackendConfigDocument, AppCoreError>> {
    let mut payload = Map::new();

    if let Some(context_window) = config.spec.context_window {
        payload.insert("context_length".into(), Value::from(context_window));
    }
    if let Some(chat_template) = config.spec.chat_template.as_deref() {
        payload.insert("chat_template".into(), Value::from(chat_template.to_owned()));
    }

    (!payload.is_empty()).then_some(Ok(BackendConfigDocument {
        id: GENERATED_LOAD_CONFIG_ID.to_owned(),
        label: "Generated Load Config".to_owned(),
        scope: BackendConfigScope::Load,
        description: Some("Generated from catalog state".to_owned()),
        payload: Value::Object(payload),
        metadata: BTreeMap::new(),
    }))
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
        id: GENERATED_INFERENCE_CONFIG_ID.to_owned(),
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

fn build_pack_bytes(entries: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>, AppCoreError> {
    let mut cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(&mut cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for (path, payload) in entries {
        writer.start_file(&path, options).map_err(|error| {
            AppCoreError::Internal(format!("failed to create pack entry '{path}': {error}"))
        })?;
        writer.write_all(&payload).map_err(|error| {
            AppCoreError::Internal(format!("failed to write pack entry '{path}': {error}"))
        })?;
    }

    writer.finish().map_err(|error| {
        AppCoreError::Internal(format!("failed to finalize model pack bytes: {error}"))
    })?;
    Ok(cursor.into_inner())
}

fn collect_pack_entries(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, AppCoreError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        AppCoreError::Internal(format!("failed to open model pack archive: {error}"))
    })?;
    let mut entries = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to access model pack archive entry {index}: {error}"
            ))
        })?;
        if entry.is_dir() {
            continue;
        }

        let path = entry.name().trim().to_owned();
        let mut payload = Vec::new();
        entry.read_to_end(&mut payload).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to read model pack archive entry '{path}': {error}"
            ))
        })?;
        entries.push((path, payload));
    }

    Ok(entries)
}

fn read_pack_entry_bytes(bytes: &[u8], entry_name: &str) -> Result<Option<Vec<u8>>, AppCoreError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        AppCoreError::Internal(format!("failed to open model pack archive: {error}"))
    })?;

    match archive.by_name(entry_name) {
        Ok(mut entry) => {
            let mut payload = Vec::new();
            entry.read_to_end(&mut payload).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to read model pack archive entry '{entry_name}': {error}"
                ))
            })?;
            Ok(Some(payload))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(error) => Err(AppCoreError::Internal(format!(
            "failed to access model pack archive entry '{entry_name}': {error}"
        ))),
    }
}

fn manifest_sha256_from_pack_bytes(bytes: &[u8]) -> Result<String, AppCoreError> {
    let manifest_bytes = read_pack_entry_bytes(bytes, MANIFEST_FILE_NAME)?.ok_or_else(|| {
        AppCoreError::BadRequest("missing required manifest.json in .slab archive".into())
    })?;
    Ok(hash_bytes_hex(&manifest_bytes))
}

fn hash_bytes_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

fn infer_runtime_backend_from_config(config: &StoredModelConfig) -> Option<RuntimeBackendId> {
    if config.kind != UnifiedModelKind::Local {
        return None;
    }

    config.backend_id.map(Into::into)
}

fn infer_model_family(
    kind: UnifiedModelKind,
    backend_id: Option<ManagedModelBackendId>,
) -> ModelFamily {
    let Some(backend_id) = (kind == UnifiedModelKind::Local).then_some(backend_id).flatten() else {
        return ModelFamily::Llama;
    };

    match backend_id {
        ManagedModelBackendId::GgmlWhisper => ModelFamily::Whisper,
        ManagedModelBackendId::GgmlDiffusion => ModelFamily::Diffusion,
        ManagedModelBackendId::GgmlLlama => ModelFamily::Llama,
    }
}

fn pack_status_from_unified(status: UnifiedModelStatus) -> PackModelStatus {
    match status {
        UnifiedModelStatus::Ready => PackModelStatus::Ready,
        UnifiedModelStatus::NotDownloaded => PackModelStatus::NotDownloaded,
        UnifiedModelStatus::Downloading => PackModelStatus::Downloading,
        UnifiedModelStatus::Error => PackModelStatus::Error,
    }
}

fn pack_runtime_presets_from_model(runtime_presets: &RuntimePresets) -> Option<PackRuntimePresets> {
    (runtime_presets.temperature.is_some() || runtime_presets.top_p.is_some()).then_some(
        PackRuntimePresets {
            temperature: runtime_presets.temperature,
            top_p: runtime_presets.top_p,
        },
    )
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
mod tests {
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::path::Path;

    use serde_json::json;
    use slab_types::Capability;
    use zip::CompressionMethod;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    use super::{
        attach_persisted_state_to_pack_bytes, build_generated_model_pack_bytes,
        build_model_command_from_pack_bytes, build_pack_bytes, collect_pack_entries,
        manifest_sha256_from_pack_bytes, read_persisted_model_config_from_pack_bytes,
    };
    use crate::domain::models::{
        CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
        ManagedModelBackendId, ModelPackSelection, ModelSpec, RuntimePresets, StoredModelConfig,
        UnifiedModelKind, UnifiedModelStatus,
    };
    use crate::error::AppCoreError;

    fn build_pack(entries: Vec<(&str, String)>) -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(&mut cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (path, content) in entries {
            writer.start_file(path, options).expect("start file");
            writer.write_all(content.as_bytes()).expect("write file");
        }

        writer.finish().expect("finish zip");
        cursor.into_inner()
    }

    #[test]
    fn builds_cloud_model_command_from_pack_manifest() {
        let bytes = build_pack(vec![(
            "manifest.json",
            json!({
                "version": 2,
                "id": "gpt_4_1_mini",
                "label": "GPT-4.1 mini",
                "status": "ready",
                "family": "llama",
                "context_window": 128000,
                "pricing": {
                    "input": 0.4,
                    "output": 1.6
                },
                "runtime_presets": {
                    "temperature": 0.7,
                    "top_p": 0.95
                },
                "source": {
                    "kind": "cloud",
                    "provider_id": "openai-main",
                    "remote_model_id": "gpt-4.1-mini"
                }
            })
            .to_string(),
        )]);

        let command = build_model_command_from_pack_bytes(Path::new("gpt-4.1-mini.slab"), &bytes)
            .expect("cloud command");

        assert_eq!(command.id.as_deref(), Some("gpt_4_1_mini"));
        assert_eq!(command.display_name, "GPT-4.1 mini");
        assert_eq!(command.kind, UnifiedModelKind::Cloud);
        assert_eq!(command.backend_id, None);
        assert_eq!(command.status, Some(UnifiedModelStatus::Ready));
        assert_eq!(command.spec.provider_id.as_deref(), Some("openai-main"));
        assert_eq!(command.spec.remote_model_id.as_deref(), Some("gpt-4.1-mini"));
        assert_eq!(command.spec.context_window, Some(128000));
        assert_eq!(command.spec.pricing.as_ref().map(|pricing| pricing.input), Some(0.4));
        assert_eq!(command.spec.pricing.as_ref().map(|pricing| pricing.output), Some(1.6));
        assert_eq!(
            command.runtime_presets.as_ref().and_then(|presets| presets.temperature),
            Some(0.7)
        );
        assert_eq!(command.runtime_presets.as_ref().and_then(|presets| presets.top_p), Some(0.95));
    }

    #[test]
    fn builds_local_model_command_from_pack_manifest() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
                json!({
                    "version": 2,
                    "id": "qwen2.5-7b-instruct",
                    "label": "Qwen2.5 7B Instruct",
                    "family": "llama",
                    "context_window": 32768,
                    "runtime_presets": {
                        "temperature": 0.4,
                        "top_p": 0.9
                    },
                    "capabilities": ["text_generation"],
                    "backend_hints": {
                        "prefer_drivers": ["ggml.llama"],
                        "avoid_drivers": [],
                        "require_streaming": false
                    },
                    "source": {
                        "kind": "hugging_face",
                        "repo_id": "bartowski/Qwen2.5-7B-Instruct-GGUF",
                        "files": [
                            {
                                "id": "model",
                                "path": "Qwen2.5-7B-Instruct-Q4_K_M.gguf"
                            }
                        ]
                    },
                    "variants": [{"id": "q4_k_m", "label": "Q4_K_M", "$config": "ref://models/variants/q4.json"}],
                    "presets": [{"id": "default", "label": "Default", "$config": "ref://models/presets/default.json"}],
                    "default_preset": "default"
                })
                .to_string(),
            ),
            (
                "models/configs/load.json",
                json!({
                    "kind": "backend_config",
                    "id": "load",
                    "label": "Load",
                    "scope": "load",
                    "payload": {
                        "context_length": 8192,
                        "chat_template": "chatml",
                        "num_workers": 2
                    }
                })
                .to_string(),
            ),
            (
                "models/configs/inference.json",
                json!({
                    "kind": "backend_config",
                    "id": "inference",
                    "label": "Inference",
                    "scope": "inference",
                    "payload": {
                        "temperature": 0.7,
                        "top_p": 0.95
                    }
                })
                .to_string(),
            ),
            (
                "models/variants/q4.json",
                json!({
                    "kind": "variant",
                    "id": "q4_k_m",
                    "label": "Q4"
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default",
                    "$load_config": "ref://models/configs/load.json",
                    "$inference_config": "ref://models/configs/inference.json"
                })
                .to_string(),
            ),
        ]);

        let command =
            build_model_command_from_pack_bytes(Path::new("qwen2.5-7b-instruct.slab"), &bytes)
                .expect("local command");

        assert_eq!(command.kind, UnifiedModelKind::Local);
        assert_eq!(command.backend_id, Some(ManagedModelBackendId::GgmlLlama));
        assert_eq!(command.status, Some(UnifiedModelStatus::NotDownloaded));
        assert_eq!(command.spec.repo_id.as_deref(), Some("bartowski/Qwen2.5-7B-Instruct-GGUF"));
        assert_eq!(command.spec.filename.as_deref(), Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf"));
        assert_eq!(command.spec.local_path, None);
        assert_eq!(command.spec.context_window, Some(32768));
        assert_eq!(command.spec.chat_template.as_deref(), Some("chatml"));
        assert_eq!(
            command.runtime_presets.as_ref().and_then(|presets| presets.temperature),
            Some(0.4)
        );
        assert_eq!(command.runtime_presets.as_ref().and_then(|presets| presets.top_p), Some(0.9));
    }

    #[test]
    fn builds_diffusion_model_command_from_hugging_face_pack_without_local_paths() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
                json!({
                    "version": 2,
                    "id": "sdxl-turbo",
                    "label": "SDXL Turbo",
                    "family": "diffusion",
                    "capabilities": ["image_generation"],
                    "backend_hints": {
                        "prefer_drivers": ["ggml.diffusion"],
                        "avoid_drivers": [],
                        "require_streaming": false
                    },
                    "source": {
                        "kind": "hugging_face",
                        "repo_id": "stabilityai/sdxl-turbo",
                        "files": [
                            {
                                "id": "model",
                                "path": "sdxl_turbo.safetensors"
                            }
                        ]
                    },
                    "presets": [{"id": "default", "label": "Default", "$config": "ref://models/presets/default.json"}],
                    "default_preset": "default"
                })
                .to_string(),
            ),
            (
                "models/configs/load.json",
                json!({
                    "kind": "backend_config",
                    "id": "load",
                    "label": "Load",
                    "scope": "load",
                    "payload": {
                        "flash_attn": true,
                        "vae_device": "cpu"
                    }
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default",
                    "$load_config": "ref://models/configs/load.json"
                })
                .to_string(),
            ),
        ]);

        let command = build_model_command_from_pack_bytes(Path::new("sdxl-turbo.slab"), &bytes)
            .expect("diffusion command");

        assert_eq!(command.kind, UnifiedModelKind::Local);
        assert_eq!(command.backend_id, Some(ManagedModelBackendId::GgmlDiffusion));
        assert_eq!(command.status, Some(UnifiedModelStatus::NotDownloaded));
        assert_eq!(command.spec.repo_id.as_deref(), Some("stabilityai/sdxl-turbo"));
        assert_eq!(command.spec.filename.as_deref(), Some("sdxl_turbo.safetensors"));
        assert_eq!(command.spec.local_path, None);
    }

    #[test]
    fn builds_local_model_command_using_selected_variant_file_from_manifest_source() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
                json!({
                    "version": 2,
                    "id": "qwen2.5-0.5b-instruct",
                    "label": "Qwen2.5 0.5B Instruct",
                    "family": "llama",
                    "capabilities": ["text_generation"],
                    "backend_hints": {
                        "prefer_drivers": ["ggml.llama"],
                        "avoid_drivers": [],
                        "require_streaming": false
                    },
                    "source": {
                        "kind": "hugging_face",
                        "repo_id": "bartowski/Qwen2.5-0.5B-Instruct-GGUF",
                        "files": [
                            {
                                "id": "model",
                                "path": "Qwen2.5-0.5B-Instruct-f16.gguf"
                            },
                            {
                                "id": "Q4_K_M",
                                "path": "Qwen2.5-0.5B-Instruct-Q4_K_M.gguf"
                            },
                            {
                                "id": "Q8_0",
                                "path": "Qwen2.5-0.5B-Instruct-Q8_0.gguf"
                            }
                        ]
                    },
                    "variants": [{"id": "Q8_0", "label": "Q8_0", "$config": "ref://models/variants/q8_0.json"}],
                    "presets": [{"id": "default", "label": "Default", "$config": "ref://models/presets/default.json"}],
                    "default_preset": "default"
                })
                .to_string(),
            ),
            (
                "models/variants/q8_0.json",
                json!({
                    "kind": "variant",
                    "id": "Q8_0",
                    "label": "Q8_0"
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default",
                    "variant_id": "Q8_0"
                })
                .to_string(),
            ),
        ]);

        let command =
            build_model_command_from_pack_bytes(Path::new("qwen2.5-0.5b-instruct.slab"), &bytes)
                .expect("local command");

        assert_eq!(command.kind, UnifiedModelKind::Local);
        assert_eq!(command.backend_id, Some(ManagedModelBackendId::GgmlLlama));
        assert_eq!(command.status, Some(UnifiedModelStatus::NotDownloaded));
        assert_eq!(command.spec.repo_id.as_deref(), Some("bartowski/Qwen2.5-0.5B-Instruct-GGUF"));
        assert_eq!(command.spec.filename.as_deref(), Some("Qwen2.5-0.5B-Instruct-Q8_0.gguf"));
        assert_eq!(command.spec.local_path, None);
    }

    #[test]
    fn builds_local_model_command_using_manifest_preset_variant_override() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
                json!({
                    "version": 2,
                    "id": "qwen2.5-0.5b-instruct",
                    "label": "Qwen2.5 0.5B Instruct",
                    "family": "llama",
                    "capabilities": ["text_generation"],
                    "backend_hints": {
                        "prefer_drivers": ["ggml.llama"],
                        "avoid_drivers": [],
                        "require_streaming": false
                    },
                    "source": {
                        "kind": "hugging_face",
                        "repo_id": "bartowski/Qwen2.5-0.5B-Instruct-GGUF",
                        "files": [
                            {
                                "id": "model",
                                "path": "Qwen2.5-0.5B-Instruct-f16.gguf"
                            },
                            {
                                "id": "Q8_0",
                                "path": "Qwen2.5-0.5B-Instruct-Q8_0.gguf"
                            }
                        ]
                    },
                    "variants": [{"id": "Q8_0", "label": "Q8_0", "$config": "ref://models/variants/q8_0.json"}],
                    "presets": [{
                        "id": "default",
                        "label": "Default",
                        "variant_id": "Q8_0",
                        "$config": "ref://models/presets/default.json"
                    }],
                    "default_preset": "default"
                })
                .to_string(),
            ),
            (
                "models/variants/q8_0.json",
                json!({
                    "kind": "variant",
                    "id": "Q8_0",
                    "label": "Q8_0"
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default"
                })
                .to_string(),
            ),
        ]);

        let command =
            build_model_command_from_pack_bytes(Path::new("qwen2.5-0.5b-instruct.slab"), &bytes)
                .expect("local command");

        assert_eq!(command.spec.filename.as_deref(), Some("Qwen2.5-0.5B-Instruct-Q8_0.gguf"));
    }

    #[test]
    fn manifest_remains_the_source_of_truth_when_persisted_state_matches() {
        let base_bytes = build_pack(vec![(
            "manifest.json",
            json!({
                "version": 1,
                "id": "openrouter-llama-3_1-8b-instruct",
                "label": "Manifest Label",
                "family": "llama",
                "capabilities": ["text_generation"],
                "source": {
                    "kind": "cloud",
                    "provider_id": "openrouter-main",
                    "remote_model_id": "meta-llama/llama-3.1-8b-instruct"
                }
            })
            .to_string(),
        )]);
        let config = StoredModelConfig {
            schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
            policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
            id: "openrouter-llama-3_1-8b-instruct".to_owned(),
            display_name: "Persisted Label".to_owned(),
            kind: UnifiedModelKind::Cloud,
            backend_id: None,
            capabilities: vec![Capability::TextGeneration, Capability::ChatGeneration],
            status: Some(UnifiedModelStatus::Ready),
            spec: ModelSpec {
                provider_id: Some("openrouter-main".to_owned()),
                remote_model_id: Some("meta-llama/llama-3.1-8b-instruct".to_owned()),
                ..Default::default()
            },
            runtime_presets: Some(RuntimePresets { temperature: Some(0.2), top_p: Some(0.8) }),
            materialized_artifacts: BTreeMap::new(),
            pack_selection: None,
            selected_download_source: None,
        };

        let bytes = attach_persisted_state_to_pack_bytes(&base_bytes, &config)
            .expect("attach persisted state");
        let command = build_model_command_from_pack_bytes(Path::new("openrouter.slab"), &bytes)
            .expect("command from pack");

        assert_eq!(command.display_name, "Manifest Label");
        assert!(command.runtime_presets.is_none());
    }

    #[test]
    fn ignores_persisted_state_after_manifest_change() {
        let base_bytes = build_pack(vec![(
            "manifest.json",
            json!({
                "version": 1,
                "id": "openrouter-llama-3_1-8b-instruct",
                "label": "Original Manifest Label",
                "family": "llama",
                "capabilities": ["text_generation"],
                "source": {
                    "kind": "cloud",
                    "provider_id": "openrouter-main",
                    "remote_model_id": "meta-llama/llama-3.1-8b-instruct"
                }
            })
            .to_string(),
        )]);
        let config = StoredModelConfig {
            schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
            policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
            id: "openrouter-llama-3_1-8b-instruct".to_owned(),
            display_name: "Persisted Label".to_owned(),
            kind: UnifiedModelKind::Cloud,
            backend_id: None,
            capabilities: vec![Capability::TextGeneration, Capability::ChatGeneration],
            status: Some(UnifiedModelStatus::Ready),
            spec: ModelSpec {
                provider_id: Some("openrouter-main".to_owned()),
                remote_model_id: Some("meta-llama/llama-3.1-8b-instruct".to_owned()),
                ..Default::default()
            },
            runtime_presets: None,
            materialized_artifacts: BTreeMap::new(),
            pack_selection: None,
            selected_download_source: None,
        };

        let bytes = attach_persisted_state_to_pack_bytes(&base_bytes, &config)
            .expect("attach persisted state");
        let mut entries = collect_pack_entries(&bytes).expect("collect entries");
        for (path, payload) in &mut entries {
            if path == "manifest.json" {
                *payload = serde_json::to_vec_pretty(&json!({
                    "version": 1,
                    "id": "openrouter-llama-3_1-8b-instruct",
                    "label": "Changed Manifest Label",
                    "family": "llama",
                    "capabilities": ["text_generation"],
                    "source": {
                        "kind": "cloud",
                        "provider_id": "openrouter-main",
                        "remote_model_id": "meta-llama/llama-3.1-8b-instruct"
                    }
                }))
                .expect("serialize manifest");
            }
        }
        let bytes = build_pack_bytes(entries).expect("rebuild pack");
        let command = build_model_command_from_pack_bytes(Path::new("openrouter.slab"), &bytes)
            .expect("command from pack");

        assert_eq!(command.display_name, "Changed Manifest Label");
    }

    #[test]
    fn generated_pack_carries_persisted_state() {
        let config = StoredModelConfig {
            schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
            policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
            id: "local-qwen".to_owned(),
            display_name: "Local Qwen".to_owned(),
            kind: UnifiedModelKind::Local,
            backend_id: Some(ManagedModelBackendId::GgmlLlama),
            capabilities: vec![Capability::TextGeneration, Capability::ChatGeneration],
            status: Some(UnifiedModelStatus::NotDownloaded),
            spec: ModelSpec {
                repo_id: Some("bartowski/Qwen2.5-7B-Instruct-GGUF".to_owned()),
                filename: Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf".to_owned()),
                context_window: Some(8192),
                ..Default::default()
            },
            runtime_presets: Some(RuntimePresets { temperature: Some(0.6), top_p: Some(0.9) }),
            materialized_artifacts: BTreeMap::new(),
            pack_selection: None,
            selected_download_source: None,
        };

        let bytes = build_generated_model_pack_bytes(&config).expect("generate pack");
        let restored = read_persisted_model_config_from_pack_bytes(&bytes)
            .expect("read state")
            .expect("state exists");
        let command = build_model_command_from_pack_bytes(Path::new("local-qwen.slab"), &bytes)
            .expect("command from pack");

        assert_eq!(restored.id, "local-qwen");
        assert_eq!(restored.schema_version, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION);
        assert_eq!(restored.policy_version, CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION);
        assert_eq!(command.display_name, "Local Qwen");
        assert_eq!(command.spec.repo_id.as_deref(), Some("bartowski/Qwen2.5-7B-Instruct-GGUF"));
        assert_eq!(command.spec.filename.as_deref(), Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf"));
    }

    #[test]
    fn legacy_persisted_state_without_versions_still_loads() {
        let base_bytes = build_pack(vec![(
            "manifest.json",
            json!({
                "version": 2,
                "id": "gpt_4_1_mini",
                "label": "GPT-4.1 mini",
                "status": "ready",
                "family": "llama",
                "source": {
                    "kind": "cloud",
                    "provider_id": "openai-main",
                    "remote_model_id": "gpt-4.1-mini"
                }
            })
            .to_string(),
        )]);
        let manifest_sha256 = manifest_sha256_from_pack_bytes(&base_bytes).expect("manifest hash");
        let mut entries = collect_pack_entries(&base_bytes).expect("collect entries");
        entries.push((
            "internal/stored-model-config".to_owned(),
            serde_json::to_vec_pretty(&json!({
                "manifest_sha256": manifest_sha256,
                "config": {
                    "id": "gpt_4_1_mini",
                    "display_name": "Persisted GPT-4.1 mini",
                    "kind": "cloud",
                    "status": "ready",
                    "spec": {
                        "provider_id": "openai-main",
                        "remote_model_id": "gpt-4.1-mini",
                        "context_window": 128000
                    },
                    "runtime_presets": {
                        "temperature": 0.6
                    }
                }
            }))
            .expect("serialize state"),
        ));
        let bytes = build_pack_bytes(entries).expect("build pack");

        let restored = read_persisted_model_config_from_pack_bytes(&bytes)
            .expect("read state")
            .expect("state exists");

        assert_eq!(restored.schema_version, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION);
        assert_eq!(restored.policy_version, CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION);
        assert_eq!(restored.display_name, "Persisted GPT-4.1 mini");
        assert_eq!(restored.spec.context_window, Some(128000));
    }

    #[test]
    fn future_persisted_state_versions_are_rejected() {
        let base_bytes = build_pack(vec![(
            "manifest.json",
            json!({
                "version": 2,
                "id": "gpt_4_1_mini",
                "label": "GPT-4.1 mini",
                "status": "ready",
                "family": "llama",
                "source": {
                    "kind": "cloud",
                    "provider_id": "openai-main",
                    "remote_model_id": "gpt-4.1-mini"
                }
            })
            .to_string(),
        )]);
        let manifest_sha256 = manifest_sha256_from_pack_bytes(&base_bytes).expect("manifest hash");
        let mut entries = collect_pack_entries(&base_bytes).expect("collect entries");
        entries.push((
            "internal/stored-model-config".to_owned(),
            serde_json::to_vec_pretty(&json!({
                "manifest_sha256": manifest_sha256,
                "config": {
                    "schema_version": CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION + 1,
                    "policy_version": CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
                    "id": "gpt_4_1_mini",
                    "display_name": "Persisted GPT-4.1 mini",
                    "kind": "cloud",
                    "status": "ready",
                    "spec": {
                        "provider_id": "openai-main",
                        "remote_model_id": "gpt-4.1-mini"
                    }
                }
            }))
            .expect("serialize state"),
        ));
        let bytes = build_pack_bytes(entries).expect("build pack");

        let error = read_persisted_model_config_from_pack_bytes(&bytes)
            .expect_err("future version should be rejected");

        assert!(
            matches!(error, AppCoreError::BadRequest(message) if message.contains("unsupported stored model config schema_version"))
        );
    }

    #[test]
    fn persisted_state_preserves_download_projection_without_overriding_pack_selection() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
                json!({
                    "version": 2,
                    "id": "local-qwen",
                    "label": "Local Qwen",
                    "family": "llama",
                    "capabilities": ["text_generation", "chat_generation"],
                    "backend_hints": {
                        "prefer_drivers": ["ggml.llama"],
                        "avoid_drivers": [],
                        "require_streaming": false
                    },
                    "source": {
                        "kind": "hugging_face",
                        "repo_id": "bartowski/Qwen2.5-7B-Instruct-GGUF",
                        "files": [{ "id": "model", "path": "Qwen2.5-7B-Instruct-Q4_K_M.gguf" }]
                    },
                    "variants": [{"id": "q4_k_m", "label": "Q4_K_M", "$config": "ref://models/variants/q4.json"}],
                    "presets": [{"id": "default", "label": "Default", "$config": "ref://models/presets/default.json"}],
                    "default_preset": "default"
                })
                .to_string(),
            ),
            (
                "models/variants/q4.json",
                json!({
                    "kind": "variant",
                    "id": "q4_k_m",
                    "label": "Q4_K_M"
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default"
                })
                .to_string(),
            ),
        ]);
        let persisted = StoredModelConfig {
            schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
            policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
            id: "local-qwen".to_owned(),
            display_name: "Old Local Qwen".to_owned(),
            kind: UnifiedModelKind::Local,
            backend_id: Some(ManagedModelBackendId::GgmlLlama),
            capabilities: vec![Capability::TextGeneration, Capability::ChatGeneration],
            status: Some(UnifiedModelStatus::Ready),
            spec: ModelSpec {
                repo_id: Some("bartowski/Qwen2.5-7B-Instruct-GGUF".to_owned()),
                filename: Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf".to_owned()),
                local_path: Some("C:/models/Qwen2.5-7B-Instruct-Q4_K_M.gguf".to_owned()),
                ..Default::default()
            },
            runtime_presets: Some(RuntimePresets { temperature: Some(0.7), top_p: Some(0.95) }),
            materialized_artifacts: BTreeMap::new(),
            pack_selection: Some(ModelPackSelection {
                preset_id: Some("default".to_owned()),
                variant_id: Some("q8_0".to_owned()),
            }),
            selected_download_source: None,
        };
        let bytes = attach_persisted_state_to_pack_bytes(&bytes, &persisted)
            .expect("attach persisted state");

        let command = build_model_command_from_pack_bytes(Path::new("local-qwen.slab"), &bytes)
            .expect("command");

        assert_eq!(command.display_name, "Local Qwen");
        assert_eq!(command.spec.filename.as_deref(), Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf"));
        assert_eq!(
            command.spec.local_path.as_deref(),
            Some("C:/models/Qwen2.5-7B-Instruct-Q4_K_M.gguf")
        );
        assert_eq!(command.status, Some(UnifiedModelStatus::Ready));
    }
}

fn build_local_model_command(
    _path: &Path,
    manifest: &ModelPackManifest,
    resolved: &slab_model_pack::ResolvedModelPack,
) -> Result<CreateModelCommand, AppCoreError> {
    let bridge = resolved.compile_default_runtime_bridge().map_err(map_model_pack_error)?;
    let backend_id = ManagedModelBackendId::try_from(bridge.backend).map_err(|error| {
        AppCoreError::BadRequest(format!(
            "model pack backend '{}' is not supported by managed local models: {}",
            bridge.backend, error
        ))
    })?;
    let status = manifest_status(manifest.status)
        .unwrap_or_else(|| default_status_for_runtime_bridge(&bridge));
    let runtime_presets = build_runtime_presets_from_manifest(manifest.runtime_presets.as_ref())
        .or_else(|| build_runtime_presets(&bridge.inference_defaults));
    let (repo_id, filename, local_path) = local_source_fields(resolved, &bridge);
    let allow_local_path_fallback = repo_id.is_none();

    Ok(CreateModelCommand {
        id: Some(manifest.id.clone()),
        display_name: manifest.label.clone(),
        kind: UnifiedModelKind::Local,
        backend_id: Some(backend_id),
        capabilities: Some(manifest.capabilities.clone()),
        status: Some(status),
        spec: ModelSpec {
            pricing: build_pricing_from_manifest(manifest.pricing.as_ref()),
            repo_id,
            filename,
            local_path: local_path.or_else(|| {
                allow_local_path_fallback
                    .then(|| {
                        bridge
                            .model_spec
                            .source
                            .primary_path()
                            .map(|value| value.to_string_lossy().into_owned())
                    })
                    .flatten()
            }),
            context_window: manifest.context_window.or(bridge.load_defaults.context_length),
            chat_template: bridge.load_defaults.chat_template.clone(),
            ..Default::default()
        },
        runtime_presets,
    })
}

fn build_cloud_model_command(
    manifest: &ModelPackManifest,
    provider_id: &str,
    remote_model_id: &str,
) -> Result<CreateModelCommand, AppCoreError> {
    let provider_id = normalize_required_manifest_text(provider_id, "source.provider_id")?;
    let remote_model_id =
        normalize_required_manifest_text(remote_model_id, "source.remote_model_id")?;

    Ok(CreateModelCommand {
        id: Some(manifest.id.clone()),
        display_name: manifest.label.clone(),
        kind: UnifiedModelKind::Cloud,
        backend_id: None,
        capabilities: Some(manifest.capabilities.clone()),
        status: manifest_status(manifest.status),
        spec: ModelSpec {
            provider_id: Some(provider_id),
            remote_model_id: Some(remote_model_id),
            pricing: build_pricing_from_manifest(manifest.pricing.as_ref()),
            context_window: manifest.context_window,
            ..Default::default()
        },
        runtime_presets: build_runtime_presets_from_manifest(manifest.runtime_presets.as_ref()),
    })
}

fn manifest_status(status: Option<PackModelStatus>) -> Option<UnifiedModelStatus> {
    status.map(|status| match status {
        PackModelStatus::Ready => UnifiedModelStatus::Ready,
        PackModelStatus::NotDownloaded => UnifiedModelStatus::NotDownloaded,
        PackModelStatus::Downloading => UnifiedModelStatus::Downloading,
        PackModelStatus::Error => UnifiedModelStatus::Error,
    })
}

fn build_pricing_from_manifest(pricing: Option<&PackPricing>) -> Option<Pricing> {
    pricing.map(|pricing| Pricing { input: pricing.input, output: pricing.output })
}

fn build_runtime_presets_from_manifest(
    runtime_presets: Option<&PackRuntimePresets>,
) -> Option<RuntimePresets> {
    let runtime_presets = runtime_presets?;
    (runtime_presets.temperature.is_some() || runtime_presets.top_p.is_some()).then_some(
        RuntimePresets { temperature: runtime_presets.temperature, top_p: runtime_presets.top_p },
    )
}

fn normalize_optional_manifest_text(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
    })
}

fn normalize_required_manifest_text(value: &str, label: &str) -> Result<String, AppCoreError> {
    normalize_optional_manifest_text(Some(value))
        .ok_or_else(|| AppCoreError::BadRequest(format!("{} must not be empty", label)))
}

fn local_source_fields(
    resolved: &slab_model_pack::ResolvedModelPack,
    bridge: &ModelPackRuntimeBridge,
) -> (Option<String>, Option<String>, Option<String>) {
    let source = resolved
        .default_preset()
        .and_then(|preset| {
            preset.variant.effective_sources.first().map(|candidate| &candidate.source).or_else(
                || {
                    preset
                        .variant
                        .components
                        .get("model")
                        .map(|component| &component.document.source)
                        .or_else(|| {
                            preset
                                .variant
                                .components
                                .values()
                                .next()
                                .map(|component| &component.document.source)
                        })
                },
            )
        })
        .or_else(|| resolved.manifest.sources.first().map(|candidate| &candidate.source));

    match source {
        Some(PackSource::HuggingFace { repo_id, files, .. }) => {
            let filename = files
                .iter()
                .find(|file| file.id == "model")
                .or_else(|| files.first())
                .map(|file| file.path.clone());
            (Some(repo_id.clone()), filename, None)
        }
        Some(PackSource::LocalPath { path }) => (None, None, Some(path.clone())),
        Some(PackSource::LocalFiles { files }) => {
            let local_path = files
                .iter()
                .find(|file| file.id == "model")
                .or_else(|| files.first())
                .map(|file| file.path.clone());
            (None, None, local_path)
        }
        _ => (
            None,
            None,
            bridge
                .model_spec
                .source
                .primary_path()
                .map(|value| value.to_string_lossy().into_owned()),
        ),
    }
}
