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

use crate::PythonRuntime;
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
    runtime: Arc<PythonRuntime>,
    outbound: mpsc::UnboundedReceiver<String>,
) -> anyhow::Result<()> {
    serve_io(host, runtime, outbound, tokio::io::stdin(), tokio::io::stdout()).await
}

pub async fn serve_uds(
    host: Arc<JsonRpcRuntimeHost>,
    runtime: Arc<PythonRuntime>,
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

    serve_io(host, runtime, outbound, socket_reader, socket_writer).await
}

async fn serve_io<R, W>(
    host: Arc<JsonRpcRuntimeHost>,
    runtime: Arc<PythonRuntime>,
    outbound: mpsc::UnboundedReceiver<String>,
    reader: R,
    writer: W,
) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
{
    tokio::spawn(drain_outbound(outbound, writer));
    host.send_notification("runtime.ready", ready_payload());
    serve_reader(host, runtime, BufReader::new(reader)).await
}

fn ready_payload() -> Value {
    serde_json::json!({
        "runtime": "slab-python-runtime",
        "engine": "cpython-pyo3"
    })
}

async fn serve_reader<R>(
    host: Arc<JsonRpcRuntimeHost>,
    runtime: Arc<PythonRuntime>,
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

        match method.as_str() {
            "plugin.call" => {
                let Some(id) = id else {
                    continue;
                };
                let host = host.clone();
                let runtime = runtime.clone();
                tokio::spawn(async move {
                    let result = match serde_json::from_value(incoming.params) {
                        Ok(request) => runtime
                            .call(request)
                            .await
                            .and_then(|response| serde_json::to_value(response).map_err(Into::into))
                            .map_err(|error| error.to_string()),
                        Err(error) => Err(format!("invalid plugin.call params: {error}")),
                    };
                    host.send_response(id, result);
                });
            }
            _ => {
                if let Some(id) = id {
                    host.send_response(id, Err(format!("unknown runtime method `{method}`")));
                }
            }
        }
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
    use std::path::Path;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::task::{Context, Poll};
    use std::time::Duration;

    use serde_json::{Value, json};
    use slab_types::{PluginPermissionsManifest, PluginRuntimeCallRequest};
    use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
    use tokio::sync::mpsc;

    use super::{JsonRpcRuntimeHost, drain_outbound, serve_io, serve_reader};
    use crate::PythonRuntime;

    fn plugin_call_request(root_dir: &str) -> PluginRuntimeCallRequest {
        plugin_call_request_with_params(root_dir, json!({"value": 2}))
    }

    fn plugin_call_request_with_params(root_dir: &str, params: Value) -> PluginRuntimeCallRequest {
        PluginRuntimeCallRequest {
            call_id: "call-1".to_owned(),
            plugin_id: "plugin-1".to_owned(),
            root_dir: root_dir.to_owned(),
            entry: "plugin.py".to_owned(),
            bundle: None,
            export_name: "run".to_owned(),
            params,
            permissions: PluginPermissionsManifest::default(),
            file_grants: Vec::new(),
            blocked_fetch_origins: Vec::new(),
        }
    }

    fn side_effect_plugin_request(root_dir: &Path, marker_path: &Path) -> PluginRuntimeCallRequest {
        let root_dir = root_dir.to_string_lossy().into_owned();
        let marker_path = marker_path.to_string_lossy().into_owned();
        plugin_call_request_with_params(&root_dir, json!({ "marker": marker_path }))
    }

    fn write_side_effect_plugin(root_dir: &Path) {
        std::fs::write(
            root_dir.join("plugin.py"),
            "from pathlib import Path\n\n\
def run(params):\n\
    Path(params['marker']).write_text('called')\n\
    return {'called': True}\n",
        )
        .expect("write plugin");
    }

    async fn serve_messages(lines: Vec<Value>) -> Vec<Value> {
        let (host, mut outbound) = JsonRpcRuntimeHost::new();
        let host = Arc::new(host);
        let runtime = Arc::new(PythonRuntime::new());
        let (mut writer, reader) = tokio::io::duplex(4096);
        let serve = tokio::spawn(serve_reader(Arc::clone(&host), runtime, BufReader::new(reader)));

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
        let temp_dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp_dir.path().join("plugin.py"),
            "def run(params):\n    return {'value': params['value'] + 1}\n",
        )
        .expect("write plugin");

        let (host, mut outbound) = JsonRpcRuntimeHost::new();
        let host = Arc::new(host);
        host.send_notification(
            "runtime.ready",
            json!({
                "runtime": "slab-python-runtime",
                "engine": "cpython-pyo3"
            }),
        );
        let runtime = Arc::new(PythonRuntime::new());

        let (mut writer, reader) = tokio::io::duplex(1024);
        let serve = tokio::spawn(serve_reader(Arc::clone(&host), runtime, BufReader::new(reader)));

        let line = json!({
            "jsonrpc": "2.0",
            "id": "1",
            "method": "plugin.call",
            "params": plugin_call_request(&temp_dir.path().to_string_lossy()),
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
                    "engine": "cpython-pyo3",
                    "runtime": "slab-python-runtime"
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
        assert_eq!(response["result"], json!({ "result": { "value": 3 } }));

        serve.await.expect("join").expect("serve");
    }

    #[tokio::test]
    async fn rejects_invalid_jsonrpc_payloads() {
        let (host, mut outbound) = JsonRpcRuntimeHost::new();
        let host = Arc::new(host);
        host.send_notification(
            "runtime.ready",
            json!({
                "runtime": "slab-python-runtime",
                "engine": "cpython-pyo3"
            }),
        );
        let runtime = Arc::new(PythonRuntime::new());

        let (mut writer, reader) = tokio::io::duplex(1024);
        let serve = tokio::spawn(serve_reader(Arc::clone(&host), runtime, BufReader::new(reader)));

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
        let temp_dir = tempfile::tempdir().expect("tempdir");
        write_side_effect_plugin(temp_dir.path());
        let marker_path = temp_dir.path().join("called.txt");

        let messages = serve_messages(vec![json!({
            "jsonrpc": "2.0",
            "method": "plugin.call",
            "params": side_effect_plugin_request(temp_dir.path(), &marker_path),
        })])
        .await;

        assert!(messages.is_empty());
        assert!(!marker_path.exists());
    }

    #[tokio::test]
    async fn rejects_bad_jsonrpc_version_with_id_without_running_plugins() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        write_side_effect_plugin(temp_dir.path());
        let marker_path = temp_dir.path().join("called.txt");

        let messages = serve_messages(vec![json!({
            "jsonrpc": "1.0",
            "id": "bad-version",
            "method": "plugin.call",
            "params": side_effect_plugin_request(temp_dir.path(), &marker_path),
        })])
        .await;

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["id"], "bad-version");
        assert_eq!(messages[0]["error"]["message"], "jsonrpc must be `2.0`");
        assert!(!marker_path.exists());
    }

    #[tokio::test]
    async fn ignores_bad_version_notifications_without_running_plugins() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        write_side_effect_plugin(temp_dir.path());
        let marker_path = temp_dir.path().join("called.txt");

        let messages = serve_messages(vec![json!({
            "jsonrpc": "1.0",
            "method": "plugin.call",
            "params": side_effect_plugin_request(temp_dir.path(), &marker_path),
        })])
        .await;

        assert!(messages.is_empty());
        assert!(!marker_path.exists());
    }

    #[tokio::test]
    async fn response_messages_do_not_run_plugins() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        write_side_effect_plugin(temp_dir.path());
        let marker_path = temp_dir.path().join("called.txt");

        let messages = serve_messages(vec![json!({
            "jsonrpc": "2.0",
            "id": "host-1",
            "result": { "ok": true },
            "params": side_effect_plugin_request(temp_dir.path(), &marker_path),
        })])
        .await;

        assert!(messages.is_empty());
        assert!(!marker_path.exists());
    }

    #[tokio::test]
    async fn serve_io_sends_ready_and_dispatches_reader_messages() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp_dir.path().join("plugin.py"),
            "def run(params):\n    return {'value': params['value'] + 1}\n",
        )
        .expect("write plugin");

        let (host, outbound) = JsonRpcRuntimeHost::new();
        let host = Arc::new(host);
        let runtime = Arc::new(PythonRuntime::new());
        let (mut input_host, input_runtime) = tokio::io::duplex(4096);
        let (output_runtime, output_host) = tokio::io::duplex(4096);

        input_host
            .write_all(
                serde_json::to_string(&json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "plugin.call",
                    "params": plugin_call_request(&temp_dir.path().to_string_lossy()),
                }))
                .unwrap()
                .as_bytes(),
            )
            .await
            .unwrap();
        input_host.write_all(b"\n").await.unwrap();
        input_host.shutdown().await.unwrap();

        let serve = tokio::spawn(serve_io(host, runtime, outbound, input_runtime, output_runtime));
        let mut lines = BufReader::new(output_host).lines();
        let ready: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        let response: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();

        tokio::time::timeout(Duration::from_secs(1), serve).await.unwrap().unwrap().unwrap();

        assert_eq!(ready["method"], "runtime.ready");
        assert_eq!(response["id"], 1);
        assert_eq!(response["result"], json!({ "result": { "value": 3 } }));
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
