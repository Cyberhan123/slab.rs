use chrono::Utc;
use uuid::Uuid;

use crate::context::ModelState;
use crate::error::ServerError;
use crate::infra::db::{ChatSession, ChatStore, SessionStore};

#[derive(Debug, Clone)]
pub struct CreateSessionCommand {
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SessionView {
    pub id: String,
    pub name: String,
    pub state_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct SessionMessageView {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct SessionService {
    state: ModelState,
}

impl SessionService {
    pub fn new(state: ModelState) -> Self {
        Self { state }
    }

    pub async fn create_session(
        &self,
        req: CreateSessionCommand,
    ) -> Result<SessionView, ServerError> {
        let now = Utc::now();
        let session = ChatSession {
            id: Uuid::new_v4().to_string(),
            name: req.name.unwrap_or_default(),
            state_path: None,
            created_at: now,
            updated_at: now,
        };
        self.state.store().create_session(session.clone()).await?;
        Ok(to_session_view(&session))
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionView>, ServerError> {
        let sessions = self.state.store().list_sessions().await?;
        Ok(sessions
            .into_iter()
            .map(|session| to_session_view(&session))
            .collect())
    }

    pub async fn delete_session(&self, id: &str) -> Result<serde_json::Value, ServerError> {
        self.state.store().delete_session(id).await?;
        Ok(serde_json::json!({ "deleted": true }))
    }

    pub async fn list_session_messages(
        &self,
        id: &str,
    ) -> Result<Vec<SessionMessageView>, ServerError> {
        let messages = self.state.store().list_messages(id).await?;
        Ok(messages
            .into_iter()
            .map(|message| to_session_message_view(&message))
            .collect())
    }
}

fn to_session_view(session: &ChatSession) -> SessionView {
    SessionView {
        id: session.id.clone(),
        name: session.name.clone(),
        state_path: session.state_path.clone(),
        created_at: session.created_at.to_rfc3339(),
        updated_at: session.updated_at.to_rfc3339(),
    }
}

fn to_session_message_view(message: &crate::infra::db::ChatMessage) -> SessionMessageView {
    SessionMessageView {
        id: message.id.clone(),
        session_id: message.session_id.clone(),
        role: message.role.clone(),
        content: message.content.clone(),
        created_at: message.created_at.to_rfc3339(),
    }
}
