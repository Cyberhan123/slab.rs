//! HTTP and WebSocket handlers for `/v1/agents/responses`.

use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use futures::SinkExt;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use slab_app_core::context::AppState;
use slab_app_core::domain::services::{AgentService, WorkspaceService};
use slab_app_core::error::AppCoreError;
use slab_app_core::infra::agent::event_hub::AgentEventEnvelope;
use slab_app_core::schemas::chat::{OpenAiError, OpenAiErrorResponse};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use utoipa::OpenApi;

use crate::api::v1::agent::openai_compat::{
    AdapterInput, StreamCtx, build_response, envelope_to_events,
};
use crate::api::v1::agent::schema::{
    AgentConfigInput, AgentResponsesClientMessage, AgentResponsesServerMessage, AgentStatusValue,
    MessageInput, WorkspaceMigrationResponse,
};
use crate::api::v1::chat::schema::{ChatToolCall, ChatToolFunction};
use crate::api::validation::{ValidatedJson, validate};
use crate::error::ServerError;

#[derive(OpenApi)]
#[openapi(
    paths(agent_responses_get, agent_responses_post, migrate_workspace),
    components(schemas(
        AgentResponsesClientMessage,
        AgentConfigInput,
        MessageInput,
        ChatToolCall,
        ChatToolFunction,
        OpenAiErrorResponse,
        WorkspaceMigrationResponse
    ))
)]
pub struct AgentApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/agents/responses", get(agent_responses_get).post(agent_responses_post))
        .route("/agents/migrate", post(migrate_workspace))
}

#[derive(Debug, Deserialize)]
struct AgentResponsesQuery {
    transport: Option<String>,
    thread_id: Option<String>,
}

struct CommandResult {
    message: AgentResponsesServerMessage,
    subscribe_thread_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Error boundary — route-local OpenAI-canonical error shape.
//
// The global `ServerError::IntoResponse` is intentionally left untouched;
// `AgentCompatError` wraps it and renders `{"error":{message,type,param,code}}`
// per the OpenAI Responses spec for the `/v1/agents/responses` routes only.
// ---------------------------------------------------------------------------

/// Route-local error wrapper that renders the OpenAI-compatible error body
/// for the `/v1/agents/responses` HTTP handlers.
struct AgentCompatError(ServerError);

impl From<ServerError> for AgentCompatError {
    fn from(error: ServerError) -> Self {
        AgentCompatError(error)
    }
}

impl From<AppCoreError> for AgentCompatError {
    fn from(error: AppCoreError) -> Self {
        AgentCompatError(ServerError::from(error))
    }
}

impl IntoResponse for AgentCompatError {
    fn into_response(self) -> Response {
        let (status, error_type, code, message) = map_server_error(self.0);
        render_openai_error(status, error_type, code, message, None)
    }
}

/// Map a [`ServerError`] onto the OpenAI error taxonomy.
fn map_server_error(error: ServerError) -> (StatusCode, &'static str, &'static str, String) {
    match error {
        ServerError::BadRequest(message) | ServerError::RequestValidationFailed(message) => {
            (StatusCode::BAD_REQUEST, "invalid_request_error", "invalid_request", message)
        }
        ServerError::BadRequestData { message, .. } => {
            (StatusCode::BAD_REQUEST, "invalid_request_error", "invalid_request", message)
        }
        ServerError::NotFound(message) => {
            (StatusCode::NOT_FOUND, "invalid_request_error", "not_found", message)
        }
        ServerError::Forbidden(message) => {
            (StatusCode::FORBIDDEN, "invalid_request_error", "forbidden", message)
        }
        ServerError::Conflict(message) => {
            (StatusCode::CONFLICT, "invalid_request_error", "conflict", message)
        }
        ServerError::TooManyRequests(message) => {
            (StatusCode::TOO_MANY_REQUESTS, "invalid_request_error", "too_many_requests", message)
        }
        ServerError::BackendNotReady(message) => {
            (StatusCode::SERVICE_UNAVAILABLE, "invalid_request_error", "backend_not_ready", message)
        }
        ServerError::NotImplemented(message) => {
            (StatusCode::NOT_IMPLEMENTED, "invalid_request_error", "not_implemented", message)
        }
        ServerError::RuntimeFailure { message, .. } => {
            (StatusCode::INTERNAL_SERVER_ERROR, "server_error", "internal_error", message)
        }
        ServerError::Runtime(_) | ServerError::Database(_) | ServerError::Internal(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "server_error",
            "internal_error",
            "internal server error".to_owned(),
        ),
    }
}

/// Render an OpenAI-canonical error response body.
fn render_openai_error(
    status: StatusCode,
    error_type: &str,
    code: &str,
    message: String,
    param: Option<String>,
) -> Response {
    let body = OpenAiErrorResponse {
        error: OpenAiError {
            message,
            error_type: error_type.to_owned(),
            param,
            code: Some(code.to_owned()),
            i18n: None,
        },
    };
    (status, Json(body)).into_response()
}

/// Wrap an [`OpenAiError`] in the canonical streaming error envelope
/// `{"type":"error","error":{...}}` used by SSE and WS frames.
fn openai_error_frame(error: OpenAiError) -> serde_json::Value {
    let error_value = serde_json::to_value(&error).expect("OpenAiError serializable");
    serde_json::json!({ "type": "error", "error": error_value })
}

/// Build a `{"type":"error","error":{...}}` frame from a [`ServerError`].
///
/// Uses [`ServerError::agent_code_message`] so the WS transport retains the
/// slab-localized `i18n` payload (the HTTP path discards it via
/// [`map_server_error`] to stay strictly OpenAI-canonical).
fn server_error_to_frame(error: ServerError) -> serde_json::Value {
    let (code, message, i18n) = error.agent_code_message();
    openai_error_frame(OpenAiError {
        message,
        error_type: openai_error_type_for_code(&code).to_owned(),
        param: None,
        code: Some(code),
        i18n: Some(i18n),
    })
}

/// Classify a slab agent error code into an OpenAI error `type`.
fn openai_error_type_for_code(code: &str) -> &'static str {
    match code {
        "internal_error" | "runtime_failure" => "server_error",
        c if c.starts_with("runtime_") => "server_error",
        _ => "invalid_request_error",
    }
}

// ---------------------------------------------------------------------------
// HTTP handlers
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/v1/agents/responses",
    tag = "agents",
    params(
        ("transport" = Option<String>, Query, description = "Use `sse` for the fallback event stream"),
        ("thread_id" = Option<String>, Query, description = "Agent thread ID for SSE fallback")
    ),
    responses(
        (status = 101, description = "WebSocket upgrade for bidirectional agent responses"),
        (status = 200, description = "SSE fallback stream of canonical Responses server events"),
        (status = 400, description = "Bad request", body = OpenAiErrorResponse),
    )
)]
async fn agent_responses_get(
    State(service): State<AgentService>,
    Query(query): Query<AgentResponsesQuery>,
    headers: HeaderMap,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Result<Response, AgentCompatError> {
    if let Ok(ws) = ws {
        return Ok(ws
            .on_upgrade(move |socket| agent_responses_socket(socket, service))
            .into_response());
    }

    if query.transport.as_deref() != Some("sse") {
        return Err(AgentCompatError(ServerError::BadRequest(
            "GET /v1/agents/responses requires a websocket upgrade or transport=sse".into(),
        )));
    }

    let Some(thread_id) = query.thread_id.filter(|value| !value.trim().is_empty()) else {
        return Err(AgentCompatError(ServerError::BadRequest(
            "thread_id is required for SSE fallback".into(),
        )));
    };
    let last_event_id = parse_last_event_id(&headers);
    Ok(agent_events_sse(service, thread_id, last_event_id))
}

#[utoipa::path(
    post,
    path = "/v1/agents/responses",
    tag = "agents",
    request_body = AgentResponsesClientMessage,
    responses(
        (status = 200, description = "OpenAI Responses-canonical Response object"),
        (status = 400, description = "Bad request", body = OpenAiErrorResponse),
        (status = 404, description = "Thread not found", body = OpenAiErrorResponse),
        (status = 429, description = "Thread is already running", body = OpenAiErrorResponse),
        (status = 500, description = "Internal error", body = OpenAiErrorResponse),
    )
)]
async fn agent_responses_post(
    State(service): State<AgentService>,
    ValidatedJson(command): ValidatedJson<AgentResponsesClientMessage>,
) -> Result<Json<slab_proto::openai::Response>, AgentCompatError> {
    match command {
        AgentResponsesClientMessage::ResponseCreate { session_id, config, messages, .. } => {
            let model = config.model.clone().unwrap_or_default();
            let agent_messages = messages.into_iter().map(Into::into).collect();
            let thread_id = service.spawn(session_id, (*config).into(), agent_messages).await?;
            let subscription = service.subscribe_events(&thread_id);
            let response = build_response(AdapterInput {
                response_id: &thread_id,
                model: &model,
                created_at_unix: Utc::now().timestamp() as f64,
                service_tier: None,
                envelopes: &subscription.replay,
                ..Default::default()
            });
            Ok(Json(response))
        }
        _ => Err(AgentCompatError(ServerError::BadRequest(
            "POST /v1/agents/responses only accepts response.create commands; use the WebSocket for other actions"
                .to_owned(),
        ))),
    }
}

#[utoipa::path(
    post,
    path = "/v1/agents/migrate",
    tag = "agents",
    responses(
        (status = 200, description = "Active threads interrupted + project-scoped snapshot written", body = WorkspaceMigrationResponse),
        (status = 400, description = "No active workspace to migrate"),
        (status = 500, description = "Backend error"),
    )
)]
async fn migrate_workspace(
    State(state): State<Arc<AppState>>,
) -> Result<Json<WorkspaceMigrationResponse>, ServerError> {
    let config = &state.context.config;
    let workspace_root = WorkspaceService::workspace_root_from_config(config)
        .ok_or_else(|| ServerError::BadRequest("no active workspace to migrate".into()))?;
    let snapshot_dir = PathBuf::from(&config.session_state_dir);
    let outcome =
        state.services.agent.prepare_workspace_migration(&workspace_root, &snapshot_dir).await?;
    Ok(Json(outcome.into()))
}

// ---------------------------------------------------------------------------
// WebSocket transport
// ---------------------------------------------------------------------------

async fn agent_responses_socket(socket: WebSocket, service: AgentService) {
    if let Err(error) = run_agent_responses_socket(socket, service).await {
        tracing::warn!(error = %error, "agent responses websocket ended");
    }
}

async fn run_agent_responses_socket(
    socket: WebSocket,
    service: AgentService,
) -> Result<(), String> {
    let (mut sender, mut receiver) = socket.split();
    let mut active_thread_id: Option<String> = None;
    let mut active_events: Option<broadcast::Receiver<AgentEventEnvelope>> = None;
    let mut stream_ctx: Option<Arc<Mutex<StreamCtx>>> = None;

    loop {
        // Snapshot the current stream context so the live-event branch can use it
        // without mutably borrowing `stream_ctx` inside the `select!` body.
        let event_ctx = stream_ctx.clone();

        tokio::select! {
            message = receiver.next() => {
                let Some(message) = message else {
                    break;
                };
                let message = message.map_err(|error| format!("websocket receive failed: {error}"))?;
                let payload = match message {
                    Message::Text(payload) => payload,
                    Message::Close(_) => {
                        break;
                    }
                    _ => {
                        continue;
                    }
                };
                let command = match parse_client_message(&payload) {
                    Ok(command) => command,
                    Err(error) => {
                        let frame = openai_error_frame(OpenAiError {
                            message: error.clone(),
                            error_type: "invalid_request_error".to_owned(),
                            param: None,
                            code: Some("bad_request".to_owned()),
                            i18n: None,
                        });
                        send_serialized(&mut sender, serialize_json(&frame)).await?;
                        continue;
                    }
                };
                match handle_agent_command(&service, command).await {
                    Ok(result) => {
                        let CommandResult { message, subscribe_thread_id } = result;
                        if let Some(thread_id) = subscribe_thread_id.as_deref() {
                            let already_subscribed =
                                active_thread_id.as_deref() == Some(thread_id)
                                    && active_events.is_some();
                            if !already_subscribed {
                                let subscription = service.subscribe_events(thread_id);
                                active_thread_id = Some(thread_id.to_owned());
                                active_events = Some(subscription.receiver);
                                let ctx = stream_ctx.get_or_insert_with(|| {
                                    Arc::new(Mutex::new(StreamCtx::new(
                                        thread_id.to_owned(),
                                        thread_id.to_owned(),
                                        Utc::now().timestamp() as f64,
                                        None,
                                    )))
                                });
                                send_ws_replay(
                                    &mut sender,
                                    &subscription.replay,
                                    Arc::clone(ctx),
                                )
                                .await?;
                            }
                        }
                        send_server_message(&mut sender, &message).await?;
                    }
                    Err(error) => {
                        let frame = server_error_to_frame(error);
                        send_serialized(&mut sender, serialize_json(&frame)).await?;
                    }
                }
            }
            event = recv_active_event(&mut active_events), if active_events.is_some() => {
                match event {
                    Some(Ok(envelope)) => {
                        let Some(ctx) = event_ctx.as_ref() else {
                            continue;
                        };
                        send_ws_envelope_events(&mut sender, &envelope, Arc::clone(ctx)).await?;
                    }
                    Some(Err(broadcast::error::RecvError::Lagged(_))) => {
                        let frame = openai_error_frame(OpenAiError {
                            message: "agent event stream lagged; some events may have been dropped"
                                .to_owned(),
                            error_type: "server_error".to_owned(),
                            param: None,
                            code: Some("stream_lagged".to_owned()),
                            i18n: None,
                        });
                        send_serialized(&mut sender, serialize_json(&frame)).await?;
                    }
                    Some(Err(broadcast::error::RecvError::Closed)) | None => {
                        active_events = None;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn recv_active_event(
    receiver: &mut Option<broadcast::Receiver<AgentEventEnvelope>>,
) -> Option<Result<AgentEventEnvelope, broadcast::error::RecvError>> {
    match receiver {
        Some(receiver) => Some(receiver.recv().await),
        None => None,
    }
}

fn parse_client_message(payload: &str) -> Result<AgentResponsesClientMessage, String> {
    let command = serde_json::from_str::<AgentResponsesClientMessage>(payload)
        .map_err(|error| format!("invalid agent responses message: {error}"))?;
    validate(command).map_err(|error| error.to_string())
}

async fn handle_agent_command(
    service: &AgentService,
    command: AgentResponsesClientMessage,
) -> Result<CommandResult, ServerError> {
    let action = command.action();
    let request_id = command.request_id().map(str::to_owned);

    match command {
        AgentResponsesClientMessage::SessionRestore { session_id, .. } => {
            let restored = service.restore_session(&session_id).await?;
            let subscribe_thread_id = restored.thread.as_ref().map(|thread| thread.id.clone());
            let message = AgentResponsesServerMessage::SessionRestored {
                request_id,
                session_id,
                thread: restored.thread.map(Into::into),
                messages: restored.messages.into_iter().map(Into::into).collect(),
            };
            Ok(CommandResult { message, subscribe_thread_id })
        }
        AgentResponsesClientMessage::ResponseCreate { session_id, config, messages, .. } => {
            let messages = messages.into_iter().map(Into::into).collect();
            let thread_id = service.spawn(session_id, (*config).into(), messages).await?;
            Ok(CommandResult {
                message: AgentResponsesServerMessage::Ack {
                    request_id,
                    action,
                    accepted: true,
                    thread_id: Some(thread_id.clone()),
                    status: Some(AgentStatusValue::Pending),
                    delivered: None,
                },
                subscribe_thread_id: Some(thread_id),
            })
        }
        AgentResponsesClientMessage::Input { thread_id, content, .. } => {
            service.send_input(&thread_id, content).await?;
            Ok(CommandResult {
                message: AgentResponsesServerMessage::Ack {
                    request_id,
                    action,
                    accepted: true,
                    thread_id: Some(thread_id.clone()),
                    status: None,
                    delivered: None,
                },
                subscribe_thread_id: Some(thread_id),
            })
        }
        AgentResponsesClientMessage::ApprovalResolve { thread_id, call_id, approved, .. } => {
            let delivered = service.approve_call(&thread_id, &call_id, approved);
            Ok(CommandResult {
                message: AgentResponsesServerMessage::Ack {
                    request_id,
                    action,
                    accepted: delivered,
                    thread_id: Some(thread_id.clone()),
                    status: None,
                    delivered: Some(delivered),
                },
                subscribe_thread_id: Some(thread_id),
            })
        }
        AgentResponsesClientMessage::Interrupt { thread_id, .. } => {
            service.interrupt(&thread_id).await?;
            Ok(CommandResult {
                message: AgentResponsesServerMessage::Ack {
                    request_id,
                    action,
                    accepted: true,
                    thread_id: Some(thread_id.clone()),
                    status: Some(AgentStatusValue::Interrupting),
                    delivered: None,
                },
                subscribe_thread_id: Some(thread_id),
            })
        }
        AgentResponsesClientMessage::Shutdown { thread_id, .. } => {
            service.shutdown(&thread_id).await?;
            Ok(CommandResult {
                message: AgentResponsesServerMessage::Ack {
                    request_id,
                    action,
                    accepted: true,
                    thread_id: Some(thread_id.clone()),
                    status: Some(AgentStatusValue::Shutdown),
                    delivered: None,
                },
                subscribe_thread_id: Some(thread_id),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// SSE transport — canonical Responses server events via `envelope_to_events`.
// ---------------------------------------------------------------------------

fn agent_events_sse(
    service: AgentService,
    thread_id: String,
    last_event_id: Option<u64>,
) -> Response {
    let subscription = service.subscribe_events(&thread_id);
    let created_at = Utc::now().timestamp() as f64;
    let mut ctx = StreamCtx::new(thread_id.clone(), thread_id.clone(), created_at, None);

    // Replay is processed synchronously so the live stream can take ownership
    // of the (now-seeded) `StreamCtx` — no locking needed across the stream.
    let replay_events: Vec<Event> = subscription
        .replay
        .into_iter()
        .filter(|env| should_replay_event(last_event_id, env.id))
        .flat_map(|env| {
            let id = env.id;
            envelope_to_events(&env, &mut ctx)
                .into_iter()
                .map(move |event| Event::default().id(id.to_string()).data(serialize_json(&event)))
                .collect::<Vec<_>>()
        })
        .collect();

    let replay = stream::iter(replay_events.into_iter().map(Ok::<Event, Infallible>));

    let live = BroadcastStream::new(subscription.receiver).flat_map(move |msg| {
        let events: Vec<Event> = match msg {
            Ok(env) => {
                let id = env.id;
                envelope_to_events(&env, &mut ctx)
                    .into_iter()
                    .map(move |event| {
                        Event::default().id(id.to_string()).data(serialize_json(&event))
                    })
                    .collect()
            }
            Err(_) => {
                vec![Event::default().data(serialize_json(&openai_error_frame(OpenAiError {
                    message:
                        "agent event stream lagged; some events may have been dropped".to_owned(),
                    error_type: "server_error".to_owned(),
                    param: None,
                    code: Some("stream_lagged".to_owned()),
                    i18n: None,
                })))]
            }
        };
        stream::iter(events.into_iter().map(Ok::<Event, Infallible>))
    });

    Sse::new(Box::pin(replay.chain(live))).keep_alive(KeepAlive::default()).into_response()
}

// ---------------------------------------------------------------------------
// Shared WS helpers
// ---------------------------------------------------------------------------

/// Send replay envelopes as canonical WS frames, sharing the stream context.
async fn send_ws_replay<S>(
    sender: &mut S,
    replay: &[AgentEventEnvelope],
    ctx: Arc<Mutex<StreamCtx>>,
) -> Result<(), String>
where
    S: futures::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    for envelope in replay {
        send_ws_envelope_events(sender, envelope, Arc::clone(&ctx)).await?;
    }
    Ok(())
}

/// Convert one slab envelope into 0..N canonical events and send each as a WS
/// text frame. Locks the shared `StreamCtx` only for the synchronous
/// `envelope_to_events` call — no await while holding the guard.
async fn send_ws_envelope_events<S>(
    sender: &mut S,
    envelope: &AgentEventEnvelope,
    ctx: Arc<Mutex<StreamCtx>>,
) -> Result<(), String>
where
    S: futures::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let events = {
        let mut guard = ctx.lock().map_err(|error| format!("stream ctx poisoned: {error}"))?;
        envelope_to_events(envelope, &mut guard)
    };
    for event in events {
        send_serialized(sender, serialize_json(&event)).await?;
    }
    Ok(())
}

async fn send_server_message<S>(
    sender: &mut S,
    message: &AgentResponsesServerMessage,
) -> Result<(), String>
where
    S: futures::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    send_serialized(sender, serialize_json(message)).await
}

async fn send_serialized<S>(sender: &mut S, payload: String) -> Result<(), String>
where
    S: futures::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    sender
        .send(Message::Text(payload.into()))
        .await
        .map_err(|error| format!("websocket send failed: {error}"))
}

fn should_replay_event(last_event_id: Option<u64>, event_id: u64) -> bool {
    match last_event_id {
        Some(last_event_id) => event_id > last_event_id,
        None => true,
    }
}

fn parse_last_event_id(headers: &HeaderMap) -> Option<u64> {
    headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse().ok())
}

fn serialize_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| {
        r#"{"type":"error","error":{"message":"failed to serialize agent message","type":"server_error","code":"serialization_failed"}}"#
            .to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::{
        AgentApi, AgentCompatError, map_server_error, parse_client_message, should_replay_event,
    };
    use crate::error::ServerError;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use utoipa::OpenApi;

    #[test]
    fn parses_typed_client_message() {
        let command = parse_client_message(
            r#"{"type":"agent.input","request_id":"r1","thread_id":"thread-1","content":"hello"}"#,
        )
        .expect("valid command");

        assert_eq!(command.request_id(), Some("r1"));
    }

    #[test]
    fn rejects_blank_client_message_fields() {
        let error = parse_client_message(
            r#"{"type":"agent.input","request_id":"r1","thread_id":" ","content":"hello"}"#,
        )
        .expect_err("invalid command");

        assert!(error.contains("thread_id"));
    }

    #[test]
    fn last_event_id_replays_only_later_events() {
        assert!(!should_replay_event(Some(7), 7));
        assert!(should_replay_event(Some(7), 8));
        assert!(should_replay_event(None, 0));
    }

    #[test]
    fn openapi_only_publishes_responses_agent_route() {
        let openapi = serde_json::to_value(AgentApi::openapi()).expect("serialize openapi");
        let paths = openapi["paths"].as_object().expect("paths");

        assert!(paths.contains_key("/v1/agents/responses"));
        assert!(!paths.contains_key("/v1/agents/spawn"));
        assert!(!paths.contains_key("/v1/agents/{id}/events"));
    }

    #[tokio::test]
    async fn agent_compat_error_renders_openai_shape() {
        let response = AgentCompatError(ServerError::BadRequest("x".to_owned())).into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("body");
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("json body");

        assert_eq!(payload["error"]["message"], "x");
        assert_eq!(payload["error"]["type"], "invalid_request_error");
        assert_eq!(payload["error"]["code"], "invalid_request");
        // `param` is `skip_serializing_if = Option::is_none`, so absent when `None`.
        assert!(
            payload["error"].get("param").is_none() || payload["error"]["param"].is_null(),
            "param should be absent or null, got: {payload}"
        );
        assert!(payload.get("code").is_none(), "top-level slab code must not leak");
        assert!(payload.get("i18n").is_none(), "top-level i18n must not leak");
    }

    #[test]
    fn map_server_error_classifies_common_variants() {
        let (status, type_, code, _) =
            map_server_error(ServerError::NotFound("missing".to_owned()));
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(type_, "invalid_request_error");
        assert_eq!(code, "not_found");

        let (status, type_, code, _) = map_server_error(ServerError::Conflict("busy".to_owned()));
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(type_, "invalid_request_error");
        assert_eq!(code, "conflict");

        let (status, type_, code, _) = map_server_error(ServerError::Internal("boom".to_owned()));
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(type_, "server_error");
        assert_eq!(code, "internal_error");
    }

    /// Exercises the `envelope_to_events` signature end-to-end. Panics until the
    /// adapter teammate fills in the streaming state machine.
    #[test]
    #[ignore = "pending adapter envelope_to_events impl"]
    fn envelope_to_events_emits_canonical_frames() {
        use crate::api::v1::agent::openai_compat::{StreamCtx, envelope_to_events};

        let envelope = slab_app_core::infra::agent::event_hub::AgentEventEnvelope {
            id: 1,
            event: slab_agent::TurnEvent::Response {
                turn_index: Some(0),
                event: slab_agent::AgentEventKind::ResponseOutputTextDone {
                    item_id: "msg-1".to_owned(),
                    output_index: 0,
                    content_index: 0,
                    text: "hello".to_owned(),
                    artifact_refs: vec![],
                    reason: None,
                    phase: None,
                },
            },
        };
        let mut ctx = StreamCtx::new("resp_1".to_owned(), "gpt-5.3-codex".to_owned(), 0.0, None);
        let events = envelope_to_events(&envelope, &mut ctx);
        assert!(!events.is_empty(), "adapter should emit at least one canonical event");
    }
}
