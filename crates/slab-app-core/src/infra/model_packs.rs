use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use slab_model_pack::{
    ModelPack, ModelPackCatalogSummary, ModelPackError, ModelPackLoadDefaults,
    ModelPackRuntimeBridge, PACK_EXTENSION,
};
use slab_types::{DiffusionLoadOptions, RuntimeBackendId};
use uuid::Uuid;

use crate::domain::models::{CreateModelCommand, ModelSpec, RuntimePresets, UnifiedModelStatus};
use crate::error::AppCoreError;
use crate::infra::model_configs;

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
            "failed to read model config directory '{}': {error}",
            dir.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to iterate model config directory '{}': {error}",
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
    let summary = read_model_pack_summary(path)?;
    let bridge = read_model_pack_runtime_bridge(path)?;
    Ok(build_model_command(path, &summary, &bridge))
}

pub fn build_model_command_from_pack_bytes(
    path: &Path,
    bytes: &[u8],
) -> Result<CreateModelCommand, AppCoreError> {
    let summary = read_model_pack_summary_from_bytes(bytes)?;
    let bridge = read_model_pack_runtime_bridge_from_bytes(bytes)?;
    Ok(build_model_command(path, &summary, &bridge))
}

pub fn model_pack_file_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(model_pack_file_name(id))
}

pub fn write_model_pack(dir: &Path, id: &str, bytes: &[u8]) -> Result<PathBuf, AppCoreError> {
    model_configs::ensure_model_config_dir(dir)?;

    let path = model_pack_file_path(dir, id);
    write_bytes_file(&path, bytes)?;
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
    summary: &ModelPackCatalogSummary,
    bridge: &ModelPackRuntimeBridge,
) -> CreateModelCommand {
    let provider = format!("local.{}", bridge.backend.canonical_id());
    let status = Some(default_status_for_runtime_bridge(&bridge));
    let runtime_presets = build_runtime_presets(&bridge.inference_defaults);

    CreateModelCommand {
        id: Some(summary.id.clone()),
        display_name: summary.label.clone(),
        provider,
        status,
        spec: ModelSpec {
            local_path: Some(path.display().to_string()),
            context_window: bridge.load_defaults.context_length,
            chat_template: bridge.load_defaults.chat_template.clone(),
            ..Default::default()
        },
        runtime_presets,
    }
}

pub fn build_model_pack_load_target(
    path: &Path,
) -> Result<ModelPackLoadTarget, AppCoreError> {
    let bridge = read_model_pack_runtime_bridge(path)?;
    let load_spec = bridge
        .runtime_load_spec(bridge.model_spec.metadata.get("default_preset").map(String::as_str).unwrap_or("default"))
        .map_err(map_model_pack_error)?;

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
    model_configs::ensure_model_config_dir(parent)?;

    let file_name = path.file_name().and_then(|value| value.to_str()).ok_or_else(|| {
        AppCoreError::Internal(format!(
            "model pack path '{}' has an invalid file name",
            path.display()
        ))
    })?;
    let temp_path = parent.join(format!(".{}.tmp-{}", file_name, Uuid::new_v4()));

    let write_result = (|| -> Result<(), AppCoreError> {
        let mut temp_file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|error| {
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