//! SQL-backed implementation of [`AgentStorePort`] for the shared [`SqlxStore`].

use async_trait::async_trait;
use slab_agent::port::{AgentStorePort, ToolCallRecord, ThreadSnapshot};
use slab_agent::port::ThreadStatus;
use slab_types::agent::ToolCallStatus;

use super::SqlxStore;

fn status_str(s: ThreadStatus) -> &'static str {
    match s {
        ThreadStatus::Pending => "pending",
        ThreadStatus::Running => "running",
        ThreadStatus::Completed => "completed",
        ThreadStatus::Errored => "errored",
        ThreadStatus::Shutdown => "shutdown",
    }
}

fn parse_status(s: &str) -> ThreadStatus {
    match s {
        "running" => ThreadStatus::Running,
        "completed" => ThreadStatus::Completed,
        "errored" => ThreadStatus::Errored,
        "shutdown" => ThreadStatus::Shutdown,
        _ => ThreadStatus::Pending,
    }
}

fn tool_call_status_str(s: ToolCallStatus) -> &'static str {
    match s {
        ToolCallStatus::Pending => "pending",
        ToolCallStatus::Running => "running",
        ToolCallStatus::Completed => "completed",
        ToolCallStatus::Failed => "failed",
    }
}

/// sqlx row type for the `agent_threads` table.
#[derive(sqlx::FromRow)]
struct AgentThreadRow {
    id: String,
    session_id: String,
    parent_id: Option<String>,
    depth: i64,
    status: String,
    role_name: Option<String>,
    config_json: String,
    completion_text: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<AgentThreadRow> for ThreadSnapshot {
    fn from(r: AgentThreadRow) -> Self {
        ThreadSnapshot {
            id: r.id,
            session_id: r.session_id,
            parent_id: r.parent_id,
            depth: r.depth as u32,
            status: parse_status(&r.status),
            role_name: r.role_name,
            config_json: r.config_json,
            completion_text: r.completion_text,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[async_trait]
impl AgentStorePort for SqlxStore {
    async fn upsert_thread(
        &self,
        snapshot: &ThreadSnapshot,
    ) -> Result<(), slab_agent::AgentError> {
        sqlx::query(
            "INSERT INTO agent_threads \
             (id, session_id, parent_id, depth, status, role_name, config_json, \
              completion_text, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
             ON CONFLICT(id) DO UPDATE SET \
               status=excluded.status, \
               completion_text=excluded.completion_text, \
               updated_at=excluded.updated_at",
        )
        .bind(&snapshot.id)
        .bind(&snapshot.session_id)
        .bind(&snapshot.parent_id)
        .bind(snapshot.depth as i64)
        .bind(status_str(snapshot.status))
        .bind(&snapshot.role_name)
        .bind(&snapshot.config_json)
        .bind(&snapshot.completion_text)
        .bind(&snapshot.created_at)
        .bind(&snapshot.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_thread(
        &self,
        id: &str,
    ) -> Result<Option<ThreadSnapshot>, slab_agent::AgentError> {
        let row: Option<AgentThreadRow> = sqlx::query_as(
            "SELECT id, session_id, parent_id, depth, status, role_name, \
             config_json, completion_text, created_at, updated_at \
             FROM agent_threads WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;

        Ok(row.map(Into::into))
    }

    async fn update_thread_status(
        &self,
        id: &str,
        status: ThreadStatus,
        completion_text: Option<&str>,
    ) -> Result<(), slab_agent::AgentError> {
        sqlx::query(
            "UPDATE agent_threads SET status = ?1, completion_text = ?2, \
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id = ?3",
        )
        .bind(status_str(status))
        .bind(completion_text)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }

    async fn insert_tool_call(
        &self,
        record: &ToolCallRecord,
    ) -> Result<(), slab_agent::AgentError> {
        sqlx::query(
            "INSERT INTO agent_tool_calls \
             (id, thread_id, tool_name, arguments, output, status, created_at, completed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&record.id)
        .bind(&record.thread_id)
        .bind(&record.tool_name)
        .bind(&record.arguments)
        .bind(&record.output)
        .bind(tool_call_status_str(record.status))
        .bind(&record.created_at)
        .bind(&record.completed_at)
        .execute(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }

    async fn update_tool_call(
        &self,
        id: &str,
        output: Option<&str>,
        status: ToolCallStatus,
        completed_at: &str,
    ) -> Result<(), slab_agent::AgentError> {
        sqlx::query(
            "UPDATE agent_tool_calls SET output = ?1, status = ?2, completed_at = ?3 \
             WHERE id = ?4",
        )
        .bind(output)
        .bind(tool_call_status_str(status))
        .bind(completed_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }
}
