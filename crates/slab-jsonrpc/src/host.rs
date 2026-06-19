use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use futures::FutureExt;
use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{Mutex, Semaphore, mpsc, oneshot};
use tokio::task::JoinSet;

use crate::{
    IncomingMessage, application_error_response, id_key, notification, parse_message, request,
    success_response,
};

type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>>;

#[derive(Debug, Clone)]
pub struct HostConfig {
    pub request_timeout: Duration,
    pub pending_cap: usize,
    pub concurrency_limit: usize,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self { request_timeout: Duration::from_secs(30), pending_cap: 256, concurrency_limit: 16 }
    }
}

/// Handles inbound JSON-RPC requests after transport validation.
///
/// Implementations own business dispatch for a specific runtime. They should
/// return JSON-serializable response payloads and leave transport errors,
/// request limits, and response envelope formatting to [`JsonRpcRuntimeHost`].
#[async_trait::async_trait]
pub trait RequestHandler: Send + Sync + 'static {
    async fn handle_request(&self, method: String, params: Value) -> Result<Value, String>;
}

#[derive(Clone)]
pub struct JsonRpcRuntimeHost {
    outbound: mpsc::UnboundedSender<String>,
    pending: PendingMap,
    next_id: Arc<AtomicU64>,
    config: Arc<HostConfig>,
}

impl JsonRpcRuntimeHost {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<String>) {
        Self::with_config(HostConfig::default())
    }

    pub fn with_config(config: HostConfig) -> (Self, mpsc::UnboundedReceiver<String>) {
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        (
            Self {
                outbound: tx,
                pending: Arc::new(Mutex::new(HashMap::new())),
                next_id: Arc::new(AtomicU64::new(1)),
                config: Arc::new(config),
            },
            rx,
        )
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = Value::String(format!("host-{}", self.next_id.fetch_add(1, Ordering::Relaxed)));
        let key = id_key(&id);
        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending.lock().await;
            if pending.len() >= self.config.pending_cap {
                return Err(format!(
                    "host request `{method}` rejected because pending request cap {} was reached",
                    self.config.pending_cap
                ));
            }
            pending.insert(key.clone(), tx);
        }

        if let Err(error) = self.send_serialized(&request(id, method, params)) {
            self.pending.lock().await.remove(&key);
            return Err(error);
        }

        match tokio::time::timeout(self.config.request_timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                self.pending.lock().await.remove(&key);
                Err(format!("host request `{method}` response channel closed"))
            }
            Err(_) => {
                self.pending.lock().await.remove(&key);
                Err(format!("host request `{method}` timed out"))
            }
        }
    }

    pub async fn resolve_response(&self, response: IncomingMessage) {
        let Some(id) = response.id.as_ref().map(id_key) else {
            return;
        };
        let sender = self.pending.lock().await.remove(&id);
        if let Some(sender) = sender {
            let result = if let Some(error) = response.error {
                Err(error.message)
            } else {
                Ok(response.result.unwrap_or(Value::Null))
            };
            let _ = sender.send(result);
        }
    }

    pub fn send_response(&self, id: Value, result: Result<Value, String>) {
        let response = match result {
            Ok(result) => success_response(id, result),
            Err(message) => application_error_response(id, message),
        };
        let _ = self.send_serialized(&response);
    }

    pub fn send_notification(&self, method: &str, params: Value) {
        let _ = self.send_serialized(&notification(method, params));
    }

    fn send_serialized<T: Serialize>(&self, value: &T) -> Result<(), String> {
        let line = serde_json::to_string(value)
            .map_err(|error| format!("failed to serialize json-rpc message: {error}"))?;
        self.outbound.send(line).map_err(|_| "json-rpc outbound receiver is closed".to_owned())
    }

    #[cfg(test)]
    async fn pending_count(&self) -> usize {
        self.pending.lock().await.len()
    }
}

pub async fn serve_io<R, W, H>(
    host: Arc<JsonRpcRuntimeHost>,
    handler: Arc<H>,
    outbound: mpsc::UnboundedReceiver<String>,
    reader: R,
    writer: W,
) -> std::io::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
    H: RequestHandler,
{
    tokio::spawn(drain_outbound(outbound, writer));
    serve_reader(host, handler, tokio::io::BufReader::new(reader)).await
}

pub async fn serve_reader<R, H>(
    host: Arc<JsonRpcRuntimeHost>,
    handler: Arc<H>,
    reader: R,
) -> std::io::Result<()>
where
    R: AsyncBufRead + Unpin,
    H: RequestHandler,
{
    let mut lines = reader.lines();
    let semaphore = Arc::new(Semaphore::new(host.config.concurrency_limit));
    let mut tasks = JoinSet::new();
    while let Some(line) = lines.next_line().await? {
        drain_finished_tasks(&mut tasks);
        let incoming = match parse_message(&line) {
            Ok(incoming) => incoming,
            Err(error) => {
                host.send_response(
                    Value::Null,
                    Err(format!("invalid json-rpc payload from host: {error}")),
                );
                continue;
            }
        };

        if incoming.method.is_none() {
            host.resolve_response(incoming).await;
            continue;
        }

        let method = incoming.method.clone().unwrap_or_default();
        let id = incoming.id.clone();
        if !incoming.has_valid_version() {
            if let Some(id) = id {
                host.send_response(id, Err("jsonrpc must be `2.0`".to_owned()));
            }
            continue;
        }

        let Some(id) = id else {
            continue;
        };
        let Ok(permit) = Arc::clone(&semaphore).try_acquire_owned() else {
            host.send_response(id, Err("runtime request concurrency limit exceeded".to_owned()));
            continue;
        };
        let host = Arc::clone(&host);
        let handler = Arc::clone(&handler);
        tasks.spawn(async move {
            let _permit = permit;
            let result = AssertUnwindSafe(handler.handle_request(method, incoming.params))
                .catch_unwind()
                .await
                .unwrap_or_else(|_| Err("runtime request handler panicked".to_owned()));
            host.send_response(id, result);
        });
    }

    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result {
            tracing::warn!(%error, "json-rpc request task failed after reader closed");
        }
    }

    Ok(())
}

pub async fn drain_outbound<W>(mut outbound: mpsc::UnboundedReceiver<String>, mut writer: W)
where
    W: AsyncWrite + Unpin,
{
    while let Some(line) = outbound.recv().await {
        if writer.write_all(line.as_bytes()).await.is_err() {
            break;
        }
        if writer.write_all(b"\n").await.is_err() {
            break;
        }
        if writer.flush().await.is_err() {
            break;
        }
    }
}

fn drain_finished_tasks(tasks: &mut JoinSet<()>) {
    while let Some(result) = tasks.try_join_next() {
        if let Err(error) = result {
            tracing::warn!(%error, "json-rpc request task failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll};
    use std::time::Duration;

    use serde_json::{Value, json};
    use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
    use tokio::sync::{Mutex, mpsc};

    use super::{
        HostConfig, JsonRpcRuntimeHost, RequestHandler, drain_outbound, serve_io, serve_reader,
    };

    #[derive(Default)]
    struct EchoHandler {
        seen: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl RequestHandler for EchoHandler {
        async fn handle_request(&self, method: String, params: Value) -> Result<Value, String> {
            self.seen.lock().await.push(method.clone());
            match method.as_str() {
                "echo" => Ok(params),
                "wait" => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(json!({"done": true}))
                }
                "panic" => panic!("handler panic for test"),
                _ => Err(format!("unknown method `{method}`")),
            }
        }
    }

    fn test_config() -> HostConfig {
        HostConfig {
            request_timeout: Duration::from_millis(30),
            pending_cap: 1,
            concurrency_limit: 1,
        }
    }

    async fn recv_json(outbound: &mut mpsc::UnboundedReceiver<String>) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(1), outbound.recv())
            .await
            .expect("message timeout")
            .expect("message");
        serde_json::from_str(&line).expect("json message")
    }

    #[tokio::test]
    async fn request_timeout_cleans_pending_entry() {
        let (host, _outbound) = JsonRpcRuntimeHost::with_config(test_config());

        let error = host.request("never.responds", Value::Null).await.expect_err("timeout");

        assert!(error.contains("timed out"));
        assert_eq!(host.pending_count().await, 0);
    }

    #[tokio::test]
    async fn pending_cap_rejects_new_request() {
        let config = HostConfig { request_timeout: Duration::from_secs(5), ..test_config() };
        let (host, _outbound) = JsonRpcRuntimeHost::with_config(config);
        let pending_host = host.clone();
        let first = tokio::spawn(async move { pending_host.request("hold", Value::Null).await });

        while host.pending_count().await == 0 {
            tokio::task::yield_now().await;
        }
        let error = host.request("second", Value::Null).await.expect_err("cap");

        assert!(error.contains("pending request cap"));
        first.abort();
        assert_eq!(host.pending_count().await, 1);
    }

    #[tokio::test]
    async fn closed_response_channel_cleans_pending_entry() {
        let (host, mut outbound) = JsonRpcRuntimeHost::with_config(test_config());
        let pending_host = host.clone();
        let request = tokio::spawn(async move { pending_host.request("close", Value::Null).await });

        let line = outbound.recv().await.expect("outbound request");
        let id = serde_json::from_str::<Value>(&line).expect("request json")["id"].clone();
        request.abort();
        host.resolve_response(crate::IncomingMessage {
            jsonrpc: Some(crate::VERSION.to_owned()),
            id: Some(id),
            method: None,
            params: Value::Null,
            result: Some(json!({"ok": true})),
            error: None,
        })
        .await;

        assert_eq!(host.pending_count().await, 0);
    }

    #[tokio::test]
    async fn response_channel_close_after_send_failure_cleans_pending_entry() {
        let (host, outbound) = JsonRpcRuntimeHost::with_config(test_config());
        drop(outbound);

        let error = host.request("send.closed", Value::Null).await.expect_err("send closed");

        assert!(error.contains("outbound receiver is closed"));
        assert_eq!(host.pending_count().await, 0);
    }

    #[tokio::test]
    async fn serve_reader_rejects_concurrency_overflow() {
        let (host, mut outbound) = JsonRpcRuntimeHost::with_config(test_config());
        let host = Arc::new(host);
        let handler = Arc::new(EchoHandler::default());
        let (mut writer, reader) = tokio::io::duplex(4096);
        let serve = tokio::spawn(serve_reader(Arc::clone(&host), handler, BufReader::new(reader)));

        writer
            .write_all(
                format!(
                    "{}\n{}\n",
                    json!({"jsonrpc":"2.0","id":"slow","method":"wait"}),
                    json!({"jsonrpc":"2.0","id":"overflow","method":"echo","params":{"x":1}})
                )
                .as_bytes(),
            )
            .await
            .expect("write");
        writer.shutdown().await.expect("shutdown");

        let first = recv_json(&mut outbound).await;
        let second = recv_json(&mut outbound).await;
        let messages = [first, second];
        assert!(messages.iter().any(|message| message["id"] == "slow"));
        assert!(messages.iter().any(|message| {
            message["id"] == "overflow"
                && message["error"]["message"] == "runtime request concurrency limit exceeded"
        }));

        serve.await.expect("join").expect("serve");
    }

    #[tokio::test]
    async fn serve_reader_returns_panic_fallback_response() {
        let (host, mut outbound) = JsonRpcRuntimeHost::with_config(test_config());
        let host = Arc::new(host);
        let handler = Arc::new(EchoHandler::default());
        let (mut writer, reader) = tokio::io::duplex(4096);
        let serve = tokio::spawn(serve_reader(Arc::clone(&host), handler, BufReader::new(reader)));

        writer
            .write_all(
                format!(
                    "{}\n",
                    json!({"jsonrpc":"2.0","id":"panic","method":"panic","params":null})
                )
                .as_bytes(),
            )
            .await
            .expect("write");
        writer.shutdown().await.expect("shutdown");

        let response = recv_json(&mut outbound).await;

        assert_eq!(response["id"], "panic");
        assert_eq!(response["error"]["message"], "runtime request handler panicked");
        serve.await.expect("join").expect("serve");
    }

    #[tokio::test]
    async fn serve_io_dispatches_reader_and_writer() {
        let (host, outbound) = JsonRpcRuntimeHost::with_config(test_config());
        let host = Arc::new(host);
        let handler = Arc::new(EchoHandler::default());
        let (mut input_host, input_runtime) = tokio::io::duplex(4096);
        let (output_runtime, output_host) = tokio::io::duplex(4096);

        input_host
            .write_all(
                format!(
                    "{}\n",
                    json!({"jsonrpc":"2.0","id":1,"method":"echo","params":{"ok":true}})
                )
                .as_bytes(),
            )
            .await
            .expect("write");
        input_host.shutdown().await.expect("shutdown");

        let serve = tokio::spawn(serve_io(host, handler, outbound, input_runtime, output_runtime));
        let response: Value = serde_json::from_str(
            &BufReader::new(output_host).lines().next_line().await.unwrap().unwrap(),
        )
        .unwrap();

        assert_eq!(response["result"], json!({"ok": true}));
        serve.await.expect("join").expect("serve");
    }

    #[tokio::test]
    async fn drain_outbound_stops_on_writer_and_flush_failures() {
        for writer in [FailingWriter::write_error(), FailingWriter::flush_error()] {
            let (tx, rx) = mpsc::unbounded_channel();
            tx.send("{}".to_string()).expect("queue message");
            drop(tx);

            tokio::time::timeout(Duration::from_secs(1), drain_outbound(rx, writer))
                .await
                .expect("drain should stop on writer failure");
        }
    }

    #[tokio::test]
    async fn serve_reader_does_not_dispatch_invalid_messages() {
        let (host, mut outbound) = JsonRpcRuntimeHost::with_config(test_config());
        let host = Arc::new(host);
        let handler = Arc::new(CountingHandler::default());
        let (mut writer, reader) = tokio::io::duplex(4096);
        let serve = tokio::spawn(serve_reader(
            Arc::clone(&host),
            Arc::clone(&handler),
            BufReader::new(reader),
        ));

        writer
            .write_all(
                format!(
                    "{{not-json}}\n{}\n{}\n",
                    json!({"jsonrpc":"1.0","id":"bad","method":"echo"}),
                    json!({"jsonrpc":"2.0","method":"echo","params":{"ignored":true}})
                )
                .as_bytes(),
            )
            .await
            .expect("write");
        writer.shutdown().await.expect("shutdown");

        let parse_error = recv_json(&mut outbound).await;
        let version_error = recv_json(&mut outbound).await;

        assert_eq!(parse_error["id"], Value::Null);
        assert!(parse_error["error"]["message"].as_str().unwrap().contains("invalid json-rpc"));
        assert_eq!(version_error["id"], "bad");
        assert_eq!(handler.calls.load(Ordering::SeqCst), 0);
        serve.await.expect("join").expect("serve");
    }

    #[derive(Default)]
    struct CountingHandler {
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl RequestHandler for CountingHandler {
        async fn handle_request(&self, _method: String, _params: Value) -> Result<Value, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Value::Null)
        }
    }

    struct FailingWriter {
        fail_on_write: bool,
    }

    impl FailingWriter {
        fn write_error() -> Self {
            Self { fail_on_write: true }
        }

        fn flush_error() -> Self {
            Self { fail_on_write: false }
        }
    }

    impl AsyncWrite for FailingWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            if self.fail_on_write {
                Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "write failed")))
            } else {
                Poll::Ready(Ok(buf.len()))
            }
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            if self.fail_on_write {
                Poll::Ready(Ok(()))
            } else {
                Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "flush failed")))
            }
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }
}
