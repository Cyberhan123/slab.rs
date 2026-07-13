//! HTTP and WebSocket handlers for `/v1/agents/responses`.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use futures::SinkExt;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use slab_app_core::context::AppState;
use slab_app_core::domain::services::ResponseService;
use slab_app_core::error::AppCoreError;
use slab_app_core::schemas::chat::{OpenAiError, OpenAiErrorResponse};
use slab_proto::openai::{ResponseCompletedEvent, ResponseCompletedType, ResponsesServerEvent};
use utoipa::OpenApi;

use crate::api::v1::agent::openai_compat::{
    StreamCtx, StreamFrame, build_terminal_event, envelope_to_events,
};
use crate::api::v1::agent::schema::{
    AgentConfigInput, MessageInput, OpenAICreateRequest, OpenAIReasoningInput, OpenAITextInput,
};
use crate::api::v1::chat::schema::{ChatToolCall, ChatToolFunction};
use crate::error::ServerError;

#[derive(OpenApi)]
#[openapi(
    paths(
        agent_responses_get,
        agent_responses_post,
        crate::api::v1::agent::harness::agent_harness
    ),
    components(schemas(
        AgentConfigInput,
        MessageInput,
        OpenAICreateRequest,
        OpenAIReasoningInput,
        OpenAITextInput,
        ChatToolCall,
        ChatToolFunction,
        OpenAiErrorResponse
    ))
)]
pub struct AgentApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/agents/responses", get(agent_responses_get).post(agent_responses_post))
        .route("/agents/harness", get(crate::api::v1::agent::harness::agent_harness))
}

#[derive(Debug, Deserialize)]
struct AgentResponsesQuery {
    transport: Option<String>,
    thread_id: Option<String>,
    /// Slab session id for the canonical (openai-protocol) WS mode. Browsers
    /// cannot set headers on a WebSocket handshake, so the SDK carries the
    /// session as `?token=` (the slab-dialect mode reads it from the first
    /// client message body instead).
    #[serde(default)]
    token: Option<String>,
}

/// Parsed inbound WS command. The responses socket only accepts canonical
/// `response.create` events whose body is an [`OpenAICreateRequest`].
#[derive(Debug)]
enum InboundCommand {
    ResponseCreate(OpenAICreateRequest),
}

/// `response.create` client event over the canonical WS channel. The `type`
/// tag selects the arm; the rest is the standard OpenAI `CreateResponse` body,
/// reused via [`OpenAICreateRequest`] (the POST path's translators).
#[derive(Debug, Deserialize)]
struct OpenAIResponseCreateEvent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(flatten)]
    body: OpenAICreateRequest,
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
    State(service): State<ResponseService>,
    Query(query): Query<AgentResponsesQuery>,
    headers: HeaderMap,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Result<Response, AgentCompatError> {
    if let Ok(ws) = ws {
        // Keep accepting the historical `slab.responses` subprotocol as a
        // handshake signal; all websocket payloads are canonical Responses events.
        let is_canonical = is_canonical_ws_request(&headers);
        let ws = if is_canonical { ws.protocols(["slab.responses"]) } else { ws };
        let session_id = query
            .token
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| bearer_session_id(&headers));
        return Ok(ws
            .on_upgrade(move |socket| {
                agent_responses_socket(socket, service, session_id, is_canonical)
            })
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
    // Slice C2: `/responses` is single-shot per request — there is no in-flight
    // hub stream to resume. GET SSE reconstructs the finalized response from
    // the thread store and replays it as a terminal `response.completed`.
    Ok(agent_events_sse_resume(service, thread_id).await)
}

#[utoipa::path(
    post,
    path = "/v1/agents/responses",
    tag = "agents",
    // Standard OpenAI Responses `ResponseCreateParamsBase` body (consumed by
    // the official `openai` SDK). Not typed here because utoipa can't model the
    // `input` untagged string|items union; see `OpenAICreateRequest`.
    responses(
        (status = 200, description = "OpenAI Responses-canonical Response object; an SSE stream of ResponseStreamEvent when `stream: true`"),
        (status = 400, description = "Bad request", body = OpenAiErrorResponse),
        (status = 404, description = "Thread not found", body = OpenAiErrorResponse),
        (status = 429, description = "Thread is already running", body = OpenAiErrorResponse),
        (status = 500, description = "Internal error", body = OpenAiErrorResponse),
    )
)]
async fn agent_responses_post(
    State(service): State<ResponseService>,
    headers: HeaderMap,
    Json(req): Json<OpenAICreateRequest>,
) -> Result<Response, AgentCompatError> {
    let session_id = bearer_session_id(&headers);
    if req.stream.unwrap_or(false) {
        let model = req.model.clone().unwrap_or_default();
        let (response_id, frames) = service.stream_response(req, session_id).await?;
        Ok(agent_events_sse_from_frames(response_id, model, frames))
    } else {
        let response = service.create_response(req, session_id).await?;
        Ok(Json(response).into_response())
    }
}

/// Read the slab session id from an `Authorization: Bearer <session>` header
/// (the `openai` SDK is constructed with `apiKey: <session>`). Falls back to the
/// assistant default when absent.
fn bearer_session_id(headers: &HeaderMap) -> String {
    let default = "assistant-default".to_owned();
    let Some(value) = headers.get(axum::http::header::AUTHORIZATION).and_then(|h| h.to_str().ok())
    else {
        return default;
    };
    let trimmed = value.trim();
    let token = trimmed
        .strip_prefix("Bearer ")
        .or_else(|| trimmed.strip_prefix("bearer "))
        .unwrap_or(trimmed);
    let token = token.trim();
    if token.is_empty() { default } else { token.to_owned() }
}

// ---------------------------------------------------------------------------
// WebSocket transport
// ---------------------------------------------------------------------------

async fn agent_responses_socket(
    socket: WebSocket,
    service: ResponseService,
    session_id: String,
    is_canonical: bool,
) {
    if let Err(error) = run_agent_responses_socket(socket, service, session_id, is_canonical).await
    {
        tracing::warn!(error = %error, "agent responses websocket ended");
    }
}

async fn run_agent_responses_socket(
    socket: WebSocket,
    service: ResponseService,
    session_id: String,
    is_canonical: bool,
) -> Result<(), String> {
    let (mut sender, mut receiver) = socket.split();

    loop {
        let message = receiver.next().await;
        let Some(message) = message else { break };
        let message = message.map_err(|error| format!("websocket receive failed: {error}"))?;
        let payload = match message {
            Message::Text(payload) => payload,
            Message::Close(_) => break,
            _ => continue,
        };
        let req = match parse_client_message(&payload, is_canonical) {
            Ok(InboundCommand::ResponseCreate(req)) => req,
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

        let model = req.model.clone().unwrap_or_default();
        match service.stream_response(req, session_id.clone()).await {
            Ok((response_id, frames)) => {
                let ctx = Arc::new(Mutex::new(StreamCtx::new(
                    response_id,
                    model,
                    Utc::now().timestamp() as f64,
                    None,
                )));
                send_ws_frame_stream(&mut sender, frames, Arc::clone(&ctx)).await?;
            }
            Err(error) => {
                let frame = server_error_to_frame(error.into());
                send_serialized(&mut sender, serialize_json(&frame)).await?;
            }
        }
    }

    Ok(())
}

fn parse_client_message(payload: &str, _is_canonical: bool) -> Result<InboundCommand, String> {
    let event = serde_json::from_str::<OpenAIResponseCreateEvent>(payload)
        .map_err(|error| format!("invalid canonical response.create event: {error}"))?;
    if event.kind != "response.create" {
        return Err(format!(
            "responses websocket only accepts `response.create`; got type={:?}",
            event.kind
        ));
    }
    Ok(InboundCommand::ResponseCreate(event.body))
}

/// True when the WS client offered the historical `slab.responses` subprotocol.
fn is_canonical_ws_request(headers: &HeaderMap) -> bool {
    headers
        .get("sec-websocket-protocol")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(',').any(|proto| proto.trim() == "slab.responses"))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// SSE + WS framing — canonical Responses server events from synthesized frames.
// ---------------------------------------------------------------------------

/// Expand one synthesized [`StreamFrame`] into 0..N canonical wire events,
/// sharing the per-response [`StreamCtx`].
fn frame_to_events(frame: &StreamFrame, ctx: &mut StreamCtx) -> Vec<ResponsesServerEvent> {
    match frame {
        StreamFrame::Envelope(env) => envelope_to_events(env, ctx),
        StreamFrame::Terminal(kind) => build_terminal_event(ctx, kind),
    }
}

/// POST `stream:true` — frame the synthesized single-shot stream as SSE.
fn agent_events_sse_from_frames(
    response_id: String,
    model: String,
    frames: impl futures::Stream<Item = StreamFrame> + Send + 'static,
) -> Response {
    let created_at = Utc::now().timestamp() as f64;
    let mut ctx = StreamCtx::new(response_id, model, created_at, None);
    let events = frames.flat_map(move |frame| {
        let expanded: Vec<Event> = frame_to_events(&frame, &mut ctx)
            .into_iter()
            .map(|event| Event::default().data(serialize_json(&event)))
            .collect();
        stream::iter(expanded)
    });
    Sse::new(Box::pin(events.map(Ok::<_, Infallible>)))
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// GET SSE resume — `/responses` is single-shot per request, so reconstruct the
/// finalized response from the thread store and replay it as a terminal
/// `response.completed`.
async fn agent_events_sse_resume(service: ResponseService, thread_id: String) -> Response {
    let response = match service.get_response(&thread_id).await {
        Ok(response) => response,
        Err(error) => {
            let frame = openai_error_frame(OpenAiError {
                message: error.to_string(),
                error_type: "server_error".to_owned(),
                param: None,
                code: Some("not_found".to_owned()),
                i18n: None,
            });
            let event = Event::default().data(serialize_json(&frame));
            return Sse::new(Box::pin(stream::iter(vec![Ok::<_, Infallible>(event)])))
                .keep_alive(KeepAlive::default())
                .into_response();
        }
    };
    let event = ResponsesServerEvent::ResponseCompletedEvent(Box::new(
        ResponseCompletedEvent::new(ResponseCompletedType::ResponseCompleted, response, 0),
    ));
    let sse_event = Event::default().data(serialize_json(&event));
    Sse::new(Box::pin(stream::iter(vec![Ok::<_, Infallible>(sse_event)])))
        .keep_alive(KeepAlive::default())
        .into_response()
}

// ---------------------------------------------------------------------------
// Shared WS helpers
// ---------------------------------------------------------------------------

/// Drain the synthesized single-shot frame stream and send each expanded wire
/// event as a WS text frame. Locks the shared `StreamCtx` only for the
/// synchronous projection call — no await while holding the guard.
async fn send_ws_frame_stream<S, St>(
    sender: &mut S,
    frames: St,
    ctx: Arc<Mutex<StreamCtx>>,
) -> Result<(), String>
where
    S: futures::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
    St: futures::Stream<Item = StreamFrame> + Unpin,
{
    let mut frames = frames;
    while let Some(frame) = frames.next().await {
        let events = {
            let mut guard = ctx.lock().map_err(|error| format!("stream ctx poisoned: {error}"))?;
            frame_to_events(&frame, &mut guard)
        };
        for event in events {
            send_serialized(sender, serialize_json(&event)).await?;
        }
    }
    Ok(())
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

fn serialize_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| {
        r#"{"type":"error","error":{"message":"failed to serialize agent message","type":"server_error","code":"serialization_failed"}}"#
            .to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::{
        AgentApi, AgentCompatError, InboundCommand, map_server_error, parse_client_message,
    };
    use crate::error::ServerError;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use utoipa::OpenApi;

    #[test]
    fn parses_canonical_response_create_event() {
        let command = parse_client_message(
            r#"{"type":"response.create","model":"slab-llama","input":"hi","stream":true}"#,
            true,
        )
        .expect("valid canonical event");
        let InboundCommand::ResponseCreate(req) = command;

        assert_eq!(req.model.as_deref(), Some("slab-llama"));
        assert_eq!(req.input.as_str(), Some("hi"));
        assert!(req.stream.unwrap_or(false));
        assert!(req.previous_response_id.is_none());
    }

    #[test]
    fn canonical_mode_chains_previous_response_id() {
        let command = parse_client_message(
            r#"{"type":"response.create","model":"m","input":"more","previous_response_id":"thread-9"}"#,
            true,
        )
        .expect("valid canonical event");
        let InboundCommand::ResponseCreate(req) = command;

        assert_eq!(req.previous_response_id.as_deref(), Some("thread-9"));
    }

    #[test]
    fn canonical_mode_rejects_non_response_create_type() {
        let error =
            parse_client_message(r#"{"type":"agent.input","thread_id":"t","content":"hi"}"#, true)
                .expect_err("slab-dialect type should be rejected in canonical mode");

        assert!(error.contains("response.create"), "error should mention response.create: {error}");
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
