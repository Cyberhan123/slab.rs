use std::collections::HashMap;
use std::io::BufRead;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::PythonRuntime;
use crate::domain::RuntimeHost;

#[derive(Debug, Deserialize)]
struct JsonRpcIncoming {
    #[serde(default)]
    jsonrpc: Option<String>,
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'static str,
    id: Value,
    method: &'a str,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcNotification<'a> {
    jsonrpc: &'static str,
    method: &'a str,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>>;

#[derive(Clone)]
pub struct JsonRpcRuntimeHost {
    outbound: mpsc::UnboundedSender<String>,
    pending: PendingMap,
    next_id: Arc<AtomicU64>,
}

impl JsonRpcRuntimeHost {
    pub fn new() -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        tokio::spawn(async move {
            let mut stdout = tokio::io::stdout();
            while let Some(line) = rx.recv().await {
                if stdout.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if stdout.write_all(b"\n").await.is_err() {
                    break;
                }
                if stdout.flush().await.is_err() {
                    break;
                }
            }
        });

        Self {
            outbound: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    async fn resolve_response(&self, response: JsonRpcIncoming) {
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
            Ok(result) => JsonRpcResponse { jsonrpc: "2.0", id, result: Some(result), error: None },
            Err(message) => JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: None,
                error: Some(JsonRpcError { code: -32000, message }),
            },
        };
        self.send_serialized(&response);
    }

    fn send_notification(&self, method: &str, params: Value) {
        self.send_serialized(&JsonRpcNotification { jsonrpc: "2.0", method, params });
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

impl Default for JsonRpcRuntimeHost {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl RuntimeHost for JsonRpcRuntimeHost {
    async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = Value::String(format!("host-{}", self.next_id.fetch_add(1, Ordering::Relaxed)));
        let key = id_key(&id);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(key.clone(), tx);
        self.send_serialized(&JsonRpcRequest { jsonrpc: "2.0", id, method, params });

        match rx.await {
            Ok(result) => result,
            Err(_) => Err(format!("host request `{method}` response channel closed")),
        }
    }
}

pub async fn serve_stdio(
    host: Arc<JsonRpcRuntimeHost>,
    runtime: Arc<PythonRuntime>,
) -> anyhow::Result<()> {
    host.send_notification(
        "runtime.ready",
        serde_json::json!({
            "runtime": "slab-python-runtime",
            "engine": "cpython-pyo3"
        }),
    );

    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<Result<String, String>>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let result = line.map_err(|error| error.to_string());
            if line_tx.send(result).is_err() {
                break;
            }
        }
    });

    while let Some(line) = line_rx.recv().await {
        let line = line.map_err(|error| anyhow::anyhow!(error))?;
        let incoming = match serde_json::from_str::<JsonRpcIncoming>(&line) {
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
        if incoming.jsonrpc.as_deref() != Some("2.0") {
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

fn id_key(id: &Value) -> String {
    match id {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        other => other.to_string(),
    }
}
