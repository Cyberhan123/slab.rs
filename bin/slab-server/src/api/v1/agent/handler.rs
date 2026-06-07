//! HTTP and WebSocket handlers for `/v1/agents/responses`.

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use futures::SinkExt;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use slab_agent::{AgentEventKind, AgentStreamEvent, TurnEvent};
use slab_app_core::context::AppState;
use slab_app_core::domain::services::AgentService;
use slab_app_core::infra::agent_event_hub::AgentEventEnvelope;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use utoipa::OpenApi;

use crate::api::v1::agent::schema::{
    AgentConfigInput, AgentResponsesAction, AgentResponsesClientMessage,
    AgentResponsesServerMessage, AgentStatusValue, AgentThreadMessageResponse, AgentThreadResponse,
    MessageInput,
};
use crate::api::v1::chat::schema::{ChatToolCall, ChatToolFunction};
use crate::api::validation::{ValidatedJson, validate};
use crate::error::ServerError;

#[derive(OpenApi)]
#[openapi(
    paths(agent_responses_get, agent_responses_post),
    components(schemas(
        AgentResponsesClientMessage,
        AgentResponsesServerMessage,
        AgentResponsesAction,
        AgentThreadResponse,
        AgentThreadMessageResponse,
        AgentConfigInput,
        MessageInput,
        AgentStatusValue,
        ChatToolCall,
        ChatToolFunction,
    ))
)]
pub struct AgentApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/agents/responses", get(agent_responses_get).post(agent_responses_post))
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
        (status = 200, description = "SSE fallback stream of agent response events"),
        (status = 400, description = "Bad request"),
    )
)]
async fn agent_responses_get(
    State(service): State<AgentService>,
    Query(query): Query<AgentResponsesQuery>,
    headers: HeaderMap,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Result<Response, ServerError> {
    if let Ok(ws) = ws {
        return Ok(ws
            .on_upgrade(move |socket| agent_responses_socket(socket, service))
            .into_response());
    }

    if query.transport.as_deref() != Some("sse") {
        return Err(ServerError::BadRequest(
            "GET /v1/agents/responses requires a websocket upgrade or transport=sse".into(),
        ));
    }

    let Some(thread_id) = query.thread_id.filter(|value| !value.trim().is_empty()) else {
        return Err(ServerError::BadRequest("thread_id is required for SSE fallback".into()));
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
        (status = 200, description = "Agent response command accepted", body = AgentResponsesServerMessage),
        (status = 400, description = "Bad request"),
        (status = 404, description = "Thread not found"),
        (status = 429, description = "Thread is already running"),
        (status = 500, description = "Internal error"),
    )
)]
async fn agent_responses_post(
    State(service): State<AgentService>,
    ValidatedJson(command): ValidatedJson<AgentResponsesClientMessage>,
) -> Result<Json<AgentResponsesServerMessage>, ServerError> {
    Ok(Json(handle_agent_command(&service, command).await?.message))
}

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

    loop {
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
                        send_server_message(
                            &mut sender,
                            &AgentResponsesServerMessage::Error {
                                request_id: None,
                                code: "bad_request".to_owned(),
                                message: error,
                                thread_id: active_thread_id.clone(),
                            },
                        )
                        .await?;
                        continue;
                    }
                };
                let request_id = command.request_id().map(str::to_owned);
                match handle_agent_command(&service, command).await {
                    Ok(result) => {
                        if let Some(thread_id) = result.subscribe_thread_id.as_deref() {
                            let already_subscribed =
                                active_thread_id.as_deref() == Some(thread_id)
                                    && active_events.is_some();
                            if !already_subscribed {
                                let subscription = service.subscribe_events(thread_id);
                                active_thread_id = Some(thread_id.to_owned());
                                active_events = Some(subscription.receiver);
                                send_replay(&mut sender, thread_id, subscription.replay, None)
                                    .await?;
                            }
                        }
                        send_server_message(&mut sender, &result.message).await?;
                    }
                    Err(error) => {
                        let message = server_error_message(error, request_id, active_thread_id.clone());
                        send_server_message(&mut sender, &message).await?;
                    }
                }
            }
            event = recv_active_event(&mut active_events), if active_events.is_some() => {
                match event {
                    Some(Ok(envelope)) => {
                        let Some(thread_id) = active_thread_id.as_deref() else {
                            continue;
                        };
                        send_agent_event(&mut sender, thread_id, &envelope).await?;
                    }
                    Some(Err(broadcast::error::RecvError::Lagged(_))) => {
                        let Some(thread_id) = active_thread_id.as_deref() else {
                            continue;
                        };
                        let event = AgentStreamEvent::new(
                            thread_id.to_owned(),
                            None,
                            0,
                            AgentEventKind::AgentStreamLagged,
                        );
                        send_serialized(&mut sender, serialize_json(&event)).await?;
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
            let thread_id = service.spawn(session_id, config.into(), messages).await?;
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

fn agent_events_sse(
    service: AgentService,
    thread_id: String,
    last_event_id: Option<u64>,
) -> Response {
    let subscription = service.subscribe_events(&thread_id);
    let replay_id = thread_id.clone();
    let replay = stream::iter(subscription.replay.into_iter().filter_map(move |envelope| {
        should_replay_event(last_event_id, envelope.id).then(|| {
            Ok::<Event, Infallible>(
                Event::default().id(envelope.id.to_string()).data(turn_event_to_sse_data(
                    &replay_id,
                    envelope.id,
                    &envelope.event,
                )),
            )
        })
    }));
    let live_id = thread_id.clone();
    let live = BroadcastStream::new(subscription.receiver).map(move |msg| {
        let event = match msg {
            Ok(envelope) => {
                let data = turn_event_to_sse_data(&live_id, envelope.id, &envelope.event);
                Event::default().id(envelope.id.to_string()).data(data)
            }
            Err(_) => Event::default().data(serialize_json(&AgentStreamEvent::new(
                live_id.clone(),
                None,
                0,
                AgentEventKind::AgentStreamLagged,
            ))),
        };
        Ok::<Event, Infallible>(event)
    });

    Sse::new(Box::pin(replay.chain(live))).keep_alive(KeepAlive::default()).into_response()
}

async fn send_replay<S>(
    sender: &mut S,
    thread_id: &str,
    replay: Vec<AgentEventEnvelope>,
    last_event_id: Option<u64>,
) -> Result<(), String>
where
    S: futures::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    for envelope in replay {
        if should_replay_event(last_event_id, envelope.id) {
            send_agent_event(sender, thread_id, &envelope).await?;
        }
    }
    Ok(())
}

async fn send_agent_event<S>(
    sender: &mut S,
    thread_id: &str,
    envelope: &AgentEventEnvelope,
) -> Result<(), String>
where
    S: futures::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let event = turn_event_to_agent_stream_event(thread_id, envelope.id, &envelope.event);
    send_serialized(sender, serialize_json(&event)).await
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
        r#"{"type":"agent.error","code":"serialization_failed","message":"failed to serialize agent message"}"#.to_owned()
    })
}

fn turn_event_to_agent_stream_event(
    thread_id: &str,
    sequence_number: u64,
    event: &TurnEvent,
) -> AgentStreamEvent {
    let TurnEvent::Response { turn_index, event } = event;
    AgentStreamEvent::new(thread_id.to_owned(), *turn_index, sequence_number, event.clone())
}

fn turn_event_to_sse_data(thread_id: &str, sequence_number: u64, event: &TurnEvent) -> String {
    serialize_json(&turn_event_to_agent_stream_event(thread_id, sequence_number, event))
}

fn server_error_message(
    error: ServerError,
    request_id: Option<String>,
    thread_id: Option<String>,
) -> AgentResponsesServerMessage {
    let (code, message) = match error {
        ServerError::NotFound(message) => ("not_found", message),
        ServerError::BadRequest(message) => ("bad_request", message),
        ServerError::BadRequestData { message, .. } => ("bad_request", message),
        ServerError::Conflict(message) => ("conflict", message),
        ServerError::BackendNotReady(message) => ("backend_not_ready", message),
        ServerError::NotImplemented(message) => ("not_implemented", message),
        ServerError::TooManyRequests(message) => ("too_many_requests", message),
        ServerError::Runtime(_) | ServerError::Database(_) | ServerError::Internal(_) => {
            ("internal_error", "internal server error".to_owned())
        }
    };

    AgentResponsesServerMessage::Error { request_id, code: code.to_owned(), message, thread_id }
}

#[cfg(test)]
mod tests {
    use super::{AgentApi, parse_client_message, should_replay_event, turn_event_to_sse_data};
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

    #[test]
    fn turn_event_serializes_response_style_envelope() {
        let data = turn_event_to_sse_data(
            "thread-1",
            7,
            &slab_agent::TurnEvent::Response {
                turn_index: Some(2),
                event: slab_agent::AgentEventKind::ResponseOutputTextDone {
                    item_id: "item-1".to_owned(),
                    output_index: 0,
                    content_index: 0,
                    text: "done".to_owned(),
                },
            },
        );
        let value: serde_json::Value = serde_json::from_str(&data).expect("json");

        assert_eq!(value["thread_id"], "thread-1");
        assert_eq!(value["turn_index"], 2);
        assert_eq!(value["sequence_number"], 7);
        assert_eq!(value["type"], "response.output_text.done");
        assert_eq!(value["text"], "done");
    }

    #[test]
    fn completed_event_does_not_duplicate_output_text() {
        let data = turn_event_to_sse_data(
            "thread-1",
            8,
            &slab_agent::TurnEvent::Response {
                turn_index: Some(2),
                event: slab_agent::AgentEventKind::ResponseCompleted {
                    response: slab_agent::AgentResponseRef {
                        id: "thread-1".to_owned(),
                        status: slab_agent::ThreadStatus::Completed,
                    },
                },
            },
        );
        let value: serde_json::Value = serde_json::from_str(&data).expect("json");

        assert_eq!(value["thread_id"], "thread-1");
        assert_eq!(value["turn_index"], 2);
        assert_eq!(value["sequence_number"], 8);
        assert_eq!(value["type"], "response.completed");
        assert!(value.get("text").is_none());
    }
}
