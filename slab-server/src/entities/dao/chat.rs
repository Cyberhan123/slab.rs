
use chrono::{DateTime, Utc};

/// A single message row in the `chat_messages` table.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    /// `"user"`, `"assistant"`, or `"system"`.
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}
