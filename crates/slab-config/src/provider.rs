use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::SystemTime;

use chrono::Utc;
use serde_json::{Map, Value, json};
use slab_utils::fs::{AtomicWriteOptions, atomic_write_bytes_with_options};
use tokio::sync::RwLock;
use tracing::warn;

use crate::SettingsDocument;

use crate::app_config::default_plugin_install_dir_for_settings_path;
use crate::descriptor::{set_document_value, setting_value};
use crate::{ConfigError, SettingValue, UpdateSettingCommand, UpdateSettingOperation};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Debug, Clone)]
pub struct SettingsDocumentProvider {
    path: PathBuf,
    base_path: PathBuf,
    default_document: Arc<StdRwLock<SettingsDocument>>,
    overlay_path: Option<PathBuf>,
    state: Arc<RwLock<SettingsRuntimeState>>,
}

#[derive(Debug, Clone, Default)]
struct SettingsRuntimeState {
    document: SettingsDocument,
    overlay: Option<Value>,
    warnings: Vec<String>,
    fingerprints: SettingsFileFingerprints,
}

#[derive(Debug)]
struct LoadedSettingsRuntimeState {
    state: SettingsRuntimeState,
    default_document: SettingsDocument,
}

trait EnvSeedSource {
    fn var(&self, key: &str) -> Option<String>;
}

struct ProcessEnvSeedSource;

impl EnvSeedSource for ProcessEnvSeedSource {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SettingsFileFingerprints {
    base: Option<SettingsFileFingerprint>,
    overlay: Option<SettingsFileFingerprint>,
}

pub fn seed_settings_document_from_env_if_missing(path: &Path) -> Result<bool, ConfigError> {
    seed_settings_document_from_env_source_if_missing(path, &ProcessEnvSeedSource)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SettingsFileFingerprint {
    len: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy)]
enum CorruptSettingsFilePolicy {
    Recover,
    PreserveLastValid,
}

fn settings_file_fingerprints(
    base_path: &Path,
    overlay_path: Option<&Path>,
) -> Result<SettingsFileFingerprints, ConfigError> {
    Ok(SettingsFileFingerprints {
        base: settings_file_fingerprint(base_path)?,
        overlay: overlay_path.map(settings_file_fingerprint).transpose()?.flatten(),
    })
}

fn settings_file_fingerprint(path: &Path) -> Result<Option<SettingsFileFingerprint>, ConfigError> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(Some(SettingsFileFingerprint {
            len: metadata.len(),
            modified: metadata.modified().ok(),
        })),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ConfigError::Internal(format!(
            "failed to inspect settings file '{}': {error}",
            path.display()
        ))),
    }
}

impl SettingsDocumentProvider {
    pub async fn load(path: PathBuf) -> Result<Self, ConfigError> {
        Self::load_with_overlay(path, None).await
    }

    pub async fn load_with_overlay(
        path: PathBuf,
        overlay_path: Option<PathBuf>,
    ) -> Result<Self, ConfigError> {
        let base_path_for_load = path.clone();
        let overlay_path_for_load = overlay_path.clone();
        let loaded = tokio::task::spawn_blocking(move || {
            load_runtime_state(&base_path_for_load, overlay_path_for_load.as_deref())
        })
        .await
        .map_err(|error| {
            ConfigError::Internal(format!("settings loader task failed: {error}"))
        })??;
        let write_path = overlay_path.clone().unwrap_or_else(|| path.clone());

        Ok(Self {
            path: write_path,
            base_path: path,
            default_document: Arc::new(StdRwLock::new(loaded.default_document)),
            overlay_path,
            state: Arc::new(RwLock::new(loaded.state)),
        })
    }

    pub async fn document(&self) -> SettingsDocument {
        self.refresh_if_changed().await;
        self.state.read().await.document.clone()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn warnings(&self) -> Vec<String> {
        self.refresh_if_changed().await;
        self.state.read().await.warnings.clone()
    }

    pub fn default_document(&self) -> SettingsDocument {
        self.default_document.read().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }

    pub async fn value(&self, path: &str) -> Result<SettingValue, ConfigError> {
        self.refresh_if_changed().await;
        let state = self.state.read().await;
        setting_value(&state.document, path)
    }

    pub async fn update(
        &self,
        path: &str,
        command: UpdateSettingCommand,
    ) -> Result<SettingValue, ConfigError> {
        self.refresh_if_changed().await;
        let mut state = self.state.write().await;
        if self.overlay_path.is_some() {
            return self.update_overlay(&mut state, path, command);
        }

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

        apply_dynamic_defaults(&self.base_path, &mut next_settings);
        let next_value = setting_value(&next_settings, path)?;

        write_settings_document_file(&self.path, &next_settings)?;
        state.document = next_settings;
        state.fingerprints =
            settings_file_fingerprints(&self.base_path, self.overlay_path.as_deref())?;

        Ok(next_value)
    }

    async fn refresh_if_changed(&self) {
        let next_fingerprints =
            match settings_file_fingerprints(&self.base_path, self.overlay_path.as_deref()) {
                Ok(fingerprints) => fingerprints,
                Err(error) => {
                    let mut state = self.state.write().await;
                    state.warnings =
                        vec![format!("Failed to inspect settings files for reload: {error}")];
                    return;
                }
            };

        if self.state.read().await.fingerprints == next_fingerprints {
            return;
        }

        let base_path = self.base_path.clone();
        let overlay_path = self.overlay_path.clone();
        let loaded = tokio::task::spawn_blocking(move || {
            load_runtime_state_with_policy(
                &base_path,
                overlay_path.as_deref(),
                CorruptSettingsFilePolicy::PreserveLastValid,
            )
        })
        .await;

        match loaded {
            Ok(Ok(loaded)) => {
                *self.default_document.write().unwrap_or_else(|poisoned| poisoned.into_inner()) =
                    loaded.default_document;
                let mut state = self.state.write().await;
                state.document = loaded.state.document;
                state.overlay = loaded.state.overlay;
                state.warnings = loaded.state.warnings;
                state.fingerprints = loaded.state.fingerprints;
            }
            Ok(Err(error)) => {
                let mut state = self.state.write().await;
                state.fingerprints = next_fingerprints;
                state.warnings = vec![format!(
                    "Failed to reload settings after external file change; using last valid settings: {error}"
                )];
            }
            Err(error) => {
                let mut state = self.state.write().await;
                state.fingerprints = next_fingerprints;
                state.warnings = vec![format!(
                    "Failed to reload settings after external file change; using last valid settings: {error}"
                )];
            }
        }
    }

    fn update_overlay(
        &self,
        state: &mut SettingsRuntimeState,
        path: &str,
        command: UpdateSettingCommand,
    ) -> Result<SettingValue, ConfigError> {
        let mut overlay = state.overlay.clone().unwrap_or_else(empty_json_object);

        match command.op {
            UpdateSettingOperation::Set => {
                let requested_value = command.value.ok_or_else(|| {
                    ConfigError::BadRequest(format!(
                        "setting '{}' update requires a value when op=set",
                        path
                    ))
                })?;
                let mut next_settings = state.document.clone();
                set_document_value(&mut next_settings, path, requested_value.clone())?;
                apply_dynamic_defaults(&self.base_path, &mut next_settings);
                let next_value = setting_value(&next_settings, path)?;

                set_json_path(&mut overlay, path, requested_value.try_into_json_value()?)?;
                prune_empty_objects(&mut overlay);
                write_json_value_file(&self.path, &overlay)?;
                state.document = next_settings;
                state.overlay = Some(overlay);
                state.fingerprints =
                    settings_file_fingerprints(&self.base_path, self.overlay_path.as_deref())?;

                Ok(next_value)
            }
            UpdateSettingOperation::Unset => {
                let default_document = self.default_document();
                setting_value(&default_document, path)?;
                remove_json_path(&mut overlay, path)?;
                prune_empty_objects(&mut overlay);
                let next_settings =
                    merged_settings_document(&default_document, &overlay, &self.base_path)?;
                let next_value = setting_value(&next_settings, path)?;

                write_json_value_file(&self.path, &overlay)?;
                state.document = next_settings;
                state.overlay = Some(overlay);
                state.fingerprints =
                    settings_file_fingerprints(&self.base_path, self.overlay_path.as_deref())?;

                Ok(next_value)
            }
        }
    }
}

fn load_runtime_state(
    path: &Path,
    overlay_path: Option<&Path>,
) -> Result<LoadedSettingsRuntimeState, ConfigError> {
    load_runtime_state_with_policy(path, overlay_path, CorruptSettingsFilePolicy::Recover)
}

fn load_runtime_state_with_policy(
    path: &Path,
    overlay_path: Option<&Path>,
    corrupt_policy: CorruptSettingsFilePolicy,
) -> Result<LoadedSettingsRuntimeState, ConfigError> {
    let base_state = load_base_runtime_state(path, corrupt_policy)?;
    let Some(overlay_path) = overlay_path else {
        let mut loaded = LoadedSettingsRuntimeState {
            state: base_state,
            default_document: default_settings_document_for_path(path),
        };
        loaded.state.fingerprints = settings_file_fingerprints(path, None)?;
        return Ok(loaded);
    };

    let overlay = load_overlay_json(overlay_path)?;
    let document = merged_settings_document(&base_state.document, &overlay, path)?;

    let mut loaded = LoadedSettingsRuntimeState {
        default_document: base_state.document.clone(),
        state: SettingsRuntimeState {
            document,
            overlay: Some(overlay),
            warnings: base_state.warnings,
            fingerprints: SettingsFileFingerprints::default(),
        },
    };
    loaded.state.fingerprints = settings_file_fingerprints(path, Some(overlay_path))?;
    Ok(loaded)
}

fn load_base_runtime_state(
    path: &Path,
    corrupt_policy: CorruptSettingsFilePolicy,
) -> Result<SettingsRuntimeState, ConfigError> {
    ensure_settings_parent_dir(path)?;

    if !path.exists() {
        let document = default_settings_document_for_path(path);
        write_settings_document_file(path, &document)?;
        return Ok(SettingsRuntimeState {
            document,
            overlay: None,
            warnings: Vec::new(),
            fingerprints: SettingsFileFingerprints::default(),
        });
    }

    let raw = fs::read_to_string(path).map_err(|error| {
        ConfigError::Internal(format!("failed to read settings file '{}': {error}", path.display()))
    })?;

    let raw_json: Value = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            return match corrupt_policy {
                CorruptSettingsFilePolicy::Recover => {
                    let warning = recover_corrupt_settings_file(path, &error.to_string())?;
                    Ok(SettingsRuntimeState {
                        document: default_settings_document_for_path(path),
                        overlay: None,
                        warnings: vec![warning],
                        fingerprints: SettingsFileFingerprints::default(),
                    })
                }
                CorruptSettingsFilePolicy::PreserveLastValid => Err(ConfigError::BadRequest(
                    format!("settings file '{}' is not valid JSON: {error}", path.display()),
                )),
            };
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
        return Ok(SettingsRuntimeState {
            document,
            overlay: None,
            warnings: Vec::new(),
            fingerprints: SettingsFileFingerprints::default(),
        });
    }

    Err(invalid_document_error())
}

fn seed_settings_document_from_env_source_if_missing(
    path: &Path,
    source: &impl EnvSeedSource,
) -> Result<bool, ConfigError> {
    ensure_settings_parent_dir(path)?;
    if path.exists() {
        return Ok(false);
    }

    let mut document = default_settings_document_for_path(path);
    seed_env_string(source, &mut document, "SLAB_BIND", "server.address");
    seed_env_string(source, &mut document, "SLAB_ADMIN_TOKEN", "server.admin.token");
    seed_env_string(source, &mut document, "SLAB_LOG", "logging.level");
    seed_env_bool(source, &mut document, "SLAB_LOG_JSON", "logging.json");
    seed_env_u32(source, &mut document, "SLAB_QUEUE_CAPACITY", "runtime.capacity.queue");
    seed_env_u32(
        source,
        &mut document,
        "SLAB_BACKEND_CAPACITY",
        "runtime.capacity.concurrent_requests",
    );
    seed_env_bool(source, &mut document, "SLAB_ENABLE_SWAGGER", "server.swagger.enabled");
    seed_env_bool(source, &mut document, "SLAB_CLOUD_HTTP_TRACE", "server.cloud_http_trace");
    seed_env_string(source, &mut document, "SLAB_TRANSPORT", "runtime.transport");
    seed_env_string_list(source, &mut document, "SLAB_CORS_ORIGINS", "server.cors.allowed_origins");

    write_settings_document_file(path, &document)?;
    Ok(true)
}

fn seed_env_string(
    source: &impl EnvSeedSource,
    document: &mut SettingsDocument,
    env_var: &'static str,
    pmid: &'static str,
) {
    let Some(value) = source.var(env_var) else {
        return;
    };
    set_seeded_env_value(document, env_var, pmid, SettingValue::String(value));
}

fn seed_env_u32(
    source: &impl EnvSeedSource,
    document: &mut SettingsDocument,
    env_var: &'static str,
    pmid: &'static str,
) {
    let Some(raw) = source.var(env_var) else {
        return;
    };
    let value = match raw.parse::<u32>() {
        Ok(value) => value,
        Err(error) => {
            warn!(env_var, value = %raw, %error, "invalid settings seed value; skipping");
            return;
        }
    };
    set_seeded_env_value(document, env_var, pmid, SettingValue::Unsigned(u64::from(value)));
}

fn seed_env_bool(
    source: &impl EnvSeedSource,
    document: &mut SettingsDocument,
    env_var: &'static str,
    pmid: &'static str,
) {
    let Some(raw) = source.var(env_var) else {
        return;
    };
    let value = match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => {
            warn!(env_var, value = %raw, "invalid boolean settings seed value; skipping");
            return;
        }
    };
    set_seeded_env_value(document, env_var, pmid, SettingValue::Boolean(value));
}

fn seed_env_string_list(
    source: &impl EnvSeedSource,
    document: &mut SettingsDocument,
    env_var: &'static str,
    pmid: &'static str,
) {
    let Some(raw) = source.var(env_var) else {
        return;
    };
    let values = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| SettingValue::String(value.to_owned()))
        .collect::<Vec<_>>();
    set_seeded_env_value(document, env_var, pmid, SettingValue::Array(values));
}

fn set_seeded_env_value(
    document: &mut SettingsDocument,
    env_var: &'static str,
    pmid: &'static str,
    value: SettingValue,
) {
    if let Err(error) = set_document_value(document, pmid, value) {
        warn!(env_var, pmid, %error, "failed to seed settings value from environment");
    }
}

fn load_overlay_json(path: &Path) -> Result<Value, ConfigError> {
    if !path.exists() {
        return Ok(empty_json_object());
    }

    let raw = fs::read_to_string(path).map_err(|error| {
        ConfigError::Internal(format!(
            "failed to read settings overlay file '{}': {error}",
            path.display()
        ))
    })?;
    let value: Value = serde_json::from_str(&raw).map_err(|error| {
        ConfigError::BadRequest(format!(
            "settings overlay file '{}' is not valid JSON: {error}",
            path.display()
        ))
    })?;

    if let Some(schema_version) = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
    {
        ensure_current_schema_version(schema_version)?;
    }

    if !value.is_object() {
        return Err(ConfigError::BadRequest(format!(
            "settings overlay file '{}' must contain a JSON object",
            path.display()
        )));
    }

    Ok(value)
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

    let mut payload = serde_json::to_vec_pretty(document).map_err(|error| {
        ConfigError::Internal(format!("failed to serialize settings file: {error}"))
    })?;
    payload.push(b'\n');

    atomic_write_bytes_with_options(
        path,
        &payload,
        AtomicWriteOptions { unix_mode: Some(0o600), sync_parent_dir: true },
    )
    .map_err(|error| {
        ConfigError::Internal(format!(
            "failed to write settings file '{}': {error}",
            path.display()
        ))
    })
}

fn write_json_value_file(path: &Path, value: &Value) -> Result<(), ConfigError> {
    ensure_settings_parent_dir(path)?;

    let mut payload = serde_json::to_vec_pretty(value).map_err(|error| {
        ConfigError::Internal(format!("failed to serialize settings overlay file: {error}"))
    })?;
    payload.push(b'\n');

    atomic_write_bytes_with_options(
        path,
        &payload,
        AtomicWriteOptions { unix_mode: Some(0o600), sync_parent_dir: true },
    )
    .map_err(|error| {
        ConfigError::Internal(format!(
            "failed to write settings overlay file '{}': {error}",
            path.display()
        ))
    })
}

fn merged_settings_document(
    base_document: &SettingsDocument,
    overlay: &Value,
    base_path: &Path,
) -> Result<SettingsDocument, ConfigError> {
    let mut merged = serde_json::to_value(base_document).map_err(|error| {
        ConfigError::Internal(format!("failed to serialize base settings document: {error}"))
    })?;
    merge_json_values(&mut merged, overlay);
    let mut document: SettingsDocument = serde_json::from_value(merged).map_err(|error| {
        ConfigError::BadRequest(format!("settings overlay has invalid shape: {error}"))
    })?;
    ensure_current_schema_version(document.schema_version)?;
    apply_dynamic_defaults(base_path, &mut document);
    Ok(document)
}

fn merge_json_values(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(key) {
                    Some(existing) => merge_json_values(existing, value),
                    None => {
                        base.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, overlay) => {
            *base = overlay.clone();
        }
    }
}

fn empty_json_object() -> Value {
    Value::Object(Map::new())
}

fn set_json_path(root: &mut Value, path: &str, value: Value) -> Result<(), ConfigError> {
    let segments = setting_path_segments(path)?;
    let mut current = root;

    for segment in &segments[..segments.len() - 1] {
        if !current.is_object() {
            *current = empty_json_object();
        }
        let object = current.as_object_mut().expect("object checked");
        current = object.entry((*segment).to_owned()).or_insert_with(empty_json_object);
    }

    if !current.is_object() {
        *current = empty_json_object();
    }
    current
        .as_object_mut()
        .expect("object checked")
        .insert(segments[segments.len() - 1].to_owned(), value);
    Ok(())
}

fn remove_json_path(root: &mut Value, path: &str) -> Result<(), ConfigError> {
    let segments = setting_path_segments(path)?;
    remove_json_path_segments(root, &segments);
    Ok(())
}

fn setting_path_segments(path: &str) -> Result<Vec<&str>, ConfigError> {
    let segments = path.split('.').filter(|segment| !segment.is_empty()).collect::<Vec<_>>();
    if segments.is_empty() {
        return Err(ConfigError::BadRequest("setting path cannot be empty".to_owned()));
    }
    Ok(segments)
}

fn remove_json_path_segments(value: &mut Value, segments: &[&str]) {
    let Value::Object(object) = value else {
        return;
    };

    if segments.len() == 1 {
        object.remove(segments[0]);
        return;
    }

    if let Some(child) = object.get_mut(segments[0]) {
        remove_json_path_segments(child, &segments[1..]);
        if child.as_object().is_some_and(Map::is_empty) {
            object.remove(segments[0]);
        }
    }
}

fn prune_empty_objects(value: &mut Value) {
    let Value::Object(object) = value else {
        return;
    };

    let keys = object.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        if let Some(child) = object.get_mut(&key) {
            prune_empty_objects(child);
            if child.as_object().is_some_and(Map::is_empty) {
                object.remove(&key);
            }
        }
    }
}

pub fn settings_document_to_json_value(document: &SettingsDocument) -> Value {
    serde_json::to_value(document).unwrap_or_else(|_| json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    impl EnvSeedSource for HashMap<String, String> {
        fn var(&self, key: &str) -> Option<String> {
            self.get(key).cloned()
        }
    }

    fn temp_settings_path() -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("slab-settings-test-{}", uuid::Uuid::new_v4()));
        base.join("settings.json")
    }

    fn temp_overlay_path(settings_path: &Path) -> PathBuf {
        settings_path.parent().expect("parent").join("workspace").join("settings.json")
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

    #[test]
    fn env_seed_creates_missing_document_without_overlaying_existing_settings() {
        let path = temp_settings_path();
        let env = HashMap::from([
            ("SLAB_BIND".to_owned(), "0.0.0.0:17890".to_owned()),
            ("SLAB_ADMIN_TOKEN".to_owned(), "seed-admin-token".to_owned()),
            ("SLAB_LOG".to_owned(), "debug".to_owned()),
            ("SLAB_LOG_JSON".to_owned(), "yes".to_owned()),
            ("SLAB_QUEUE_CAPACITY".to_owned(), "16".to_owned()),
            ("SLAB_BACKEND_CAPACITY".to_owned(), "3".to_owned()),
            ("SLAB_ENABLE_SWAGGER".to_owned(), "no".to_owned()),
            ("SLAB_CLOUD_HTTP_TRACE".to_owned(), "on".to_owned()),
            ("SLAB_TRANSPORT".to_owned(), "http".to_owned()),
            (
                "SLAB_CORS_ORIGINS".to_owned(),
                "https://app.example.com, https://admin.example.com".to_owned(),
            ),
        ]);

        assert!(
            seed_settings_document_from_env_source_if_missing(&path, &env)
                .expect("seed missing settings")
        );

        let document: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        assert_eq!(document.server.address, "0.0.0.0:17890");
        assert_eq!(document.server.admin.token.as_deref(), Some("seed-admin-token"));
        assert_eq!(document.logging.level, "debug");
        assert!(document.logging.json);
        assert_eq!(document.runtime.capacity.queue, 16);
        assert_eq!(document.runtime.capacity.concurrent_requests, 3);
        assert!(!document.server.swagger.enabled);
        assert!(document.server.cloud_http_trace);
        assert_eq!(document.runtime.transport, crate::RuntimeTransportMode::Http);
        assert_eq!(
            document.server.cors.allowed_origins,
            vec!["https://app.example.com", "https://admin.example.com"]
        );

        let overwrite_env =
            HashMap::from([("SLAB_ADMIN_TOKEN".to_owned(), "overwritten-admin-token".to_owned())]);
        assert!(
            !seed_settings_document_from_env_source_if_missing(&path, &overwrite_env)
                .expect("existing settings skipped")
        );
        let persisted: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        assert_eq!(persisted.server.admin.token.as_deref(), Some("seed-admin-token"));

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
    async fn provider_refreshes_after_external_base_file_change() {
        let path = temp_settings_path();
        let provider = SettingsDocumentProvider::load(path.clone()).await.expect("provider");
        let mut document: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("file")).expect("json");
        document.logging.level = "debug".to_owned();
        fs::write(&path, serde_json::to_string_pretty(&document).expect("serialize"))
            .expect("external write");

        assert_eq!(provider.value("logging.level").await.expect("value"), json!("debug").into());

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn provider_preserves_last_valid_document_after_external_invalid_json() {
        let path = temp_settings_path();
        let provider = SettingsDocumentProvider::load(path.clone()).await.expect("provider");
        let original = provider.document().await;
        fs::write(&path, "{ not valid json").expect("external corrupt write");

        assert_eq!(provider.document().await, original);
        assert!(
            provider
                .warnings()
                .await
                .iter()
                .any(|warning| warning.contains("using last valid settings"))
        );
        assert_eq!(fs::read_to_string(&path).expect("file"), "{ not valid json");

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
    async fn provider_loads_workspace_overlay_on_top_of_base_document() {
        let path = temp_settings_path();
        let overlay_path = temp_overlay_path(&path);
        ensure_settings_parent_dir(&path).expect("base dir");
        ensure_settings_parent_dir(&overlay_path).expect("overlay dir");
        let mut base_document = SettingsDocument::default();
        base_document.logging.level = "warn".to_owned();
        fs::write(&path, serde_json::to_string_pretty(&base_document).expect("serialize"))
            .expect("write base");
        fs::write(
            &overlay_path,
            serde_json::to_string_pretty(&json!({
                "logging": {
                    "level": "debug"
                }
            }))
            .expect("serialize"),
        )
        .expect("write overlay");

        let provider =
            SettingsDocumentProvider::load_with_overlay(path.clone(), Some(overlay_path.clone()))
                .await
                .expect("provider");
        let document = provider.document().await;

        assert_eq!(provider.path(), overlay_path.as_path());
        assert_eq!(document.logging.level, "debug");
        assert_eq!(provider.default_document().logging.level, "warn");

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn provider_overlay_update_writes_only_overlay_file() {
        let path = temp_settings_path();
        let overlay_path = temp_overlay_path(&path);
        let provider =
            SettingsDocumentProvider::load_with_overlay(path.clone(), Some(overlay_path.clone()))
                .await
                .expect("provider");

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

        let base: SettingsDocument =
            serde_json::from_str(&fs::read_to_string(&path).expect("base")).expect("base json");
        let overlay: Value =
            serde_json::from_str(&fs::read_to_string(&overlay_path).expect("overlay"))
                .expect("overlay json");

        assert_eq!(base.logging.level, "info");
        assert_eq!(overlay, json!({ "logging": { "level": "debug" } }));

        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[tokio::test]
    async fn provider_overlay_unset_removes_override_and_restores_base() {
        let path = temp_settings_path();
        let overlay_path = temp_overlay_path(&path);
        ensure_settings_parent_dir(&path).expect("base dir");
        ensure_settings_parent_dir(&overlay_path).expect("overlay dir");
        let mut base_document = SettingsDocument::default();
        base_document.logging.level = "warn".to_owned();
        fs::write(&path, serde_json::to_string_pretty(&base_document).expect("serialize"))
            .expect("write base");
        fs::write(
            &overlay_path,
            serde_json::to_string_pretty(&json!({
                "logging": {
                    "level": "debug"
                }
            }))
            .expect("serialize"),
        )
        .expect("write overlay");
        let provider =
            SettingsDocumentProvider::load_with_overlay(path.clone(), Some(overlay_path.clone()))
                .await
                .expect("provider");

        provider
            .update(
                "logging.level",
                UpdateSettingCommand { op: UpdateSettingOperation::Unset, value: None },
            )
            .await
            .expect("unset");

        let overlay: Value =
            serde_json::from_str(&fs::read_to_string(&overlay_path).expect("overlay"))
                .expect("overlay json");

        assert_eq!(provider.value("logging.level").await.expect("value"), json!("warn").into());
        assert_eq!(overlay, json!({}));

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
