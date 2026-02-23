use crate::entities::{ChatMessage, ChatSession};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateSessionRequest {
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

impl ChatSession {
    pub fn to_response(&self) -> SessionResponse {
        SessionResponse {
            id: self.id.clone(),
            name: self.name.clone(),
            state_path: self.state_path.clone(),
            created_at: self.created_at.to_rfc3339(),
            updated_at: self.updated_at.to_rfc3339(),
        }
    }
}

impl ChatMessage {
    pub fn to_response(&self) -> MessageResponse {
        MessageResponse {
            id: self.id.clone(),
            session_id: self.session_id.clone(),
            role: self.role.clone(),
            content: self.content.clone(),
            created_at: self.created_at.to_rfc3339(),
        }
    }
}
