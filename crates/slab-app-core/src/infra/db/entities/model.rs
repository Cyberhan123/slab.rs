use chrono::{DateTime, Utc};

use std::collections::BTreeMap;

use crate::domain::models::{
    ManagedModelBackendId, ModelSpec, RuntimePresets, StoredModelConfig, UnifiedModel,
    UnifiedModelKind, UnifiedModelStatus, upgrade_stored_model_config,
};
use slab_types::Capability;

/// A model entry in the unified `models` table.
/// Both local and cloud models share this structure.
/// `spec` and `runtime_presets` are stored as JSON strings.
#[derive(Debug, Clone)]
pub struct UnifiedModelRecord {
    pub id: String,
    pub display_name: String,
    /// Legacy provider identifier retained only for storage migration.
    pub provider: String,
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
    pub config_schema_version: i64,
    pub config_policy_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn derive_kind_from_legacy_provider(provider: &str) -> UnifiedModelKind {
    if provider.trim().starts_with("cloud.") || provider.trim() == "cloud" {
        UnifiedModelKind::Cloud
    } else {
        UnifiedModelKind::Local
    }
}

fn derive_backend_id_from_legacy_provider(provider: &str) -> Option<ManagedModelBackendId> {
    provider
        .trim()
        .strip_prefix("local.")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse().ok())
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
    })
}

fn parse_backend_id(
    raw_backend_id: Option<String>,
    fallback_provider: &str,
    id: &str,
) -> Result<Option<ManagedModelBackendId>, String> {
    match normalize_optional_text(raw_backend_id).or_else(|| {
        derive_backend_id_from_legacy_provider(fallback_provider).map(|backend| backend.to_string())
    }) {
        Some(value) => value.parse::<ManagedModelBackendId>().map(Some).map_err(|error| {
            format!("invalid backend_id '{}' for model '{}': {}", value, id, error)
        }),
        None => Ok(None),
    }
}

fn parse_config_version(raw: i64, field: &str) -> Result<u32, String> {
    u32::try_from(raw).map_err(|_| format!("invalid model {field}: {raw}"))
}

impl TryFrom<UnifiedModelRecord> for UnifiedModel {
    type Error = String;

    fn try_from(record: UnifiedModelRecord) -> Result<Self, Self::Error> {
        let UnifiedModelRecord {
            id,
            display_name,
            provider,
            kind: raw_kind,
            backend_id: raw_backend_id,
            capabilities: raw_capabilities,
            status: raw_status,
            spec: raw_spec,
            runtime_presets: raw_runtime_presets,
            config_schema_version,
            config_policy_version,
            created_at,
            updated_at,
        } = record;

        let kind = raw_kind.parse::<UnifiedModelKind>().unwrap_or_else(|error| {
            tracing::warn!(
                id = %id,
                raw_kind = %raw_kind,
                raw_provider = %provider,
                error = %error,
                "failed to parse model kind; deriving from legacy provider"
            );
            derive_kind_from_legacy_provider(&provider)
        });
        let status = raw_status.parse::<UnifiedModelStatus>().unwrap_or_else(|error| {
            tracing::warn!(
                id = %id,
                raw_status = %raw_status,
                error = %error,
                "failed to parse model status; defaulting to Error"
            );
            UnifiedModelStatus::Error
        });

        let spec: ModelSpec = serde_json::from_str(&raw_spec).unwrap_or_else(|error| {
            tracing::warn!(
                id = %id,
                error = %error,
                "failed to deserialize model spec JSON; using empty spec"
            );
            ModelSpec::default()
        });

        let runtime_presets: Option<RuntimePresets> =
            raw_runtime_presets.as_deref().and_then(|value| serde_json::from_str(value).ok());
        let capabilities: Vec<Capability> =
            serde_json::from_str(&raw_capabilities).unwrap_or_else(|error| {
                tracing::warn!(
                    id = %id,
                    error = %error,
                    "failed to deserialize model capabilities JSON; using empty capabilities"
                );
                Vec::new()
            });
        let backend_id = if kind == UnifiedModelKind::Local {
            parse_backend_id(raw_backend_id, &provider, &id)?
        } else {
            None
        };
        let default_status = status.clone();
        let config = upgrade_stored_model_config(StoredModelConfig {
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
            materialized_artifacts: BTreeMap::new(),
            selected_download_source: None,
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

    #[test]
    fn converts_records_with_current_config_versions() {
        let now = Utc::now();
        let model = UnifiedModel::try_from(UnifiedModelRecord {
            id: "cloud-model".to_owned(),
            display_name: "Cloud Model".to_owned(),
            provider: "cloud.openai-main".to_owned(),
            kind: "cloud".to_owned(),
            backend_id: None,
            capabilities: serde_json::to_string(&vec![
                Capability::TextGeneration,
                Capability::ChatGeneration,
            ])
            .unwrap(),
            status: "ready".to_owned(),
            spec: json!({
                "provider_id": "openai-main",
                "remote_model_id": "gpt-4.1-mini",
                "context_window": 128000
            })
            .to_string(),
            runtime_presets: None,
            config_schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION as i64,
            config_policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION as i64,
            created_at: now,
            updated_at: now,
        })
        .expect("record should deserialize");

        assert_eq!(model.kind, UnifiedModelKind::Cloud);
        assert_eq!(model.spec.provider_id.as_deref(), Some("openai-main"));
        assert_eq!(model.spec.remote_model_id.as_deref(), Some("gpt-4.1-mini"));
        assert_eq!(model.spec.context_window, Some(128000));
    }

    #[test]
    fn converts_records_with_schema_version_one() {
        let now = Utc::now();
        let model = UnifiedModel::try_from(UnifiedModelRecord {
            id: "cloud-model".to_owned(),
            display_name: "Cloud Model".to_owned(),
            provider: "cloud.openai-main".to_owned(),
            kind: "cloud".to_owned(),
            backend_id: None,
            capabilities: "[]".to_owned(),
            status: "ready".to_owned(),
            spec: json!({
                "provider_id": "openai-main",
                "remote_model_id": "gpt-4.1-mini"
            })
            .to_string(),
            runtime_presets: None,
            config_schema_version: 1,
            config_policy_version: 1,
            created_at: now,
            updated_at: now,
        })
        .expect("schema version one record should deserialize");

        assert_eq!(model.kind, UnifiedModelKind::Cloud);
        assert_eq!(
            model.capabilities,
            vec![Capability::TextGeneration, Capability::ChatGeneration]
        );
    }

    #[test]
    fn rejects_future_config_schema_versions() {
        let now = Utc::now();
        let error = UnifiedModel::try_from(UnifiedModelRecord {
            id: "cloud-model".to_owned(),
            display_name: "Cloud Model".to_owned(),
            provider: "cloud.openai-main".to_owned(),
            kind: "cloud".to_owned(),
            backend_id: None,
            capabilities: serde_json::to_string(&vec![
                Capability::TextGeneration,
                Capability::ChatGeneration,
            ])
            .unwrap(),
            status: "ready".to_owned(),
            spec: json!({
                "provider_id": "openai-main",
                "remote_model_id": "gpt-4.1-mini"
            })
            .to_string(),
            runtime_presets: None,
            config_schema_version: (CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION + 1) as i64,
            config_policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION as i64,
            created_at: now,
            updated_at: now,
        })
        .expect_err("future schema version should fail");

        assert!(error.contains("unsupported stored model config schema_version"));
    }
}
