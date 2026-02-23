use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::Utc;
use uuid::Uuid;
use utoipa::OpenApi;

use crate::entities::{ChatSession, ChatStore, SessionStore};
use crate::error::ServerError;
use crate::schemas::v1::session::{CreateSessionRequest,MessageResponse, SessionResponse};
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    paths(create_session, list_sessions, delete_session, list_session_messages),
    components(schemas(
        CreateSessionRequest,
        SessionResponse,
        MessageResponse
    ))
)]
pub struct SessionApi;

/// Register session routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions",       post(create_session).get(list_sessions))
        .route("/sessions/{id}",  delete(delete_session))
        .route("/sessions/{id}/messages", get(list_session_messages))
}

// ── Session handlers ──────────────────────────────────────────────────────────
#[utoipa::path(
    post,
    path = "/v1/sessions",
    tag = "sessions",
    request_body = CreateSessionRequest,
    responses(
        (status = 200, description = "Session created", body = SessionResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<SessionResponse>, ServerError> {
    let now = Utc::now();
    let session = ChatSession {
        id: Uuid::new_v4().to_string(),
        name: req.name.unwrap_or_default(),
        state_path: None,
        created_at: now,
        updated_at: now,
    };
    state.store.create_session(session.clone()).await?;
    Ok(Json(session.to_response()))
}

#[utoipa::path(
    post,
    path = "/v1/sessions",
    tag = "sessions",
    responses(
        (status = 200, description = "Session list retrieved", body = Vec<SessionResponse>),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<SessionResponse>>, ServerError> {
    let sessions = state.store.list_sessions().await?;
    Ok(Json(sessions.into_iter().map(|s| s.to_response()).collect()))
}

#[utoipa::path(
    delete,
    path = "/v1/sessions/{id}",
    tag = "sessions",
    responses(
        (status = 200, description = "Session deleted", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ServerError> {
    state.store.delete_session(&id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[utoipa::path(
    get,
    path = "/v1/sessions/{id}/messages",
    tag = "sessions",
    responses(
        (status = 200, description = "Session messages retrieved", body = Vec<MessageResponse>),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn list_session_messages(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<MessageResponse>>, ServerError> {
    let messages = state.store.list_messages(&id).await?;
    Ok(Json(messages.into_iter().map(|m| m.to_response()).collect()))
}
