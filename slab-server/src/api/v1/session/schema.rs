use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::domain::models::{SessionMessageView, SessionView};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct CreateSessionRequest {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "name must not be empty"
    ))]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SessionResponse {
    pub id: String,
    pub name: String,
    pub state_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MessageResponse {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

impl From<SessionView> for SessionResponse {
    fn from(session: SessionView) -> Self {
        Self {
            id: session.id,
            name: session.name,
            state_path: session.state_path,
            created_at: session.created_at,
            updated_at: session.updated_at,
        }
    }
}

impl From<SessionMessageView> for MessageResponse {
    fn from(message: SessionMessageView) -> Self {
        Self {
            id: message.id,
            session_id: message.session_id,
            role: message.role,
            content: message.content,
            created_at: message.created_at,
        }
    }
}
