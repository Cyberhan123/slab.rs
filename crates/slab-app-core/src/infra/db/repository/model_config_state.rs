use super::AnyStore;
use crate::infra::db::entities::ModelConfigStateRecord;
use chrono::{DateTime, Utc};
use std::future::Future;

type ModelConfigStateRow = (String, Option<String>, Option<String>, Option<String>, DateTime<Utc>);

pub trait ModelConfigStateStore: Send + Sync + 'static {
    fn upsert_model_config_state(
        &self,
        record: ModelConfigStateRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn get_model_config_state(
        &self,
        model_id: &str,
    ) -> impl Future<Output = Result<Option<ModelConfigStateRecord>, sqlx::Error>> + Send;
    fn delete_model_config_state(
        &self,
        model_id: &str,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
}

fn row_to_record(
    (
        model_id,
        selected_preset_id,
        selected_variant_id,
        selected_engine_id,
        updated_at,
    ): ModelConfigStateRow,
) -> ModelConfigStateRecord {
    ModelConfigStateRecord {
        model_id,
        selected_preset_id,
        selected_variant_id,
        selected_engine_id,
        updated_at,
    }
}

impl ModelConfigStateStore for AnyStore {
    async fn upsert_model_config_state(
        &self,
        record: ModelConfigStateRecord,
    ) -> Result<(), sqlx::Error> {
        let updated_at = record.updated_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO model_config_state (model_id, selected_preset_id, selected_variant_id, selected_engine_id, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(model_id) DO UPDATE SET \
                selected_preset_id = excluded.selected_preset_id, \
                selected_variant_id = excluded.selected_variant_id, \
                selected_engine_id = excluded.selected_engine_id, \
                updated_at = excluded.updated_at",
        )
        .bind(&record.model_id)
        .bind(&record.selected_preset_id)
        .bind(&record.selected_variant_id)
        .bind(&record.selected_engine_id)
        .bind(&updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_model_config_state(
        &self,
        model_id: &str,
    ) -> Result<Option<ModelConfigStateRecord>, sqlx::Error> {
        let row: Option<ModelConfigStateRow> = sqlx::query_as(
            "SELECT model_id, selected_preset_id, selected_variant_id, selected_engine_id, updated_at \
             FROM model_config_state WHERE model_id = ?1",
        )
        .bind(model_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(row_to_record))
    }

    async fn delete_model_config_state(&self, model_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM model_config_state WHERE model_id = ?1")
            .bind(model_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ModelConfigStateStore;
    use crate::domain::models::{
        CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
        ModelSpec, UnifiedModelKind, UnifiedModelStatus,
    };
    use crate::infra::db::{AnyStore, ModelStore, UnifiedModelRecord};
    use crate::test_support::migrated_test_store;
    use chrono::Utc;

    #[tokio::test]
    async fn state_store_round_trips_selection_after_migration() {
        let store = new_store().await;
        let now = Utc::now();
        let spec = serde_json::to_string(&ModelSpec {
            repo_id: Some("bartowski/Qwen2.5-0.5B-Instruct-GGUF".to_owned()),
            filename: Some("Qwen2.5-0.5B-Instruct-Q8_0.gguf".to_owned()),
            ..ModelSpec::default()
        })
        .expect("serialize spec");

        store
            .upsert_model(UnifiedModelRecord {
                id: "local-qwen".to_owned(),
                display_name: "Local Qwen".to_owned(),
                kind: UnifiedModelKind::Local.as_str().to_owned(),
                backend_id: Some("ggml.llama".to_owned()),
                capabilities: serde_json::to_string(&vec![
                    slab_types::Capability::TextGeneration,
                    slab_types::Capability::ChatGeneration,
                ])
                .expect("serialize capabilities"),
                status: UnifiedModelStatus::NotDownloaded.as_str().to_owned(),
                spec,
                runtime_presets: None,
                materialized_artifacts: "{}".to_owned(),
                selected_download_source: None,
                config_schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION as i64,
                config_policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION as i64,
                created_at: now,
                updated_at: now,
            })
            .await
            .expect("insert model row");

        store
            .upsert_model_config_state(crate::infra::db::entities::ModelConfigStateRecord {
                model_id: "local-qwen".to_owned(),
                selected_preset_id: Some("default".to_owned()),
                selected_variant_id: Some("Q8_0".to_owned()),
                selected_engine_id: Some("ggml.llama".to_owned()),
                updated_at: now,
            })
            .await
            .expect("upsert state row");

        let record = store
            .get_model_config_state("local-qwen")
            .await
            .expect("load state row")
            .expect("state row exists");

        assert_eq!(record.selected_preset_id.as_deref(), Some("default"));
        assert_eq!(record.selected_variant_id.as_deref(), Some("Q8_0"));
        assert_eq!(record.selected_engine_id.as_deref(), Some("ggml.llama"));
    }

    async fn new_store() -> AnyStore {
        migrated_test_store().await
    }
}
