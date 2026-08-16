//! `/v1/agents/harness` — slab-owned WebSocket JSON-RPC 2.0 control plane.
//!
//! This module owns the transport (axum WS ↔ [`slab_jsonrpc::ws`]) and the
//! wire-type mapping helpers. Behavior is split across submodules:
//! - [`session`] — per-connection transient state + the agent event fan-out.
//! - [`body`] — typed request handlers (one per method).
//! - [`host`] — the `RequestHandler` that gates `initialize` and dispatches
//!   everything else through a [`slab_jsonrpc::router::Router`].
//!
//! The wire contract lives in `slab_proto::harness`.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use slab_agent::port::{ThreadMessageRecord, ThreadSnapshot, TurnItemRecord, TurnStateRecord};
use slab_agent::protocol::{Thread, Turn, TurnItem, UserMessageContent};
use slab_app_core::context::AppState;
use slab_app_core::domain::services::HarnessService;
use slab_cloud_provider::CloudModelSpec;
use slab_jsonrpc::JSONRPCMessage;
use slab_jsonrpc::host::{HostConfig, NotificationHandler};
use slab_jsonrpc::notifier::Notifier;
use slab_jsonrpc::ws::{WsFrame, serve_websocket};
use slab_proto::harness::messages::ReasoningEffort;
use slab_proto::harness::{ModelInfo, ReasoningEffortOption};
use slab_types::{ConversationContentPart, ConversationMessage, ConversationMessageContent};
use tokio::sync::mpsc;

mod body;
mod host;
mod session;
mod transform;

use host::HarnessHost;

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
    let service = state.services.harness.clone();
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
    service: HarnessService,
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
    let notifier = Notifier::new(outbound_tx.clone());
    let host = Arc::new(HarnessHost::new(session_id, state, service, notifier));
    let notifications = Arc::new(NoopNotifications);

    if let Err(error) = serve_websocket(
        host,
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

// ── wire-type mapping helpers (persisted snapshot → harness wire types) ────

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

/// Map one persisted message into one or more harness [`TurnItem`]s.
///
/// `role:"user"` → a single `UserMessage`; `role:"assistant"` → an
/// `AgentMessage` (text) plus one `McpToolCall` per emitted tool call; any other
/// role (e.g. tool results) → a `CommandExecution` surfacing the rendered output.
fn turn_items_for_message(message: &ThreadMessageRecord) -> Vec<TurnItem> {
    let id = message.id.clone();
    let record = &message.message;
    match record.role.as_str() {
        "user" => vec![TurnItem::UserMessage {
            id,
            content: vec![UserMessageContent::Text { text: record.content.rendered_text() }],
        }],
        "assistant" => {
            let mut items = Vec::new();
            let text = record.content.rendered_text();
            if !text.trim().is_empty() {
                items.push(TurnItem::AgentMessage { id: format!("{id}-text"), text });
            }
            for (index, call) in record.tool_calls.iter().enumerate() {
                let arguments =
                    serde_json::from_str(&call.function.arguments).unwrap_or(Value::Null);
                items.push(TurnItem::McpToolCall {
                    id: call.id.clone().unwrap_or_else(|| format!("{id}-tool-{index}")),
                    server: String::new(),
                    tool: call.function.name.clone(),
                    arguments,
                    status: "completed".to_owned(),
                    result: None,
                    error: None,
                    duration_ms: None,
                });
            }
            if items.is_empty() {
                items.push(TurnItem::AgentMessage { id, text: String::new() });
            }
            items
        }
        _ => vec![TurnItem::CommandExecution {
            id,
            command: String::new(),
            cwd: String::new(),
            process_id: None,
            status: "completed".to_owned(),
            aggregated_output: Some(record.rendered_text()),
            exit_code: None,
            duration_ms: None,
        }],
    }
}

/// Like [`thread_from_snapshot_with_id`] but populates `turns` for `thread/resume`.
///
/// Turns with persisted full-fidelity `TurnItem` snapshots (in `turn_items`)
/// are rebuilt verbatim from those — user prompt (from `messages`) followed by
/// the assistant-side items in arrival order. Turns WITHOUT snapshots
/// (pre-migration history that was never captured) fall back to lossy synthesis
/// from `messages` via [`turn_items_for_message`], so old threads keep rendering
/// whatever was persisted instead of going blank.
fn thread_from_snapshot_with_turns(
    id: &str,
    snapshot: &ThreadSnapshot,
    messages: &[ThreadMessageRecord],
    turn_states: &[TurnStateRecord],
    turn_items: &[TurnItemRecord],
) -> Thread {
    let created_at = chrono::DateTime::parse_from_rfc3339(&snapshot.created_at)
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0);

    // Decode persisted TurnItem snapshots, grouped by turn. SQL returns them
    // ordered by (turn_index, seq); decode failures are skipped with a warning.
    let mut items_by_turn: std::collections::BTreeMap<u32, Vec<TurnItem>> =
        std::collections::BTreeMap::new();
    for record in turn_items {
        match serde_json::from_str::<TurnItem>(&record.item_json) {
            Ok(item) => items_by_turn.entry(record.turn_index).or_default().push(item),
            Err(error) => {
                tracing::warn!(
                    thread_id = %snapshot.id,
                    turn_index = record.turn_index,
                    item_id = %record.id,
                    error = %error,
                    "failed to decode persisted TurnItem; skipping",
                );
            }
        }
    }

    // Group messages by turn index, preserving chronological order within a turn.
    let mut msgs_by_turn: std::collections::BTreeMap<u32, Vec<&ThreadMessageRecord>> =
        std::collections::BTreeMap::new();
    for message in messages {
        msgs_by_turn.entry(message.turn_index).or_default().push(message);
    }
    for msgs in msgs_by_turn.values_mut() {
        msgs.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    }

    // Union of turn indices across snapshots and messages.
    let mut indices: std::collections::BTreeSet<u32> = items_by_turn.keys().copied().collect();
    indices.extend(msgs_by_turn.keys().copied());

    let turns = indices
        .into_iter()
        .map(|index| {
            let persisted = items_by_turn.remove(&index).unwrap_or_default();
            let msgs = msgs_by_turn.get(&index);
            let items = if !persisted.is_empty() {
                // Full-fidelity turn: user prompt (from messages) + persisted items.
                let mut items = Vec::new();
                if let Some(user) = msgs.and_then(|ms| ms.iter().find(|m| m.message.role == "user"))
                    && let Some(item) = user_message_item(user)
                {
                    items.push(item);
                }
                items.extend(persisted);
                items
            } else {
                // Pre-migration turn (no snapshots): synthesize from messages.
                let mut items = Vec::new();
                if let Some(msgs) = msgs {
                    for message in msgs {
                        items.extend(turn_items_for_message(message));
                    }
                }
                items
            };
            let status = turn_states
                .iter()
                .find(|state| state.turn_index == index)
                .map(|state| state.status.clone())
                .filter(|status| !status.trim().is_empty())
                .unwrap_or_else(|| "completed".to_owned());
            Turn { id: index.to_string(), items, status, error: None }
        })
        .collect();

    Thread {
        id: id.to_owned(),
        preview: snapshot.completion_text.clone().unwrap_or_default(),
        model_provider: String::new(),
        created_at,
        turns,
        ..Default::default()
    }
}

/// Build a `UserMessage` item from a persisted user-role message (the user
/// prompt prefix for a full-fidelity turn).
fn user_message_item(message: &ThreadMessageRecord) -> Option<TurnItem> {
    if message.message.role != "user" {
        return None;
    }
    Some(TurnItem::UserMessage {
        id: message.id.clone(),
        content: vec![UserMessageContent::Text { text: message.message.content.rendered_text() }],
    })
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

/// Scan workspace + global skills for the active workspace. Used to expand
/// skills the user names in their message (slash-mention or exact name token).
fn scan_known_skills(
    workspace_root: Option<&std::path::Path>,
) -> Vec<slab_agent_context::skill_manager::SkillRecord> {
    slab_agent_context::skill_manager::scan_skills(
        workspace_root,
        &slab_utils::app_home::skills_dir(),
    )
}

/// Render a `<skill>` block for `name` located at `path`, or `None` if the
/// skill contents cannot be read. Shared by the text-only and structured input
/// builders so skill expansion stays consistent.
fn render_skill_block(name: &str, path: &std::path::Path) -> Option<String> {
    let contents = slab_agent_context::skill_manager::read_skill_contents(path).ok()?;
    Some(slab_agent_context::helper::render_skill_block(name, &path.to_string_lossy(), &contents))
}

fn push_skill_block(blocks: &mut Vec<String>, name: &str, path: &std::path::Path) {
    if let Some(block) = render_skill_block(name, path) {
        blocks.push(block);
    }
}

/// Map an [`UserInput::Image`] detail hint to its wire string (`"low"` /
/// `"high"` / `"auto"`), matching [`slab_proto::harness::user_input::ImageDetail`]'s
/// `rename_all = "lowercase"`.
fn image_detail_str(detail: &slab_proto::harness::user_input::ImageDetail) -> &'static str {
    use slab_proto::harness::user_input::ImageDetail;
    match detail {
        ImageDetail::Low => "low",
        ImageDetail::High => "high",
        ImageDetail::Auto => "auto",
    }
}

/// Extract the media type from a `data:<mediatype>;base64,…` URI, mirroring the
/// decoding in `slab_app_core::domain::services::chat::local::decode_image_url`.
fn mime_type_from_data_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("data:")?;
    let comma = rest.find(',')?;
    let meta = &rest[..comma];
    meta.split(';').next().filter(|s| s.contains('/')).map(str::to_owned)
}

/// Build structured user content from harness input.
///
/// - `None` when there is no meaningful content (empty text and no images).
/// - `Some(Text(joined))` when only text/skills are present — byte-identical to
///   [`join_user_text`] today (the text-only path is preserved exactly).
/// - `Some(Parts([...]))` when at least one image is present — text parts and
///   image parts in the order the user supplied them, with expanded skill blocks
///   appended after the user text. `LocalImage` paths are passed through verbatim
///   as `image_url`; `decode_image_url` reads them from disk server-side (the
///   Tauri path optimization — no base64 round-trip).
fn user_content_from_input(
    input: &[slab_proto::harness::UserInput],
    skills: &[slab_agent_context::skill_manager::SkillRecord],
) -> Option<ConversationMessageContent> {
    let has_visual = input.iter().any(|item| {
        matches!(
            item,
            slab_proto::harness::UserInput::Image { .. }
                | slab_proto::harness::UserInput::LocalImage { .. }
        )
    });

    if !has_visual {
        let text = join_user_text(input, skills);
        return if text.trim().is_empty() {
            None
        } else {
            Some(ConversationMessageContent::Text(text))
        };
    }

    let mut parts: Vec<ConversationContentPart> = Vec::new();
    let mut skill_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut skill_blocks: Vec<String> = Vec::new();
    let mut text_buf = String::new();

    for item in input {
        match item {
            slab_proto::harness::UserInput::Text { text, .. } => {
                text_buf.push_str(text);
                parts.push(ConversationContentPart::Text { text: text.clone() });
            }
            slab_proto::harness::UserInput::Skill { name, path }
                if skill_names.insert(name.clone()) =>
            {
                if let Some(block) = render_skill_block(name, path) {
                    skill_blocks.push(block);
                }
            }
            slab_proto::harness::UserInput::Image { image_url, detail } => {
                parts.push(ConversationContentPart::Image {
                    image_url: Some(image_url.clone()),
                    mime_type: mime_type_from_data_url(image_url),
                    detail: detail.as_ref().map(image_detail_str).map(str::to_owned),
                });
            }
            slab_proto::harness::UserInput::LocalImage { path, detail } => {
                parts.push(ConversationContentPart::Image {
                    image_url: Some(path.to_string_lossy().into_owned()),
                    mime_type: None,
                    detail: detail.as_ref().map(image_detail_str).map(str::to_owned),
                });
            }
            _ => {}
        }
    }

    for skill in
        slab_agent_context::user_instruction::SkillFragment::detect_in_text(&text_buf, skills)
    {
        if skill_names.insert(skill.name.clone())
            && let Some(block) = render_skill_block(&skill.name, &skill.path)
        {
            skill_blocks.push(block);
        }
    }

    for block in skill_blocks {
        parts.push(ConversationContentPart::Text { text: block });
    }

    if parts.is_empty() { None } else { Some(ConversationMessageContent::Parts(parts)) }
}

/// Build the single user [`ConversationMessage`] from harness input, or `None`
/// when there is no meaningful content. Shared by the first-turn (`spawn`) and
/// subsequent-turn (`send_input_message`) paths so both carry image parts.
fn build_user_message_from_input(
    input: &[slab_proto::harness::UserInput],
    skills: &[slab_agent_context::skill_manager::SkillRecord],
) -> Option<ConversationMessage> {
    user_content_from_input(input, skills).map(|content| ConversationMessage {
        role: "user".to_owned(),
        content,
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
    })
}

/// Flatten the text of all [`slab_proto::harness::UserInput::Text`] items into a
/// single user string, and expand any skills the user invoked: explicit
/// [`slab_proto::harness::UserInput::Skill`] items, plus skills named in the
/// text via `/name`/`$name` or an exact name token (no fuzzy matching). Expanded
/// `<skill>` blocks are appended after the user text, de-duplicated by name.
fn join_user_text(
    input: &[slab_proto::harness::UserInput],
    skills: &[slab_agent_context::skill_manager::SkillRecord],
) -> String {
    let mut text = String::new();
    let mut blocks = Vec::new();
    let mut expanded_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for item in input {
        match item {
            slab_proto::harness::UserInput::Text { text: t, .. } => text.push_str(t),
            slab_proto::harness::UserInput::Skill { name, path }
                if expanded_names.insert(name.clone()) =>
            {
                push_skill_block(&mut blocks, name, path);
            }
            _ => {}
        }
    }

    for skill in slab_agent_context::user_instruction::SkillFragment::detect_in_text(&text, skills)
    {
        if expanded_names.insert(skill.name.clone()) {
            push_skill_block(&mut blocks, &skill.name, &skill.path);
        }
    }

    if blocks.is_empty() { text } else { format!("{}\n\n{}", text, blocks.join("\n\n")) }
}

fn messages_from_input(
    input: &[slab_proto::harness::UserInput],
    skills: &[slab_agent_context::skill_manager::SkillRecord],
) -> Vec<ConversationMessage> {
    build_user_message_from_input(input, skills).into_iter().collect()
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
        assert_eq!(join_user_text(&input, &[]), "hello");
    }

    #[test]
    fn messages_from_input_builds_single_user_message() {
        let input =
            vec![slab_proto::harness::UserInput::Text { text: "hi".into(), text_elements: vec![] }];
        let messages = messages_from_input(&input, &[]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
    }

    #[test]
    fn messages_from_input_text_only_is_plain_text_content() {
        // Text-only input must stay byte-identical: a single Text content (not Parts).
        let input = vec![slab_proto::harness::UserInput::Text {
            text: "describe".into(),
            text_elements: vec![],
        }];
        let messages = messages_from_input(&input, &[]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, ConversationMessageContent::Text("describe".to_owned()));
    }

    #[test]
    fn messages_from_input_carries_image_part_as_parts() {
        let input = vec![
            slab_proto::harness::UserInput::Text {
                text: "what is this".into(),
                text_elements: vec![],
            },
            slab_proto::harness::UserInput::Image {
                image_url: "data:image/png;base64,iVBORw0KG=".into(),
                detail: Some(slab_proto::harness::user_input::ImageDetail::Auto),
            },
        ];
        let messages = messages_from_input(&input, &[]);
        assert_eq!(messages.len(), 1);
        let ConversationMessageContent::Parts(parts) = &messages[0].content else {
            panic!("expected Parts content for image input, got {:?}", messages[0].content);
        };
        assert_eq!(parts.len(), 2, "text + image part");
        assert!(matches!(parts[0], ConversationContentPart::Text { .. }));
        match &parts[1] {
            ConversationContentPart::Image { image_url, mime_type, detail } => {
                assert_eq!(image_url.as_deref(), Some("data:image/png;base64,iVBORw0KG="));
                assert_eq!(mime_type.as_deref(), Some("image/png"));
                assert_eq!(detail.as_deref(), Some("auto"));
            }
            other => panic!("expected Image part, got {other:?}"),
        }
    }

    #[test]
    fn messages_from_input_carries_local_image_path_verbatim() {
        // A LocalImage path must flow through as the image_url with no base64
        // encoding — decode_image_url reads it from disk server-side.
        let input = vec![slab_proto::harness::UserInput::LocalImage {
            path: std::path::PathBuf::from("/tmp/pic.png"),
            detail: None,
        }];
        let messages = messages_from_input(&input, &[]);
        assert_eq!(messages.len(), 1);
        let ConversationMessageContent::Parts(parts) = &messages[0].content else {
            panic!("expected Parts content for local image input");
        };
        match &parts[0] {
            ConversationContentPart::Image { image_url, mime_type, detail } => {
                assert_eq!(image_url.as_deref(), Some("/tmp/pic.png"));
                assert!(mime_type.is_none(), "path form carries no mime_type");
                assert!(detail.is_none());
            }
            other => panic!("expected Image part, got {other:?}"),
        }
    }

    #[test]
    fn messages_from_input_empty_is_no_message() {
        let messages: Vec<ConversationMessage> = messages_from_input(&[], &[]);
        assert!(messages.is_empty(), "empty input must produce no message");
    }

    #[test]
    fn join_user_text_expands_explicit_skill_input() {
        let dir = tempfile::tempdir().unwrap();
        let skill_md = dir.path().join("SKILL.md");
        std::fs::write(&skill_md, "# do the thing\n").unwrap();
        let input = vec![
            slab_proto::harness::UserInput::Text {
                text: "please help".into(),
                text_elements: vec![],
            },
            slab_proto::harness::UserInput::Skill { name: "thing".into(), path: skill_md.clone() },
        ];
        let joined = join_user_text(&input, &[]);
        assert!(joined.starts_with("please help"));
        assert!(joined.contains("<skill>"));
        assert!(joined.contains("<name>thing</name>"));
        assert!(joined.contains("# do the thing"));
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
        // Mirrors the inline parse in `body::thread_rollback`.
        let parsed: u32 = "3".parse().expect("numeric turn id");
        assert_eq!(parsed, 3);
        assert!("x".parse::<u32>().is_err(), "non-numeric turn id must be rejected");
    }

    fn record(id: &str, turn: u32, role: &str, text: &str, created: &str) -> ThreadMessageRecord {
        ThreadMessageRecord {
            id: id.to_owned(),
            thread_id: "t1".to_owned(),
            turn_index: turn,
            message: ConversationMessage {
                role: role.to_owned(),
                content: ConversationMessageContent::Text(text.to_owned()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
            created_at: created.to_owned(),
        }
    }

    fn snapshot() -> ThreadSnapshot {
        ThreadSnapshot {
            id: "t1".to_owned(),
            session_id: "s1".to_owned(),
            parent_id: None,
            depth: 0,
            status: slab_types::agent::AgentThreadStatus::Completed,
            role_name: None,
            config_json: "{}".to_owned(),
            completion_text: Some("hi there".to_owned()),
            created_at: "2024-01-01T00:00:00Z".to_owned(),
            updated_at: "2024-01-01T00:00:00Z".to_owned(),
            archived_at: None,
        }
    }

    #[test]
    fn thread_from_snapshot_with_turns_groups_messages_into_turns() {
        let messages = vec![
            record("m1", 0, "user", "hello", "2024-01-01T00:00:01Z"),
            record("m2", 0, "assistant", "hi there", "2024-01-01T00:00:02Z"),
            record("m3", 1, "user", "again", "2024-01-01T00:00:03Z"),
            record("m4", 1, "assistant", "yes", "2024-01-01T00:00:04Z"),
        ];
        let turn_states = vec![TurnStateRecord {
            thread_id: "t1".to_owned(),
            turn_index: 0,
            status: "completed".to_owned(),
            input_messages_json: None,
            tool_specs_json: None,
            llm_response_json: None,
            error: None,
            started_at: "2024-01-01T00:00:01Z".to_owned(),
            completed_at: None,
        }];

        let thread =
            thread_from_snapshot_with_turns("hthread-1", &snapshot(), &messages, &turn_states, &[]);

        assert_eq!(thread.id, "hthread-1");
        assert_eq!(thread.preview, "hi there");
        assert_eq!(thread.turns.len(), 2);

        let turn0 = &thread.turns[0];
        assert_eq!(turn0.id, "0");
        assert_eq!(turn0.status, "completed");
        assert_eq!(turn0.items.len(), 2);
        // No persisted items → fallback synthesizes from messages in created_at order.
        assert!(matches!(turn0.items[0], TurnItem::UserMessage { .. }));
        assert!(matches!(turn0.items[1], TurnItem::AgentMessage { .. }));

        // Turn 1 has no turn-state record → status defaults to "completed".
        let turn1 = &thread.turns[1];
        assert_eq!(turn1.id, "1");
        assert_eq!(turn1.status, "completed");
        assert!(turn1.items.iter().any(|item| matches!(item, TurnItem::UserMessage { .. })));
    }

    #[test]
    fn thread_from_snapshot_with_turns_replays_full_fidelity_items() {
        // Turn 0 has persisted snapshots → rendered verbatim; turn 1 has none →
        // falls back to message synthesis.
        let messages = vec![
            record("u1", 0, "user", "hello", "2024-01-01T00:00:01Z"),
            record("a1", 0, "assistant", "stale lossy text", "2024-01-01T00:00:02Z"),
        ];
        let persisted = vec![
            TurnItemRecord {
                id: "r1".to_owned(),
                thread_id: "t1".to_owned(),
                turn_index: 0,
                seq: 0,
                item_json: serde_json::to_string(&TurnItem::Reasoning {
                    id: "r1".to_owned(),
                    summary: slab_agent::protocol::ReasoningText::one("recap"),
                    content: slab_agent::protocol::ReasoningText::one("full trace"),
                })
                .unwrap(),
                created_at: "2024-01-01T00:00:01Z".to_owned(),
            },
            TurnItemRecord {
                id: "c1".to_owned(),
                thread_id: "t1".to_owned(),
                turn_index: 0,
                seq: 1,
                item_json: serde_json::to_string(&TurnItem::CommandExecution {
                    id: "c1".to_owned(),
                    command: "ls -la".to_owned(),
                    cwd: "/workspace".to_owned(),
                    process_id: None,
                    status: "completed".to_owned(),
                    aggregated_output: Some("out".to_owned()),
                    exit_code: Some(0),
                    duration_ms: None,
                })
                .unwrap(),
                created_at: "2024-01-01T00:00:02Z".to_owned(),
            },
        ];

        let thread =
            thread_from_snapshot_with_turns("hthread-1", &snapshot(), &messages, &[], &persisted);

        assert_eq!(thread.turns.len(), 1);
        let turn0 = &thread.turns[0];
        // User prompt prefix from messages, then persisted items in seq order.
        // The stale assistant message is NOT synthesized (snapshots win).
        assert_eq!(turn0.items.len(), 3);
        assert!(matches!(turn0.items[0], TurnItem::UserMessage { .. }));
        assert!(matches!(
            &turn0.items[1],
            TurnItem::Reasoning { content, .. }
            if matches!(content, slab_agent::protocol::ReasoningText::One(s) if s == "full trace")
        ));
        assert!(matches!(
            &turn0.items[2],
            TurnItem::CommandExecution { command, cwd, exit_code: Some(0), .. }
            if command == "ls -la" && cwd == "/workspace"
        ));
    }
}
