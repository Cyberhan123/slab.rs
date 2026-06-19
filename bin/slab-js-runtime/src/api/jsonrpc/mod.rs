use std::path::Path;
use std::sync::Arc;

use serde_json::Value;
pub use slab_jsonrpc::host::JsonRpcRuntimeHost;
use slab_jsonrpc::host::RequestHandler;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

use crate::application::PluginRuntimeServer;
use crate::domain::RuntimeHost;
use slab_utils::uds::UnixStream;

struct JsRequestHandler {
    server: Arc<PluginRuntimeServer>,
}

#[async_trait::async_trait]
impl RuntimeHost for JsonRpcRuntimeHost {
    async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        JsonRpcRuntimeHost::request(self, method, params).await
    }
}

#[async_trait::async_trait]
impl RequestHandler for JsRequestHandler {
    async fn handle_request(&self, method: String, params: Value) -> Result<Value, String> {
        self.server.handle_request(&method, params).await
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
    host.send_notification("runtime.ready", server.ready_payload());
    let handler = Arc::new(JsRequestHandler { server });
    slab_jsonrpc::host::serve_io(host, handler, outbound, reader, writer).await.map_err(Into::into)
}

#[cfg(test)]
async fn serve_reader<R>(
    host: Arc<JsonRpcRuntimeHost>,
    server: Arc<PluginRuntimeServer>,
    reader: R,
) -> anyhow::Result<()>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    let handler = Arc::new(JsRequestHandler { server });
    slab_jsonrpc::host::serve_reader(host, handler, reader).await.map_err(Into::into)
}

#[cfg(test)]
async fn drain_outbound<W>(outbound: mpsc::UnboundedReceiver<String>, writer: W)
where
    W: AsyncWrite + Unpin,
{
    slab_jsonrpc::host::drain_outbound(outbound, writer).await;
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
