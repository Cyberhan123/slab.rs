pub mod agent;
pub mod chat;
pub mod config;
pub mod media_task;
pub mod model;
pub mod model_config_state;
pub mod model_download;
pub mod plugin;
pub mod session;
pub mod task;
pub mod ui_state;

pub use chat::ChatStore;
pub use media_task::MediaTaskStore;
pub use model::ModelStore;
pub use model_config_state::ModelConfigStateStore;
pub use model_download::ModelDownloadStore;
pub use plugin::PluginStateStore;
pub use session::SessionStore;
pub use task::TaskStore;
pub use ui_state::UiStateStore;

use crate::infra::db::entities::TaskRecord;
use std::str::FromStr;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct SqlxStore {
    pub(crate) pool: sqlx::Pool<sqlx::Sqlite>,
}

impl SqlxStore {
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        let options = sqlx::sqlite::SqliteConnectOptions::from_str(url)?
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_millis(5_000));
        let pool = sqlx::SqlitePool::connect_with(options).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }
}

pub type AnyStore = SqlxStore;

async fn insert_task_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    record: &TaskRecord,
    result_data: Option<&str>,
) -> Result<(), sqlx::Error> {
    let created_at = record.created_at.to_rfc3339();
    let updated_at = record.updated_at.to_rfc3339();
    sqlx::query(
        "INSERT INTO tasks (id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )
    .bind(&record.id)
    .bind(&record.task_type)
    .bind(record.status.as_str())
    .bind(&record.model_id)
    .bind(&record.input_data)
    .bind(result_data)
    .bind(&record.error_msg)
    .bind(record.core_task_id)
    .bind(&created_at)
    .bind(&updated_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::migrated_test_pool;
    use std::collections::HashSet;

    #[tokio::test]
    async fn connect_configures_sqlite_concurrency_pragmas() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let database_url = slab_types::sqlite_url_for_path(&temp_dir.path().join("slab.db"));
        let store = SqlxStore::connect(&database_url).await.expect("connect store");

        let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&store.pool)
            .await
            .expect("journal mode");
        let busy_timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(&store.pool)
            .await
            .expect("busy timeout");
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&store.pool)
            .await
            .expect("foreign keys");

        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
        assert_eq!(busy_timeout, 5_000);
        assert_eq!(foreign_keys, 1);
    }

    #[tokio::test]
    async fn migrations_apply_expected_constraints_and_indexes() {
        let pool = migrated_test_pool().await;

        assert_foreign_key(&pool, "model_config_state", "model_id", "models", "CASCADE").await;
        for table in
            ["image_generation_tasks", "video_generation_tasks", "audio_transcription_tasks"]
        {
            assert_foreign_key(&pool, table, "task_id", "tasks", "CASCADE").await;
        }
        for table in ["agent_turn_states", "agent_memory_phase1_outputs"] {
            assert_foreign_key(&pool, table, "thread_id", "agent_threads", "CASCADE").await;
        }

        let usage_columns = table_columns(&pool, "agent_memory_usage_events").await;
        assert!(usage_columns.contains("source_kind"));
        let config_state_columns = table_columns(&pool, "model_config_state").await;
        assert!(config_state_columns.contains("selected_engine_id"));
        let (not_null, default_value): (i64, Option<String>) = sqlx::query_as(
            "SELECT [notnull], dflt_value \
             FROM pragma_table_info('agent_memory_usage_events') \
             WHERE name = 'source_kind'",
        )
        .fetch_one(&pool)
        .await
        .expect("source_kind column metadata");
        assert_eq!(not_null, 1);
        assert_eq!(default_value.as_deref(), Some("'unknown'"));

        for index in [
            "idx_agent_memory_usage_events_source_kind",
            "idx_image_generation_tasks_created_at",
            "idx_video_generation_tasks_created_at",
            "idx_audio_transcription_tasks_created_at",
            "idx_model_config_state_updated_at",
            "idx_agent_threads_session",
            "idx_agent_turn_states_status",
            "idx_agent_memory_phase1_status",
            "idx_agent_memory_phase2_runs_status",
            "idx_agent_memory_usage_events_thread",
        ] {
            assert_index_exists(&pool, index).await;
        }

        let invalid_status = sqlx::query(
            "INSERT INTO tasks (id, task_type, status, created_at, updated_at) \
             VALUES ('bad-status', 'test', 'bogus', '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z')",
        )
        .execute(&pool)
        .await;
        assert!(invalid_status.is_err());

        let invalid_model_status = sqlx::query(
            "INSERT INTO models (\
                id, display_name, status, spec, created_at, updated_at, kind, \
                config_schema_version, config_policy_version, capabilities\
             ) VALUES (\
                'bad-model-status', 'Bad', 'bogus', '{}', '2026-06-17T00:00:00Z', \
                '2026-06-17T00:00:00Z', 'local', 2, 1, '[]'\
             )",
        )
        .execute(&pool)
        .await;
        assert!(invalid_model_status.is_err());

        let invalid_lock = sqlx::query(
            "INSERT INTO agent_memory_phase2_lock (id, status, updated_at) \
             VALUES (2, 'idle', '2026-06-17T00:00:00Z')",
        )
        .execute(&pool)
        .await;
        assert!(invalid_lock.is_err());

        sqlx::query(
            "INSERT INTO models (\
                id, display_name, status, spec, created_at, updated_at, kind, \
                config_schema_version, config_policy_version, capabilities\
             ) VALUES (\
                'model-1', 'Model', 'ready', '{}', '2026-06-17T00:00:00Z', \
                '2026-06-17T00:00:00Z', 'local', 2, 1, '[]'\
             )",
        )
        .execute(&pool)
        .await
        .expect("insert model with defaults");
        let materialized_artifacts: String =
            sqlx::query_scalar("SELECT materialized_artifacts FROM models WHERE id = 'model-1'")
                .fetch_one(&pool)
                .await
                .expect("materialized artifacts default");
        assert_eq!(materialized_artifacts, "{}");

        let orphan_config_state = sqlx::query(
            "INSERT INTO model_config_state (model_id, updated_at) \
             VALUES ('missing-model', '2026-06-17T00:00:00Z')",
        )
        .execute(&pool)
        .await;
        assert!(orphan_config_state.is_err());

        sqlx::query(
            "INSERT INTO chat_sessions (id, created_at, updated_at) \
             VALUES ('session-defaults', '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert default session");
        sqlx::query(
            "INSERT INTO agent_threads (id, session_id, created_at, updated_at) \
             VALUES ('thread-defaults', 'session-defaults', '2026-06-17T00:00:00Z', \
                '2026-06-17T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert agent thread defaults");
        let (depth, status, config_json): (i64, String, String) = sqlx::query_as(
            "SELECT depth, status, config_json FROM agent_threads WHERE id = 'thread-defaults'",
        )
        .fetch_one(&pool)
        .await
        .expect("agent thread defaults");
        assert_eq!(depth, 0);
        assert_eq!(status, "pending");
        assert_eq!(config_json, "{}");
    }

    #[tokio::test]
    async fn migrations_preserve_media_and_agent_memory_cascades() {
        let pool = migrated_test_pool().await;

        insert_task(&pool, "image-task").await;
        insert_task(&pool, "video-task").await;
        insert_task(&pool, "audio-task").await;
        insert_image_task(&pool, "image-task").await;
        insert_video_task(&pool, "video-task").await;
        insert_audio_task(&pool, "audio-task").await;

        for table in
            ["image_generation_tasks", "video_generation_tasks", "audio_transcription_tasks"]
        {
            assert_eq!(row_count(&pool, table).await, 1);
        }

        sqlx::query("DELETE FROM tasks").execute(&pool).await.expect("delete parent tasks");
        for table in
            ["image_generation_tasks", "video_generation_tasks", "audio_transcription_tasks"]
        {
            assert_eq!(row_count(&pool, table).await, 0);
        }

        sqlx::query(
            "INSERT INTO chat_sessions (id, name, created_at, updated_at) \
             VALUES ('session-1', 'Session', '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert chat session");
        sqlx::query(
            "INSERT INTO agent_threads (id, session_id, status, created_at, updated_at) \
             VALUES ('thread-1', 'session-1', 'running', '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert agent thread");
        sqlx::query(
            "INSERT INTO agent_memory_phase1_outputs (thread_id, session_id, updated_at) \
             VALUES ('thread-1', 'session-1', '2026-06-17T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert memory phase1 output");
        sqlx::query(
            "INSERT INTO agent_memory_usage_events (id, thread_id, source, used_at) \
             VALUES ('usage-1', 'thread-1', 'rollout', '2026-06-17T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert usage event");

        let source_kind: String = sqlx::query_scalar(
            "SELECT source_kind FROM agent_memory_usage_events WHERE id = 'usage-1'",
        )
        .fetch_one(&pool)
        .await
        .expect("usage source kind default");
        assert_eq!(source_kind, "unknown");

        sqlx::query("DELETE FROM chat_sessions WHERE id = 'session-1'")
            .execute(&pool)
            .await
            .expect("delete chat session");
        assert_eq!(row_count(&pool, "agent_threads").await, 0);
        assert_eq!(row_count(&pool, "agent_memory_phase1_outputs").await, 0);
        assert_eq!(row_count(&pool, "agent_memory_usage_events").await, 1);
    }

    async fn table_columns(pool: &sqlx::SqlitePool, table: &str) -> HashSet<String> {
        let sql = format!("SELECT name FROM pragma_table_info('{table}')");
        sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(sql))
            .fetch_all(pool)
            .await
            .expect("table columns")
            .into_iter()
            .collect()
    }

    async fn assert_foreign_key(
        pool: &sqlx::SqlitePool,
        table: &str,
        from_column: &str,
        target_table: &str,
        on_delete: &str,
    ) {
        let sql = format!(
            "SELECT [table], [on_delete] FROM pragma_foreign_key_list('{table}') WHERE [from] = ?1"
        );
        let (actual_target, actual_on_delete): (String, String) =
            sqlx::query_as(sqlx::AssertSqlSafe(sql))
                .bind(from_column)
                .fetch_one(pool)
                .await
                .expect("foreign key metadata");
        assert_eq!(actual_target, target_table);
        assert_eq!(actual_on_delete, on_delete);
    }

    async fn assert_index_exists(pool: &sqlx::SqlitePool, index: &str) {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
        )
        .bind(index)
        .fetch_one(pool)
        .await
        .expect("index metadata");
        assert_eq!(exists, 1, "missing index {index}");
    }

    async fn row_count(pool: &sqlx::SqlitePool, table: &str) -> i64 {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.expect("row count")
    }

    async fn insert_task(pool: &sqlx::SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO tasks (id, task_type, status, created_at, updated_at) \
             VALUES (?1, 'test', 'pending', '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z')",
        )
        .bind(id)
        .execute(pool)
        .await
        .expect("insert task");
    }

    async fn insert_image_task(pool: &sqlx::SqlitePool, task_id: &str) {
        sqlx::query(
            "INSERT INTO image_generation_tasks (\
                task_id, backend_id, model_path, prompt, mode, width, height, requested_count, \
                request_data, created_at, updated_at\
             ) VALUES (?1, 'ggml.diffusion', 'model.safetensors', 'prompt', 'txt2img', 512, 512, 1, '{}', \
                '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z')",
        )
        .bind(task_id)
        .execute(pool)
        .await
        .expect("insert image task");
    }

    async fn insert_video_task(pool: &sqlx::SqlitePool, task_id: &str) {
        sqlx::query(
            "INSERT INTO video_generation_tasks (\
                task_id, backend_id, model_path, prompt, width, height, frames, fps, request_data, \
                created_at, updated_at\
             ) VALUES (?1, 'ggml.diffusion', 'model.safetensors', 'prompt', 512, 512, 16, 8.0, '{}', \
                '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z')",
        )
        .bind(task_id)
        .execute(pool)
        .await
        .expect("insert video task");
    }

    async fn insert_audio_task(pool: &sqlx::SqlitePool, task_id: &str) {
        sqlx::query(
            "INSERT INTO audio_transcription_tasks (\
                task_id, backend_id, source_path, request_data, created_at, updated_at\
             ) VALUES (?1, 'ggml.whisper', 'audio.wav', '{}', '2026-06-17T00:00:00Z', \
                '2026-06-17T00:00:00Z')",
        )
        .bind(task_id)
        .execute(pool)
        .await
        .expect("insert audio task");
    }
}
