//! HTTP handlers for `/v1/agents/*`.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::stream::StreamExt;
use serde::Serialize;
use tokio_stream::wrappers::BroadcastStream;
use utoipa::OpenApi;

use crate::api::v1::agent::schema::{
    AgentApproveRequest, AgentApproveResponse, AgentInputRequest, AgentInputResponse,
    AgentInterruptResponse, AgentShutdownResponse, AgentStatusResponse, SpawnAgentRequest,
    SpawnAgentResponse,
};
use crate::api::validation::ValidatedJson;
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::AgentService;

#[derive(OpenApi)]
#[openapi(
    paths(
        spawn_agent,
        agent_input,
        agent_status,
        agent_shutdown,
        agent_approve,
        agent_interrupt,
        agent_events
    ),
    components(schemas(
        SpawnAgentRequest,
        SpawnAgentResponse,
        AgentInputRequest,
        AgentInputResponse,
        AgentStatusResponse,
        AgentShutdownResponse,
        AgentApproveRequest,
        AgentApproveResponse,
        AgentInterruptResponse,
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
        .route("/agents/{id}/approve", post(agent_approve))
        .route("/agents/{id}/interrupt", post(agent_interrupt))
        .route("/agents/{id}/events", get(agent_events))
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
    ValidatedJson(req): ValidatedJson<SpawnAgentRequest>,
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
        (status = 501, description = "Not implemented"),
    )
)]
async fn agent_input(
    State(_service): State<AgentService>,
    Path(_id): Path<String>,
    Json(req): Json<AgentInputRequest>,
) -> Result<(axum::http::StatusCode, Json<AgentInputResponse>), ServerError> {
    let _content = req.content;
    Ok((
        axum::http::StatusCode::NOT_IMPLEMENTED,
        Json(AgentInputResponse {
            accepted: false,
            message: "send_input is not yet implemented; the agent runs autonomously once spawned"
                .into(),
        }),
    ))
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

#[utoipa::path(
    post,
    path = "/v1/agents/{id}/approve",
    tag = "agents",
    params(
        ("id" = String, Path, description = "Agent thread ID")
    ),
    request_body = AgentApproveRequest,
    responses(
        (status = 200, description = "Approval decision delivered", body = AgentApproveResponse),
    )
)]
async fn agent_approve(
    State(service): State<AgentService>,
    Path(id): Path<String>,
    Json(req): Json<AgentApproveRequest>,
) -> Json<AgentApproveResponse> {
    let delivered = service.approve_call(&id, &req.call_id, req.approved);
    Json(AgentApproveResponse { call_id: req.call_id, delivered })
}

#[utoipa::path(
    post,
    path = "/v1/agents/{id}/interrupt",
    tag = "agents",
    params(
        ("id" = String, Path, description = "Agent thread ID to interrupt")
    ),
    responses(
        (status = 200, description = "Thread interrupted", body = AgentInterruptResponse),
        (status = 404, description = "Thread not found"),
    )
)]
async fn agent_interrupt(
    State(service): State<AgentService>,
    Path(id): Path<String>,
) -> Result<Json<AgentInterruptResponse>, ServerError> {
    service.shutdown(&id).await?;
    Ok(Json(AgentInterruptResponse { thread_id: id, interrupted: true }))
}

#[utoipa::path(
    get,
    path = "/v1/agents/{id}/events",
    tag = "agents",
    params(
        ("id" = String, Path, description = "Agent thread ID")
    ),
    responses(
        (status = 200, description = "SSE stream of turn events"),
    )
)]
async fn agent_events(
    State(service): State<AgentService>,
    Path(id): Path<String>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = service.subscribe_events(&id);
    let stream = BroadcastStream::new(rx).map(|msg| {
        let event = match msg {
            Ok(event) => {
                let data = turn_event_to_sse_data(&event);
                Event::default().data(data)
            }
            Err(_) => Event::default().data(serialize_agent_sse_event(&AgentSseEvent::Lagged)),
        };
        Ok::<Event, std::convert::Infallible>(event)
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AgentSseEvent<'a> {
    AssistantDelta { text: &'a str },
    ToolCallStarted { tool_name: &'a str, call_id: &'a str, arguments: &'a str },
    ToolCallOutput { call_id: &'a str, output: &'a str },
    ApprovalRequired { call_id: &'a str, tool_name: &'a str, command: &'a str },
    TurnCompleted { text: &'a str },
    TurnFailed { error: &'a str },
    AgentStatus { status: String },
    Lagged,
}

fn serialize_agent_sse_event(event: &AgentSseEvent<'_>) -> String {
    serde_json::to_string(event).unwrap_or_else(|_| {
        r#"{"type":"turn_failed","error":"failed to serialize event"}"#.to_owned()
    })
}

fn turn_event_to_sse_data(event: &slab_agent::TurnEvent) -> String {
    let event = match event {
        slab_agent::TurnEvent::AssistantDelta { text } => AgentSseEvent::AssistantDelta { text },
        slab_agent::TurnEvent::ToolCallStarted { tool_name, call_id, arguments } => {
            AgentSseEvent::ToolCallStarted { tool_name, call_id, arguments }
        }
        slab_agent::TurnEvent::ToolCallOutput { call_id, output } => {
            AgentSseEvent::ToolCallOutput { call_id, output }
        }
        slab_agent::TurnEvent::ApprovalRequired { call_id, tool_name, command } => {
            AgentSseEvent::ApprovalRequired { call_id, tool_name, command }
        }
        slab_agent::TurnEvent::TurnCompleted { text } => AgentSseEvent::TurnCompleted { text },
        slab_agent::TurnEvent::TurnFailed { error } => AgentSseEvent::TurnFailed { error },
        slab_agent::TurnEvent::AgentStatus { status } => {
            AgentSseEvent::AgentStatus { status: format!("{status:?}") }
        }
    };
    serialize_agent_sse_event(&event)
}
