use chrono::Utc;
use uuid::Uuid;

use crate::api::v1::session::schema::{CreateSessionRequest, MessageResponse, SessionResponse};
use crate::context::ModelState;
use crate::error::ServerError;
use crate::infra::db::{ChatStore, ChatSession, SessionStore};

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
        req: CreateSessionRequest,
    ) -> Result<SessionResponse, ServerError> {
        let now = Utc::now();
        let session = ChatSession {
            id: Uuid::new_v4().to_string(),
            name: req.name.unwrap_or_default(),
            state_path: None,
            created_at: now,
            updated_at: now,
        };
        self.state.store().create_session(session.clone()).await?;
        Ok(session.to_response())
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionResponse>, ServerError> {
        let sessions = self.state.store().list_sessions().await?;
        Ok(sessions.into_iter().map(|session| session.to_response()).collect())
    }

    pub async fn delete_session(&self, id: &str) -> Result<serde_json::Value, ServerError> {
        self.state.store().delete_session(id).await?;
        Ok(serde_json::json!({ "deleted": true }))
    }

    pub async fn list_session_messages(
        &self,
        id: &str,
    ) -> Result<Vec<MessageResponse>, ServerError> {
        let messages = self.state.store().list_messages(id).await?;
        Ok(messages
            .into_iter()
            .map(|message| message.to_response())
            .collect())
    }
}
