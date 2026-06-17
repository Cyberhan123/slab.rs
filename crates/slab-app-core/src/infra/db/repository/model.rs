use super::AnyStore;
use crate::infra::db::entities::UnifiedModelRecord;
use chrono::{DateTime, Utc};
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
    /// Update a local model's downloaded local path and materialized artifact state.
    fn update_model_download_state(
        &self,
        id: &str,
        local_path: &str,
        status: &str,
        materialized_artifacts: &str,
        selected_download_source: Option<&str>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
}

type ModelRow = (
    String,         // id
    String,         // display_name
    String,         // kind
    Option<String>, // backend_id
    String,         // capabilities
    String,         // status
    String,         // spec
    Option<String>, // runtime_presets
    String,         // materialized_artifacts
    Option<String>, // selected_download_source
    i64,            // config_schema_version
    i64,            // config_policy_version
    DateTime<Utc>,  // created_at
    DateTime<Utc>,  // updated_at
);

fn row_to_record(
    (
        id,
        display_name,
        kind,
        backend_id,
        capabilities,
        status,
        spec,
        runtime_presets,
        materialized_artifacts,
        selected_download_source,
        config_schema_version,
        config_policy_version,
        created_at,
        updated_at,
    ): ModelRow,
) -> UnifiedModelRecord {
    UnifiedModelRecord {
        id,
        display_name,
        kind,
        backend_id,
        capabilities,
        status,
        spec,
        runtime_presets,
        materialized_artifacts,
        selected_download_source,
        config_schema_version,
        config_policy_version,
        created_at,
        updated_at,
    }
}

impl ModelStore for AnyStore {
    async fn upsert_model(&self, record: UnifiedModelRecord) -> Result<(), sqlx::Error> {
        let created_at = record.created_at.to_rfc3339();
        let updated_at = record.updated_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO models \
             (id, display_name, kind, backend_id, capabilities, status, spec, runtime_presets, materialized_artifacts, selected_download_source, config_schema_version, config_policy_version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14) \
             ON CONFLICT(id) DO UPDATE SET \
                  display_name = excluded.display_name, \
                  kind = excluded.kind, \
                  backend_id = excluded.backend_id, \
                  capabilities = excluded.capabilities, \
                  status = excluded.status, \
                  spec = excluded.spec, \
                  runtime_presets = excluded.runtime_presets, \
                  materialized_artifacts = excluded.materialized_artifacts, \
                  selected_download_source = excluded.selected_download_source, \
                  config_schema_version = excluded.config_schema_version, \
                  config_policy_version = excluded.config_policy_version, \
                  created_at = excluded.created_at, \
                  updated_at = excluded.updated_at",
        )
        .bind(&record.id)
        .bind(&record.display_name)
        .bind(&record.kind)
        .bind(&record.backend_id)
        .bind(&record.capabilities)
        .bind(&record.status)
        .bind(&record.spec)
        .bind(&record.runtime_presets)
        .bind(&record.materialized_artifacts)
        .bind(&record.selected_download_source)
        .bind(record.config_schema_version)
        .bind(record.config_policy_version)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_model(&self, id: &str) -> Result<Option<UnifiedModelRecord>, sqlx::Error> {
        let row: Option<ModelRow> = sqlx::query_as(
            "SELECT id, display_name, kind, backend_id, capabilities, status, spec, runtime_presets, materialized_artifacts, selected_download_source, config_schema_version, config_policy_version, created_at, updated_at \
             FROM models WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(row_to_record))
    }

    async fn list_models(&self) -> Result<Vec<UnifiedModelRecord>, sqlx::Error> {
        let rows: Vec<ModelRow> = sqlx::query_as(
            "SELECT id, display_name, kind, backend_id, capabilities, status, spec, runtime_presets, materialized_artifacts, selected_download_source, config_schema_version, config_policy_version, created_at, updated_at \
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

    async fn update_model_download_state(
        &self,
        id: &str,
        local_path: &str,
        status: &str,
        materialized_artifacts: &str,
        selected_download_source: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let updated_at = Utc::now().to_rfc3339();
        // Use SQLite's json_set to update the local_path field inside the spec JSON column.
        sqlx::query(
            "UPDATE models \
             SET spec = json_set(spec, '$.local_path', ?1), \
                 status = ?2, \
                 materialized_artifacts = ?3, \
                 selected_download_source = ?4, \
                 updated_at = ?5 \
             WHERE id = ?6",
        )
        .bind(local_path)
        .bind(status)
        .bind(materialized_artifacts)
        .bind(selected_download_source)
        .bind(&updated_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::migrated_test_pool;

    #[tokio::test]
    async fn remove_provider_migration_keeps_canonical_model_columns() {
        let options =
            sqlx::sqlite::SqliteConnectOptions::new().filename(":memory:").create_if_missing(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect in-memory db");

        sqlx::query(
            "CREATE TABLE models (
                id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                provider TEXT NOT NULL,
                kind TEXT NOT NULL,
                backend_id TEXT,
                capabilities TEXT NOT NULL,
                status TEXT NOT NULL,
                spec TEXT NOT NULL,
                runtime_presets TEXT,
                config_schema_version INTEGER NOT NULL,
                config_policy_version INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create models");
        sqlx::query("CREATE INDEX idx_models_provider ON models(provider)")
            .execute(&pool)
            .await
            .expect("create provider index");
        sqlx::query(
            "INSERT INTO models (
                id, display_name, provider, kind, backend_id, capabilities, status, spec,
                runtime_presets, config_schema_version, config_policy_version, created_at,
                updated_at
             ) VALUES (
                'local-qwen', 'Local Qwen', 'local.ggml.llama', 'local', 'ggml.llama',
                '[]', 'ready', '{}', NULL, 2, 3, '2026-05-30T00:00:00Z',
                '2026-05-30T00:00:00Z'
             )",
        )
        .execute(&pool)
        .await
        .expect("insert model");

        for statement in
            include_str!("../../../../migrations/20260530000000_remove_models_provider.sql")
                .split(';')
                .map(str::trim)
                .filter(|statement| !statement.is_empty())
        {
            sqlx::query(statement).execute(&pool).await.expect("run migration statement");
        }

        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('models')")
                .fetch_all(&pool)
                .await
                .expect("read table info");
        let row: (String, Option<String>) =
            sqlx::query_as("SELECT kind, backend_id FROM models WHERE id = 'local-qwen'")
                .fetch_one(&pool)
                .await
                .expect("read migrated row");

        assert!(!columns.iter().any(|column| column == "provider"));
        assert_eq!(row.0, "local");
        assert_eq!(row.1.as_deref(), Some("ggml.llama"));
    }

    #[tokio::test]
    async fn storage_value_checks_reject_invalid_closed_values() {
        let pool = migrated_test_pool().await;

        sqlx::query(
            "INSERT INTO models (
                id, display_name, status, spec, runtime_presets, created_at, updated_at, kind,
                backend_id, config_schema_version, config_policy_version, capabilities,
                materialized_artifacts, selected_download_source
             ) VALUES (
                'model-ok', 'Model OK', 'ready', '{}', NULL, '2026-06-08T00:00:00Z',
                '2026-06-08T00:00:00Z', 'local', 'ggml.llama', 2, 3, '[]', '{}', NULL
             )",
        )
        .execute(&pool)
        .await
        .expect("insert valid model");
        sqlx::query(
            "INSERT INTO tasks (
                id, task_type, status, created_at, updated_at
             ) VALUES (
                'task-ok', 'custom.producer', 'pending', '2026-06-08T00:00:00Z',
                '2026-06-08T00:00:00Z'
             )",
        )
        .execute(&pool)
        .await
        .expect("insert valid task with open task_type");
        sqlx::query(
            "INSERT INTO chat_sessions (id, name, created_at, updated_at)
             VALUES ('session-ok', '', '2026-06-08T00:00:00Z', '2026-06-08T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert valid session");
        sqlx::query(
            "INSERT INTO chat_messages (id, session_id, role, content, created_at)
             VALUES ('message-ok', 'session-ok', 'assistant', '{}', '2026-06-08T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert valid chat message");
        sqlx::query(
            "INSERT INTO model_downloads (
                task_id, model_id, source_key, repo_id, filename, status, created_at, updated_at
             ) VALUES (
                'task-ok', 'model-ok', 'source-ok', 'repo', 'model.gguf', 'running',
                '2026-06-08T00:00:00Z', '2026-06-08T00:00:00Z'
             )",
        )
        .execute(&pool)
        .await
        .expect("insert valid model download");

        assert!(
            sqlx::query(
                "INSERT INTO models (
                    id, display_name, status, spec, runtime_presets, created_at, updated_at, kind,
                    backend_id, config_schema_version, config_policy_version, capabilities,
                    materialized_artifacts, selected_download_source
                 ) VALUES (
                    'model-bad-kind', 'Bad Kind', 'ready', '{}', NULL, '2026-06-08T00:00:00Z',
                    '2026-06-08T00:00:00Z', 'plugin', NULL, 2, 3, '[]', '{}', NULL
                 )",
            )
            .execute(&pool)
            .await
            .is_err()
        );
        assert!(
            sqlx::query(
                "INSERT INTO models (
                    id, display_name, status, spec, runtime_presets, created_at, updated_at, kind,
                    backend_id, config_schema_version, config_policy_version, capabilities,
                    materialized_artifacts, selected_download_source
                 ) VALUES (
                    'model-bad-status', 'Bad Status', 'archived', '{}', NULL,
                    '2026-06-08T00:00:00Z', '2026-06-08T00:00:00Z', 'cloud', NULL, 2, 3, '[]',
                    '{}', NULL
                 )",
            )
            .execute(&pool)
            .await
            .is_err()
        );
        assert!(
            sqlx::query(
                "INSERT INTO tasks (id, task_type, status, created_at, updated_at)
                 VALUES ('task-bad-status', 'custom.producer', 'paused', '2026-06-08T00:00:00Z',
                         '2026-06-08T00:00:00Z')",
            )
            .execute(&pool)
            .await
            .is_err()
        );
        assert!(
            sqlx::query(
                "INSERT INTO chat_messages (id, session_id, role, content, created_at)
                 VALUES ('message-bad-role', 'session-ok', 'moderator', '{}',
                         '2026-06-08T00:00:00Z')",
            )
            .execute(&pool)
            .await
            .is_err()
        );
        assert!(
            sqlx::query(
                "INSERT INTO tasks (id, task_type, status, created_at, updated_at)
                 VALUES ('task-download-bad-status', 'custom.producer', 'pending',
                         '2026-06-08T00:00:00Z', '2026-06-08T00:00:00Z')",
            )
            .execute(&pool)
            .await
            .is_ok()
        );
        assert!(
            sqlx::query(
                "INSERT INTO model_downloads (
                    task_id, model_id, source_key, repo_id, filename, status, created_at,
                    updated_at
                 ) VALUES (
                    'task-download-bad-status', 'model-ok', 'source-bad', 'repo', 'model.gguf',
                    'paused', '2026-06-08T00:00:00Z', '2026-06-08T00:00:00Z'
                 )",
            )
            .execute(&pool)
            .await
            .is_err()
        );
    }
}
