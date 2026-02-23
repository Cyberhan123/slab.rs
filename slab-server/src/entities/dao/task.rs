use chrono::{DateTime, Utc};

/// A row in the `tasks` table.
#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub id: String,
    pub task_type: String,
    pub status: String,
    pub input_data: Option<String>,
    pub result_data: Option<String>,
    pub error_msg: Option<String>,
    /// slab-core runtime `TaskId` (u64) for tasks submitted via `api::backend(...).run()`.
    /// `None` for server-only tasks (e.g. pure ffmpeg conversion, download).
    pub core_task_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
