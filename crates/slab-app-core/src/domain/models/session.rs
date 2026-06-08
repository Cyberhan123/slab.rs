use crate::infra::db::{ChatMessage, ChatSession};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct CreateSessionCommand {
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SessionView {
    pub id: String,
    pub name: String,
    pub state_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SessionMessageView {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DeleteSessionView {
    pub deleted: bool,
}

impl From<&ChatSession> for SessionView {
    fn from(session: &ChatSession) -> Self {
        Self {
            id: session.id.clone(),
            name: session.name.clone(),
            state_path: session.state_path.clone(),
            created_at: session.created_at,
            updated_at: session.updated_at,
        }
    }
}

impl From<&ChatMessage> for SessionMessageView {
    fn from(message: &ChatMessage) -> Self {
        Self {
            id: message.id.clone(),
            session_id: message.session_id.clone(),
            role: message.role.clone(),
            content: message.content.clone(),
            created_at: message.created_at,
        }
    }
}
