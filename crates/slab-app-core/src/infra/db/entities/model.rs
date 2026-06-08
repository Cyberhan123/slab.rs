use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;

use std::collections::BTreeMap;

use crate::domain::models::{
    ManagedModelBackendId, ModelSpec, RuntimePresets, SelectedModelDownloadSource,
    StoredModelConfig, UnifiedModel, UnifiedModelKind, UnifiedModelStatus,
    validate_stored_model_config,
};
use slab_types::Capability;

/// A model entry in the unified `models` table.
/// Both local and cloud models share this structure.
/// `spec` and `runtime_presets` are stored as JSON strings.
#[derive(Debug, Clone)]
pub struct UnifiedModelRecord {
    pub id: String,
    pub display_name: String,
    /// Canonical model kind (`"local"` or `"cloud"`).
    pub kind: String,
    /// Optional runtime backend identifier for local models.
    pub backend_id: Option<String>,
    /// JSON-serialized `Capability[]`.
    pub capabilities: String,
    /// Status string: `"ready"`, `"not_downloaded"`, `"downloading"`, `"error"`.
    pub status: String,
    /// JSON-serialized `ModelSpec`.
    pub spec: String,
    /// JSON-serialized `RuntimePresets`, if any.
    pub runtime_presets: Option<String>,
    /// JSON-serialized materialized artifact id to local path map.
    pub materialized_artifacts: String,
    /// JSON-serialized selected download source metadata, if any.
    pub selected_download_source: Option<String>,
    pub config_schema_version: i64,
    pub config_policy_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
    })
}

fn parse_backend_id(
    raw_backend_id: Option<String>,
    id: &str,
) -> Result<Option<ManagedModelBackendId>, String> {
    match normalize_optional_text(raw_backend_id) {
        Some(value) => value.parse::<ManagedModelBackendId>().map(Some).map_err(|error| {
            format!("invalid backend_id '{}' for model '{}': {}", value, id, error)
        }),
        None => Ok(None),
    }
}

fn parse_config_version(raw: i64, field: &str) -> Result<u32, String> {
    u32::try_from(raw).map_err(|_| format!("invalid model {field}: {raw}"))
}

fn parse_json_field<T>(raw: &str, id: &str, field: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    serde_json::from_str(raw)
        .map_err(|error| format!("invalid {field} JSON for model '{id}': {error}"))
}

fn parse_optional_json_field<T>(
    raw: Option<String>,
    id: &str,
    field: &str,
) -> Result<Option<T>, String>
where
    T: DeserializeOwned,
{
    match normalize_optional_text(raw) {
        Some(value) => parse_json_field(&value, id, field).map(Some),
        None => Ok(None),
    }
}

impl TryFrom<UnifiedModelRecord> for UnifiedModel {
    type Error = String;

    fn try_from(record: UnifiedModelRecord) -> Result<Self, Self::Error> {
        let UnifiedModelRecord {
            id,
            display_name,
            kind: raw_kind,
            backend_id: raw_backend_id,
            capabilities: raw_capabilities,
            status: raw_status,
            spec: raw_spec,
            runtime_presets: raw_runtime_presets,
            materialized_artifacts: raw_materialized_artifacts,
            selected_download_source: raw_selected_download_source,
            config_schema_version,
            config_policy_version,
            created_at,
            updated_at,
        } = record;

        let kind = raw_kind.parse::<UnifiedModelKind>().map_err(|error| {
            format!("invalid kind '{}' for model '{}': {}", raw_kind, id, error)
        })?;
        let status = raw_status.parse::<UnifiedModelStatus>().map_err(|error| {
            format!("invalid status '{}' for model '{}': {}", raw_status, id, error)
        })?;

        let spec: ModelSpec = parse_json_field(&raw_spec, &id, "spec")?;
        let runtime_presets: Option<RuntimePresets> =
            parse_optional_json_field(raw_runtime_presets, &id, "runtime_presets")?;
        let materialized_artifacts: BTreeMap<String, String> =
            parse_json_field(&raw_materialized_artifacts, &id, "materialized_artifacts")?;
        let selected_download_source: Option<SelectedModelDownloadSource> =
            parse_optional_json_field(
                raw_selected_download_source,
                &id,
                "selected_download_source",
            )?;
        let capabilities: Vec<Capability> =
            parse_json_field(&raw_capabilities, &id, "capabilities")?;
        let backend_id = if kind == UnifiedModelKind::Local {
            parse_backend_id(raw_backend_id, &id)?
        } else {
            None
        };
        let default_status = status.clone();
        let config = validate_stored_model_config(StoredModelConfig {
            schema_version: parse_config_version(config_schema_version, "config_schema_version")?,
            policy_version: parse_config_version(config_policy_version, "config_policy_version")?,
            id,
            display_name,
            kind,
            backend_id,
            capabilities,
            status: Some(status),
            spec,
            runtime_presets,
            materialized_artifacts,
            selected_download_source,
            pack_selection: None,
        })?;

        Ok(UnifiedModel {
            id: config.id,
            display_name: config.display_name,
            kind: config.kind,
            backend_id: config.backend_id,
            capabilities: config.capabilities,
            status: config.status.unwrap_or(default_status),
            spec: config.spec,
            runtime_presets: config.runtime_presets,
            materialized_artifacts: config.materialized_artifacts,
            selected_download_source: config.selected_download_source,
            created_at,
            updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::UnifiedModelRecord;
    use crate::domain::models::{
        CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
        UnifiedModel, UnifiedModelKind,
    };
    use chrono::Utc;
    use serde_json::json;
    use slab_types::Capability;

    fn current_cloud_record() -> UnifiedModelRecord {
        let now = Utc::now();
        UnifiedModelRecord {
            id: "cloud-model".to_owned(),
            display_name: "Cloud Model".to_owned(),
            kind: "cloud".to_owned(),
            backend_id: None,
            capabilities: serde_json::to_string(&vec![
                Capability::TextGeneration,
                Capability::ChatGeneration,
            ])
            .expect("serialize capabilities"),
            status: "ready".to_owned(),
            spec: json!({
                "provider_id": "openai-main",
                "remote_model_id": "gpt-4.1-mini",
                "context_window": 128000
            })
            .to_string(),
            runtime_presets: None,
            materialized_artifacts: "{}".to_owned(),
            selected_download_source: None,
            config_schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION as i64,
            config_policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION as i64,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn converts_records_with_current_config_versions() {
        let model =
            UnifiedModel::try_from(current_cloud_record()).expect("record should deserialize");

        assert_eq!(model.kind, UnifiedModelKind::Cloud);
        assert_eq!(model.spec.provider_id.as_deref(), Some("openai-main"));
        assert_eq!(model.spec.remote_model_id.as_deref(), Some("gpt-4.1-mini"));
        assert_eq!(model.spec.context_window, Some(128000));
    }

    #[test]
    fn rejects_records_with_invalid_required_json() {
        let mut record = current_cloud_record();
        record.spec = "{".to_owned();
        let error = UnifiedModel::try_from(record).expect_err("invalid spec should fail");
        assert!(error.contains("invalid spec JSON"));

        let mut record = current_cloud_record();
        record.capabilities = "{".to_owned();
        let error = UnifiedModel::try_from(record).expect_err("invalid capabilities should fail");
        assert!(error.contains("invalid capabilities JSON"));

        let mut record = current_cloud_record();
        record.materialized_artifacts = "{".to_owned();
        let error =
            UnifiedModel::try_from(record).expect_err("invalid materialized artifacts should fail");
        assert!(error.contains("invalid materialized_artifacts JSON"));
    }

    #[test]
    fn rejects_records_with_invalid_optional_json_when_present() {
        let mut record = current_cloud_record();
        record.runtime_presets = Some("{".to_owned());
        let error =
            UnifiedModel::try_from(record).expect_err("invalid runtime presets should fail");
        assert!(error.contains("invalid runtime_presets JSON"));

        let mut record = current_cloud_record();
        record.selected_download_source = Some("{".to_owned());
        let error = UnifiedModel::try_from(record)
            .expect_err("invalid selected download source should fail");
        assert!(error.contains("invalid selected_download_source JSON"));
    }

    #[test]
    fn rejects_records_with_invalid_status() {
        let mut record = current_cloud_record();
        record.status = "unknown".to_owned();
        let error = UnifiedModel::try_from(record).expect_err("invalid status should fail");
        assert!(error.contains("invalid status"));
    }

    #[test]
    fn rejects_records_with_schema_version_one() {
        let mut record = current_cloud_record();
        record.config_schema_version = 1;
        let error =
            UnifiedModel::try_from(record).expect_err("schema version one record should fail");

        assert!(error.contains("unsupported stored model config schema_version"));
    }

    #[test]
    fn rejects_future_config_schema_versions() {
        let mut record = current_cloud_record();
        record.config_schema_version = (CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION + 1) as i64;
        let error = UnifiedModel::try_from(record).expect_err("future schema version should fail");

        assert!(error.contains("unsupported stored model config schema_version"));
    }
}
