use crate::entities::{dao::ModelCatalogRecord, AnyStore};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::future::Future;

pub trait ModelStore: Send + Sync + 'static {
    fn insert_model(
        &self,
        record: ModelCatalogRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn get_model(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<ModelCatalogRecord>, sqlx::Error>> + Send;
    fn list_models(&self) -> impl Future<Output = Result<Vec<ModelCatalogRecord>, sqlx::Error>> + Send;
    fn update_model_metadata(
        &self,
        id: &str,
        display_name: &str,
        repo_id: &str,
        filename: &str,
        backend_ids: &[String],
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn delete_model(&self, id: &str) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn mark_model_downloaded(
        &self,
        id: &str,
        local_path: &str,
        task_id: &str,
        downloaded_at: DateTime<Utc>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
}

fn parse_rfc3339_or_now(raw: String, field: &'static str) -> DateTime<Utc> {
    raw.parse().unwrap_or_else(|e: chrono::ParseError| {
        tracing::warn!(raw = %raw, error = %e, field, "failed to parse model timestamp; using now");
        Utc::now()
    })
}

fn parse_optional_rfc3339(raw: Option<String>, field: &'static str) -> Option<DateTime<Utc>> {
    raw.and_then(|v| {
        v.parse().map_err(|e: chrono::ParseError| {
            tracing::warn!(raw = %v, error = %e, field, "failed to parse optional model timestamp; dropping value");
            e
        }).ok()
    })
}

fn normalize_backend_ids(ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = ids
        .iter()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

impl ModelStore for AnyStore {
    async fn insert_model(&self, record: ModelCatalogRecord) -> Result<(), sqlx::Error> {
        let created_at = record.created_at.to_rfc3339();
        let updated_at = record.updated_at.to_rfc3339();
        let backend_ids = normalize_backend_ids(&record.backend_ids);

        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO model_catalog \
             (id, display_name, repo_id, filename, local_path, last_download_task_id, last_downloaded_at, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(&record.id)
        .bind(&record.display_name)
        .bind(&record.repo_id)
        .bind(&record.filename)
        .bind(&record.local_path)
        .bind(&record.last_download_task_id)
        .bind(record.last_downloaded_at.map(|v| v.to_rfc3339()))
        .bind(&created_at)
        .bind(&updated_at)
        .execute(&mut *tx)
        .await?;

        for backend_id in backend_ids {
            sqlx::query(
                "INSERT INTO model_catalog_backend (model_id, backend_id) VALUES (?1, ?2)",
            )
            .bind(&record.id)
            .bind(backend_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get_model(&self, id: &str) -> Result<Option<ModelCatalogRecord>, sqlx::Error> {
        let row: Option<(
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            String,
        )> = sqlx::query_as(
            "SELECT id, display_name, repo_id, filename, local_path, last_download_task_id, last_downloaded_at, created_at, updated_at \
             FROM model_catalog WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        let Some((
            id,
            display_name,
            repo_id,
            filename,
            local_path,
            last_download_task_id,
            last_downloaded_at,
            created_at,
            updated_at,
        )) = row
        else {
            return Ok(None);
        };

        let backend_rows: Vec<(String,)> = sqlx::query_as(
            "SELECT backend_id FROM model_catalog_backend WHERE model_id = ?1 ORDER BY backend_id ASC",
        )
        .bind(&id)
        .fetch_all(&self.pool)
        .await?;
        let backend_ids = backend_rows.into_iter().map(|(v,)| v).collect();

        Ok(Some(ModelCatalogRecord {
            id,
            display_name,
            repo_id,
            filename,
            backend_ids,
            local_path,
            last_download_task_id,
            last_downloaded_at: parse_optional_rfc3339(last_downloaded_at, "last_downloaded_at"),
            created_at: parse_rfc3339_or_now(created_at, "created_at"),
            updated_at: parse_rfc3339_or_now(updated_at, "updated_at"),
        }))
    }

    async fn list_models(&self) -> Result<Vec<ModelCatalogRecord>, sqlx::Error> {
        let rows: Vec<(
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            String,
        )> = sqlx::query_as(
            "SELECT id, display_name, repo_id, filename, local_path, last_download_task_id, last_downloaded_at, created_at, updated_at \
             FROM model_catalog ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let backend_rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT model_id, backend_id FROM model_catalog_backend ORDER BY backend_id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut backend_map: HashMap<String, Vec<String>> = HashMap::new();
        for (model_id, backend_id) in backend_rows {
            backend_map.entry(model_id).or_default().push(backend_id);
        }

        let models = rows
            .into_iter()
            .map(
                |(
                    id,
                    display_name,
                    repo_id,
                    filename,
                    local_path,
                    last_download_task_id,
                    last_downloaded_at,
                    created_at,
                    updated_at,
                )| ModelCatalogRecord {
                    backend_ids: backend_map.remove(&id).unwrap_or_default(),
                    id,
                    display_name,
                    repo_id,
                    filename,
                    local_path,
                    last_download_task_id,
                    last_downloaded_at: parse_optional_rfc3339(
                        last_downloaded_at,
                        "last_downloaded_at",
                    ),
                    created_at: parse_rfc3339_or_now(created_at, "created_at"),
                    updated_at: parse_rfc3339_or_now(updated_at, "updated_at"),
                },
            )
            .collect();

        Ok(models)
    }

    async fn update_model_metadata(
        &self,
        id: &str,
        display_name: &str,
        repo_id: &str,
        filename: &str,
        backend_ids: &[String],
    ) -> Result<(), sqlx::Error> {
        let updated_at = Utc::now().to_rfc3339();
        let backend_ids = normalize_backend_ids(backend_ids);

        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE model_catalog SET display_name = ?1, repo_id = ?2, filename = ?3, updated_at = ?4 WHERE id = ?5",
        )
        .bind(display_name)
        .bind(repo_id)
        .bind(filename)
        .bind(&updated_at)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM model_catalog_backend WHERE model_id = ?1")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        for backend_id in backend_ids {
            sqlx::query(
                "INSERT INTO model_catalog_backend (model_id, backend_id) VALUES (?1, ?2)",
            )
            .bind(id)
            .bind(backend_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn delete_model(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM model_catalog WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn mark_model_downloaded(
        &self,
        id: &str,
        local_path: &str,
        task_id: &str,
        downloaded_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        let downloaded_at = downloaded_at.to_rfc3339();
        let updated_at = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE model_catalog \
             SET local_path = ?1, last_download_task_id = ?2, last_downloaded_at = ?3, updated_at = ?4 \
             WHERE id = ?5",
        )
        .bind(local_path)
        .bind(task_id)
        .bind(downloaded_at)
        .bind(updated_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
