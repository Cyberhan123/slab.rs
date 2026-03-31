use super::AnyStore;
use crate::infra::db::entities::UnifiedModelRecord;
use chrono::Utc;
use std::future::Future;

pub trait ModelStore: Send + Sync + 'static {
    fn upsert_model(
        &self,
        record: UnifiedModelRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn get_model(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<UnifiedModelRecord>, sqlx::Error>> + Send;
    fn list_models(
        &self,
    ) -> impl Future<Output = Result<Vec<UnifiedModelRecord>, sqlx::Error>> + Send;
    fn delete_model(&self, id: &str) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    /// Update a local model's `spec.local_path` and set its status after a successful download.
    fn update_model_local_path(
        &self,
        id: &str,
        local_path: &str,
        status: &str,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
}

fn parse_rfc3339_or_now(raw: String, field: &'static str) -> chrono::DateTime<Utc> {
    raw.parse().unwrap_or_else(|e: chrono::ParseError| {
        tracing::warn!(raw = %raw, error = %e, field, "failed to parse model timestamp; using now");
        Utc::now()
    })
}

type ModelRow = (
    String,         // id
    String,         // display_name
    String,         // provider
    String,         // status
    String,         // spec
    Option<String>, // runtime_presets
    String,         // created_at
    String,         // updated_at
);

fn row_to_record(
    (id, display_name, provider, status, spec, runtime_presets, created_at, updated_at): ModelRow,
) -> UnifiedModelRecord {
    UnifiedModelRecord {
        id,
        display_name,
        provider,
        status,
        spec,
        runtime_presets,
        created_at: parse_rfc3339_or_now(created_at, "created_at"),
        updated_at: parse_rfc3339_or_now(updated_at, "updated_at"),
    }
}

impl ModelStore for AnyStore {
    async fn upsert_model(&self, record: UnifiedModelRecord) -> Result<(), sqlx::Error> {
        let created_at = record.created_at.to_rfc3339();
        let updated_at = record.updated_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO models \
             (id, display_name, provider, status, spec, runtime_presets, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
             ON CONFLICT(id) DO UPDATE SET \
                 display_name = excluded.display_name, \
                 provider = excluded.provider, \
                 status = excluded.status, \
                 spec = excluded.spec, \
                 runtime_presets = excluded.runtime_presets, \
                 created_at = excluded.created_at, \
                 updated_at = excluded.updated_at",
        )
        .bind(&record.id)
        .bind(&record.display_name)
        .bind(&record.provider)
        .bind(&record.status)
        .bind(&record.spec)
        .bind(&record.runtime_presets)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_model(&self, id: &str) -> Result<Option<UnifiedModelRecord>, sqlx::Error> {
        let row: Option<ModelRow> = sqlx::query_as(
            "SELECT id, display_name, provider, status, spec, runtime_presets, created_at, updated_at \
             FROM models WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(row_to_record))
    }

    async fn list_models(&self) -> Result<Vec<UnifiedModelRecord>, sqlx::Error> {
        let rows: Vec<ModelRow> = sqlx::query_as(
            "SELECT id, display_name, provider, status, spec, runtime_presets, created_at, updated_at \
             FROM models ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(row_to_record).collect())
    }

    async fn delete_model(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM models WHERE id = ?1").bind(id).execute(&self.pool).await?;
        Ok(())
    }

    async fn update_model_local_path(
        &self,
        id: &str,
        local_path: &str,
        status: &str,
    ) -> Result<(), sqlx::Error> {
        let updated_at = Utc::now().to_rfc3339();
        // Use SQLite's json_set to update the local_path field inside the spec JSON column.
        sqlx::query(
            "UPDATE models \
             SET spec = json_set(spec, '$.local_path', ?1), status = ?2, updated_at = ?3 \
             WHERE id = ?4",
        )
        .bind(local_path)
        .bind(status)
        .bind(&updated_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
