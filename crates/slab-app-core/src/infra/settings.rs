use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

use slab_types::settings::{
    CloudProviderConfig, ProviderAuthConfig, ProviderDefaultsConfig, ProviderFamily,
    ProviderRegistryEntry, SettingsDocument,
};

use crate::domain::models::{UpdateSettingCommand, UpdateSettingOperation};
use crate::domain::services::setup::{SETUP_INITIALIZED_CONFIG_KEY, SETUP_INITIALIZED_CONFIG_NAME};
use crate::error::AppCoreError;
use crate::infra::db::repository::config::ConfigStore;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

const LEGACY_SETTINGS_VERSION: u32 = 1;
const MIGRATION_BACKUP_SUFFIX: &str = "legacy-v1";

#[derive(Debug, Clone)]
pub struct SettingsDocumentProvider {
    path: PathBuf,
    state: Arc<RwLock<SettingsRuntimeState>>,
}

#[derive(Debug, Clone, Default)]
struct SettingsRuntimeState {
    document: SettingsDocument,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsMigrationResult {
    pub migrated: bool,
    pub backup_path: Option<PathBuf>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacySettingsValuesFile {
    pub version: u32,
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default)]
struct LegacyMigrationState {
    document_json: Value,
    consumed_keys: BTreeSet<String>,
    warnings: Vec<String>,
    setup_initialized: Option<bool>,
}

impl SettingsDocumentProvider {
    pub async fn load(path: PathBuf) -> Result<Self, AppCoreError> {
        let path_for_load = path.clone();
        let state = tokio::task::spawn_blocking(move || load_runtime_state(&path_for_load))
            .await
            .map_err(|error| {
                AppCoreError::Internal(format!("settings loader task failed: {error}"))
            })??;

        Ok(Self { path, state: Arc::new(RwLock::new(state)) })
    }

    pub async fn document(&self) -> SettingsDocument {
        self.state.read().await.document.clone()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn warnings(&self) -> Vec<String> {
        self.state.read().await.warnings.clone()
    }

    pub async fn value(&self, path: &str) -> Result<Value, AppCoreError> {
        let state = self.state.read().await;
        let document = settings_document_to_json_value(&state.document);

        value_at_path(&document, path)
            .cloned()
            .ok_or_else(|| AppCoreError::NotFound(format!("setting pmid '{}' not found", path)))
    }

    pub async fn update(
        &self,
        path: &str,
        command: UpdateSettingCommand,
    ) -> Result<Value, AppCoreError> {
        let mut state = self.state.write().await;
        let mut next_document = settings_document_to_json_value(&state.document);
        let default_document = settings_document_to_json_value(&SettingsDocument::default());

        let next_value = match command.op {
            UpdateSettingOperation::Set => command.value.ok_or_else(|| {
                AppCoreError::BadRequest(format!(
                    "setting '{}' update requires a value when op=set",
                    path
                ))
            })?,
            UpdateSettingOperation::Unset => {
                value_at_path(&default_document, path).cloned().ok_or_else(|| {
                    AppCoreError::NotFound(format!("setting pmid '{}' not found", path))
                })?
            }
        };

        set_value_at_path(&mut next_document, path, next_value.clone())?;

        let next_settings: SettingsDocument =
            serde_json::from_value(next_document).map_err(|error| {
                AppCoreError::BadRequest(format!(
                    "setting '{}' update produced an invalid settings document: {error}",
                    path
                ))
            })?;

        write_settings_document_file(&self.path, &next_settings)?;
        state.document = next_settings;

        Ok(next_value)
    }
}

pub async fn migrate_legacy_settings_if_needed<S>(
    path: &Path,
    store: &S,
) -> Result<SettingsMigrationResult, AppCoreError>
where
    S: ConfigStore,
{
    ensure_settings_parent_dir(path)?;

    if !path.exists() {
        return Ok(SettingsMigrationResult::default());
    }

    let raw = fs::read_to_string(path).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to read settings file '{}': {error}",
            path.display()
        ))
    })?;

    let raw_json: Value = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(_) => return Ok(SettingsMigrationResult::default()),
    };

    if !is_legacy_settings_file(&raw_json) {
        return Ok(SettingsMigrationResult::default());
    }

    let parsed: LegacySettingsValuesFile = serde_json::from_value(raw_json).map_err(|error| {
        AppCoreError::BadRequest(format!(
            "settings file '{}' looks like a legacy override document but could not be parsed: {error}",
            path.display()
        ))
    })?;

    if parsed.version != LEGACY_SETTINGS_VERSION {
        return Err(AppCoreError::BadRequest(format!(
            "unsupported legacy settings version '{}'; expected '{}'",
            parsed.version, LEGACY_SETTINGS_VERSION
        )));
    }

    let (document, setup_initialized, warnings) = migrate_legacy_settings_document(parsed.values)?;
    let backup_path = create_settings_backup(path, MIGRATION_BACKUP_SUFFIX)?;
    write_settings_document_file(path, &document)?;

    if let Some(initialized) = setup_initialized {
        let value = if initialized { "true" } else { "false" };
        store
            .set_config_entry(
                SETUP_INITIALIZED_CONFIG_KEY,
                Some(SETUP_INITIALIZED_CONFIG_NAME),
                value,
            )
            .await
            .map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to persist migrated setup state in config_store: {error}"
                ))
            })?;
    }

    info!(
        settings_path = %path.display(),
        backup_path = %backup_path.display(),
        "migrated legacy settings file to the current document format"
    );
    for warning_message in &warnings {
        warn!(settings_path = %path.display(), "{warning_message}");
    }

    Ok(SettingsMigrationResult { migrated: true, backup_path: Some(backup_path), warnings })
}

fn load_runtime_state(path: &Path) -> Result<SettingsRuntimeState, AppCoreError> {
    ensure_settings_parent_dir(path)?;

    if !path.exists() {
        let document = SettingsDocument::default();
        write_settings_document_file(path, &document)?;
        return Ok(SettingsRuntimeState { document, warnings: Vec::new() });
    }

    let raw = fs::read_to_string(path).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to read settings file '{}': {error}",
            path.display()
        ))
    })?;

    let raw_json: Value = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            let warning = recover_corrupt_settings_file(path, &error.to_string())?;
            return Ok(SettingsRuntimeState {
                document: SettingsDocument::default(),
                warnings: vec![warning],
            });
        }
    };

    if is_legacy_settings_file(&raw_json) {
        return Err(AppCoreError::NotImplemented(
            "legacy PMID override settings are no longer supported; run the settings migration before loading"
                .to_owned(),
        ));
    }

    if let Ok(document) = serde_json::from_value::<SettingsDocument>(raw_json) {
        ensure_current_schema_version(document.schema_version)?;
        return Ok(SettingsRuntimeState { document, warnings: Vec::new() });
    }

    Err(AppCoreError::BadRequest(format!(
        "settings file '{}' is valid JSON but does not match the current settings document format",
        path.display()
    )))
}

fn ensure_current_schema_version(schema_version: u32) -> Result<(), AppCoreError> {
    let current = SettingsDocument::default().schema_version;
    if schema_version == current {
        return Ok(());
    }

    Err(AppCoreError::BadRequest(format!(
        "unsupported settings schema_version '{}'; expected '{}'",
        schema_version, current
    )))
}

fn migrate_legacy_settings_document(
    values: BTreeMap<String, Value>,
) -> Result<(SettingsDocument, Option<bool>, Vec<String>), AppCoreError> {
    let mut state = LegacyMigrationState {
        document_json: settings_document_to_json_value(&SettingsDocument::default()),
        ..Default::default()
    };

    for &(legacy_path, current_path) in direct_legacy_mappings() {
        if let Some(value) = values.get(legacy_path) {
            set_value_at_path(&mut state.document_json, current_path, value.clone())?;
            state.consumed_keys.insert(legacy_path.to_owned());
        }
    }

    if let Some(value) = values.get("setup.initialized") {
        match value.as_bool() {
            Some(initialized) => {
                state.setup_initialized = Some(initialized);
                state.consumed_keys.insert("setup.initialized".to_owned());
            }
            None => state.warnings.push(
                "Skipping legacy setting 'setup.initialized' because it is not a boolean."
                    .to_owned(),
            ),
        }
    }

    if let Some(value) = values.get("chat.providers") {
        match serde_json::from_value::<Vec<CloudProviderConfig>>(value.clone()) {
            Ok(providers) => {
                let registry = providers
                    .into_iter()
                    .map(|provider| ProviderRegistryEntry {
                        id: provider.id.clone(),
                        family: ProviderFamily::OpenaiCompatible,
                        display_name: if provider.name.trim().is_empty() {
                            provider.id
                        } else {
                            provider.name
                        },
                        api_base: provider.api_base,
                        auth: ProviderAuthConfig {
                            api_key: provider.api_key,
                            api_key_env: provider.api_key_env,
                        },
                        defaults: ProviderDefaultsConfig::default(),
                    })
                    .collect::<Vec<_>>();
                let registry_value = serde_json::to_value(&registry).map_err(|error| {
                    AppCoreError::Internal(format!(
                        "failed to serialize migrated provider registry: {error}"
                    ))
                })?;
                set_value_at_path(&mut state.document_json, "providers.registry", registry_value)?;
                state.consumed_keys.insert("chat.providers".to_owned());
            }
            Err(error) => state.warnings.push(format!(
                "Skipping legacy setting 'chat.providers' because it is invalid: {error}"
            )),
        }
    }

    for key in values.keys().filter(|key| !state.consumed_keys.contains(*key)) {
        state.warnings.push(format!(
            "Dropping legacy setting '{}' because it has no current settings document mapping.",
            key
        ));
    }

    let document: SettingsDocument =
        serde_json::from_value(state.document_json).map_err(|error| {
            AppCoreError::BadRequest(format!(
                "failed to convert legacy settings into the current document format: {error}"
            ))
        })?;

    Ok((document, state.setup_initialized, state.warnings))
}

fn direct_legacy_mappings() -> &'static [(&'static str, &'static str)] {
    &[
        ("setup.ffmpeg.auto_download", "tools.ffmpeg.auto_download"),
        ("setup.ffmpeg.dir", "tools.ffmpeg.install_dir"),
        ("setup.backends.dir", "runtime.ggml.install_dir"),
        ("runtime.model_cache_dir", "models.cache_dir"),
        ("runtime.llama.num_workers", "runtime.ggml.backends.llama.capacity.concurrent_requests"),
        ("runtime.llama.context_length", "runtime.ggml.backends.llama.context_length"),
        ("runtime.llama.flash_attn", "runtime.ggml.backends.llama.flash_attn"),
        (
            "runtime.whisper.num_workers",
            "runtime.ggml.backends.whisper.capacity.concurrent_requests",
        ),
        ("runtime.whisper.flash_attn", "runtime.ggml.backends.whisper.flash_attn"),
        (
            "runtime.diffusion.num_workers",
            "runtime.ggml.backends.diffusion.capacity.concurrent_requests",
        ),
        ("runtime.model_auto_unload.enabled", "models.auto_unload.enabled"),
        ("runtime.model_auto_unload.idle_minutes", "models.auto_unload.idle_minutes"),
        (
            "runtime.model_auto_unload.min_free_system_memory_bytes",
            "models.auto_unload.min_free_system_memory_bytes",
        ),
        (
            "runtime.model_auto_unload.min_free_gpu_memory_bytes",
            "models.auto_unload.min_free_gpu_memory_bytes",
        ),
        (
            "runtime.model_auto_unload.max_pressure_evictions_per_load",
            "models.auto_unload.max_pressure_evictions_per_load",
        ),
        ("launch.transport", "runtime.transport"),
        ("launch.queue_capacity", "runtime.capacity.queue"),
        ("launch.backend_capacity", "runtime.capacity.concurrent_requests"),
        ("launch.runtime_log_dir", "runtime.logging.path"),
        ("launch.backends.llama.enabled", "runtime.ggml.backends.llama.enabled"),
        ("launch.backends.whisper.enabled", "runtime.ggml.backends.whisper.enabled"),
        ("launch.backends.diffusion.enabled", "runtime.ggml.backends.diffusion.enabled"),
        ("launch.profiles.server.gateway_bind", "server.address"),
        ("diffusion.performance.flash_attn", "runtime.ggml.backends.diffusion.flash_attn"),
    ]
}

fn is_legacy_settings_file(raw_json: &Value) -> bool {
    let Some(object) = raw_json.as_object() else {
        return false;
    };

    matches!(object.get("version"), Some(Value::Number(_)))
        && matches!(object.get("values"), Some(Value::Object(_)))
}

fn create_settings_backup(path: &Path, reason: &str) -> Result<PathBuf, AppCoreError> {
    let backup_name = format!(
        "{}.{}-{}.bak",
        path.file_name().and_then(|name| name.to_str()).unwrap_or("settings.json"),
        reason,
        Utc::now().format("%Y%m%d%H%M%S")
    );
    let backup_path = path.with_file_name(backup_name);

    fs::copy(path, &backup_path).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to create settings backup '{}' from '{}': {error}",
            backup_path.display(),
            path.display()
        ))
    })?;

    Ok(backup_path)
}

fn recover_corrupt_settings_file(path: &Path, reason: &str) -> Result<String, AppCoreError> {
    let backup_path = create_settings_backup(path, "corrupt")?;
    write_settings_document_file(path, &SettingsDocument::default())?;

    Ok(format!(
        "Recovered from corrupt settings file. Original file was backed up to '{}' ({reason}).",
        backup_path.display()
    ))
}

fn ensure_settings_parent_dir(path: &Path) -> Result<(), AppCoreError> {
    let Some(parent) = path.parent() else {
        return Err(AppCoreError::Internal(format!(
            "settings path '{}' has no parent directory",
            path.display()
        )));
    };

    fs::create_dir_all(parent).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to create settings directory '{}': {error}",
            parent.display()
        ))
    })?;

    #[cfg(unix)]
    {
        let permissions = fs::Permissions::from_mode(0o700);
        let _ = fs::set_permissions(parent, permissions);
    }

    Ok(())
}

fn write_settings_document_file(
    path: &Path,
    document: &SettingsDocument,
) -> Result<(), AppCoreError> {
    ensure_settings_parent_dir(path)?;

    let parent = path.parent().ok_or_else(|| {
        AppCoreError::Internal(format!(
            "settings path '{}' has no parent directory",
            path.display()
        ))
    })?;
    let file_name = path.file_name().and_then(|name| name.to_str()).ok_or_else(|| {
        AppCoreError::Internal(format!("settings path '{}' has invalid file name", path.display()))
    })?;

    let temp_path = parent.join(format!(".{}.tmp-{}", file_name, Uuid::new_v4()));
    let mut payload = serde_json::to_vec_pretty(document).map_err(|error| {
        AppCoreError::Internal(format!("failed to serialize settings file: {error}"))
    })?;
    payload.push(b'\n');

    let write_result = (|| -> Result<(), AppCoreError> {
        let mut temp_file =
            OpenOptions::new().create_new(true).write(true).open(&temp_path).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to create temp settings file '{}': {error}",
                    temp_path.display()
                ))
            })?;

        #[cfg(unix)]
        {
            let permissions = fs::Permissions::from_mode(0o600);
            let _ = temp_file.set_permissions(permissions);
        }

        temp_file.write_all(&payload).map_err(|error| {
            AppCoreError::Internal(format!("failed to write settings file: {error}"))
        })?;
        temp_file.sync_all().map_err(|error| {
            AppCoreError::Internal(format!("failed to flush settings file: {error}"))
        })?;

        replace_file(&temp_path, path)?;
        sync_parent_dir(parent)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result
}

#[cfg(unix)]
fn replace_file(from: &Path, to: &Path) -> Result<(), AppCoreError> {
    fs::rename(from, to).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to replace settings file '{}' with '{}': {error}",
            to.display(),
            from.display()
        ))
    })
}

#[cfg(windows)]
fn replace_file(from: &Path, to: &Path) -> Result<(), AppCoreError> {
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        MoveFileExW(
            from_wide.as_ptr(),
            to_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };

    if result == 0 {
        Err(AppCoreError::Internal(format!(
            "failed to replace settings file '{}': {}",
            to.display(),
            std::io::Error::last_os_error()
        )))
    } else {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
fn replace_file(from: &Path, to: &Path) -> Result<(), AppCoreError> {
    fs::rename(from, to).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to replace settings file '{}' with '{}': {error}",
            to.display(),
            from.display()
        ))
    })
}

#[cfg(unix)]
fn sync_parent_dir(parent: &Path) -> Result<(), AppCoreError> {
    let dir = fs::File::open(parent).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to open settings directory '{}': {error}",
            parent.display()
        ))
    })?;
    dir.sync_all().map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to sync settings directory '{}': {error}",
            parent.display()
        ))
    })
}

#[cfg(not(unix))]
fn sync_parent_dir(_parent: &Path) -> Result<(), AppCoreError> {
    Ok(())
}

pub(crate) fn settings_document_to_json_value(document: &SettingsDocument) -> Value {
    json!({
        "$schema": document.schema,
        "schema_version": document.schema_version,
        "general": {
            "language": document.general.language,
        },
        "database": {
            "url": document.database.url,
        },
        "logging": {
            "level": document.logging.level,
            "json": document.logging.json,
            "path": document.logging.path,
        },
        "tools": {
            "ffmpeg": {
                "enabled": document.tools.ffmpeg.enabled,
                "auto_download": document.tools.ffmpeg.auto_download,
                "install_dir": document.tools.ffmpeg.install_dir,
                "source": {
                    "version": document.tools.ffmpeg.source.version,
                    "artifact": document.tools.ffmpeg.source.artifact,
                }
            }
        },
        "runtime": {
            "mode": document.runtime.mode,
            "transport": document.runtime.transport,
            "sessions": {
                "state_dir": document.runtime.sessions.state_dir,
            },
            "logging": {
                "level": document.runtime.logging.level,
                "json": document.runtime.logging.json,
                "path": document.runtime.logging.path,
            },
            "capacity": {
                "queue": document.runtime.capacity.queue,
                "concurrent_requests": document.runtime.capacity.concurrent_requests,
            },
            "endpoint": {
                "http": {
                    "address": document.runtime.endpoint.http.address,
                },
                "ipc": {
                    "path": document.runtime.endpoint.ipc.path,
                }
            },
            "ggml": {
                "install_dir": document.runtime.ggml.install_dir,
                "source": {
                    "version": document.runtime.ggml.source.version,
                    "artifact": document.runtime.ggml.source.artifact,
                },
                "logging": {
                    "level": document.runtime.ggml.logging.level,
                    "json": document.runtime.ggml.logging.json,
                    "path": document.runtime.ggml.logging.path,
                },
                "capacity": {
                    "queue": document.runtime.ggml.capacity.queue,
                    "concurrent_requests": document.runtime.ggml.capacity.concurrent_requests,
                },
                "endpoint": {
                    "http": {
                        "address": document.runtime.ggml.endpoint.http.address,
                    },
                    "ipc": {
                        "path": document.runtime.ggml.endpoint.ipc.path,
                    }
                },
                "backends": {
                    "llama": {
                        "enabled": document.runtime.ggml.backends.llama.enabled,
                        "context_length": document.runtime.ggml.backends.llama.context_length,
                        "flash_attn": document.runtime.ggml.backends.llama.flash_attn,
                        "source": {
                            "version": document.runtime.ggml.backends.llama.source.version,
                            "artifact": document.runtime.ggml.backends.llama.source.artifact,
                        },
                        "logging": {
                            "level": document.runtime.ggml.backends.llama.logging.level,
                            "json": document.runtime.ggml.backends.llama.logging.json,
                            "path": document.runtime.ggml.backends.llama.logging.path,
                        },
                        "capacity": {
                            "queue": document.runtime.ggml.backends.llama.capacity.queue,
                            "concurrent_requests": document.runtime.ggml.backends.llama.capacity.concurrent_requests,
                        },
                        "endpoint": {
                            "http": {
                                "address": document.runtime.ggml.backends.llama.endpoint.http.address,
                            },
                            "ipc": {
                                "path": document.runtime.ggml.backends.llama.endpoint.ipc.path,
                            }
                        }
                    },
                    "whisper": {
                        "enabled": document.runtime.ggml.backends.whisper.enabled,
                        "flash_attn": document.runtime.ggml.backends.whisper.flash_attn,
                        "source": {
                            "version": document.runtime.ggml.backends.whisper.source.version,
                            "artifact": document.runtime.ggml.backends.whisper.source.artifact,
                        },
                        "logging": {
                            "level": document.runtime.ggml.backends.whisper.logging.level,
                            "json": document.runtime.ggml.backends.whisper.logging.json,
                            "path": document.runtime.ggml.backends.whisper.logging.path,
                        },
                        "capacity": {
                            "queue": document.runtime.ggml.backends.whisper.capacity.queue,
                            "concurrent_requests": document.runtime.ggml.backends.whisper.capacity.concurrent_requests,
                        },
                        "endpoint": {
                            "http": {
                                "address": document.runtime.ggml.backends.whisper.endpoint.http.address,
                            },
                            "ipc": {
                                "path": document.runtime.ggml.backends.whisper.endpoint.ipc.path,
                            }
                        }
                    },
                    "diffusion": {
                        "enabled": document.runtime.ggml.backends.diffusion.enabled,
                        "flash_attn": document.runtime.ggml.backends.diffusion.flash_attn,
                        "source": {
                            "version": document.runtime.ggml.backends.diffusion.source.version,
                            "artifact": document.runtime.ggml.backends.diffusion.source.artifact,
                        },
                        "logging": {
                            "level": document.runtime.ggml.backends.diffusion.logging.level,
                            "json": document.runtime.ggml.backends.diffusion.logging.json,
                            "path": document.runtime.ggml.backends.diffusion.logging.path,
                        },
                        "capacity": {
                            "queue": document.runtime.ggml.backends.diffusion.capacity.queue,
                            "concurrent_requests": document.runtime.ggml.backends.diffusion.capacity.concurrent_requests,
                        },
                        "endpoint": {
                            "http": {
                                "address": document.runtime.ggml.backends.diffusion.endpoint.http.address,
                            },
                            "ipc": {
                                "path": document.runtime.ggml.backends.diffusion.endpoint.ipc.path,
                            }
                        }
                    }
                }
            },
            "candle": {
                "enabled": document.runtime.candle.enabled,
                "install_dir": document.runtime.candle.install_dir,
                "source": {
                    "version": document.runtime.candle.source.version,
                    "artifact": document.runtime.candle.source.artifact,
                },
                "logging": {
                    "level": document.runtime.candle.logging.level,
                    "json": document.runtime.candle.logging.json,
                    "path": document.runtime.candle.logging.path,
                },
                "capacity": {
                    "queue": document.runtime.candle.capacity.queue,
                    "concurrent_requests": document.runtime.candle.capacity.concurrent_requests,
                },
                "endpoint": {
                    "http": {
                        "address": document.runtime.candle.endpoint.http.address,
                    },
                    "ipc": {
                        "path": document.runtime.candle.endpoint.ipc.path,
                    }
                }
            },
            "onnx": {
                "enabled": document.runtime.onnx.enabled,
                "install_dir": document.runtime.onnx.install_dir,
                "source": {
                    "version": document.runtime.onnx.source.version,
                    "artifact": document.runtime.onnx.source.artifact,
                },
                "logging": {
                    "level": document.runtime.onnx.logging.level,
                    "json": document.runtime.onnx.logging.json,
                    "path": document.runtime.onnx.logging.path,
                },
                "capacity": {
                    "queue": document.runtime.onnx.capacity.queue,
                    "concurrent_requests": document.runtime.onnx.capacity.concurrent_requests,
                },
                "endpoint": {
                    "http": {
                        "address": document.runtime.onnx.endpoint.http.address,
                    },
                    "ipc": {
                        "path": document.runtime.onnx.endpoint.ipc.path,
                    }
                }
            }
        },
        "providers": {
            "registry": document.providers.registry.iter().map(|entry| json!({
                "id": entry.id,
                "family": entry.family,
                "display_name": entry.display_name,
                "api_base": entry.api_base,
                "auth": {
                    "api_key": entry.auth.api_key,
                    "api_key_env": entry.auth.api_key_env,
                },
                "defaults": {
                    "headers": entry.defaults.headers,
                    "query": entry.defaults.query,
                }
            })).collect::<Vec<_>>(),
        },
        "models": {
            "cache_dir": document.models.cache_dir,
                "config_dir": document.models.config_dir,
            "download_source": document.models.download_source,
            "auto_unload": {
                "enabled": document.models.auto_unload.enabled,
                "idle_minutes": document.models.auto_unload.idle_minutes,
                "min_free_system_memory_bytes": document.models.auto_unload.min_free_system_memory_bytes,
                "min_free_gpu_memory_bytes": document.models.auto_unload.min_free_gpu_memory_bytes,
                "max_pressure_evictions_per_load": document.models.auto_unload.max_pressure_evictions_per_load,
            }
            },
            "plugin": {
                "install_dir": document.plugin.install_dir,
            },
            "server": {
            "address": document.server.address,
            "logging": {
                "level": document.server.logging.level,
                "json": document.server.logging.json,
                "path": document.server.logging.path,
            },
            "cors": {
                "allowed_origins": document.server.cors.allowed_origins,
            },
            "admin": {
                "token": document.server.admin.token,
            },
            "swagger": {
                "enabled": document.server.swagger.enabled,
            },
            "cloud_http_trace": document.server.cloud_http_trace,
        }
    })
}

fn value_at_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = root;
    for segment in settings_path_segments(path).ok()? {
        current = current.as_object()?.get(segment)?;
    }
    Some(current)
}

fn set_value_at_path(root: &mut Value, path: &str, next_value: Value) -> Result<(), AppCoreError> {
    let segments = settings_path_segments(path)?;
    let (leaf, parents) = segments
        .split_last()
        .ok_or_else(|| AppCoreError::BadRequest("settings pmid must not be empty".to_owned()))?;
    let mut current = root;

    for segment in parents {
        current = current
            .as_object_mut()
            .and_then(|object| object.get_mut(*segment))
            .ok_or_else(|| AppCoreError::NotFound(format!("setting pmid '{}' not found", path)))?;
    }

    let object = current
        .as_object_mut()
        .ok_or_else(|| AppCoreError::NotFound(format!("setting pmid '{}' not found", path)))?;
    if !object.contains_key(*leaf) {
        return Err(AppCoreError::NotFound(format!("setting pmid '{}' not found", path)));
    }

    object.insert((*leaf).to_owned(), next_value);
    Ok(())
}

fn settings_path_segments(path: &str) -> Result<Vec<&str>, AppCoreError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AppCoreError::BadRequest("settings pmid must not be empty".to_owned()));
    }

    let segments: Vec<&str> = trimmed.split('.').map(str::trim).collect();
    if segments.iter().any(|segment| segment.is_empty()) {
        return Err(AppCoreError::BadRequest(format!(
            "settings pmid '{}' contains an empty path segment",
            path
        )));
    }

    Ok(segments)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Clone, Default)]
    struct TestConfigStore {
        values: Arc<Mutex<BTreeMap<String, String>>>,
    }

    impl TestConfigStore {
        fn value(&self, key: &str) -> Option<String> {
            self.values.lock().unwrap().get(key).cloned()
        }
    }

    impl ConfigStore for TestConfigStore {
        fn get_config_entry(
            &self,
            key: &str,
        ) -> impl std::future::Future<Output = Result<Option<(String, String)>, sqlx::Error>> + Send
        {
            let values = Arc::clone(&self.values);
            let key = key.to_owned();
            async move {
                let value = values.lock().unwrap().get(&key).cloned();
                Ok(value.map(|value| (key, value)))
            }
        }

        fn get_config_value(
            &self,
            key: &str,
        ) -> impl std::future::Future<Output = Result<Option<String>, sqlx::Error>> + Send {
            let values = Arc::clone(&self.values);
            let key = key.to_owned();
            async move { Ok(values.lock().unwrap().get(&key).cloned()) }
        }

        fn set_config_entry(
            &self,
            key: &str,
            _name: Option<&str>,
            value: &str,
        ) -> impl std::future::Future<Output = Result<(), sqlx::Error>> + Send {
            let values = Arc::clone(&self.values);
            let key = key.to_owned();
            let value = value.to_owned();
            async move {
                values.lock().unwrap().insert(key, value);
                Ok(())
            }
        }

        fn list_config_values(
            &self,
        ) -> impl std::future::Future<Output = Result<Vec<(String, String, String)>, sqlx::Error>> + Send
        {
            let values = Arc::clone(&self.values);
            async move {
                Ok(values
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|(key, value)| (key.clone(), key.clone(), value.clone()))
                    .collect())
            }
        }
    }

    fn temp_settings_path() -> PathBuf {
        let base = std::env::temp_dir().join(format!("slab-settings-test-{}", Uuid::new_v4()));
        base.join("settings.json")
    }

    fn write_legacy_settings(path: &Path, values: BTreeMap<String, Value>) {
        ensure_settings_parent_dir(path).expect("dir");
        fs::write(
            path,
            serde_json::to_string_pretty(&LegacySettingsValuesFile {
                version: LEGACY_SETTINGS_VERSION,
                values,
            })
            .expect("serialize"),
        )
        .expect("write");
    }

    #[tokio::test]
    async fn creates_missing_document_file() {
        let path = temp_settings_path();
        let provider = SettingsDocumentProvider::load(path.clone()).await.expect("provider");
        let file: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");

        assert_eq!(provider.document().await, SettingsDocument::default());
        assert_eq!(file.schema_version, SettingsDocument::default().schema_version);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn migrate_legacy_settings_rewrites_file_and_copies_setup_state() {
        let path = temp_settings_path();
        write_legacy_settings(
            &path,
            BTreeMap::from([
                ("setup.initialized".to_owned(), json!(true)),
                ("setup.ffmpeg.dir".to_owned(), json!("C:/ffmpeg")),
                ("runtime.model_cache_dir".to_owned(), json!("D:/models")),
                ("launch.profiles.server.gateway_bind".to_owned(), json!("127.0.0.1:4000")),
            ]),
        );
        let store = TestConfigStore::default();

        let migration = migrate_legacy_settings_if_needed(&path, &store).await.expect("migration");
        let persisted: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");

        assert!(migration.migrated);
        assert!(migration.backup_path.as_ref().is_some_and(|backup| backup.exists()));
        assert_eq!(persisted.tools.ffmpeg.install_dir.as_deref(), Some("C:/ffmpeg"));
        assert_eq!(persisted.models.cache_dir.as_deref(), Some("D:/models"));
        assert_eq!(persisted.server.address, "127.0.0.1:4000");
        assert_eq!(store.value(SETUP_INITIALIZED_CONFIG_KEY).as_deref(), Some("true"));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn migrate_legacy_chat_providers_to_registry() {
        let path = temp_settings_path();
        write_legacy_settings(
            &path,
            BTreeMap::from([(
                "chat.providers".to_owned(),
                json!([{
                    "id": "openai-main",
                    "name": "OpenAI",
                    "api_base": "https://api.openai.com/v1",
                    "api_key_env": "OPENAI_API_KEY"
                }]),
            )]),
        );
        let store = TestConfigStore::default();

        migrate_legacy_settings_if_needed(&path, &store).await.expect("migration");

        let persisted: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        let provider = persisted.providers.registry.first().expect("provider");
        assert_eq!(provider.id, "openai-main");
        assert_eq!(provider.display_name, "OpenAI");
        assert_eq!(provider.family, ProviderFamily::OpenaiCompatible);
        assert_eq!(provider.auth.api_key_env.as_deref(), Some("OPENAI_API_KEY"));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn provider_rejects_legacy_file_without_bootstrap_migration() {
        let path = temp_settings_path();
        write_legacy_settings(
            &path,
            BTreeMap::from([("runtime.model_cache_dir".to_owned(), json!("D:/models"))]),
        );

        let error = SettingsDocumentProvider::load(path.clone())
            .await
            .expect_err("legacy format should fail");

        assert!(matches!(error, AppCoreError::NotImplemented(_)));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn provider_set_and_unset_restore_defaults() {
        let path = temp_settings_path();
        let provider = SettingsDocumentProvider::load(path.clone()).await.expect("provider");

        provider
            .update(
                "logging.level",
                UpdateSettingCommand {
                    op: UpdateSettingOperation::Set,
                    value: Some(json!("debug")),
                },
            )
            .await
            .expect("set");
        assert_eq!(provider.value("logging.level").await.expect("value"), json!("debug"));

        provider
            .update(
                "logging.level",
                UpdateSettingCommand { op: UpdateSettingOperation::Unset, value: None },
            )
            .await
            .expect("unset");

        let persisted: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        assert_eq!(persisted.logging.level, "info");
        assert_eq!(provider.value("logging.level").await.expect("value"), json!("info"));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn corrupt_file_is_backed_up_and_rebuilt() {
        let path = temp_settings_path();
        ensure_settings_parent_dir(&path).expect("dir");
        fs::write(&path, "{ not valid json").expect("corrupt");

        let provider = SettingsDocumentProvider::load(path.clone()).await.expect("provider");
        let file: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        let backup_exists = fs::read_dir(path.parent().expect("parent"))
            .expect("dir")
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"));

        assert_eq!(file, SettingsDocument::default());
        assert!(!provider.warnings().await.is_empty());
        assert!(backup_exists);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }
}
