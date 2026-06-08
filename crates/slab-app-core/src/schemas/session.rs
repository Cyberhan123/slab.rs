//! Request / response types for the session API (`/v1/sessions/...`).

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::domain::models::{
    CreateSessionCommand, DeleteSessionView, SessionMessageView, SessionView,
};

#[derive(Debug, Clone, Deserialize, ToSchema, IntoParams, Validate)]
pub struct SessionIdPath {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "id must not be empty"
    ))]
    pub id: String,
}

/// Request body for `POST /v1/sessions`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct CreateSessionRequest {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "name must not be empty"
    ))]
    pub name: Option<String>,
}

/// Request body for `PUT /v1/sessions/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct UpdateSessionRequest {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "name must not be empty"
    ))]
    pub name: String,
}

/// Response for a single chat session.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SessionResponse {
    pub id: String,
    pub name: String,
    pub state_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Response for a single session message.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MessageResponse {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

/// Response for `DELETE /v1/sessions/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DeleteSessionResponse {
    pub deleted: bool,
}

// ── conversions ───────────────────────────────────────────────────────────────

impl From<SessionView> for SessionResponse {
    fn from(session: SessionView) -> Self {
        Self {
            id: session.id,
            name: session.name,
            state_path: session.state_path,
            created_at: session.created_at.to_rfc3339(),
            updated_at: session.updated_at.to_rfc3339(),
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
            created_at: message.created_at.to_rfc3339(),
        }
    }
}

impl From<DeleteSessionView> for DeleteSessionResponse {
    fn from(view: DeleteSessionView) -> Self {
        Self { deleted: view.deleted }
    }
}

impl From<CreateSessionRequest> for CreateSessionCommand {
    fn from(request: CreateSessionRequest) -> Self {
        Self { name: request.name }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::*;

    #[test]
    fn session_schema_converts_datetime_views_to_rfc3339() {
        let created_at =
            DateTime::parse_from_rfc3339("2026-06-08T01:02:03Z").unwrap().with_timezone(&Utc);
        let updated_at =
            DateTime::parse_from_rfc3339("2026-06-08T04:05:06Z").unwrap().with_timezone(&Utc);

        let response = SessionResponse::from(SessionView {
            id: "session-1".to_owned(),
            name: "Session".to_owned(),
            state_path: Some("state.json".to_owned()),
            created_at,
            updated_at,
        });
        let message = MessageResponse::from(SessionMessageView {
            id: "message-1".to_owned(),
            session_id: "session-1".to_owned(),
            role: "assistant".to_owned(),
            content: "{}".to_owned(),
            created_at,
        });

        assert_eq!(response.created_at, "2026-06-08T01:02:03+00:00");
        assert_eq!(response.updated_at, "2026-06-08T04:05:06+00:00");
        assert_eq!(message.created_at, "2026-06-08T01:02:03+00:00");
    }
}
