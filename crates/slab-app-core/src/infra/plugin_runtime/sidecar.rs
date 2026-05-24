use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{Mutex, oneshot};

use crate::error::AppCoreError;
use crate::infra::plugin_runtime::events::PluginEventBus;
use crate::infra::plugin_runtime::host_api::{
    authorize_slab_api_request, execute_plugin_api_request,
};
use crate::infra::plugin_runtime::process::{resolve_js_runtime_exe, resolve_python_runtime_exe};
use crate::infra::process_supervisor::{
    SupervisedProcessExit, SupervisedStdioProcess, SupervisedStdioProcessConfig,
};
use slab_types::{
    PluginEventPayload, PluginPermissionsManifest, PluginRuntimeApiHostRequest,
    PluginRuntimeCallRequest, PluginRuntimeCallResponse, PluginRuntimeUiEmitRequest,
};

const PLUGIN_CALL_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Copy)]
pub enum PluginSidecarRuntimeKind {
    JavaScript,
    Python,
}

impl PluginSidecarRuntimeKind {
    fn process_label(self) -> &'static str {
        match self {
            Self::JavaScript => "slab-js-runtime",
            Self::Python => "slab-python-runtime",
        }
    }

    fn error_label(self) -> &'static str {
        match self {
            Self::JavaScript => "JS",
            Self::Python => "Python",
        }
    }

    fn resolve_current_server_exe(self) -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|server_exe| match self {
                Self::JavaScript => resolve_js_runtime_exe(&server_exe).ok(),
                Self::Python => resolve_python_runtime_exe(&server_exe).ok(),
            })
            .unwrap_or_else(|| PathBuf::from(self.process_label()))
    }
}

#[derive(Clone)]
pub struct PluginSidecarRuntimeClient {
    inner: Arc<PluginSidecarRuntimeClientInner>,
}

struct PluginSidecarRuntimeClientInner {
    kind: PluginSidecarRuntimeKind,
    runtime_exe: PathBuf,
    api_base_url: String,
    event_bus: PluginEventBus,
    state: Mutex<ClientState>,
    request_id: AtomicU64,
    pending_calls: DashMap<String, CallContext>,
}

struct ClientState {
    child: Option<RuntimeChild>,
}

#[derive(Clone)]
struct RuntimeChild {
    process: SupervisedStdioProcess,
    pending: PendingMap,
}

#[derive(Clone)]
struct CallContext {
    plugin_id: String,
    permissions: PluginPermissionsManifest,
}

type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>>;

#[derive(Debug, Deserialize)]
struct JsonRpcIncoming {
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

impl PluginSidecarRuntimeClient {
    pub fn new(
        kind: PluginSidecarRuntimeKind,
        runtime_exe: PathBuf,
        api_base_url: String,
        event_bus: PluginEventBus,
    ) -> Self {
        Self {
            inner: Arc::new(PluginSidecarRuntimeClientInner {
                kind,
                runtime_exe,
                api_base_url,
                event_bus,
                state: Mutex::new(ClientState { child: None }),
                request_id: AtomicU64::new(1),
                pending_calls: DashMap::new(),
            }),
        }
    }

    pub fn for_current_server(
        kind: PluginSidecarRuntimeKind,
        api_base_url: String,
        event_bus: PluginEventBus,
    ) -> Self {
        Self::new(kind, kind.resolve_current_server_exe(), api_base_url, event_bus)
    }

    pub async fn call(
        &self,
        mut request: PluginRuntimeCallRequest,
    ) -> Result<PluginRuntimeCallResponse, AppCoreError> {
        let child = self.ensure_child().await?;
        let id = Value::String(format!(
            "plugin-call-{}",
            self.inner.request_id.fetch_add(1, Ordering::Relaxed)
        ));
        let key = id_key(&id);
        if !request.blocked_fetch_origins.iter().any(|origin| origin == &self.inner.api_base_url) {
            request.blocked_fetch_origins.push(self.inner.api_base_url.clone());
        }

        let params = serde_json::to_value(&request).map_err(|error| {
            AppCoreError::Internal(format!("failed to serialize plugin runtime call: {error}"))
        })?;
        let payload = serde_json::to_string(&JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: "plugin.call",
            params,
        })
        .map_err(|error| {
            AppCoreError::Internal(format!("failed to serialize plugin runtime JSON-RPC: {error}"))
        })?;

        let (tx, rx) = oneshot::channel();
        child.pending.lock().await.insert(key.clone(), tx);
        self.inner.pending_calls.insert(
            request.call_id.clone(),
            CallContext {
                plugin_id: request.plugin_id.clone(),
                permissions: request.permissions.clone(),
            },
        );

        if child.process.send_line(payload).is_err() {
            child.pending.lock().await.remove(&key);
            self.inner.pending_calls.remove(&request.call_id);
            return Err(AppCoreError::Internal(format!(
                "{} stdin is closed",
                self.inner.kind.process_label()
            )));
        }

        let result = tokio::time::timeout(PLUGIN_CALL_TIMEOUT, rx).await;
        self.inner.pending_calls.remove(&request.call_id);
        let value = match result {
            Ok(Ok(Ok(value))) => value,
            Ok(Ok(Err(error))) => {
                return Err(AppCoreError::BadRequest(format!(
                    "{} plugin runtime error: {error}",
                    self.inner.kind.error_label()
                )));
            }
            Ok(Err(_)) => {
                return Err(AppCoreError::Internal(format!(
                    "{} plugin runtime response channel closed",
                    self.inner.kind.error_label()
                )));
            }
            Err(_) => {
                child.pending.lock().await.remove(&key);
                return Err(AppCoreError::Internal(format!(
                    "{} plugin runtime call timed out after {}ms",
                    self.inner.kind.error_label(),
                    PLUGIN_CALL_TIMEOUT.as_millis()
                )));
            }
        };

        serde_json::from_value(value).map_err(|error| {
            AppCoreError::Internal(format!(
                "invalid {} plugin runtime response: {error}",
                self.inner.kind.error_label()
            ))
        })
    }

    async fn ensure_child(&self) -> Result<RuntimeChild, AppCoreError> {
        let mut state = self.inner.state.lock().await;
        if let Some(child) = &state.child
            && child.process.is_alive()
        {
            return Ok(child.clone());
        }

        let child = spawn_runtime_child(self.inner.clone()).await?;
        state.child = Some(child.clone());
        Ok(child)
    }
}

async fn spawn_runtime_child(
    inner: Arc<PluginSidecarRuntimeClientInner>,
) -> Result<RuntimeChild, AppCoreError> {
    let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
    let reader_pending = pending.clone();
    let reader_inner = inner.clone();
    let stdout_handler = Arc::new(move |line: String, process: SupervisedStdioProcess| {
        let pending = reader_pending.clone();
        let inner = reader_inner.clone();
        Box::pin(async move {
            read_child_stdout_line(line, pending, process, inner).await;
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
    });
    let wait_pending = pending.clone();
    let exit_label = inner.kind.process_label();
    let exit_handler = Arc::new(move |exit: SupervisedProcessExit| {
        let pending = wait_pending.clone();
        Box::pin(async move {
            let message = exit.error.unwrap_or_else(|| {
                exit.status
                    .map(|status| format!("{exit_label} exited with {status}"))
                    .unwrap_or_else(|| format!("{exit_label} exited"))
            });
            let mut pending = pending.lock().await;
            for (_, sender) in pending.drain() {
                let _ = sender.send(Err(message.clone()));
            }
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
    });
    let process = SupervisedStdioProcess::spawn(
        SupervisedStdioProcessConfig {
            label: inner.kind.process_label().to_owned(),
            executable: inner.runtime_exe.clone(),
        },
        stdout_handler,
        exit_handler,
    )
    .await?;

    Ok(RuntimeChild { process, pending })
}

async fn read_child_stdout_line(
    line: String,
    pending: PendingMap,
    process: SupervisedStdioProcess,
    inner: Arc<PluginSidecarRuntimeClientInner>,
) {
    let incoming = match serde_json::from_str::<JsonRpcIncoming>(&line) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(
                runtime = inner.kind.process_label(),
                %error,
                "invalid JSON-RPC line from plugin runtime"
            );
            return;
        }
    };

    if let Some(method) = incoming.method.clone() {
        if method == "runtime.ready" {
            tracing::info!(runtime = inner.kind.process_label(), "plugin runtime reported ready");
            return;
        }
        let Some(id) = incoming.id.clone() else {
            return;
        };
        let result = handle_runtime_host_request(&inner, &method, incoming.params).await;
        send_child_response(&process, id, result);
        return;
    }

    let Some(id) = incoming.id.as_ref().map(id_key) else {
        return;
    };
    if let Some(sender) = pending.lock().await.remove(&id) {
        let response = if let Some(error) = incoming.error {
            Err(error.message)
        } else {
            Ok(incoming.result.unwrap_or(Value::Null))
        };
        let _ = sender.send(response);
    }
}

async fn handle_runtime_host_request(
    inner: &PluginSidecarRuntimeClientInner,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    match method {
        "slab.api.request" => {
            let request: PluginRuntimeApiHostRequest =
                serde_json::from_value(params).map_err(|error| error.to_string())?;
            let context = inner
                .pending_calls
                .get(&request.call_id)
                .ok_or_else(|| format!("unknown plugin call id `{}`", request.call_id))?;
            if context.plugin_id != request.plugin_id {
                return Err(format!(
                    "call id `{}` belongs to plugin `{}`, not `{}`",
                    request.call_id, context.plugin_id, request.plugin_id
                ));
            }
            authorize_slab_api_request(&context.permissions.slab_api, &request.request)?;
            let response =
                execute_plugin_api_request(&inner.api_base_url, &request.request).await?;
            serde_json::to_value(response).map_err(|error| error.to_string())
        }
        "slab.ui.emit" => {
            let request: PluginRuntimeUiEmitRequest =
                serde_json::from_value(params).map_err(|error| error.to_string())?;
            let context = inner
                .pending_calls
                .get(&request.call_id)
                .ok_or_else(|| format!("unknown plugin call id `{}`", request.call_id))?;
            if context.plugin_id != request.plugin_id {
                return Err(format!(
                    "call id `{}` belongs to plugin `{}`, not `{}`",
                    request.call_id, context.plugin_id, request.plugin_id
                ));
            }
            let payload = PluginEventPayload {
                plugin_id: request.plugin_id,
                topic: request.topic,
                data: request.data,
                ts: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
                    as u64,
            };
            inner.event_bus.publish(payload.clone());
            serde_json::to_value(payload).map_err(|error| error.to_string())
        }
        _ => Err(format!("unknown {} host method `{method}`", inner.kind.process_label())),
    }
}

fn send_child_response(process: &SupervisedStdioProcess, id: Value, result: Result<Value, String>) {
    let response = match result {
        Ok(result) => JsonRpcResponse { jsonrpc: "2.0", id, result: Some(result), error: None },
        Err(message) => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError { code: -32000, message }),
        },
    };
    if let Ok(line) = serde_json::to_string(&response) {
        let _ = process.send_line(line);
    }
}

fn id_key(id: &Value) -> String {
    match id {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        other => other.to_string(),
    }
}
