use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use serde_json::{Value, json};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::SettingsDocument;

use crate::app_config::default_plugin_install_dir_for_settings_path;
use crate::descriptor::{set_document_value, setting_value};
use crate::{ConfigError, SettingValue, UpdateSettingCommand, UpdateSettingOperation};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

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

impl SettingsDocumentProvider {
    pub async fn load(path: PathBuf) -> Result<Self, ConfigError> {
        let path_for_load = path.clone();
        let state = tokio::task::spawn_blocking(move || load_runtime_state(&path_for_load))
            .await
            .map_err(|error| {
                ConfigError::Internal(format!("settings loader task failed: {error}"))
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

    pub fn default_document(&self) -> SettingsDocument {
        default_settings_document_for_path(&self.path)
    }

    pub async fn value(&self, path: &str) -> Result<SettingValue, ConfigError> {
        let state = self.state.read().await;
        setting_value(&state.document, path)
    }

    pub async fn update(
        &self,
        path: &str,
        command: UpdateSettingCommand,
    ) -> Result<SettingValue, ConfigError> {
        let mut state = self.state.write().await;
        let mut next_settings = state.document.clone();
        let default_document = self.default_document();

        let requested_value = match command.op {
            UpdateSettingOperation::Set => command.value.ok_or_else(|| {
                ConfigError::BadRequest(format!(
                    "setting '{}' update requires a value when op=set",
                    path
                ))
            })?,
            UpdateSettingOperation::Unset => setting_value(&default_document, path)?,
        };

        set_document_value(&mut next_settings, path, requested_value)?;

        apply_dynamic_defaults(&self.path, &mut next_settings);
        let next_value = setting_value(&next_settings, path)?;

        write_settings_document_file(&self.path, &next_settings)?;
        state.document = next_settings;

        Ok(next_value)
    }
}

fn load_runtime_state(path: &Path) -> Result<SettingsRuntimeState, ConfigError> {
    ensure_settings_parent_dir(path)?;

    if !path.exists() {
        let document = default_settings_document_for_path(path);
        write_settings_document_file(path, &document)?;
        return Ok(SettingsRuntimeState { document, warnings: Vec::new() });
    }

    let raw = fs::read_to_string(path).map_err(|error| {
        ConfigError::Internal(format!("failed to read settings file '{}': {error}", path.display()))
    })?;

    let raw_json: Value = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            let warning = recover_corrupt_settings_file(path, &error.to_string())?;
            return Ok(SettingsRuntimeState {
                document: default_settings_document_for_path(path),
                warnings: vec![warning],
            });
        }
    };

    let invalid_document_error = || {
        ConfigError::BadRequest(format!(
            "settings file '{}' is valid JSON but does not match the current settings document format",
            path.display()
        ))
    };
    let Some(schema_version) = raw_json
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
    else {
        return Err(invalid_document_error());
    };
    ensure_current_schema_version(schema_version)?;

    if let Ok(mut document) = serde_json::from_value::<SettingsDocument>(raw_json) {
        ensure_current_schema_version(document.schema_version)?;
        if apply_dynamic_defaults(path, &mut document) {
            write_settings_document_file(path, &document)?;
        }
        return Ok(SettingsRuntimeState { document, warnings: Vec::new() });
    }

    Err(invalid_document_error())
}

fn ensure_current_schema_version(schema_version: u32) -> Result<(), ConfigError> {
    let current = SettingsDocument::default().schema_version;
    if schema_version == current {
        return Ok(());
    }

    Err(ConfigError::BadRequest(format!(
        "unsupported settings schema_version '{}'; expected '{}'",
        schema_version, current
    )))
}

fn create_settings_backup(path: &Path, reason: &str) -> Result<PathBuf, ConfigError> {
    let backup_name = format!(
        "{}.{}-{}.bak",
        path.file_name().and_then(|name| name.to_str()).unwrap_or("settings.json"),
        reason,
        Utc::now().format("%Y%m%d%H%M%S")
    );
    let backup_path = path.with_file_name(backup_name);

    fs::copy(path, &backup_path).map_err(|error| {
        ConfigError::Internal(format!(
            "failed to create settings backup '{}' from '{}': {error}",
            backup_path.display(),
            path.display()
        ))
    })?;

    Ok(backup_path)
}

fn recover_corrupt_settings_file(path: &Path, reason: &str) -> Result<String, ConfigError> {
    let backup_path = create_settings_backup(path, "corrupt")?;
    write_settings_document_file(path, &default_settings_document_for_path(path))?;

    Ok(format!(
        "Recovered from corrupt settings file. Original file was backed up to '{}' ({reason}).",
        backup_path.display()
    ))
}

fn default_settings_document_for_path(path: &Path) -> SettingsDocument {
    let mut document = SettingsDocument::default();
    apply_dynamic_defaults(path, &mut document);
    document
}

fn apply_dynamic_defaults(path: &Path, document: &mut SettingsDocument) -> bool {
    if document.plugin.install_dir.as_deref().is_some_and(|value| !value.trim().is_empty()) {
        return false;
    }

    document.plugin.install_dir =
        Some(default_plugin_install_dir_for_settings_path(path).to_string_lossy().into_owned());
    true
}

fn ensure_settings_parent_dir(path: &Path) -> Result<(), ConfigError> {
    let Some(parent) = path.parent() else {
        return Err(ConfigError::Internal(format!(
            "settings path '{}' has no parent directory",
            path.display()
        )));
    };

    fs::create_dir_all(parent).map_err(|error| {
        ConfigError::Internal(format!(
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
) -> Result<(), ConfigError> {
    ensure_settings_parent_dir(path)?;

    let parent = path.parent().ok_or_else(|| {
        ConfigError::Internal(format!("settings path '{}' has no parent directory", path.display()))
    })?;
    let file_name = path.file_name().and_then(|name| name.to_str()).ok_or_else(|| {
        ConfigError::Internal(format!("settings path '{}' has invalid file name", path.display()))
    })?;

    let temp_path = parent.join(format!(".{}.tmp-{}", file_name, Uuid::new_v4()));
    let mut payload = serde_json::to_vec_pretty(document).map_err(|error| {
        ConfigError::Internal(format!("failed to serialize settings file: {error}"))
    })?;
    payload.push(b'\n');

    let write_result = (|| -> Result<(), ConfigError> {
        let mut temp_file =
            OpenOptions::new().create_new(true).write(true).open(&temp_path).map_err(|error| {
                ConfigError::Internal(format!(
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
            ConfigError::Internal(format!("failed to write settings file: {error}"))
        })?;
        temp_file.sync_all().map_err(|error| {
            ConfigError::Internal(format!("failed to flush settings file: {error}"))
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
fn replace_file(from: &Path, to: &Path) -> Result<(), ConfigError> {
    fs::rename(from, to).map_err(|error| {
        ConfigError::Internal(format!(
            "failed to replace settings file '{}' with '{}': {error}",
            to.display(),
            from.display()
        ))
    })
}

#[cfg(windows)]
fn replace_file(from: &Path, to: &Path) -> Result<(), ConfigError> {
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
        Err(ConfigError::Internal(format!(
            "failed to replace settings file '{}': {}",
            to.display(),
            std::io::Error::last_os_error()
        )))
    } else {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
fn replace_file(from: &Path, to: &Path) -> Result<(), ConfigError> {
    fs::rename(from, to).map_err(|error| {
        ConfigError::Internal(format!(
            "failed to replace settings file '{}' with '{}': {error}",
            to.display(),
            from.display()
        ))
    })
}

#[cfg(unix)]
fn sync_parent_dir(parent: &Path) -> Result<(), ConfigError> {
    let dir = fs::File::open(parent).map_err(|error| {
        ConfigError::Internal(format!(
            "failed to open settings directory '{}': {error}",
            parent.display()
        ))
    })?;
    dir.sync_all().map_err(|error| {
        ConfigError::Internal(format!(
            "failed to sync settings directory '{}': {error}",
            parent.display()
        ))
    })
}

#[cfg(not(unix))]
fn sync_parent_dir(_parent: &Path) -> Result<(), ConfigError> {
    Ok(())
}

pub fn settings_document_to_json_value(document: &SettingsDocument) -> Value {
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
            "agent": {
                "tools": {
                    "mcp": {
                        "enabled": document.agent.tools.mcp.enabled,
                    },
                    "websearch": {
                    "default_provider": document.agent.tools.websearch.default_provider,
                    "providers": {
                        "duckduckgo": {
                            "base_url": document.agent.tools.websearch.providers.duckduckgo.base_url,
                            "user_agent": document.agent.tools.websearch.providers.duckduckgo.user_agent,
                            "use_lite": document.agent.tools.websearch.providers.duckduckgo.use_lite,
                        },
                        "arxiv": {},
                        "google": {
                            "auth": {
                                "api_key": document.agent.tools.websearch.providers.google.auth.api_key,
                                "api_key_env": document.agent.tools.websearch.providers.google.auth.api_key_env,
                            },
                            "cx": document.agent.tools.websearch.providers.google.cx,
                            "base_url": document.agent.tools.websearch.providers.google.base_url,
                        },
                        "tavily": {
                            "auth": {
                                "api_key": document.agent.tools.websearch.providers.tavily.auth.api_key,
                                "api_key_env": document.agent.tools.websearch.providers.tavily.auth.api_key_env,
                            },
                            "base_url": document.agent.tools.websearch.providers.tavily.base_url,
                            "search_depth": document.agent.tools.websearch.providers.tavily.search_depth,
                            "include_answer": document.agent.tools.websearch.providers.tavily.include_answer,
                            "include_images": document.agent.tools.websearch.providers.tavily.include_images,
                            "include_raw_content": document.agent.tools.websearch.providers.tavily.include_raw_content,
                        },
                        "exa": {
                            "auth": {
                                "api_key": document.agent.tools.websearch.providers.exa.auth.api_key,
                                "api_key_env": document.agent.tools.websearch.providers.exa.auth.api_key_env,
                            },
                            "base_url": document.agent.tools.websearch.providers.exa.base_url,
                            "model": document.agent.tools.websearch.providers.exa.model,
                            "include_contents": document.agent.tools.websearch.providers.exa.include_contents,
                        },
                        "serpapi": {
                            "auth": {
                                "api_key": document.agent.tools.websearch.providers.serpapi.auth.api_key,
                                "api_key_env": document.agent.tools.websearch.providers.serpapi.auth.api_key_env,
                            },
                            "engine": document.agent.tools.websearch.providers.serpapi.engine,
                            "base_url": document.agent.tools.websearch.providers.serpapi.base_url,
                        },
                        "brave": {
                            "auth": {
                                "api_key": document.agent.tools.websearch.providers.brave.auth.api_key,
                                "api_key_env": document.agent.tools.websearch.providers.brave.auth.api_key_env,
                            },
                        },
                        "searxng": {
                            "base_url": document.agent.tools.websearch.providers.searxng.base_url,
                        },
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_settings_path() -> PathBuf {
        let base = std::env::temp_dir().join(format!("slab-settings-test-{}", Uuid::new_v4()));
        base.join("settings.json")
    }

    #[tokio::test]
    async fn creates_missing_document_file() {
        let path = temp_settings_path();
        let provider = SettingsDocumentProvider::load(path.clone()).await.expect("provider");
        let file: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        let expected = default_settings_document_for_path(&path);

        assert_eq!(provider.document().await, expected);
        assert_eq!(file.schema_version, SettingsDocument::default().schema_version);
        assert_eq!(file.plugin.install_dir, expected.plugin.install_dir);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn provider_rejects_non_document_json() {
        let path = temp_settings_path();
        ensure_settings_parent_dir(&path).expect("dir");
        fs::write(
            &path,
            serde_json::to_string_pretty(&json!({
                "version": 1,
                "values": {
                    "runtime.model_cache_dir": "D:/models"
                }
            }))
            .expect("serialize"),
        )
        .expect("write");

        let error = SettingsDocumentProvider::load(path.clone())
            .await
            .expect_err("non-document JSON should fail");

        assert!(matches!(error, ConfigError::BadRequest(_)));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn provider_fills_missing_plugin_install_dir() {
        let path = temp_settings_path();
        ensure_settings_parent_dir(&path).expect("dir");
        fs::write(
            &path,
            serde_json::to_string_pretty(&SettingsDocument::default()).expect("serialize"),
        )
        .expect("write");

        let provider = SettingsDocumentProvider::load(path.clone()).await.expect("provider");
        let persisted: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        let expected = default_settings_document_for_path(&path);

        assert_eq!(provider.document().await.plugin.install_dir, expected.plugin.install_dir);
        assert_eq!(persisted.plugin.install_dir, expected.plugin.install_dir);

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
                    value: Some(json!("debug").into()),
                },
            )
            .await
            .expect("set");
        assert_eq!(provider.value("logging.level").await.expect("value"), json!("debug").into());

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
        assert_eq!(provider.value("logging.level").await.expect("value"), json!("info").into());

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn provider_unset_plugin_install_dir_restores_dynamic_default() {
        let path = temp_settings_path();
        let provider = SettingsDocumentProvider::load(path.clone()).await.expect("provider");
        let configured_dir = path.parent().expect("parent").join("custom-plugins");

        provider
            .update(
                "plugin.install_dir",
                UpdateSettingCommand {
                    op: UpdateSettingOperation::Set,
                    value: Some(json!(configured_dir.to_string_lossy()).into()),
                },
            )
            .await
            .expect("set");
        provider
            .update(
                "plugin.install_dir",
                UpdateSettingCommand { op: UpdateSettingOperation::Unset, value: None },
            )
            .await
            .expect("unset");

        let persisted: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        let expected = default_settings_document_for_path(&path);

        assert_eq!(persisted.plugin.install_dir, expected.plugin.install_dir);
        assert_eq!(
            provider.value("plugin.install_dir").await.expect("value"),
            json!(expected.plugin.install_dir.expect("plugin dir")).into()
        );

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

        assert_eq!(file, default_settings_document_for_path(&path));
        assert!(!provider.warnings().await.is_empty());
        assert!(backup_exists);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }
}
