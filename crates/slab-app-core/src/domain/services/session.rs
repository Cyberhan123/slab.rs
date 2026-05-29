use chrono::Utc;
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    CreateSessionCommand, DeleteSessionView, SessionMessageView, SessionView,
};
use crate::error::AppCoreError;
use crate::infra::db::{ChatSession, ChatStore, SessionStore};

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
    ) -> Result<SessionView, AppCoreError> {
        let now = Utc::now();
        let session = ChatSession {
            id: Uuid::new_v4().to_string(),
            name: req.name.unwrap_or_default(),
            state_path: None,
            created_at: now,
            updated_at: now,
        };
        self.state.store().create_session(session.clone()).await?;
        Ok(SessionView::from(&session))
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionView>, AppCoreError> {
        let sessions = self.state.store().list_sessions().await?;
        Ok(sessions.into_iter().map(|session| SessionView::from(&session)).collect())
    }

    pub async fn update_session_name(
        &self,
        id: &str,
        name: String,
    ) -> Result<SessionView, AppCoreError> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err(AppCoreError::BadRequest("session name must not be empty".to_owned()));
        }

        let session = self
            .state
            .store()
            .update_session_name(id, &name, Utc::now())
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("session {id} not found")))?;
        Ok(SessionView::from(&session))
    }

    pub async fn delete_session(&self, id: &str) -> Result<DeleteSessionView, AppCoreError> {
        self.state.store().delete_session(id).await?;
        Ok(DeleteSessionView { deleted: true })
    }

    pub async fn list_session_messages(
        &self,
        id: &str,
    ) -> Result<Vec<SessionMessageView>, AppCoreError> {
        let messages = self.state.store().list_messages(id).await?;
        Ok(messages.into_iter().map(|message| SessionMessageView::from(&message)).collect())
    }
}
