use chrono::{DateTime, Utc};

/// A row in the `chat_sessions` table.
#[derive(Debug, Clone)]
pub struct ChatSession {
    pub id: String,
    pub name: String,
    pub state_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
