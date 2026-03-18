use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use uuid::Uuid;

use crate::domain::models::StoredModelConfig;
use crate::error::ServerError;

pub fn ensure_model_config_dir(path: &Path) -> Result<(), ServerError> {
    fs::create_dir_all(path).map_err(|error| {
        ServerError::Internal(format!(
            "failed to create model config directory '{}': {error}",
            path.display()
        ))
    })
}

pub fn list_model_config_paths(dir: &Path) -> Result<Vec<PathBuf>, ServerError> {
    ensure_model_config_dir(dir)?;

    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).map_err(|error| {
        ServerError::Internal(format!(
            "failed to read model config directory '{}': {error}",
            dir.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            ServerError::Internal(format!(
                "failed to iterate model config directory '{}': {error}",
                dir.display()
            ))
        })?;
        let path = entry.path();
        let is_json = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        if path.is_file() && is_json {
            paths.push(path);
        }
    }

    paths.sort();
    Ok(paths)
}

pub fn read_model_config(path: &Path) -> Result<StoredModelConfig, ServerError> {
    let raw = fs::read_to_string(path).map_err(|error| {
        ServerError::Internal(format!(
            "failed to read model config file '{}': {error}",
            path.display()
        ))
    })?;

    serde_json::from_str(&raw).map_err(|error| {
        ServerError::BadRequest(format!(
            "invalid model config file '{}': {error}",
            path.display()
        ))
    })
}

pub fn write_model_config(
    dir: &Path,
    config: &StoredModelConfig,
) -> Result<PathBuf, ServerError> {
    ensure_model_config_dir(dir)?;

    let path = model_config_file_path(dir, &config.id);
    write_json_file(&path, config)?;
    Ok(path)
}

pub fn delete_model_config(dir: &Path, id: &str) -> Result<bool, ServerError> {
    let path = model_config_file_path(dir, id);
    if !path.exists() {
        return Ok(false);
    }

    fs::remove_file(&path).map_err(|error| {
        ServerError::Internal(format!(
            "failed to remove model config file '{}': {error}",
            path.display()
        ))
    })?;
    Ok(true)
}

pub fn model_config_file_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(model_config_file_name(id))
}

fn model_config_file_name(id: &str) -> String {
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(id.as_bytes());
    format!("{encoded}.json")
}

fn write_json_file<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), ServerError> {
    let parent = path.parent().ok_or_else(|| {
        ServerError::Internal(format!(
            "model config path '{}' has no parent directory",
            path.display()
        ))
    })?;
    ensure_model_config_dir(parent)?;

    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            ServerError::Internal(format!(
                "model config path '{}' has an invalid file name",
                path.display()
            ))
        })?;
    let temp_path = parent.join(format!(".{}.tmp-{}", file_name, Uuid::new_v4()));

    let mut payload = serde_json::to_vec_pretty(value).map_err(|error| {
        ServerError::Internal(format!(
            "failed to serialize model config '{}': {error}",
            path.display()
        ))
    })?;
    payload.push(b'\n');

    let write_result = (|| -> Result<(), ServerError> {
        let mut temp_file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|error| {
                ServerError::Internal(format!(
                    "failed to create temp model config file '{}': {error}",
                    temp_path.display()
                ))
            })?;

        temp_file.write_all(&payload).map_err(|error| {
            ServerError::Internal(format!(
                "failed to write temp model config file '{}': {error}",
                temp_path.display()
            ))
        })?;
        temp_file.flush().map_err(|error| {
            ServerError::Internal(format!(
                "failed to flush temp model config file '{}': {error}",
                temp_path.display()
            ))
        })?;
        temp_file.sync_all().map_err(|error| {
            ServerError::Internal(format!(
                "failed to sync temp model config file '{}': {error}",
                temp_path.display()
            ))
        })?;
        drop(temp_file);

        if path.exists() {
            fs::remove_file(path).map_err(|error| {
                ServerError::Internal(format!(
                    "failed to replace existing model config file '{}': {error}",
                    path.display()
                ))
            })?;
        }

        fs::rename(&temp_path, path).map_err(|error| {
            ServerError::Internal(format!(
                "failed to finalize model config file '{}': {error}",
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
