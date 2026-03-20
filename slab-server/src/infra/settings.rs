use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::RwLock;
use uuid::Uuid;

use slab_types::settings::CloudProviderConfig;

use crate::domain::models::{
    embedded_settings_schema, SettingDefinition, SettingsDocumentView, SettingsSchema,
    SettingsSectionView, SettingsSubsectionView, SettingsValuesFile, UpdateSettingCommand,
};
use crate::error::ServerError;

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

impl SettingsProvider {
    pub async fn load(path: PathBuf) -> Result<Self, ServerError> {
        let schema = embedded_settings_schema()?;
        let schema_for_load = schema.clone();
        let path_for_load = path.clone();
        let state = tokio::task::spawn_blocking(move || {
            load_runtime_state(&schema_for_load, &path_for_load)
        })
        .await
        .map_err(|error| {
            ServerError::Internal(format!("settings loader task failed: {error}"))
        })??;

        Ok(Self {
            schema,
            path,
            state: Arc::new(RwLock::new(state)),
        })
    }

    #[cfg(test)]
    pub fn schema_version(&self) -> u32 {
        self.schema.schema_version()
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
    ) -> Result<crate::domain::models::SettingPropertyView, ServerError> {
        let pmid = pmid.as_ref();
        let definition = self.definition(pmid)?;
        let state = self.state.read().await;
        Ok(definition.build_view(state.overrides.get(pmid)))
    }

    pub async fn update(
        &self,
        pmid: impl AsRef<str>,
        command: UpdateSettingCommand,
    ) -> Result<crate::domain::models::SettingPropertyView, ServerError> {
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

        persist_runtime_state(
            &self.schema,
            &self.path,
            &next_overrides,
            &state.unknown_values,
        )?;

        state.overrides = next_overrides;
        Ok(definition.build_view(state.overrides.get(pmid)))
    }

    pub async fn get_effective_value(
        &self,
        pmid: impl AsRef<str>,
    ) -> Result<serde_json::Value, ServerError> {
        let pmid = pmid.as_ref();
        let definition = self.definition(pmid)?;
        let state = self.state.read().await;
        Ok(definition
            .build_view(state.overrides.get(pmid))
            .effective_value)
    }

    pub async fn get_optional_string(
        &self,
        pmid: impl AsRef<str>,
    ) -> Result<Option<String>, ServerError> {
        let pmid = pmid.as_ref();
        let value = self.get_effective_value(pmid).await?;
        match value {
            serde_json::Value::String(value) => {
                let trimmed = value.trim().to_owned();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(trimmed))
                }
            }
            serde_json::Value::Null => Ok(None),
            other => Err(ServerError::Internal(format!(
                "settings '{}' expected string value, found {}",
                pmid,
                describe_value_type(&other)
            ))),
        }
    }

    pub async fn get_bool(&self, pmid: impl AsRef<str>) -> Result<bool, ServerError> {
        let pmid = pmid.as_ref();
        let value = self.get_effective_value(pmid).await?;
        match value {
            serde_json::Value::Bool(value) => Ok(value),
            other => Err(ServerError::Internal(format!(
                "settings '{}' expected boolean value, found {}",
                pmid,
                describe_value_type(&other)
            ))),
        }
    }

    pub async fn get_optional_u32(
        &self,
        pmid: impl AsRef<str>,
    ) -> Result<Option<u32>, ServerError> {
        let pmid = pmid.as_ref();
        let value = self.get_effective_value(pmid).await?;
        match value {
            serde_json::Value::Null => Ok(None),
            serde_json::Value::Number(number) => {
                let value = number.as_u64().ok_or_else(|| {
                    ServerError::Internal(format!(
                        "settings '{}' expected a positive integer value",
                        pmid
                    ))
                })?;
                u32::try_from(value).map(Some).map_err(|_| {
                    ServerError::Internal(format!("settings '{}' does not fit into u32", pmid))
                })
            }
            other => Err(ServerError::Internal(format!(
                "settings '{}' expected integer value, found {}",
                pmid,
                describe_value_type(&other)
            ))),
        }
    }

    pub async fn get_chat_providers(
        &self,
        pmid: impl AsRef<str>,
    ) -> Result<Vec<CloudProviderConfig>, ServerError> {
        let pmid = pmid.as_ref();
        let value = self.get_effective_value(pmid).await?;
        serde_json::from_value(value).map_err(|error| {
            ServerError::Internal(format!(
                "settings '{}' contains invalid provider payload: {error}",
                pmid
            ))
        })
    }

    fn definition(&self, pmid: &str) -> Result<&SettingDefinition, ServerError> {
        self.schema
            .property(pmid)
            .ok_or_else(|| ServerError::NotFound(format!("setting pmid '{}' not found", pmid)))
    }
}

fn load_runtime_state(
    schema: &SettingsSchema,
    path: &Path,
) -> Result<SettingsRuntimeState, ServerError> {
    ensure_settings_parent_dir(path)?;

    if !path.exists() {
        persist_runtime_state(schema, path, &BTreeMap::new(), &BTreeMap::new())?;
        return Ok(SettingsRuntimeState::default());
    }

    let raw = fs::read_to_string(path).map_err(|error| {
        ServerError::Internal(format!(
            "failed to read settings file '{}': {error}",
            path.display()
        ))
    })?;

    let parsed: SettingsValuesFile = match serde_json::from_str(&raw) {
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

    Ok(SettingsRuntimeState {
        overrides,
        unknown_values,
        warnings,
    })
}

fn recover_corrupt_settings_file(
    path: &Path,
    schema_version: u32,
    reason: &str,
) -> Result<String, ServerError> {
    let backup_name = format!(
        "{}.corrupt-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("settings.json"),
        Utc::now().format("%Y%m%d%H%M%S")
    );
    let backup_path = path.with_file_name(backup_name);

    fs::rename(path, &backup_path).map_err(|error| {
        ServerError::Internal(format!(
            "failed to back up corrupt settings file '{}': {error}",
            path.display()
        ))
    })?;

    write_values_file(
        path,
        &SettingsValuesFile {
            version: schema_version,
            values: BTreeMap::new(),
        },
    )?;

    Ok(format!(
        "Recovered from corrupt settings file. Original file was backed up to '{}' ({reason}).",
        backup_path.display()
    ))
}

fn persist_runtime_state(
    schema: &SettingsSchema,
    path: &Path,
    overrides: &BTreeMap<String, serde_json::Value>,
    unknown_values: &BTreeMap<String, serde_json::Value>,
) -> Result<(), ServerError> {
    let mut values = unknown_values.clone();
    values.extend(overrides.clone());
    write_values_file(
        path,
        &SettingsValuesFile {
            version: schema.schema_version(),
            values,
        },
    )
}

fn ensure_settings_parent_dir(path: &Path) -> Result<(), ServerError> {
    let Some(parent) = path.parent() else {
        return Err(ServerError::Internal(format!(
            "settings path '{}' has no parent directory",
            path.display()
        )));
    };

    fs::create_dir_all(parent).map_err(|error| {
        ServerError::Internal(format!(
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

fn write_values_file(path: &Path, values: &SettingsValuesFile) -> Result<(), ServerError> {
    ensure_settings_parent_dir(path)?;

    let parent = path.parent().ok_or_else(|| {
        ServerError::Internal(format!(
            "settings path '{}' has no parent directory",
            path.display()
        ))
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            ServerError::Internal(format!(
                "settings path '{}' has invalid file name",
                path.display()
            ))
        })?;

    let temp_path = parent.join(format!(".{}.tmp-{}", file_name, Uuid::new_v4()));
    let mut payload = serde_json::to_vec_pretty(values).map_err(|error| {
        ServerError::Internal(format!("failed to serialize settings file: {error}"))
    })?;
    payload.push(b'\n');

    let write_result = (|| -> Result<(), ServerError> {
        let mut temp_file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|error| {
                ServerError::Internal(format!(
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
            ServerError::Internal(format!("failed to write settings file: {error}"))
        })?;
        temp_file.sync_all().map_err(|error| {
            ServerError::Internal(format!("failed to flush settings file: {error}"))
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
fn replace_file(from: &Path, to: &Path) -> Result<(), ServerError> {
    fs::rename(from, to).map_err(|error| {
        ServerError::Internal(format!(
            "failed to replace settings file '{}' with '{}': {error}",
            to.display(),
            from.display()
        ))
    })
}

#[cfg(windows)]
fn replace_file(from: &Path, to: &Path) -> Result<(), ServerError> {
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
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
        Err(ServerError::Internal(format!(
            "failed to replace settings file '{}': {}",
            to.display(),
            std::io::Error::last_os_error()
        )))
    } else {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
fn replace_file(from: &Path, to: &Path) -> Result<(), ServerError> {
    fs::rename(from, to).map_err(|error| {
        ServerError::Internal(format!(
            "failed to replace settings file '{}' with '{}': {error}",
            to.display(),
            from.display()
        ))
    })
}

#[cfg(unix)]
fn sync_parent_dir(parent: &Path) -> Result<(), ServerError> {
    let dir = fs::File::open(parent).map_err(|error| {
        ServerError::Internal(format!(
            "failed to open settings directory '{}': {error}",
            parent.display()
        ))
    })?;
    dir.sync_all().map_err(|error| {
        ServerError::Internal(format!(
            "failed to sync settings directory '{}': {error}",
            parent.display()
        ))
    })
}

#[cfg(not(unix))]
fn sync_parent_dir(_parent: &Path) -> Result<(), ServerError> {
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
        let provider = SettingsProvider::load(path.clone())
            .await
            .expect("provider");
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

        let provider = SettingsProvider::load(path.clone())
            .await
            .expect("provider");
        let doc = provider.document().await;
        assert!(!doc.warnings.is_empty());

        let property = provider
            .property(PMID.runtime.llama.num_workers())
            .await
            .expect("property");
        assert_eq!(property.effective_value, serde_json::json!(1));
        assert!(!property.is_overridden);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn unset_removes_override_from_file() {
        let path = temp_settings_path();
        let provider = SettingsProvider::load(path.clone())
            .await
            .expect("provider");

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
        assert!(!file
            .values
            .contains_key(PMID.runtime.llama.context_length().as_str()));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn chat_provider_values_round_trip() {
        let path = temp_settings_path();
        let provider = SettingsProvider::load(path.clone())
            .await
            .expect("provider");

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

        let providers = provider
            .get_chat_providers(PMID.chat.providers())
            .await
            .expect("providers");
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

        let provider = SettingsProvider::load(path.clone())
            .await
            .expect("provider");
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

        let provider = SettingsProvider::load(path.clone())
            .await
            .expect("provider");
        let document = provider.document().await;
        let file: SettingsValuesFile =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        let backup_exists = fs::read_dir(path.parent().expect("parent"))
            .expect("dir")
            .flatten()
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("settings.json.corrupt-")
            });

        assert!(file.values.is_empty());
        assert!(!document.warnings.is_empty());
        assert!(backup_exists);

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }
}
