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
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
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
    tokio::spawn(drain_outbound(outbound, tokio::io::stdout()));

    host.send_notification(
        "runtime.ready",
        serde_json::json!({
            "runtime": "slab-python-runtime",
            "engine": "cpython-pyo3"
        }),
    );

    let stdin = tokio::io::stdin();
    let lines = BufReader::new(stdin);
    serve_reader(host, runtime, lines).await
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
    tokio::spawn(drain_outbound(outbound, socket_writer));

    host.send_notification(
        "runtime.ready",
        serde_json::json!({
            "runtime": "slab-python-runtime",
            "engine": "cpython-pyo3"
        }),
    );
    let lines = BufReader::new(socket_reader);
    serve_reader(host, runtime, lines).await
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
