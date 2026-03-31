use chrono::{DateTime, Utc};

use crate::domain::models::{ConversationMessage, deserialize_session_message};

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

impl From<ChatMessage> for ConversationMessage {
    fn from(message: ChatMessage) -> Self {
        deserialize_session_message(&message.role, &message.content)
    }
}
