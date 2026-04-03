use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::RwLock;
use uuid::Uuid;

use slab_types::settings::{CloudProviderConfig, SettingsDocumentV2};

use crate::domain::models::{
    SettingDefinition, SettingsDocumentView, SettingsSchema, SettingsSectionView,
    SettingsSubsectionView, SettingsValuesFile, UpdateSettingCommand, embedded_settings_schema,
};
use crate::error::AppCoreError;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[derive(Debug, Clone)]
pub struct SettingsProvider {
    schema: SettingsSchema,
    path: PathBuf,
    state: Arc<RwLock<SettingsRuntimeState>>,
}

#[derive(Debug, Clone, Default)]
struct SettingsRuntimeState {
    overrides: BTreeMap<String, serde_json::Value>,
    unknown_values: BTreeMap<String, serde_json::Value>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SettingsDocumentProviderV2 {
    path: PathBuf,
    state: Arc<RwLock<SettingsDocumentRuntimeStateV2>>,
}

#[derive(Debug, Clone, Default)]
struct SettingsDocumentRuntimeStateV2 {
    document: SettingsDocumentV2,
    warnings: Vec<String>,
}

impl SettingsProvider {
    pub async fn load(path: PathBuf) -> Result<Self, AppCoreError> {
        let schema = embedded_settings_schema()?;
        let schema_for_load = schema.clone();
        let path_for_load = path.clone();
        let state = tokio::task::spawn_blocking(move || {
            load_runtime_state(&schema_for_load, &path_for_load)
        })
        .await
        .map_err(|error| {
            AppCoreError::Internal(format!("settings loader task failed: {error}"))
        })??;

        Ok(Self { schema, path, state: Arc::new(RwLock::new(state)) })
    }

    #[cfg(test)]
    pub fn schema_version(&self) -> u32 {
        self.schema.schema_version()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn document(&self) -> SettingsDocumentView {
        let state = self.state.read().await;

        SettingsDocumentView {
            schema_version: self.schema.schema_version(),
            settings_path: self.path.to_string_lossy().into_owned(),
            warnings: state.warnings.clone(),
            sections: self
                .schema
                .sections()
                .iter()
                .map(|section| SettingsSectionView {
                    id: section.id.clone(),
                    title: section.title.clone(),
                    description_md: section.description_md.clone(),
                    subsections: section
                        .subsections
                        .iter()
                        .map(|subsection| SettingsSubsectionView {
                            id: subsection.id.clone(),
                            title: subsection.title.clone(),
                            description_md: subsection.description_md.clone(),
                            properties: subsection
                                .properties
                                .iter()
                                .map(|property| {
                                    property.build_view(state.overrides.get(&property.pmid))
                                })
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    pub async fn property(
        &self,
        pmid: impl AsRef<str>,
    ) -> Result<crate::domain::models::SettingPropertyView, AppCoreError> {
        let pmid = pmid.as_ref();
        let definition = self.definition(pmid)?;
        let state = self.state.read().await;
        Ok(definition.build_view(state.overrides.get(pmid)))
    }

    pub async fn update(
        &self,
        pmid: impl AsRef<str>,
        command: UpdateSettingCommand,
    ) -> Result<crate::domain::models::SettingPropertyView, AppCoreError> {
        let pmid = pmid.as_ref();
        let definition = self.definition(pmid)?;
        let next_override = definition.canonicalize_update_command(&command)?;
        let mut state = self.state.write().await;

        let mut next_overrides = state.overrides.clone();
        match next_override {
            Some(value) if value == *definition.default_value() => {
                next_overrides.remove(pmid);
            }
            Some(value) => {
                next_overrides.insert(pmid.to_owned(), value);
            }
            None => {
                next_overrides.remove(pmid);
            }
        }

        persist_runtime_state(&self.schema, &self.path, &next_overrides, &state.unknown_values)?;

        state.overrides = next_overrides;
        Ok(definition.build_view(state.overrides.get(pmid)))
    }

    pub async fn get_effective_value(
        &self,
        pmid: impl AsRef<str>,
    ) -> Result<serde_json::Value, AppCoreError> {
        let pmid = pmid.as_ref();
        let definition = self.definition(pmid)?;
        let state = self.state.read().await;
        Ok(definition.build_view(state.overrides.get(pmid)).effective_value)
    }

    pub async fn get_optional_string(
        &self,
        pmid: impl AsRef<str>,
    ) -> Result<Option<String>, AppCoreError> {
        let pmid = pmid.as_ref();
        let value = self.get_effective_value(pmid).await?;
        match value {
            serde_json::Value::String(value) => {
                let trimmed = value.trim().to_owned();
                if trimmed.is_empty() { Ok(None) } else { Ok(Some(trimmed)) }
            }
            serde_json::Value::Null => Ok(None),
            other => Err(AppCoreError::Internal(format!(
                "settings '{}' expected string value, found {}",
                pmid,
                describe_value_type(&other)
            ))),
        }
    }

    pub async fn get_bool(&self, pmid: impl AsRef<str>) -> Result<bool, AppCoreError> {
        let pmid = pmid.as_ref();
        let value = self.get_effective_value(pmid).await?;
        match value {
            serde_json::Value::Bool(value) => Ok(value),
            other => Err(AppCoreError::Internal(format!(
                "settings '{}' expected boolean value, found {}",
                pmid,
                describe_value_type(&other)
            ))),
        }
    }

    pub async fn get_optional_u32(
        &self,
        pmid: impl AsRef<str>,
    ) -> Result<Option<u32>, AppCoreError> {
        let pmid = pmid.as_ref();
        let value = self.get_effective_value(pmid).await?;
        match value {
            serde_json::Value::Null => Ok(None),
            serde_json::Value::Number(number) => {
                let value = number.as_u64().ok_or_else(|| {
                    AppCoreError::Internal(format!(
                        "settings '{}' expected a positive integer value",
                        pmid
                    ))
                })?;
                u32::try_from(value).map(Some).map_err(|_| {
                    AppCoreError::Internal(format!("settings '{}' does not fit into u32", pmid))
                })
            }
            other => Err(AppCoreError::Internal(format!(
                "settings '{}' expected integer value, found {}",
                pmid,
                describe_value_type(&other)
            ))),
        }
    }

    pub async fn get_chat_providers(
        &self,
        pmid: impl AsRef<str>,
    ) -> Result<Vec<CloudProviderConfig>, AppCoreError> {
        let pmid = pmid.as_ref();
        let value = self.get_effective_value(pmid).await?;
        serde_json::from_value(value).map_err(|error| {
            AppCoreError::Internal(format!(
                "settings '{}' contains invalid provider payload: {error}",
                pmid
            ))
        })
    }

    fn definition(&self, pmid: &str) -> Result<&SettingDefinition, AppCoreError> {
        self.schema
            .property(pmid)
            .ok_or_else(|| AppCoreError::NotFound(format!("setting pmid '{}' not found", pmid)))
    }
}

impl SettingsDocumentProviderV2 {
    pub async fn load(path: PathBuf) -> Result<Self, AppCoreError> {
        let path_for_load = path.clone();
        let state = tokio::task::spawn_blocking(move || load_runtime_state_v2(&path_for_load))
            .await
            .map_err(|error| {
                AppCoreError::Internal(format!("settings V2 loader task failed: {error}"))
            })??;

        Ok(Self { path, state: Arc::new(RwLock::new(state)) })
    }

    pub async fn document(&self) -> SettingsDocumentV2 {
        self.state.read().await.document.clone()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn warnings(&self) -> Vec<String> {
        self.state.read().await.warnings.clone()
    }

    pub async fn value(&self, path: &str) -> Result<serde_json::Value, AppCoreError> {
        let state = self.state.read().await;
        let document = settings_document_v2_to_json_value(&state.document);

        value_at_path(&document, path)
            .cloned()
            .ok_or_else(|| AppCoreError::NotFound(format!("setting pmid '{}' not found", path)))
    }

    pub async fn update(
        &self,
        path: &str,
        command: UpdateSettingCommand,
    ) -> Result<serde_json::Value, AppCoreError> {
        let mut state = self.state.write().await;
        let mut next_document = settings_document_v2_to_json_value(&state.document);
        let default_document = settings_document_v2_to_json_value(&SettingsDocumentV2::default());

        let next_value = match command.op {
            crate::domain::models::UpdateSettingOperation::Set => {
                command.value.ok_or_else(|| {
                    AppCoreError::BadRequest(format!(
                        "setting '{}' update requires a value when op=set",
                        path
                    ))
                })?
            }
            crate::domain::models::UpdateSettingOperation::Unset => {
                value_at_path(&default_document, path).cloned().ok_or_else(|| {
                    AppCoreError::NotFound(format!("setting pmid '{}' not found", path))
                })?
            }
        };

        set_value_at_path(&mut next_document, path, next_value.clone())?;

        let next_settings: SettingsDocumentV2 =
            serde_json::from_value(next_document).map_err(|error| {
                AppCoreError::BadRequest(format!(
                    "setting '{}' update produced an invalid settings document: {error}",
                    path
                ))
            })?;

        write_settings_document_v2_file(&self.path, &next_settings)?;
        state.document = next_settings;

        Ok(next_value)
    }
}

fn load_runtime_state(
    schema: &SettingsSchema,
    path: &Path,
) -> Result<SettingsRuntimeState, AppCoreError> {
    ensure_settings_parent_dir(path)?;

    if !path.exists() {
        persist_runtime_state(schema, path, &BTreeMap::new(), &BTreeMap::new())?;
        return Ok(SettingsRuntimeState::default());
    }

    let raw = fs::read_to_string(path).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to read settings file '{}': {error}",
            path.display()
        ))
    })?;

    let raw_json: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            let warning =
                recover_corrupt_settings_file(path, schema.schema_version(), &error.to_string())?;
            return Ok(SettingsRuntimeState {
                warnings: vec![warning],
                ..SettingsRuntimeState::default()
            });
        }
    };

    let parsed: SettingsValuesFile = match serde_json::from_value(raw_json.clone()) {
        Ok(parsed) => parsed,
        Err(error) => {
            if serde_json::from_value::<SettingsDocumentV2>(raw_json).is_ok() {
                return Err(AppCoreError::NotImplemented(
                    "settings file uses the V2 nested document format; load it with SettingsDocumentProviderV2"
                        .to_owned(),
                ));
            }

            return Err(AppCoreError::BadRequest(format!(
                "settings file '{}' is valid JSON but does not match the legacy PMID override format: {error}",
                path.display()
            )));
        }
    };

    let mut overrides = BTreeMap::new();
    let mut unknown_values = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut should_rewrite = parsed.version != schema.schema_version();

    for (pmid, raw_value) in parsed.values {
        let Some(definition) = schema.property(&pmid) else {
            unknown_values.insert(pmid, raw_value);
            continue;
        };

        match definition.canonicalize_loaded_override(&raw_value) {
            Ok(Some(value)) => {
                overrides.insert(pmid, value);
            }
            Ok(None) => {
                should_rewrite = true;
            }
            Err(error) => {
                warnings.push(format!("Dropped invalid value for '{}': {}", pmid, error));
                should_rewrite = true;
            }
        }
    }

    if should_rewrite {
        persist_runtime_state(schema, path, &overrides, &unknown_values)?;
    }

    Ok(SettingsRuntimeState { overrides, unknown_values, warnings })
}

fn recover_corrupt_settings_file(
    path: &Path,
    schema_version: u32,
    reason: &str,
) -> Result<String, AppCoreError> {
    let backup_name = format!(
        "{}.corrupt-{}",
        path.file_name().and_then(|name| name.to_str()).unwrap_or("settings.json"),
        Utc::now().format("%Y%m%d%H%M%S")
    );
    let backup_path = path.with_file_name(backup_name);

    fs::rename(path, &backup_path).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to back up corrupt settings file '{}': {error}",
            path.display()
        ))
    })?;

    write_values_file(
        path,
        &SettingsValuesFile { version: schema_version, values: BTreeMap::new() },
    )?;

    Ok(format!(
        "Recovered from corrupt settings file. Original file was backed up to '{}' ({reason}).",
        backup_path.display()
    ))
}

fn load_runtime_state_v2(path: &Path) -> Result<SettingsDocumentRuntimeStateV2, AppCoreError> {
    ensure_settings_parent_dir(path)?;

    if !path.exists() {
        let document = SettingsDocumentV2::default();
        write_settings_document_v2_file(path, &document)?;
        return Ok(SettingsDocumentRuntimeStateV2 { document, warnings: Vec::new() });
    }

    let raw = fs::read_to_string(path).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to read settings V2 file '{}': {error}",
            path.display()
        ))
    })?;

    let raw_json: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            let warning = recover_corrupt_settings_document_v2_file(path, &error.to_string())?;
            return Ok(SettingsDocumentRuntimeStateV2 {
                document: SettingsDocumentV2::default(),
                warnings: vec![warning],
            });
        }
    };

    match serde_json::from_value::<SettingsDocumentV2>(raw_json.clone()) {
        Ok(document) => Ok(SettingsDocumentRuntimeStateV2 { document, warnings: Vec::new() }),
        Err(error) => {
            if serde_json::from_value::<SettingsValuesFile>(raw_json).is_ok() {
                return Err(AppCoreError::NotImplemented(
                    "settings file uses the legacy PMID override format; load it with SettingsProvider"
                        .to_owned(),
                ));
            }

            Err(AppCoreError::BadRequest(format!(
                "settings file '{}' is valid JSON but does not match the V2 settings document format: {error}",
                path.display()
            )))
        }
    }
}

fn recover_corrupt_settings_document_v2_file(
    path: &Path,
    reason: &str,
) -> Result<String, AppCoreError> {
    let backup_name = format!(
        "{}.corrupt-{}",
        path.file_name().and_then(|name| name.to_str()).unwrap_or("settings.json"),
        Utc::now().format("%Y%m%d%H%M%S")
    );
    let backup_path = path.with_file_name(backup_name);

    fs::rename(path, &backup_path).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to back up corrupt settings V2 file '{}': {error}",
            path.display()
        ))
    })?;

    write_settings_document_v2_file(path, &SettingsDocumentV2::default())?;

    Ok(format!(
        "Recovered from corrupt settings V2 file. Original file was backed up to '{}' ({reason}).",
        backup_path.display()
    ))
}

fn persist_runtime_state(
    schema: &SettingsSchema,
    path: &Path,
    overrides: &BTreeMap<String, serde_json::Value>,
    unknown_values: &BTreeMap<String, serde_json::Value>,
) -> Result<(), AppCoreError> {
    let mut values = unknown_values.clone();
    values.extend(overrides.clone());
    write_values_file(path, &SettingsValuesFile { version: schema.schema_version(), values })
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

fn write_values_file(path: &Path, values: &SettingsValuesFile) -> Result<(), AppCoreError> {
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
    let mut payload = serde_json::to_vec_pretty(values).map_err(|error| {
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

fn write_settings_document_v2_file(
    path: &Path,
    document: &SettingsDocumentV2,
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
        AppCoreError::Internal(format!("failed to serialize settings V2 file: {error}"))
    })?;
    payload.push(b'\n');

    let write_result = (|| -> Result<(), AppCoreError> {
        let mut temp_file =
            OpenOptions::new().create_new(true).write(true).open(&temp_path).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to create temp settings V2 file '{}': {error}",
                    temp_path.display()
                ))
            })?;

        #[cfg(unix)]
        {
            let permissions = fs::Permissions::from_mode(0o600);
            let _ = temp_file.set_permissions(permissions);
        }

        temp_file.write_all(&payload).map_err(|error| {
            AppCoreError::Internal(format!("failed to write settings V2 file: {error}"))
        })?;
        temp_file.sync_all().map_err(|error| {
            AppCoreError::Internal(format!("failed to flush settings V2 file: {error}"))
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

fn describe_value_type(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

pub(crate) fn settings_document_v2_to_json_value(
    document: &SettingsDocumentV2,
) -> serde_json::Value {
    serde_json::json!({
        "$schema": document.schema,
        "schema_version": document.schema_version,
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
            "registry": document.providers.registry.iter().map(|entry| serde_json::json!({
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
            "auto_unload": {
                "enabled": document.models.auto_unload.enabled,
                "idle_minutes": document.models.auto_unload.idle_minutes,
            }
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

fn value_at_path<'a>(root: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut current = root;
    for segment in settings_path_segments(path).ok()? {
        current = current.as_object()?.get(segment)?;
    }
    Some(current)
}

fn set_value_at_path(
    root: &mut serde_json::Value,
    path: &str,
    next_value: serde_json::Value,
) -> Result<(), AppCoreError> {
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
    use super::*;
    use crate::domain::models::PMID;

    fn temp_settings_path() -> PathBuf {
        let base = std::env::temp_dir().join(format!("slab-settings-test-{}", Uuid::new_v4()));
        base.join("settings.json")
    }

    #[tokio::test]
    async fn creates_missing_values_file() {
        let path = temp_settings_path();
        let provider = SettingsProvider::load(path.clone()).await.expect("provider");
        let file: SettingsValuesFile =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");

        assert!(path.exists());
        assert_eq!(provider.schema_version(), 1);
        assert!(file.values.is_empty());

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn invalid_override_is_dropped_during_load() {
        let path = temp_settings_path();
        ensure_settings_parent_dir(&path).expect("dir");
        write_values_file(
            &path,
            &SettingsValuesFile {
                version: 1,
                values: BTreeMap::from([(
                    PMID.runtime.llama.num_workers().into_string(),
                    serde_json::json!(0),
                )]),
            },
        )
        .expect("seed");

        let provider = SettingsProvider::load(path.clone()).await.expect("provider");
        let doc = provider.document().await;
        assert!(!doc.warnings.is_empty());

        let property = provider.property(PMID.runtime.llama.num_workers()).await.expect("property");
        assert_eq!(property.effective_value, serde_json::json!(1));
        assert!(!property.is_overridden);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn unset_removes_override_from_file() {
        let path = temp_settings_path();
        let provider = SettingsProvider::load(path.clone()).await.expect("provider");

        provider
            .update(
                PMID.runtime.llama.context_length(),
                UpdateSettingCommand {
                    op: crate::domain::models::UpdateSettingOperation::Set,
                    value: Some(serde_json::json!(4096)),
                },
            )
            .await
            .expect("set");
        provider
            .update(
                PMID.runtime.llama.context_length(),
                UpdateSettingCommand {
                    op: crate::domain::models::UpdateSettingOperation::Unset,
                    value: None,
                },
            )
            .await
            .expect("unset");

        let file: SettingsValuesFile =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        assert!(!file.values.contains_key(PMID.runtime.llama.context_length().as_str()));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn chat_provider_values_round_trip() {
        let path = temp_settings_path();
        let provider = SettingsProvider::load(path.clone()).await.expect("provider");

        provider
            .update(
                PMID.chat.providers(),
                UpdateSettingCommand {
                    op: crate::domain::models::UpdateSettingOperation::Set,
                    value: Some(serde_json::json!([
                        {
                            "id": "openai-main",
                            "name": "OpenAI",
                            "api_base": "https://api.openai.com/v1"
                        }
                    ])),
                },
            )
            .await
            .expect("set");

        let providers =
            provider.get_chat_providers(PMID.chat.providers()).await.expect("providers");
        assert_eq!(providers.len(), 1);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn unknown_values_are_preserved_but_ignored() {
        let path = temp_settings_path();
        ensure_settings_parent_dir(&path).expect("dir");
        write_values_file(
            &path,
            &SettingsValuesFile {
                version: 1,
                values: BTreeMap::from([(
                    "legacy.unmanaged".to_owned(),
                    serde_json::json!({"kept": true}),
                )]),
            },
        )
        .expect("seed");

        let provider = SettingsProvider::load(path.clone()).await.expect("provider");
        let document = provider.document().await;
        let found = document
            .sections
            .iter()
            .flat_map(|section| section.subsections.iter())
            .flat_map(|subsection| subsection.properties.iter())
            .any(|property| property.pmid == "legacy.unmanaged");
        let file: SettingsValuesFile =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");

        assert!(!found);
        assert!(file.values.contains_key("legacy.unmanaged"));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn corrupt_file_is_backed_up_and_rebuilt() {
        let path = temp_settings_path();
        ensure_settings_parent_dir(&path).expect("dir");
        fs::write(&path, "{ not valid json").expect("corrupt");

        let provider = SettingsProvider::load(path.clone()).await.expect("provider");
        let document = provider.document().await;
        let file: SettingsValuesFile =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        let backup_exists = fs::read_dir(path.parent().expect("parent"))
            .expect("dir")
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().starts_with("settings.json.corrupt-"));

        assert!(file.values.is_empty());
        assert!(!document.warnings.is_empty());
        assert!(backup_exists);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn legacy_provider_rejects_v2_document_without_rewriting_file() {
        let path = temp_settings_path();
        ensure_settings_parent_dir(&path).expect("dir");
        let mut document = SettingsDocumentV2::default();
        document.logging.level = "debug".to_owned();
        fs::write(&path, serde_json::to_string_pretty(&document).expect("serialize"))
            .expect("write");

        let error = SettingsProvider::load(path.clone()).await.expect_err("legacy must reject V2");
        let persisted: SettingsDocumentV2 =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");

        assert!(matches!(error, AppCoreError::NotImplemented(_)));
        assert_eq!(persisted.logging.level, "debug");

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn v2_provider_creates_missing_document_file() {
        let path = temp_settings_path();

        let provider = SettingsDocumentProviderV2::load(path.clone()).await.expect("provider");
        let file: SettingsDocumentV2 =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");

        assert_eq!(provider.document().await, SettingsDocumentV2::default());
        assert_eq!(file.schema_version, 2);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn v2_provider_set_and_unset_restore_defaults() {
        let path = temp_settings_path();
        let provider = SettingsDocumentProviderV2::load(path.clone()).await.expect("provider");

        provider
            .update(
                "logging.level",
                UpdateSettingCommand {
                    op: crate::domain::models::UpdateSettingOperation::Set,
                    value: Some(serde_json::json!("debug")),
                },
            )
            .await
            .expect("set");
        assert_eq!(
            provider.value("logging.level").await.expect("value"),
            serde_json::json!("debug")
        );

        provider
            .update(
                "logging.level",
                UpdateSettingCommand {
                    op: crate::domain::models::UpdateSettingOperation::Unset,
                    value: None,
                },
            )
            .await
            .expect("unset");

        let persisted: SettingsDocumentV2 =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        assert_eq!(persisted.logging.level, "info");
        assert_eq!(
            provider.value("logging.level").await.expect("value"),
            serde_json::json!("info")
        );

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn v2_provider_rejects_type_invalid_updates() {
        let path = temp_settings_path();
        let provider = SettingsDocumentProviderV2::load(path.clone()).await.expect("provider");

        let error = provider
            .update(
                "logging.json",
                UpdateSettingCommand {
                    op: crate::domain::models::UpdateSettingOperation::Set,
                    value: Some(serde_json::json!("yes")),
                },
            )
            .await
            .expect_err("invalid type must fail");

        assert!(matches!(error, AppCoreError::BadRequest(_)));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn v2_provider_handles_optional_paths_that_default_to_null() {
        let path = temp_settings_path();
        let provider = SettingsDocumentProviderV2::load(path.clone()).await.expect("provider");

        assert_eq!(
            provider.value("runtime.ggml.install_dir").await.expect("value"),
            serde_json::Value::Null
        );

        provider
            .update(
                "runtime.ggml.install_dir",
                UpdateSettingCommand {
                    op: crate::domain::models::UpdateSettingOperation::Set,
                    value: Some(serde_json::json!("D:/runtime/libs")),
                },
            )
            .await
            .expect("set");

        assert_eq!(
            provider.value("runtime.ggml.install_dir").await.expect("value"),
            serde_json::json!("D:/runtime/libs")
        );

        provider
            .update(
                "runtime.ggml.install_dir",
                UpdateSettingCommand {
                    op: crate::domain::models::UpdateSettingOperation::Unset,
                    value: None,
                },
            )
            .await
            .expect("unset");

        assert_eq!(
            provider.value("runtime.ggml.install_dir").await.expect("value"),
            serde_json::Value::Null
        );

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }
}
