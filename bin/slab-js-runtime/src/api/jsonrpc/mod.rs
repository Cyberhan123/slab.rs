use std::collections::HashMap;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use serde::Serialize;
use serde_json::Value;
use slab_jsonrpc::{
    IncomingMessage, application_error_response, id_key, notification, parse_message, request,
    success_response,
};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::application::PluginRuntimeServer;
use crate::domain::RuntimeHost;
use slab_utils::uds::UnixStream;

type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>>;

#[derive(Clone)]
pub struct JsonRpcRuntimeHost {
    outbound: mpsc::UnboundedSender<String>,
    pending: PendingMap,
    next_id: Arc<AtomicU64>,
}

impl JsonRpcRuntimeHost {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<String>) {
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        (
            Self {
                outbound: tx,
                pending: Arc::new(Mutex::new(HashMap::new())),
                next_id: Arc::new(AtomicU64::new(1)),
            },
            rx,
        )
    }

    async fn resolve_response(&self, response: IncomingMessage) {
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

    fn send_response(&self, id: Value, result: Result<Value, String>) {
        let response = match result {
            Ok(result) => success_response(id, result),
            Err(message) => application_error_response(id, message),
        };
        self.send_serialized(&response);
    }

    fn send_notification(&self, method: &str, params: Value) {
        self.send_serialized(&notification(method, params));
    }

    fn send_serialized<T: Serialize>(&self, value: &T) {
        match serde_json::to_string(value) {
            Ok(line) => {
                let _ = self.outbound.send(line);
            }
            Err(error) => {
                eprintln!("failed to serialize json-rpc message: {error}");
            }
        }
    }
}

#[async_trait::async_trait]
impl RuntimeHost for JsonRpcRuntimeHost {
    async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = Value::String(format!("host-{}", self.next_id.fetch_add(1, Ordering::Relaxed)));
        let key = id_key(&id);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(key.clone(), tx);
        self.send_serialized(&request(id, method, params));

        match rx.await {
            Ok(result) => result,
            Err(_) => Err(format!("host request `{method}` response channel closed")),
        }
    }
}

pub async fn serve_stdio(
    host: Arc<JsonRpcRuntimeHost>,
    server: Arc<PluginRuntimeServer>,
    outbound: mpsc::UnboundedReceiver<String>,
) -> anyhow::Result<()> {
    serve_io(host, server, outbound, tokio::io::stdin(), tokio::io::stdout()).await
}

pub async fn serve_uds(
    host: Arc<JsonRpcRuntimeHost>,
    server: Arc<PluginRuntimeServer>,
    outbound: mpsc::UnboundedReceiver<String>,
    socket_path: &Path,
) -> anyhow::Result<()> {
    let stream = UnixStream::connect(socket_path).await.map_err(|error| {
        anyhow::anyhow!(
            "failed to connect runtime JSON-RPC socket {}: {error}",
            socket_path.display()
        )
    })?;
    let (socket_reader, socket_writer) = tokio::io::split(stream);

    serve_io(host, server, outbound, socket_reader, socket_writer).await
}

async fn serve_io<R, W>(
    host: Arc<JsonRpcRuntimeHost>,
    server: Arc<PluginRuntimeServer>,
    outbound: mpsc::UnboundedReceiver<String>,
    reader: R,
    writer: W,
) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
{
    tokio::spawn(drain_outbound(outbound, writer));
    host.send_notification("runtime.ready", server.ready_payload());
    serve_reader(host, server, BufReader::new(reader)).await
}

async fn serve_reader<R>(
    host: Arc<JsonRpcRuntimeHost>,
    server: Arc<PluginRuntimeServer>,
    reader: R,
) -> anyhow::Result<()>
where
    R: AsyncBufRead + Unpin,
{
    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await? {
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
        let host = host.clone();
        let server = server.clone();
        tokio::spawn(async move {
            let result = server.handle_request(&method, incoming.params).await;
            host.send_response(id, result);
        });
    }

    Ok(())
}

async fn drain_outbound<W>(mut outbound: mpsc::UnboundedReceiver<String>, mut writer: W)
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

#[cfg(test)]
mod tests {
    use std::io;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll};
    use std::time::Duration;

    use serde_json::{Value, json};
    use slab_types::{
        PluginPermissionsManifest, PluginRuntimeCallRequest, PluginRuntimeCallResponse,
    };
    use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
    use tokio::sync::mpsc;

    use super::{JsonRpcRuntimeHost, drain_outbound, serve_io, serve_reader};
    use crate::application::{PluginExecutor, PluginRuntimeServer};

    #[derive(Default)]
    struct RecordingExecutor {
        requests: Mutex<Vec<(String, String, Value)>>,
    }

    #[async_trait::async_trait]
    impl PluginExecutor for RecordingExecutor {
        async fn execute(
            &self,
            request: PluginRuntimeCallRequest,
        ) -> Result<PluginRuntimeCallResponse, anyhow::Error> {
            self.requests.lock().expect("lock").push((
                request.plugin_id.clone(),
                request.export_name.clone(),
                request.params.clone(),
            ));
            Ok(PluginRuntimeCallResponse {
                result: json!({
                    "pluginId": request.plugin_id,
                    "exportName": request.export_name,
                }),
            })
        }
    }

    fn plugin_call_request() -> PluginRuntimeCallRequest {
        PluginRuntimeCallRequest {
            call_id: "call-1".to_owned(),
            plugin_id: "plugin-1".to_owned(),
            root_dir: ".".to_owned(),
            entry: "main.ts".to_owned(),
            bundle: None,
            export_name: "run".to_owned(),
            params: json!({"value": 2}),
            permissions: PluginPermissionsManifest::default(),
            file_grants: Vec::new(),
            blocked_fetch_origins: Vec::new(),
        }
    }

    async fn serve_messages(lines: Vec<Value>, executor: Arc<RecordingExecutor>) -> Vec<Value> {
        let (host, mut outbound) = JsonRpcRuntimeHost::new();
        let host = Arc::new(host);
        let server = Arc::new(PluginRuntimeServer::new(executor));
        let (mut writer, reader) = tokio::io::duplex(4096);
        let serve = tokio::spawn(serve_reader(Arc::clone(&host), server, BufReader::new(reader)));

        for line in lines {
            writer.write_all(format!("{line}\n").as_bytes()).await.expect("write request");
        }
        writer.shutdown().await.expect("shutdown");
        serve.await.expect("join").expect("serve");

        let mut messages = Vec::new();
        while let Ok(Some(line)) =
            tokio::time::timeout(Duration::from_millis(100), outbound.recv()).await
        {
            messages.push(serde_json::from_str(&line).expect("json-rpc message"));
        }
        messages
    }

    #[tokio::test]
    async fn serves_runtime_ready_and_plugin_call() {
        let (host, mut outbound) = JsonRpcRuntimeHost::new();
        let host = Arc::new(host);
        let executor = Arc::new(RecordingExecutor::default());
        let server = Arc::new(PluginRuntimeServer::new(executor.clone()));
        host.send_notification("runtime.ready", server.ready_payload());

        let (mut writer, reader) = tokio::io::duplex(1024);
        let serve = tokio::spawn(serve_reader(Arc::clone(&host), server, BufReader::new(reader)));

        let request = plugin_call_request();
        let line = json!({
            "jsonrpc": "2.0",
            "id": "1",
            "method": "plugin.call",
            "params": request,
        });
        writer.write_all(format!("{line}\n").as_bytes()).await.expect("write request");
        writer.shutdown().await.expect("shutdown");

        let ready: Value = serde_json::from_str(
            &tokio::time::timeout(Duration::from_secs(5), outbound.recv())
                .await
                .expect("ready timeout")
                .expect("ready message"),
        )
        .expect("parse ready message");
        assert_eq!(
            ready,
            json!({
                "jsonrpc": "2.0",
                "method": "runtime.ready",
                "params": {
                    "engine": "deno",
                    "runtime": "slab-js-runtime"
                }
            })
        );

        let response: Value = serde_json::from_str(
            &tokio::time::timeout(Duration::from_secs(5), outbound.recv())
                .await
                .expect("response timeout")
                .expect("response message"),
        )
        .expect("parse response message");
        assert_eq!(response["id"], "1");
        assert_eq!(
            response["result"],
            json!({
                "result": {
                    "exportName": "run",
                    "pluginId": "plugin-1"
                }
            })
        );

        serve.await.expect("join").expect("serve");

        let requests = executor.requests.lock().expect("lock");
        assert_eq!(
            requests.as_slice(),
            &[("plugin-1".to_owned(), "run".to_owned(), json!({"value": 2}),)]
        );
    }

    #[tokio::test]
    async fn rejects_invalid_jsonrpc_payloads() {
        let (host, mut outbound) = JsonRpcRuntimeHost::new();
        let host = Arc::new(host);
        let server = Arc::new(PluginRuntimeServer::new(Arc::new(RecordingExecutor::default())));
        host.send_notification("runtime.ready", server.ready_payload());

        let (mut writer, reader) = tokio::io::duplex(1024);
        let serve = tokio::spawn(serve_reader(Arc::clone(&host), server, BufReader::new(reader)));

        writer.write_all(b"{not json}\n").await.expect("write request");
        writer.shutdown().await.expect("shutdown");

        let ready: Value = serde_json::from_str(
            &tokio::time::timeout(Duration::from_secs(5), outbound.recv())
                .await
                .expect("ready timeout")
                .expect("ready message"),
        )
        .expect("parse ready message");
        assert_eq!(ready["method"], "runtime.ready");

        let response: Value = serde_json::from_str(
            &tokio::time::timeout(Duration::from_secs(5), outbound.recv())
                .await
                .expect("response timeout")
                .expect("response message"),
        )
        .expect("parse response message");
        assert!(
            response["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("invalid json-rpc payload from host")
        );

        serve.await.expect("join").expect("serve");
    }

    #[tokio::test]
    async fn ignores_no_id_requests_without_running_plugins() {
        let executor = Arc::new(RecordingExecutor::default());

        let messages = serve_messages(
            vec![json!({
                "jsonrpc": "2.0",
                "method": "plugin.call",
                "params": plugin_call_request(),
            })],
            Arc::clone(&executor),
        )
        .await;

        assert!(messages.is_empty());
        assert!(executor.requests.lock().expect("lock").is_empty());
    }

    #[tokio::test]
    async fn rejects_bad_jsonrpc_version_with_id_without_running_plugins() {
        let executor = Arc::new(RecordingExecutor::default());

        let messages = serve_messages(
            vec![json!({
                "jsonrpc": "1.0",
                "id": "bad-version",
                "method": "plugin.call",
                "params": plugin_call_request(),
            })],
            Arc::clone(&executor),
        )
        .await;

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["id"], "bad-version");
        assert_eq!(messages[0]["error"]["message"], "jsonrpc must be `2.0`");
        assert!(executor.requests.lock().expect("lock").is_empty());
    }

    #[tokio::test]
    async fn ignores_bad_version_notifications_without_running_plugins() {
        let executor = Arc::new(RecordingExecutor::default());

        let messages = serve_messages(
            vec![json!({
                "jsonrpc": "1.0",
                "method": "plugin.call",
                "params": plugin_call_request(),
            })],
            Arc::clone(&executor),
        )
        .await;

        assert!(messages.is_empty());
        assert!(executor.requests.lock().expect("lock").is_empty());
    }

    #[tokio::test]
    async fn response_messages_do_not_run_plugins() {
        let executor = Arc::new(RecordingExecutor::default());

        let messages = serve_messages(
            vec![json!({
                "jsonrpc": "2.0",
                "id": "host-1",
                "result": { "ok": true },
            })],
            Arc::clone(&executor),
        )
        .await;

        assert!(messages.is_empty());
        assert!(executor.requests.lock().expect("lock").is_empty());
    }

    #[tokio::test]
    async fn serve_io_sends_ready_and_dispatches_reader_messages() {
        let executor = Arc::new(RecordingExecutor::default());
        let (host, outbound) = JsonRpcRuntimeHost::new();
        let host = Arc::new(host);
        let server = Arc::new(PluginRuntimeServer::new(executor));
        let (mut input_host, input_runtime) = tokio::io::duplex(4096);
        let (output_runtime, output_host) = tokio::io::duplex(4096);
        let request = plugin_call_request();

        input_host
            .write_all(
                serde_json::to_string(&json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "plugin.call",
                    "params": request,
                }))
                .unwrap()
                .as_bytes(),
            )
            .await
            .unwrap();
        input_host.write_all(b"\n").await.unwrap();
        input_host.shutdown().await.unwrap();

        let serve = tokio::spawn(serve_io(host, server, outbound, input_runtime, output_runtime));
        let mut lines = BufReader::new(output_host).lines();
        let ready: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        let response: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();

        tokio::time::timeout(Duration::from_secs(1), serve).await.unwrap().unwrap().unwrap();

        assert_eq!(ready["method"], "runtime.ready");
        assert_eq!(response["id"], 1);
        assert_eq!(response["result"]["result"]["pluginId"], "plugin-1");
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
