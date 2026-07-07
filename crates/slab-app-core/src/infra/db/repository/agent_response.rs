//! SQL-backed OpenAI Responses persistence owned by app-core.

use async_trait::async_trait;

use super::SqlxStore;

/// Persisted complete OpenAI-Responses-canonical `Response` for a single agent run.
#[derive(Debug, Clone)]
pub struct ThreadResponseRecord {
    /// Per-run identifier (the OpenAI `Response.id`).
    pub run_id: String,
    pub thread_id: String,
    pub session_id: String,
    /// `turn_index` of the run's first turn.
    pub turn_index_start: u32,
    /// `completed` / `failed` / `cancelled` / `incomplete`.
    pub status: String,
    /// Serialized `slab_proto::openai::Response` JSON.
    pub response_json: String,
    /// RFC 3339 creation timestamp.
    pub created_at: String,
    /// RFC 3339 completion timestamp, if the run reached a terminal state.
    pub completed_at: Option<String>,
}

/// App-core-owned store for canonical OpenAI Response JSON.
#[async_trait]
pub trait AgentResponseStore: Send + Sync {
    async fn insert_thread_response(
        &self,
        record: &ThreadResponseRecord,
    ) -> Result<(), slab_agent::AgentError>;

    async fn list_thread_responses(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadResponseRecord>, slab_agent::AgentError>;
}

#[derive(sqlx::FromRow)]
struct AgentThreadResponseRow {
    run_id: String,
    thread_id: String,
    session_id: String,
    turn_index_start: i64,
    status: String,
    response_json: String,
    created_at: String,
    completed_at: Option<String>,
}

impl TryFrom<AgentThreadResponseRow> for ThreadResponseRecord {
    type Error = slab_agent::AgentError;

    fn try_from(row: AgentThreadResponseRow) -> Result<Self, Self::Error> {
        let turn_index_start = u32::try_from(row.turn_index_start).map_err(|error| {
            tracing::warn!(
                run_id = %row.run_id,
                thread_id = %row.thread_id,
                turn_index_start = row.turn_index_start,
                error = %error,
                "invalid agent thread response turn_index_start in database"
            );
            slab_agent::AgentError::Store(format!(
                "invalid agent thread response turn_index_start for '{}': {} ({})",
                row.run_id, row.turn_index_start, error
            ))
        })?;
        Ok(ThreadResponseRecord {
            run_id: row.run_id,
            thread_id: row.thread_id,
            session_id: row.session_id,
            turn_index_start,
            status: row.status,
            response_json: row.response_json,
            created_at: row.created_at,
            completed_at: row.completed_at,
        })
    }
}

#[async_trait]
impl AgentResponseStore for SqlxStore {
    async fn insert_thread_response(
        &self,
        record: &ThreadResponseRecord,
    ) -> Result<(), slab_agent::AgentError> {
        sqlx::query(
            "INSERT INTO agent_thread_responses \
             (run_id, thread_id, session_id, turn_index_start, status, response_json, \
              created_at, completed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&record.run_id)
        .bind(&record.thread_id)
        .bind(&record.session_id)
        .bind(i64::from(record.turn_index_start))
        .bind(&record.status)
        .bind(&record.response_json)
        .bind(&record.created_at)
        .bind(&record.completed_at)
        .execute(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_thread_responses(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadResponseRecord>, slab_agent::AgentError> {
        let rows: Vec<AgentThreadResponseRow> = sqlx::query_as(
            "SELECT run_id, thread_id, session_id, turn_index_start, status, response_json, \
             created_at, completed_at FROM agent_thread_responses WHERE thread_id = ?1 \
             ORDER BY created_at ASC, run_id ASC",
        )
        .bind(thread_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;

        rows.into_iter().map(ThreadResponseRecord::try_from).collect()
    }
}
