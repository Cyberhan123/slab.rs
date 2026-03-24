//! HTTP handlers for `/v1/agents/*`.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::api::v1::agent::schema::{
    AgentInputRequest, AgentInputResponse, AgentShutdownResponse, AgentStatusResponse,
    SpawnAgentRequest, SpawnAgentResponse,
};
use crate::context::AppState;
use crate::domain::services::AgentService;
use crate::error::ServerError;

#[derive(OpenApi)]
#[openapi(
    paths(spawn_agent, agent_input, agent_status, agent_shutdown),
    components(schemas(
        SpawnAgentRequest,
        SpawnAgentResponse,
        AgentInputRequest,
        AgentInputResponse,
        AgentStatusResponse,
        AgentShutdownResponse,
        crate::api::v1::agent::schema::AgentConfigInput,
        crate::api::v1::agent::schema::MessageInput,
        crate::api::v1::agent::schema::AgentStatusValue,
    ))
)]
pub struct AgentApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/agents/spawn", post(spawn_agent))
        .route("/agents/{id}/input", post(agent_input))
        .route("/agents/{id}/status", get(agent_status))
        .route("/agents/{id}/shutdown", post(agent_shutdown))
}

// ── Handlers ──────────────────────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/v1/agents/spawn",
    tag = "agents",
    request_body = SpawnAgentRequest,
    responses(
        (status = 201, description = "Agent thread spawned", body = SpawnAgentResponse),
        (status = 400, description = "Bad request"),
        (status = 429, description = "Thread limit exceeded"),
        (status = 500, description = "Internal error"),
    )
)]
async fn spawn_agent(
    State(service): State<AgentService>,
    Json(req): Json<SpawnAgentRequest>,
) -> Result<(axum::http::StatusCode, Json<SpawnAgentResponse>), ServerError> {
    let messages: Vec<slab_types::ConversationMessage> =
        req.messages.into_iter().map(Into::into).collect();

    let thread_id = service.spawn(req.session_id, req.config.into(), messages).await?;
    Ok((axum::http::StatusCode::CREATED, Json(SpawnAgentResponse { thread_id })))
}

#[utoipa::path(
    post,
    path = "/v1/agents/{id}/input",
    tag = "agents",
    params(
        ("id" = String, Path, description = "Agent thread ID")
    ),
    request_body = AgentInputRequest,
    responses(
        (status = 200, description = "Input accepted", body = AgentInputResponse),
        (status = 404, description = "Thread not found"),
        (status = 501, description = "Not implemented"),
    )
)]
async fn agent_input(
    State(_service): State<AgentService>,
    Path(_id): Path<String>,
    Json(_req): Json<AgentInputRequest>,
) -> Result<Json<AgentInputResponse>, ServerError> {
    Ok(Json(AgentInputResponse {
        accepted: false,
        message: "send_input is not yet implemented; the agent runs autonomously once spawned"
            .into(),
    }))
}

#[utoipa::path(
    get,
    path = "/v1/agents/{id}/status",
    tag = "agents",
    params(
        ("id" = String, Path, description = "Agent thread ID")
    ),
    responses(
        (status = 200, description = "Agent thread status", body = AgentStatusResponse),
        (status = 404, description = "Thread not found"),
        (status = 500, description = "Internal error"),
    )
)]
async fn agent_status(
    State(service): State<AgentService>,
    Path(id): Path<String>,
) -> Result<Json<AgentStatusResponse>, ServerError> {
    let status = service.get_status(&id).await?;
    Ok(Json(AgentStatusResponse { thread_id: id, status: status.into() }))
}

#[utoipa::path(
    post,
    path = "/v1/agents/{id}/shutdown",
    tag = "agents",
    params(
        ("id" = String, Path, description = "Agent thread ID to shut down")
    ),
    responses(
        (status = 200, description = "Agent thread shut down", body = AgentShutdownResponse),
        (status = 404, description = "Thread not found"),
        (status = 500, description = "Internal error"),
    )
)]
async fn agent_shutdown(
    State(service): State<AgentService>,
    Path(id): Path<String>,
) -> Result<Json<AgentShutdownResponse>, ServerError> {
    service.shutdown(&id).await?;
    Ok(Json(AgentShutdownResponse { thread_id: id, shutdown: true }))
}
