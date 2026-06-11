use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use serde_json::Value;
use slab_jsonrpc::{
    application_error_response, id_key, parse_message, request as rpc_request, success_response,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, mpsc, oneshot};

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
use slab_utils::uds::{UnixListener, is_stale_socket_path, prepare_private_socket_directory};

const PLUGIN_CALL_TIMEOUT: Duration = Duration::from_secs(300);
const RUNTIME_SOCKET_ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);

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
    transport: PluginSidecarTransport,
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
    outbound: RuntimeOutbound,
    pending: PendingMap,
}

impl RuntimeChild {
    fn send_line(&self, line: String) -> Result<(), AppCoreError> {
        self.outbound.send_line(line)
    }
}

#[derive(Clone)]
enum RuntimeOutbound {
    Stdio(SupervisedStdioProcess),
    Uds { sender: mpsc::UnboundedSender<String>, label: Arc<str> },
}

impl RuntimeOutbound {
    fn send_line(&self, line: String) -> Result<(), AppCoreError> {
        match self {
            Self::Stdio(process) => process.send_line(line),
            Self::Uds { sender, label } => sender
                .send(line)
                .map_err(|_| AppCoreError::Internal(format!("{} socket writer is closed", label))),
        }
    }
}

#[derive(Clone, Copy)]
enum RuntimeTransportMode {
    Stdio,
    Uds,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum PluginSidecarTransport {
    #[default]
    Stdio,
    Uds,
}

#[derive(Clone)]
struct CallContext {
    plugin_id: String,
    permissions: PluginPermissionsManifest,
}

type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>>;
type ExitHandlerFuture = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;
type ExitHandler = dyn Fn(SupervisedProcessExit) -> ExitHandlerFuture + Send + Sync;

impl PluginSidecarRuntimeClient {
    pub fn new(
        kind: PluginSidecarRuntimeKind,
        transport: PluginSidecarTransport,
        runtime_exe: PathBuf,
        api_base_url: String,
        event_bus: PluginEventBus,
    ) -> Self {
        Self {
            inner: Arc::new(PluginSidecarRuntimeClientInner {
                kind,
                transport,
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
        transport: PluginSidecarTransport,
        api_base_url: String,
        event_bus: PluginEventBus,
    ) -> Self {
        Self::new(kind, transport, kind.resolve_current_server_exe(), api_base_url, event_bus)
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
        let payload =
            serde_json::to_string(&rpc_request(id, "plugin.call", params)).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to serialize plugin runtime JSON-RPC: {error}"
                ))
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

        if child.send_line(payload).is_err() {
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
    match runtime_transport_mode(inner.kind, inner.transport) {
        RuntimeTransportMode::Stdio => spawn_runtime_child_stdio(inner).await,
        RuntimeTransportMode::Uds => spawn_runtime_child_uds(inner).await,
    }
}

async fn spawn_runtime_child_stdio(
    inner: Arc<PluginSidecarRuntimeClientInner>,
) -> Result<RuntimeChild, AppCoreError> {
    let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
    let reader_pending = pending.clone();
    let reader_inner = inner.clone();
    let stdout_handler = Arc::new(move |line: String, process: SupervisedStdioProcess| {
        let pending = reader_pending.clone();
        let inner = reader_inner.clone();
        let outbound = RuntimeOutbound::Stdio(process.clone());
        Box::pin(async move {
            read_runtime_line(line, pending, outbound, inner).await;
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
    });
    let exit_handler = create_exit_handler(pending.clone(), inner.kind.process_label(), None);
    let process = SupervisedStdioProcess::spawn(
        SupervisedStdioProcessConfig {
            label: inner.kind.process_label().to_owned(),
            executable: inner.runtime_exe.clone(),
            arguments: Vec::new(),
        },
        stdout_handler,
        exit_handler,
    )
    .await?;

    Ok(RuntimeChild {
        process: process.clone(),
        outbound: RuntimeOutbound::Stdio(process),
        pending,
    })
}

async fn spawn_runtime_child_uds(
    inner: Arc<PluginSidecarRuntimeClientInner>,
) -> Result<RuntimeChild, AppCoreError> {
    let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
    let socket_path = prepare_runtime_socket_path(inner.kind).await?;
    let mut listener = UnixListener::bind(&socket_path).await.map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to bind {} runtime socket {}: {error}",
            inner.kind.process_label(),
            socket_path.display()
        ))
    })?;

    let stdout_handler = Arc::new(move |_line: String, _process: SupervisedStdioProcess| {
        Box::pin(async {}) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
    });
    let exit_handler =
        create_exit_handler(pending.clone(), inner.kind.process_label(), Some(socket_path.clone()));
    let process = SupervisedStdioProcess::spawn(
        SupervisedStdioProcessConfig {
            label: inner.kind.process_label().to_owned(),
            executable: inner.runtime_exe.clone(),
            arguments: vec!["--socket".to_owned(), socket_path.to_string_lossy().to_string()],
        },
        stdout_handler,
        exit_handler,
    )
    .await?;

    let stream = tokio::time::timeout(RUNTIME_SOCKET_ACCEPT_TIMEOUT, listener.accept())
        .await
        .map_err(|_| {
            AppCoreError::Internal(format!(
                "timed out waiting for {} to connect runtime socket {}",
                inner.kind.process_label(),
                socket_path.display()
            ))
        })?
        .map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to accept {} runtime socket {}: {error}",
                inner.kind.process_label(),
                socket_path.display()
            ))
        })?;

    let (socket_reader, mut socket_writer) = tokio::io::split(stream);
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let runtime_label = Arc::<str>::from(inner.kind.process_label());
    let outbound = RuntimeOutbound::Uds { sender: tx, label: runtime_label.clone() };

    tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            if socket_writer.write_all(line.as_bytes()).await.is_err() {
                break;
            }
            if socket_writer.write_all(b"\n").await.is_err() {
                break;
            }
            if socket_writer.flush().await.is_err() {
                break;
            }
        }
    });

    let reader_pending = pending.clone();
    let reader_inner = inner.clone();
    let reader_outbound = outbound.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(socket_reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            read_runtime_line(
                line,
                reader_pending.clone(),
                reader_outbound.clone(),
                reader_inner.clone(),
            )
            .await;
        }
    });

    Ok(RuntimeChild { process, outbound, pending })
}

async fn read_runtime_line(
    line: String,
    pending: PendingMap,
    outbound: RuntimeOutbound,
    inner: Arc<PluginSidecarRuntimeClientInner>,
) {
    let incoming = match parse_message(&line) {
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
        send_child_response(&outbound, id, result);
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

fn send_child_response(outbound: &RuntimeOutbound, id: Value, result: Result<Value, String>) {
    let response = match result {
        Ok(result) => success_response(id, result),
        Err(message) => application_error_response(id, message),
    };
    if let Ok(line) = serde_json::to_string(&response) {
        let _ = outbound.send_line(line);
    }
}

fn create_exit_handler(
    pending: PendingMap,
    exit_label: &'static str,
    socket_path: Option<PathBuf>,
) -> Arc<ExitHandler> {
    Arc::new(move |exit: SupervisedProcessExit| {
        let pending = pending.clone();
        let socket_path = socket_path.clone();
        Box::pin(async move {
            if let Some(path) = socket_path {
                let _ = tokio::fs::remove_file(path).await;
            }
            let message = exit.error.unwrap_or_else(|| {
                exit.status
                    .map(|status| format!("{exit_label} exited with {status}"))
                    .unwrap_or_else(|| format!("{exit_label} exited"))
            });
            let mut pending = pending.lock().await;
            for (_, sender) in pending.drain() {
                let _ = sender.send(Err(message.clone()));
            }
        })
    })
}

fn runtime_transport_mode(
    kind: PluginSidecarRuntimeKind,
    transport: PluginSidecarTransport,
) -> RuntimeTransportMode {
    match kind {
        PluginSidecarRuntimeKind::JavaScript | PluginSidecarRuntimeKind::Python => {
            match transport {
                PluginSidecarTransport::Stdio => RuntimeTransportMode::Stdio,
                PluginSidecarTransport::Uds => RuntimeTransportMode::Uds,
            }
        }
    }
}

async fn prepare_runtime_socket_path(
    kind: PluginSidecarRuntimeKind,
) -> Result<PathBuf, AppCoreError> {
    let socket_dir = std::env::temp_dir().join("slab-runtime").join("plugin-runtime");
    prepare_private_socket_directory(&socket_dir).await.map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to prepare {} runtime socket directory {}: {error}",
            kind.process_label(),
            socket_dir.display()
        ))
    })?;

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let socket_path =
        socket_dir.join(format!("{}-{}-{now}.sock", kind.process_label(), std::process::id()));

    if tokio::fs::try_exists(&socket_path).await.unwrap_or(false)
        && is_stale_socket_path(&socket_path).await.unwrap_or(false)
    {
        let _ = tokio::fs::remove_file(&socket_path).await;
    }

    Ok(socket_path)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio::sync::{Mutex, oneshot};

    use super::{
        PendingMap, PluginSidecarRuntimeKind, PluginSidecarTransport, SupervisedProcessExit,
        create_exit_handler, prepare_runtime_socket_path, runtime_transport_mode,
    };

    #[test]
    fn runtime_transport_mode_uses_configured_js_transport() {
        assert!(matches!(
            runtime_transport_mode(
                PluginSidecarRuntimeKind::JavaScript,
                PluginSidecarTransport::Uds
            ),
            super::RuntimeTransportMode::Uds
        ));
        assert!(matches!(
            runtime_transport_mode(
                PluginSidecarRuntimeKind::JavaScript,
                PluginSidecarTransport::Stdio
            ),
            super::RuntimeTransportMode::Stdio
        ));
        assert!(matches!(
            runtime_transport_mode(PluginSidecarRuntimeKind::Python, PluginSidecarTransport::Uds),
            super::RuntimeTransportMode::Uds
        ));
    }

    #[tokio::test]
    async fn prepare_runtime_socket_path_creates_private_directory() {
        let socket_path = prepare_runtime_socket_path(PluginSidecarRuntimeKind::JavaScript)
            .await
            .expect("socket path");

        let parent = socket_path.parent().expect("socket parent");
        assert!(parent.ends_with("plugin-runtime"));
        assert!(tokio::fs::try_exists(parent).await.expect("socket dir exists"));
    }

    #[tokio::test]
    async fn exit_handler_removes_socket_and_fails_pending_calls() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let socket_path = temp_dir.path().join("runtime.sock");
        tokio::fs::write(&socket_path, b"socket").await.expect("write socket marker");

        let pending: PendingMap = std::sync::Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx) = oneshot::channel();
        pending.lock().await.insert("test-call".to_owned(), tx);
        let exit_handler =
            create_exit_handler(pending, "slab-js-runtime", Some(socket_path.clone()));

        exit_handler(SupervisedProcessExit { status: Some("1".to_owned()), error: None }).await;

        let message = rx.await.expect("pending call response").expect_err("exit error");
        assert!(message.contains("slab-js-runtime exited with 1"));
        assert!(!tokio::fs::try_exists(&socket_path).await.expect("socket path check"));
    }
}
