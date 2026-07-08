//! `/v1/agents/harness` — slab-owned WebSocket JSON-RPC 2.0 control plane.
//!
//! Complements [`super::handler`]'s OpenAI-compatible `/v1/agents/responses`.
//! The wire contract lives in `slab_proto::harness`; this module owns transport
//! (axum WS ↔ [`slab_jsonrpc::ws`]) and dispatch (method → [`AgentService`]).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use slab_agent::port::{ThreadListFilter, ThreadSnapshot};
use slab_app_core::application::agent::projection::harness::HarnessProjection;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::{AgentService, WorkspaceService};
use slab_cloud_provider::{CloudModelSpec, default_models_for_provider};
use slab_jsonrpc::host::{HostConfig, NotificationHandler, RequestHandler};
use slab_jsonrpc::ws::{WsFrame, serve_websocket};
use slab_jsonrpc::{JSONRPCMessage, JSONRPCNotification};
use slab_proto::harness::event::EventMsg;
use slab_proto::harness::messages::{
    ApprovalPolicy, ApprovalResolveParams, ApprovalResolveResult, InitializeResult,
    ReasoningEffort, SandboxPolicy, ServerCapabilities, ServerInfo, ShutdownParams, ShutdownResult,
    Thread, ThreadArchiveParams, ThreadArchiveResult, ThreadForkParams, ThreadForkResult,
    ThreadListParams, ThreadListResult, ThreadResumeParams, ThreadResumeResult,
    ThreadRollbackParams, ThreadRollbackResult, ThreadStartParams, ThreadStartResult, Turn,
    TurnInterruptParams, TurnInterruptResult, TurnStartParams, TurnStartResult,
    WorkspaceMigrateParams, WorkspaceMigrateResult,
};
use slab_proto::harness::method;
use slab_proto::harness::notification::{ErrorParams, TurnCompletedParams};
use slab_proto::harness::{ModelInfo, ModelListParams, ModelListResult, ReasoningEffortOption};
use slab_types::{ConversationMessage, ConversationMessageContent};
use tokio::sync::{broadcast, mpsc};

use crate::api::v1::agent::schema::AgentConfigInput;

#[utoipa::path(
    get,
    path = "/v1/agents/harness",
    tag = "agents",
    params(
        ("token" = Option<String>, Query, description = "Slab session id (browsers cannot set WS headers)")
    ),
    responses(
        (status = 101, description = "WebSocket upgrade for the JSON-RPC 2.0 harness protocol"),
    )
)]
pub async fn agent_harness(
    ws: WebSocketUpgrade,
    Query(query): Query<HarnessQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let session_id = query
        .token
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "assistant-default".to_owned());
    let service = state.services.agent.clone();
    ws.on_upgrade(move |socket| run_harness_socket(socket, state, service, session_id))
}

#[derive(Debug, serde::Deserialize)]
pub struct HarnessQuery {
    #[serde(default)]
    token: Option<String>,
}

async fn run_harness_socket(
    socket: WebSocket,
    state: Arc<AppState>,
    service: AgentService,
    session_id: String,
) {
    let (sink, stream) = socket.split();
    // Adapt axum's `Message` ↔ transport-agnostic `WsFrame`. Both adapters
    // hold async blocks (so they are not `Unpin`); pin them so they satisfy
    // `serve_websocket`'s `Sink`/`Stream` bounds.
    let sink = Box::pin(
        sink.with(|frame: WsFrame| async move { Ok::<_, axum::Error>(frame_to_message(frame)) }),
    );
    let stream = Box::pin(stream.filter_map(|msg| async move {
        match msg.ok()? {
            Message::Text(text) => Some(Ok(WsFrame::Text(text.to_string()))),
            Message::Binary(bytes) => Some(Ok(WsFrame::Binary(bytes.to_vec()))),
            Message::Close(_) => Some(Ok(WsFrame::Close)),
            // Pings/pongs are handled by axum; ignore here.
            _ => None,
        }
    }));

    let (outbound_tx, outbound_rx) = mpsc::unbounded_channel::<JSONRPCMessage>();
    let dispatcher =
        Arc::new(HarnessDispatcher::new(state, service, session_id, outbound_tx.clone()));
    let notifications = Arc::new(NoopNotifications);

    if let Err(error) = serve_websocket(
        dispatcher,
        notifications,
        outbound_tx,
        outbound_rx,
        sink,
        stream,
        HostConfig::default(),
    )
    .await
    {
        tracing::warn!(%error, "harness websocket ended");
    }
}

fn frame_to_message(frame: WsFrame) -> Message {
    match frame {
        WsFrame::Text(text) => Message::Text(text.into()),
        WsFrame::Binary(bytes) => Message::Binary(bytes.into()),
        WsFrame::Close => Message::Close(None),
    }
}

/// No-op inbound-notification handler. The harness server currently sends
/// notifications and receives requests; client→server notifications are
/// accepted but ignored.
struct NoopNotifications;

#[async_trait::async_trait]
impl NotificationHandler for NoopNotifications {
    async fn handle_notification(&self, _method: String, _params: Value) {}
}

/// Per-thread binding: maps the harness-visible thread id to the real slab
/// thread id once the first turn materializes the thread.
#[derive(Default)]
struct ThreadBinding {
    real_id: Option<String>,
}

struct HarnessDispatcher {
    state: Arc<AppState>,
    service: AgentService,
    session_id: String,
    outbound: mpsc::UnboundedSender<JSONRPCMessage>,
    initialized: AtomicBool,
    bindings: StdMutex<HashMap<String, ThreadBinding>>,
    next_thread_id: AtomicU64,
}

impl HarnessDispatcher {
    fn new(
        state: Arc<AppState>,
        service: AgentService,
        session_id: String,
        outbound: mpsc::UnboundedSender<JSONRPCMessage>,
    ) -> Self {
        Self {
            state,
            service,
            session_id,
            outbound,
            initialized: AtomicBool::new(false),
            bindings: StdMutex::new(HashMap::new()),
            next_thread_id: AtomicU64::new(1),
        }
    }

    /// Resolve the real slab thread id for a harness thread id, falling back to
    /// the harness id itself (so direct/external thread ids still work).
    fn real_id_for(&self, harness_id: &str) -> String {
        self.bindings
            .lock()
            .expect("bindings mutex")
            .get(harness_id)
            .and_then(|binding| binding.real_id.clone())
            .unwrap_or_else(|| harness_id.to_owned())
    }

    fn mint_thread_id(&self) -> String {
        format!("hthread-{}", self.next_thread_id.fetch_add(1, Ordering::Relaxed))
    }
}

#[async_trait::async_trait]
impl RequestHandler for HarnessDispatcher {
    async fn handle_request(&self, method: String, params: Value) -> Result<Value, String> {
        if method.as_str() != method::INITIALIZE && !self.initialized.load(Ordering::Acquire) {
            return Err("harness socket not initialized: send `initialize` first".to_owned());
        }

        match method.as_str() {
            method::INITIALIZE => self.handle_initialize(),
            method::THREAD_START => self.handle_thread_start(parse(params)?).await,
            method::THREAD_RESUME => self.handle_thread_resume(parse(params)?).await,
            method::THREAD_FORK => self.handle_thread_fork(parse(params)?).await,
            method::THREAD_ROLLBACK => self.handle_thread_rollback(parse(params)?).await,
            method::THREAD_ARCHIVE => self.handle_thread_archive(parse(params)?).await,
            method::THREAD_LIST => self.handle_thread_list(parse(params)?).await,
            method::TURN_START => self.handle_turn_start(parse(params)?).await,
            method::TURN_INTERRUPT => self.handle_turn_interrupt(parse(params)?).await,
            method::MODEL_LIST => self.handle_model_list(parse(params)?).await,
            method::APPROVAL_RESOLVE => self.handle_approval_resolve(parse(params)?).await,
            method::SHUTDOWN => self.handle_shutdown(parse(params)?).await,
            method::WORKSPACE_MIGRATE => self.handle_workspace_migrate(parse(params)?).await,
            other => Err(format!("unknown harness method `{other}`")),
        }
    }
}

fn parse<T: DeserializeOwned>(params: Value) -> Result<T, String> {
    serde_json::from_value(params).map_err(|error| format!("invalid params: {error}"))
}

fn ok_value<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

impl HarnessDispatcher {
    fn handle_initialize(&self) -> Result<Value, String> {
        self.initialized.store(true, Ordering::Release);
        ok_value(InitializeResult {
            server_info: Some(ServerInfo {
                name: "slab-server".to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            }),
            protocol_version: Some("1.0".to_owned()),
            capabilities: Some(ServerCapabilities::default()),
        })
    }

    async fn handle_thread_start(&self, params: ThreadStartParams) -> Result<Value, String> {
        let thread_id = self.mint_thread_id();
        let model_provider =
            params.model_provider.clone().or_else(|| params.model.clone()).unwrap_or_default();
        self.bindings
            .lock()
            .expect("bindings mutex")
            .insert(thread_id.clone(), ThreadBinding::default());

        let cwd =
            params.cwd.as_ref().map(|path| path.to_string_lossy().into_owned()).unwrap_or_default();
        let thread = Thread {
            id: thread_id.clone(),
            preview: String::new(),
            model_provider: model_provider.clone(),
            created_at: Utc::now().timestamp_millis(),
            cwd: (!cwd.is_empty()).then_some(cwd.clone()),
            ..Default::default()
        };
        ok_value(ThreadStartResult {
            thread,
            model: params.model.clone().unwrap_or_default(),
            model_provider,
            cwd,
            approval_policy: params.approval_policy.unwrap_or(ApprovalPolicy::OnRequest),
            sandbox: SandboxPolicy::default(),
            reasoning_effort: None,
        })
    }

    async fn handle_turn_start(&self, params: TurnStartParams) -> Result<Value, String> {
        let existing_real = self
            .bindings
            .lock()
            .expect("bindings mutex")
            .get(&params.thread_id)
            .and_then(|binding| binding.real_id.clone());

        match existing_real {
            Some(real_id) => {
                let content = join_user_text(&params.input);
                self.service
                    .send_input(&real_id, content)
                    .await
                    .map_err(|error| error.to_string())?;
            }
            None => {
                // First turn materializes the slab thread (create + run).
                let config =
                    AgentConfigInput { model: params.model.clone(), ..Default::default() }.into();
                let messages = messages_from_input(&params.input);
                let real_id = self
                    .service
                    .spawn(self.session_id.clone(), config, messages)
                    .await
                    .map_err(|error| error.to_string())?;
                self.bindings.lock().expect("bindings mutex").insert(
                    params.thread_id.clone(),
                    ThreadBinding { real_id: Some(real_id.clone()) },
                );
                spawn_event_fanout(
                    self.service.clone(),
                    real_id,
                    params.thread_id.clone(),
                    self.outbound.clone(),
                );
            }
        }

        ok_value(TurnStartResult {
            turn: Turn {
                id: "0".to_owned(),
                items: vec![],
                status: "inProgress".to_owned(),
                error: None,
            },
        })
    }

    async fn handle_turn_interrupt(&self, params: TurnInterruptParams) -> Result<Value, String> {
        let real_id = self.real_id_for(&params.thread_id);
        self.service.interrupt(&real_id).await.map_err(|error| error.to_string())?;
        ok_value(TurnInterruptResult { status: Some("interrupting".to_owned()) })
    }

    async fn handle_approval_resolve(
        &self,
        params: ApprovalResolveParams,
    ) -> Result<Value, String> {
        let real_id = self.real_id_for(&params.thread_id);
        let delivered = self.service.approve_call(&real_id, &params.item_id, params.approved);
        ok_value(ApprovalResolveResult { delivered: Some(delivered), status: None })
    }

    async fn handle_shutdown(&self, params: ShutdownParams) -> Result<Value, String> {
        let real_id = self.real_id_for(&params.thread_id);
        self.service.shutdown(&real_id).await.map_err(|error| error.to_string())?;
        ok_value(ShutdownResult { status: Some("shutdown".to_owned()) })
    }

    async fn handle_workspace_migrate(
        &self,
        params: WorkspaceMigrateParams,
    ) -> Result<Value, String> {
        let config = &self.state.context.config;
        let workspace_root = params
            .workspace_root
            .or_else(|| WorkspaceService::workspace_root_from_config(config))
            .ok_or_else(|| "no active workspace to migrate".to_owned())?;
        let snapshot_dir = std::path::PathBuf::from(&config.session_state_dir);
        let outcome = self
            .service
            .prepare_workspace_migration(&workspace_root, &snapshot_dir)
            .await
            .map_err(|error| error.to_string())?;
        ok_value(WorkspaceMigrateResult {
            project_id: Some(outcome.project_id),
            suspended_count: outcome.suspended_count as u32,
        })
    }

    async fn handle_thread_list(&self, params: ThreadListParams) -> Result<Value, String> {
        let filter = ThreadListFilter {
            limit: params.limit,
            before_updated_at: params.cursor.clone(),
            // Archived threads (soft-deleted via `thread/archive`) are hidden
            // from the default list. Callers opt in via `include_archived`.
            include_archived: false,
        };
        let snapshots = self
            .service
            .list_session_threads_filtered(&self.session_id, &filter)
            .await
            .map_err(|error| error.to_string())?;
        let next_cursor = match (params.limit, snapshots.last()) {
            (Some(limit), Some(last)) if (snapshots.len() as u32) >= limit => {
                Some(last.updated_at.clone())
            }
            _ => None,
        };
        let data: Vec<Thread> = snapshots.iter().map(thread_from_snapshot).collect();
        ok_value(ThreadListResult { data, next_cursor })
    }

    async fn handle_model_list(&self, params: ModelListParams) -> Result<Value, String> {
        // Curated catalog of *configured* providers only — `default_models_for_provider`
        // returns the static flagship set for each provider's family. The optional
        // `model_providers` filter matches on provider id.
        let providers = &self.state.context.pmid.config().chat.providers;
        let data: Vec<ModelInfo> = providers
            .iter()
            .filter(|provider| match params.model_providers.as_ref() {
                Some(ids) => ids.iter().any(|id| id == &provider.id),
                None => true,
            })
            .flat_map(|provider| {
                let provider_id = provider.id.clone();
                default_models_for_provider(provider)
                    .into_iter()
                    .map(move |spec| model_info_from_spec(&provider_id, &spec))
            })
            .collect();
        ok_value(ModelListResult { data, next_cursor: None })
    }

    async fn handle_thread_fork(&self, params: ThreadForkParams) -> Result<Value, String> {
        // `sandbox_override` is accepted but not applied: `AgentConfig` has no
        // sandbox field, matching `thread/start` which also keeps sandbox out of
        // the agent config. The model override is honored.
        let real_parent = self.real_id_for(&params.thread_id);
        let snapshot = self
            .service
            .fork_thread(&real_parent, params.model_override.clone())
            .await
            .map_err(|error| error.to_string())?;
        let harness_id = self.mint_thread_id();
        self.bindings
            .lock()
            .expect("bindings mutex")
            .insert(harness_id.clone(), ThreadBinding { real_id: Some(snapshot.id.clone()) });
        // Establish the event stream so a subsequent `turn/start` (append path)
        // has a fan-out task to deliver through.
        spawn_event_fanout(
            self.service.clone(),
            snapshot.id.clone(),
            harness_id.clone(),
            self.outbound.clone(),
        );
        ok_value(ThreadForkResult { thread: thread_from_snapshot_with_id(&harness_id, &snapshot) })
    }

    async fn handle_thread_rollback(&self, params: ThreadRollbackParams) -> Result<Value, String> {
        let to_turn_index: u32 = params
            .to_turn_id
            .parse()
            .map_err(|error| format!("invalid to_turn_id `{}`: {error}", params.to_turn_id))?;
        let real_id = self.real_id_for(&params.thread_id);
        let snapshot = self
            .service
            .rollback_thread(&real_id, to_turn_index)
            .await
            .map_err(|error| error.to_string())?;
        ok_value(ThreadRollbackResult { thread: thread_from_snapshot(&snapshot) })
    }

    async fn handle_thread_archive(&self, params: ThreadArchiveParams) -> Result<Value, String> {
        let real_id = self.real_id_for(&params.thread_id);
        let snapshot =
            self.service.archive_thread(&real_id).await.map_err(|error| error.to_string())?;
        ok_value(ThreadArchiveResult { thread: thread_from_snapshot(&snapshot) })
    }

    async fn handle_thread_resume(&self, params: ThreadResumeParams) -> Result<Value, String> {
        // Resolve the target thread: an explicit id wins (harness id or real id);
        // otherwise fall back to the session's most-recent root thread.
        let (harness_id, snapshot) = match params.thread_id.as_deref() {
            Some(id) => {
                let real_id = self.real_id_for(id);
                let snapshot = self
                    .service
                    .thread_snapshot(&real_id)
                    .await
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| format!("thread not found: {id}"))?;
                (id.to_owned(), snapshot)
            }
            None => {
                let restored = self
                    .service
                    .restore_session(&self.session_id)
                    .await
                    .map_err(|error| error.to_string())?;
                let snapshot =
                    restored.thread.ok_or_else(|| "no thread to resume for session".to_owned())?;
                (self.mint_thread_id(), snapshot)
            }
        };
        self.bindings
            .lock()
            .expect("bindings mutex")
            .insert(harness_id.clone(), ThreadBinding { real_id: Some(snapshot.id.clone()) });
        // Replay persisted history + establish the live event stream.
        spawn_event_fanout(
            self.service.clone(),
            snapshot.id.clone(),
            harness_id.clone(),
            self.outbound.clone(),
        );
        ok_value(ThreadResumeResult {
            thread: thread_from_snapshot_with_id(&harness_id, &snapshot),
        })
    }
}

/// Map a persisted [`ThreadSnapshot`] to the harness [`Thread`] wire type.
fn thread_from_snapshot(snapshot: &ThreadSnapshot) -> Thread {
    thread_from_snapshot_with_id(&snapshot.id, snapshot)
}

/// Like [`thread_from_snapshot`] but overrides the wire id (used by
/// `thread/fork` / `thread/resume`, which surface a harness-local id).
fn thread_from_snapshot_with_id(id: &str, snapshot: &ThreadSnapshot) -> Thread {
    let created_at = chrono::DateTime::parse_from_rfc3339(&snapshot.created_at)
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0);
    Thread {
        id: id.to_owned(),
        preview: snapshot.completion_text.clone().unwrap_or_default(),
        // model_provider would require parsing the AgentConfig JSON; left empty
        // until a structured accessor exists.
        model_provider: String::new(),
        created_at,
        ..Default::default()
    }
}

/// Project a curated [`CloudModelSpec`] for a configured provider into the
/// harness [`ModelInfo`] wire type.
fn model_info_from_spec(provider_id: &str, spec: &CloudModelSpec) -> ModelInfo {
    ModelInfo {
        id: format!("{provider_id}:{}", spec.remote_model_id),
        model: spec.remote_model_id.clone(),
        display_name: spec.display_name.clone(),
        description: spec.description.clone(),
        // v1: every model advertises the full standard reasoning-effort set
        // with a Medium default; per-model curation can refine this later.
        supported_reasoning_efforts: standard_reasoning_efforts(),
        default_reasoning_effort: ReasoningEffort::Medium,
        is_default: spec.is_default,
    }
}

/// The default reasoning-effort options surfaced for every model in v1.
fn standard_reasoning_efforts() -> Vec<ReasoningEffortOption> {
    [
        ReasoningEffort::Off,
        ReasoningEffort::Low,
        ReasoningEffort::Medium,
        ReasoningEffort::High,
        ReasoningEffort::Xhigh,
    ]
    .into_iter()
    .map(|effort| ReasoningEffortOption {
        reasoning_effort: effort,
        description: format!("{effort:?}").to_lowercase(),
    })
    .collect()
}

/// Spawn a per-thread fan-out task: subscribe to the agent event stream,
/// project each envelope to harness events, and push them as JSON-RPC
/// notifications onto the shared outbound channel.
fn spawn_event_fanout(
    service: AgentService,
    real_thread_id: String,
    harness_thread_id: String,
    outbound: mpsc::UnboundedSender<JSONRPCMessage>,
) {
    tokio::spawn(async move {
        let subscription = service.subscribe_events(&real_thread_id);
        let mut proj = HarnessProjection::new();

        for envelope in &subscription.replay {
            for msg in proj.project(&harness_thread_id, envelope) {
                push_event(&outbound, &harness_thread_id, msg);
            }
        }

        let mut receiver = subscription.receiver;
        loop {
            match receiver.recv().await {
                Ok(envelope) => {
                    for msg in proj.project(&harness_thread_id, &envelope) {
                        push_event(&outbound, &harness_thread_id, msg);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let _ = outbound.send(error_notification(
                        Some(&harness_thread_id),
                        "stream_lagged",
                        "agent event stream lagged; some events may have been dropped",
                        None,
                    ));
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// Push one projected harness event onto the outbound channel as a
/// JSON-RPC notification. `Error`/`TurnAborted` are adapted (they do not lift
/// directly via [`EventMsg::into_notification`]); `Warning` is dropped.
fn push_event(outbound: &mpsc::UnboundedSender<JSONRPCMessage>, thread_id: &str, msg: EventMsg) {
    let notification = match msg {
        EventMsg::Error(error) => error_notification(
            Some(thread_id),
            error.code.as_deref().unwrap_or("error"),
            &error.message,
            error.data,
        ),
        EventMsg::TurnAborted(aborted) => jsonrpc_notification(
            method::TURN_COMPLETED,
            serde_json::to_value(TurnCompletedParams {
                thread_id: aborted.thread_id,
                turn: aborted.turn,
            })
            .unwrap_or(Value::Null),
        ),
        EventMsg::Warning(_) => return,
        other => match other.into_notification() {
            Some(notification) => {
                jsonrpc_notification(notification.method(), notification.payload())
            }
            None => return,
        },
    };
    let _ = outbound.send(notification);
}

fn jsonrpc_notification(method: &str, params: Value) -> JSONRPCMessage {
    JSONRPCMessage::Notification(JSONRPCNotification {
        method: method.to_owned(),
        params: Some(params),
    })
}

fn error_notification(
    thread_id: Option<&str>,
    code: &str,
    message: &str,
    data: Option<Value>,
) -> JSONRPCMessage {
    jsonrpc_notification(
        method::ERROR,
        serde_json::to_value(ErrorParams {
            thread_id: thread_id.map(str::to_owned),
            turn_id: None,
            item_id: None,
            code: code.to_owned(),
            message: message.to_owned(),
            data,
        })
        .unwrap_or(Value::Null),
    )
}

/// Flatten the text of all [`slab_proto::harness::UserInput::Text`] items into a
/// single user string (other input kinds are not yet wired).
fn join_user_text(input: &[slab_proto::harness::UserInput]) -> String {
    let mut text = String::new();
    for item in input {
        if let slab_proto::harness::UserInput::Text { text: t, .. } = item {
            text.push_str(t);
        }
    }
    text
}

fn messages_from_input(input: &[slab_proto::harness::UserInput]) -> Vec<ConversationMessage> {
    let text = join_user_text(input);
    if text.trim().is_empty() {
        Vec::new()
    } else {
        vec![ConversationMessage {
            role: "user".to_owned(),
            content: ConversationMessageContent::Text(text),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_user_text_concatenates_text_inputs() {
        let input = vec![
            slab_proto::harness::UserInput::Text { text: "hel".into(), text_elements: vec![] },
            slab_proto::harness::UserInput::Text { text: "lo".into(), text_elements: vec![] },
        ];
        assert_eq!(join_user_text(&input), "hello");
    }

    #[test]
    fn messages_from_input_builds_single_user_message() {
        let input =
            vec![slab_proto::harness::UserInput::Text { text: "hi".into(), text_elements: vec![] }];
        let messages = messages_from_input(&input);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
    }

    #[test]
    fn error_notification_carries_thread_and_code() {
        let msg = error_notification(Some("t1"), "turn_failed", "boom", None);
        let JSONRPCMessage::Notification(n) = msg else {
            panic!("expected notification");
        };
        assert_eq!(n.method, "error");
        let params = n.params.expect("params");
        assert_eq!(params["threadId"], "t1");
        assert_eq!(params["code"], "turn_failed");
    }

    fn spec(remote: &str, label: &str, is_default: bool) -> CloudModelSpec {
        CloudModelSpec {
            remote_model_id: remote.to_owned(),
            display_name: label.to_owned(),
            description: label.to_owned(),
            is_default,
        }
    }

    #[test]
    fn model_info_from_spec_namespaces_id_under_provider() {
        let info = model_info_from_spec(
            "anthropic",
            &spec("claude-sonnet-4-5", "Claude Sonnet 4.5", true),
        );
        assert_eq!(info.id, "anthropic:claude-sonnet-4-5");
        assert_eq!(info.model, "claude-sonnet-4-5");
        assert_eq!(info.display_name, "Claude Sonnet 4.5");
        assert!(info.is_default);
        assert_eq!(info.default_reasoning_effort, ReasoningEffort::Medium);
    }

    #[test]
    fn standard_reasoning_efforts_covers_full_set() {
        let efforts: Vec<ReasoningEffort> =
            standard_reasoning_efforts().into_iter().map(|o| o.reasoning_effort).collect();
        assert_eq!(
            efforts,
            vec![
                ReasoningEffort::Off,
                ReasoningEffort::Low,
                ReasoningEffort::Medium,
                ReasoningEffort::High,
                ReasoningEffort::Xhigh,
            ]
        );
    }

    #[test]
    fn rollback_to_turn_id_parses_as_index() {
        // Mirrors the inline parse in `handle_thread_rollback`.
        let parsed: u32 = "3".parse().expect("numeric turn id");
        assert_eq!(parsed, 3);
        assert!("x".parse::<u32>().is_err(), "non-numeric turn id must be rejected");
    }
}
