//! Bidirectional JSON-RPC 2.0 over a WebSocket-like stream.
//!
//! Transport-agnostic: the caller adapts its concrete socket (axum `WebSocket`,
//! `tokio-tungstenite`, etc.) to a [`futures::Sink`]/[`futures::Stream`] of
//! [`WsFrame`]. This module owns inbound dispatch (requests → [`RequestHandler`],
//! notifications → [`NotificationHandler`]) and outbound fan-out (drain a channel
//! of [`JSONRPCMessage`]s to the sink), reusing the concurrency-limit and
//! panic-catch shape of [`crate::host::serve_reader`] but supporting
//! client→server notifications and full-duplex traffic.
//!
//! Unlike [`crate::host::serve_reader`], this loop does NOT drop inbound
//! notifications and does not assume the host is the initiator — both peers may
//! send requests and notifications.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use futures::FutureExt;
use futures::{Sink, SinkExt, Stream, StreamExt};
use serde_json::Value;
use tokio::sync::{Semaphore, mpsc};
use tokio::task::JoinSet;

use crate::host::HostConfig;
use crate::host::{
    NotificationHandler, ParseWireMessageError, RequestHandler, fallback_error_id,
    parse_wire_message, serialize_wire_message,
};
use crate::{
    APPLICATION_ERROR, INVALID_REQUEST, JSONRPCError, JSONRPCErrorError, JSONRPCMessage,
    JSONRPCResponse, PARSE_ERROR,
};

/// A single decoded WebSocket frame, independent of any specific WS library.
#[derive(Debug, Clone)]
pub enum WsFrame {
    /// A UTF-8 text frame (JSON-RPC messages are always text).
    Text(String),
    /// A binary frame; interpreted as UTF-8 for JSON-RPC if encountered.
    Binary(Vec<u8>),
    /// A close frame; ends the session.
    Close,
}

impl WsFrame {
    /// Decode to JSON-RPC text if this is a text frame or a UTF-8 binary frame.
    /// Returns `None` for close frames (and non-UTF-8 binary).
    fn into_json_text(self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text),
            Self::Binary(bytes) => String::from_utf8(bytes).ok(),
            Self::Close => None,
        }
    }
}

/// Run a bidirectional JSON-RPC session over a WebSocket-like transport.
///
/// `outbound_tx` / `outbound_rx` are the two ends of a single multi-producer
/// channel that fans every server→client message (request responses emitted by
/// this loop, plus notifications/responses pushed by the caller — e.g. streamed
/// agent events) out to `sink`. The caller constructs the channel, hands a clone
/// of `outbound_tx` to anything that needs to push notifications, and passes
/// both ends here.
///
/// Returns `Ok(())` when the peer closes or the sink/stream errors.
#[allow(clippy::too_many_arguments)]
pub async fn serve_websocket<Rq, N, S, R, E>(
    handler: Arc<Rq>,
    notifications: Arc<N>,
    outbound_tx: mpsc::UnboundedSender<JSONRPCMessage>,
    outbound_rx: mpsc::UnboundedReceiver<JSONRPCMessage>,
    sink: S,
    stream: R,
    config: HostConfig,
) -> std::io::Result<()>
where
    Rq: RequestHandler,
    N: NotificationHandler,
    S: Sink<WsFrame, Error = E> + Unpin,
    R: Stream<Item = Result<WsFrame, E>> + Unpin,
    E: std::fmt::Debug,
{
    let mut outbound_rx = outbound_rx;
    let mut sink = sink;
    let mut stream = stream;

    let semaphore = Arc::new(Semaphore::new(config.concurrency_limit));
    let mut tasks: JoinSet<()> = JoinSet::new();

    loop {
        tokio::select! {
            maybe_outbound = outbound_rx.recv() => {
                let Some(message) = maybe_outbound else { break; };
                let line = match serialize_wire_message(&message) {
                    Ok(line) => line,
                    Err(error) => {
                        tracing::warn!(%error, "failed to serialize outbound json-rpc message");
                        continue;
                    }
                };
                if sink.send(WsFrame::Text(line)).await.is_err() {
                    break;
                }
            }
            maybe_frame = stream.next() => {
                let Some(frame) = maybe_frame else { break; };
                let frame = match frame {
                    Ok(frame) => frame,
                    Err(error) => {
                        tracing::debug!(?error, "websocket stream error");
                        break;
                    }
                };
                let Some(text) = frame.into_json_text() else { break };

                let message = match parse_wire_message(&text) {
                    Ok(message) => message,
                    Err(ParseWireMessageError::InvalidJson(error)) => {
                        let _ = outbound_tx.send(JSONRPCMessage::Error(JSONRPCError {
                            error: JSONRPCErrorError {
                                code: PARSE_ERROR,
                                data: None,
                                message: format!("invalid json-rpc payload: {error}"),
                            },
                            id: fallback_error_id(),
                        }));
                        continue;
                    }
                    Err(ParseWireMessageError::InvalidVersion(message)) => {
                        if let Some(id) = message.id {
                            let _ = outbound_tx.send(JSONRPCMessage::Error(JSONRPCError {
                                error: JSONRPCErrorError {
                                    code: INVALID_REQUEST,
                                    data: None,
                                    message: "jsonrpc must be `2.0`".to_owned(),
                                },
                                id,
                            }));
                        }
                        continue;
                    }
                };

                match message {
                    JSONRPCMessage::Request(request) => {
                        let Ok(permit) = Arc::clone(&semaphore).try_acquire_owned() else {
                            let _ = outbound_tx.send(JSONRPCMessage::Error(JSONRPCError {
                                error: JSONRPCErrorError {
                                    code: APPLICATION_ERROR,
                                    data: None,
                                    message: "request concurrency limit exceeded".to_owned(),
                                },
                                id: request.id,
                            }));
                            continue;
                        };
                        let handler = Arc::clone(&handler);
                        let outbound_tx = outbound_tx.clone();
                        let id = request.id;
                        let method = request.method;
                        let params = request.params.unwrap_or(Value::Null);
                        tasks.spawn(async move {
                            let _permit = permit;
                            let result = AssertUnwindSafe(handler.handle_request(method.clone(), params))
                                .catch_unwind()
                                .await
                                .unwrap_or_else(|_| {
                                    Err("request handler panicked".to_owned())
                                });
                            let message = match result {
                                Ok(result) => JSONRPCMessage::Response(JSONRPCResponse { id, result }),
                                Err(message) => JSONRPCMessage::Error(JSONRPCError {
                                    error: JSONRPCErrorError {
                                        code: APPLICATION_ERROR,
                                        data: None,
                                        message,
                                    },
                                    id,
                                }),
                            };
                            let _ = outbound_tx.send(message);
                        });
                    }
                    JSONRPCMessage::Notification(notification) => {
                        let notifications = Arc::clone(&notifications);
                        let method = notification.method;
                        let params = notification.params.unwrap_or(Value::Null);
                        tokio::spawn(async move {
                            notifications.handle_notification(method, params).await;
                        });
                    }
                    // Server→client requests are not used by the harness host;
                    // stray responses/errors from the peer are ignored.
                    JSONRPCMessage::Response(_) | JSONRPCMessage::Error(_) => {}
                }
            }
        }

        // Reap completed handler tasks without blocking the loop.
        while tasks.try_join_next().is_some() {}
    }

    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result {
            tracing::warn!(%error, "json-rpc websocket request task failed after stream closed");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::channel::mpsc as fmpsc;
    use futures::{SinkExt, StreamExt};
    use serde_json::{Value, json};
    use std::time::Duration;

    struct EchoHandler;

    #[async_trait::async_trait]
    impl RequestHandler for EchoHandler {
        async fn handle_request(&self, method: String, params: Value) -> Result<Value, String> {
            if method == "echo" { Ok(params) } else { Err(format!("unknown method `{method}`")) }
        }
    }

    struct RecordingNotifications(std::sync::Mutex<Vec<String>>);

    #[async_trait::async_trait]
    impl NotificationHandler for RecordingNotifications {
        async fn handle_notification(&self, method: String, _params: Value) {
            self.0.lock().unwrap().push(method);
        }
    }

    #[tokio::test]
    async fn request_round_trips_and_notification_is_routed() {
        // `futures::mpsc::Sender`/`Receiver` already implement `Sink`/`Stream`
        // with a matching `SendError`, so they double as an in-memory WS pair.
        let (client_to_server_tx, client_to_server_rx) = fmpsc::channel::<WsFrame>(8);
        let (server_to_client_tx, server_to_client_rx) = fmpsc::channel::<WsFrame>(8);
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel::<JSONRPCMessage>();

        let notifications = Arc::new(RecordingNotifications(std::sync::Mutex::new(vec![])));
        let stream = client_to_server_rx.map(Ok::<_, fmpsc::SendError>);
        let serve = tokio::spawn(serve_websocket(
            Arc::new(EchoHandler),
            Arc::clone(&notifications),
            outbound_tx,
            outbound_rx,
            server_to_client_tx,
            stream,
            HostConfig::default(),
        ));

        // Client sends a request, then a notification, keeping the socket open.
        let mut client_to_server_tx = client_to_server_tx;
        client_to_server_tx
            .send(WsFrame::Text(
                json!({"jsonrpc":"2.0","id":1,"method":"echo","params":{"x":1}}).to_string(),
            ))
            .await
            .unwrap();
        client_to_server_tx
            .send(WsFrame::Text(
                json!({"jsonrpc":"2.0","method":"something/happened","params":{}}).to_string(),
            ))
            .await
            .unwrap();

        // Client reads the response while the socket is still open.
        let mut server_to_client_rx = server_to_client_rx;
        let frame = tokio::time::timeout(Duration::from_secs(2), server_to_client_rx.next())
            .await
            .expect("response timeout")
            .expect("response frame");
        let WsFrame::Text(line) = frame else {
            panic!("expected text frame");
        };
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["id"], 1);
        assert_eq!(response["result"], json!({"x": 1}));

        // Allow the notification spawn to run.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(notifications.0.lock().unwrap().as_slice(), &["something/happened"]);

        // Now close the inbound direction so the server task can finish.
        drop(client_to_server_tx);
        let _ = tokio::time::timeout(Duration::from_secs(2), serve).await;
    }

    #[test]
    fn wsframe_decodes_text_and_binary() {
        assert_eq!(WsFrame::Text("x".to_owned()).into_json_text(), Some("x".to_owned()));
        assert_eq!(WsFrame::Binary(b"y".to_vec()).into_json_text(), Some("y".to_owned()));
        assert_eq!(WsFrame::Close.into_json_text(), None);
    }
}
