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
use slab_agent::port::{ThreadMessageRecord, ThreadSnapshot, TurnStateRecord};
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
        preview: snapshot
            .completion_text
            .as_deref()
            .map(slab_agent::strip_think_blocks)
            .unwrap_or_default(),
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
/// `AgentMessage` (text) plus one `McpToolCall` per emitted tool call (its
/// result is attached afterwards by the caller, see [`merge_tool_result`]);
/// tool results that the caller could NOT pair with a call card fall through
/// to a `CommandExecution` surfacing the rendered output.
/// `system` / `developer` roles (the injected init-context batch) are
/// LLM-visible only and NEVER render in the restored UI history — returning
/// empty keeps them out of the conversation timeline. The same holds for the
/// user-ROLE injected fragments (`slab_agents_md` etc.): they carry a
/// fragment `name` tag, while real user prompts never do (see
/// `latest_user_input`), so `name`-tagged user messages are dropped too —
/// otherwise the workspace `AGENTS.md` body renders as a user bubble.
fn turn_items_for_message(message: &ThreadMessageRecord) -> Vec<TurnItem> {
    let id = message.id.clone();
    let record = &message.message;
    match record.role.as_str() {
        "system" | "developer" => Vec::new(),
        "user" if record.name.is_some() => Vec::new(),
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
            // Content only — `ConversationMessage::rendered_text` would append
            // a `tool_call_id: …` line, leaking the raw id into the card body.
            aggregated_output: Some(record.content.rendered_text()),
            exit_code: None,
            duration_ms: None,
        }],
    }
}

/// The join key between a synthesized call card and the `role:"tool"` message
/// carrying its result.
fn call_item_id(item: &TurnItem) -> Option<&str> {
    match item {
        TurnItem::McpToolCall { id, .. } | TurnItem::ToolCall { id, .. } => Some(id),
        _ => None,
    }
}

/// Attach a `role:"tool"` message's output to the call card awaiting it — the
/// lossy-restore counterpart of the live render's `result` field (a plain
/// string, matching `turn_tool_call`'s `serde_json::json!(text)` convention).
fn merge_tool_result(item: &mut TurnItem, result: String) {
    match item {
        TurnItem::McpToolCall { result: slot, .. } | TurnItem::ToolCall { result: slot, .. } => {
            *slot = Some(Value::String(result));
        }
        _ => {}
    }
}

/// How many previously rendered messages the duplicate guard remembers. The
/// historical emit-anchor drift re-appended a contiguous tail of old messages
/// before each new user input, so duplicates always appear close together — a
/// short window suppresses them without masking legitimate far-apart repeats.
const RESTORE_DEDUPE_WINDOW: usize = 8;

/// Duplicate guard for restored message entries.
///
/// `true` when an identical (role, rendered text) message was already rendered
/// within the last [`RESTORE_DEDUPE_WINDOW`] rendered messages; rendering a
/// message records it in the window.
#[derive(Default)]
struct RestoreDedupe {
    window: std::collections::VecDeque<(String, String)>,
}

impl RestoreDedupe {
    fn seen(&self, role: &str, text: &str) -> bool {
        self.window.iter().any(|(r, t)| r == role && t == text)
    }

    fn record(&mut self, role: &str, text: &str) {
        if self.window.len() == RESTORE_DEDUPE_WINDOW {
            self.window.pop_front();
        }
        self.window.push_back((role.to_owned(), text.to_owned()));
    }
}

/// Like [`thread_from_snapshot_with_id`] but populates `turns` for
/// `thread/resume`, from the interleaved rollout timeline.
///
/// The timeline (`list_turn_timeline`) carries `TurnItem` artifacts and
/// `MessageAppend` records in the order their rollout lines were written, so
/// the restored history renders in the same order the live event stream
/// produced — no per-turn bucket re-merge. Per turn:
/// - turns WITH persisted `TurnItem` snapshots render the turn's user
///   messages followed by the items verbatim (assistant/tool messages are
///   carried by the items);
/// - turns WITHOUT snapshots (interrupted turns, legacy data) synthesize
///   lossily from their messages via [`turn_items_for_message`], skipping the
///   `system`/`developer` init-context batch;
/// - message entries whose (role, text) matches a recently rendered message
///   are dropped — rollout files written by the historical emit-anchor drift
///   re-appended a tail of old messages before each new input.
fn thread_from_timeline(
    id: &str,
    snapshot: &ThreadSnapshot,
    turn_states: &[TurnStateRecord],
    timeline: &[slab_app_core::domain::services::TurnTimelineEntry],
) -> Thread {
    let created_at = chrono::DateTime::parse_from_rfc3339(&snapshot.created_at)
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0);

    // Bucket the timeline by turn, preserving entry (file) order within a turn.
    let mut by_turn: std::collections::BTreeMap<
        u32,
        Vec<&slab_app_core::domain::services::TurnTimelineEntry>,
    > = std::collections::BTreeMap::new();
    for entry in timeline {
        let turn_index = match entry {
            slab_app_core::domain::services::TurnTimelineEntry::Item(record) => record.turn_index,
            slab_app_core::domain::services::TurnTimelineEntry::Message(record) => {
                record.turn_index
            }
        };
        by_turn.entry(turn_index).or_default().push(entry);
    }

    let mut dedupe = RestoreDedupe::default();
    let turns = by_turn
        .into_iter()
        .map(|(index, entries)| {
            // Decode the turn's persisted TurnItem snapshots; decode failures
            // are skipped with a warning.
            let mut persisted: Vec<TurnItem> = Vec::new();
            for entry in &entries {
                if let slab_app_core::domain::services::TurnTimelineEntry::Item(record) = entry {
                    match serde_json::from_str::<TurnItem>(&record.item_json) {
                        Ok(item) => persisted.push(item),
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
            }

            let messages: Vec<&ThreadMessageRecord> = entries
                .iter()
                .filter_map(|entry| match entry {
                    slab_app_core::domain::services::TurnTimelineEntry::Message(record) => {
                        Some(record)
                    }
                    _ => None,
                })
                .collect();

            let items = if !persisted.is_empty() {
                // Full-fidelity turn: user prompts (from the timeline's
                // message entries) + persisted items; assistant/tool messages
                // are carried by the items and skipped.
                let mut items = Vec::new();
                for record in &messages {
                    if record.message.role != "user" {
                        continue;
                    }
                    let text = record.message.content.rendered_text();
                    if dedupe.seen("user", &text) {
                        continue;
                    }
                    dedupe.record("user", &text);
                    if let Some(item) = user_message_item(record) {
                        items.push(item);
                    }
                }
                items.extend(persisted);
                items
            } else {
                // Snapshot-less turn (interrupted / legacy): synthesize from
                // the message entries, skipping system/developer. A tool
                // result merges into the call card its `tool_call_id` pairs
                // with; only orphans fall back to a standalone output card.
                let mut items = Vec::new();
                let mut awaiting_result: std::collections::HashMap<String, usize> =
                    std::collections::HashMap::new();
                for record in &messages {
                    let role = record.message.role.as_str();
                    let text = record.message.content.rendered_text();
                    if dedupe.seen(role, &text) {
                        continue;
                    }
                    dedupe.record(role, &text);
                    if role == "tool"
                        && let Some(&index) = record
                            .message
                            .tool_call_id
                            .as_deref()
                            .and_then(|call_id| awaiting_result.get(call_id))
                    {
                        merge_tool_result(&mut items[index], text);
                        continue;
                    }
                    let before = items.len();
                    items.extend(turn_items_for_message(record));
                    for (offset, item) in items[before..].iter().enumerate() {
                        if let Some(call_id) = call_item_id(item) {
                            awaiting_result.insert(call_id.to_owned(), before + offset);
                        }
                    }
                }
                items
            };
            // LAST-wins: a turn emits several TurnState lines (sampling entry
            // → phase lines → terminal); `.find()` used to pick the FIRST,
            // so restored turns always showed the entry status ("running").
            let status = turn_states
                .iter()
                .rev()
                .find(|state| state.turn_index == index)
                .map(|state| state.status.clone())
                .filter(|status| !status.trim().is_empty())
                .unwrap_or_else(|| "completed".to_owned());
            Turn { id: index.to_string(), items, status, error: None }
        })
        .collect();

    Thread {
        id: id.to_owned(),
        preview: snapshot
            .completion_text
            .as_deref()
            .map(slab_agent::strip_think_blocks)
            .unwrap_or_default(),
        model_provider: String::new(),
        created_at,
        turns,
        ..Default::default()
    }
}

/// Build a `UserMessage` item from a persisted user-role message (the user
/// prompt prefix for a full-fidelity turn). Fragment-tagged user messages
/// (`slab_agents_md` and the other injected init-context fragments, which ride
/// the user role) return `None` — they are LLM-visible context, not user turns.
fn user_message_item(message: &ThreadMessageRecord) -> Option<TurnItem> {
    if message.message.role != "user" || message.message.name.is_some() {
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

/// Map the harness wire [`ReasoningEffort`] onto the agent-config enum the
/// LLM adapter consumes. `xhigh` clamps to `High` — the config enum has no
/// xhigh variant (same closed set the REST `parse_reasoning_effort` accepts).
pub(crate) fn chat_reasoning_effort_from_proto(
    effort: ReasoningEffort,
) -> slab_types::chat::ChatReasoningEffort {
    match effort {
        ReasoningEffort::Off => slab_types::chat::ChatReasoningEffort::None,
        ReasoningEffort::Low => slab_types::chat::ChatReasoningEffort::Low,
        ReasoningEffort::Medium => slab_types::chat::ChatReasoningEffort::Medium,
        ReasoningEffort::High | ReasoningEffort::Xhigh => {
            slab_types::chat::ChatReasoningEffort::High
        }
    }
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

/// Map a [`slab_proto::harness::user_input::UserInput::Image`] detail hint to its wire string (`"low"` /
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

    use slab_agent::port::TurnItemRecord;

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
            context_window: None,
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
    fn chat_reasoning_effort_from_proto_maps_full_wire_set() {
        use slab_types::chat::ChatReasoningEffort as Config;
        assert_eq!(chat_reasoning_effort_from_proto(ReasoningEffort::Off), Config::None);
        assert_eq!(chat_reasoning_effort_from_proto(ReasoningEffort::Low), Config::Low);
        assert_eq!(chat_reasoning_effort_from_proto(ReasoningEffort::Medium), Config::Medium);
        assert_eq!(chat_reasoning_effort_from_proto(ReasoningEffort::High), Config::High);
        // No xhigh variant on the config side — clamped to High.
        assert_eq!(chat_reasoning_effort_from_proto(ReasoningEffort::Xhigh), Config::High);
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

    /// `record` + a fragment `name` tag (the init-context injection identity,
    /// e.g. `slab_agents_md` for the workspace AGENTS.md body).
    fn tagged_record(
        id: &str,
        turn: u32,
        role: &str,
        name: &str,
        text: &str,
    ) -> ThreadMessageRecord {
        let mut record = record(id, turn, role, text, "2024-01-01T00:00:00Z");
        record.message.name = Some(name.to_owned());
        record
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
    fn thread_from_timeline_groups_messages_into_turns() {
        let messages = [
            record("m1", 0, "user", "hello", "2024-01-01T00:00:01Z"),
            record("m2", 0, "assistant", "hi there", "2024-01-01T00:00:02Z"),
            record("m3", 1, "user", "again", "2024-01-01T00:00:03Z"),
            record("m4", 1, "assistant", "yes", "2024-01-01T00:00:04Z"),
        ];
        let timeline: Vec<slab_app_core::domain::services::TurnTimelineEntry> = messages
            .iter()
            .map(|m| slab_app_core::domain::services::TurnTimelineEntry::Message(m.clone()))
            .collect();
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

        let thread = thread_from_timeline("hthread-1", &snapshot(), &turn_states, &timeline);

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

    // Legacy snapshot-less turns restore tool results INTO their call card
    // (paired by `tool_call_id`), not as a detached empty-command
    // CommandExecution; orphan results keep the output-card fallback, minus
    // the raw `tool_call_id` leak in the card body.
    #[test]
    fn thread_from_timeline_merges_tool_results_into_call_cards() {
        let user = record("u1", 0, "user", "list the files", "2024-01-01T00:00:01Z");
        let mut assistant = record("a1", 0, "assistant", "", "2024-01-01T00:00:02Z");
        assistant.message.tool_calls = vec![slab_types::ConversationToolCall {
            id: Some("call_1".to_owned()),
            r#type: "function".to_owned(),
            function: slab_types::ConversationToolFunction {
                name: "read_file".to_owned(),
                arguments: serde_json::json!({ "path": "a.rs" }).to_string(),
            },
        }];
        let mut paired = record("t1", 0, "tool", "file contents", "2024-01-01T00:00:03Z");
        paired.message.tool_call_id = Some("call_1".to_owned());
        let mut orphan = record("t2", 0, "tool", "orphan output", "2024-01-01T00:00:04Z");
        orphan.message.tool_call_id = Some("call_missing".to_owned());

        let timeline: Vec<slab_app_core::domain::services::TurnTimelineEntry> =
            [user, assistant, paired, orphan]
                .into_iter()
                .map(slab_app_core::domain::services::TurnTimelineEntry::Message)
                .collect();

        let thread = thread_from_timeline("hthread-1", &snapshot(), &[], &timeline);
        assert_eq!(thread.turns.len(), 1);
        let items = &thread.turns[0].items;
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], TurnItem::UserMessage { .. }));
        // The paired result rides the call card — no extra output card.
        assert!(matches!(
            &items[1],
            TurnItem::McpToolCall { tool, arguments, result: Some(res), .. }
                if tool == "read_file"
                    && arguments["path"] == "a.rs"
                    && res == &serde_json::json!("file contents")
        ));
        // The orphan keeps the fallback card, without the id leak.
        assert!(matches!(
            &items[2],
            TurnItem::CommandExecution { command, aggregated_output: Some(out), .. }
                if command.is_empty() && out == "orphan output"
        ));
    }

    #[test]
    fn thread_from_timeline_replays_full_fidelity_items() {
        // Turn 0 has persisted snapshots → rendered verbatim; turn 1 has none →
        // falls back to message synthesis.
        let messages = [
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

        // Timeline: user message, then the turn's items in write order, then
        // the assistant append (carried by the items — must not re-render).
        let mut timeline: Vec<slab_app_core::domain::services::TurnTimelineEntry> = Vec::new();
        timeline
            .push(slab_app_core::domain::services::TurnTimelineEntry::Message(messages[0].clone()));
        for item in &persisted {
            timeline.push(slab_app_core::domain::services::TurnTimelineEntry::Item(item.clone()));
        }
        timeline
            .push(slab_app_core::domain::services::TurnTimelineEntry::Message(messages[1].clone()));

        let thread = thread_from_timeline("hthread-1", &snapshot(), &[], &timeline);

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

    // Regression for the restore-ordering bug: a timeline shaped like the real
    // broken session (init batch, historical emit-drift re-appends, tool
    // items, snapshot-less tail turn) must restore as the LIVE event sequence
    // — user prompt + items per turn, duplicates and init-context messages
    // dropped — not a bare assistant block followed by re-merged history.
    #[test]
    fn thread_from_timeline_restores_live_order_from_drifted_file() {
        let mut timeline: Vec<slab_app_core::domain::services::TurnTimelineEntry> = Vec::new();
        let mut msg_id = 0;
        let mut msg = |turn: u32, role: &str, text: &str| {
            msg_id += 1;
            slab_app_core::domain::services::TurnTimelineEntry::Message(record(
                &format!("m{msg_id}"),
                turn,
                role,
                text,
                "2024-01-01T00:00:00Z",
            ))
        };
        let item = |id: &str, turn: u32, item: TurnItem| {
            slab_app_core::domain::services::TurnTimelineEntry::Item(TurnItemRecord {
                id: id.to_owned(),
                thread_id: "t1".to_owned(),
                turn_index: turn,
                seq: 0,
                item_json: serde_json::to_string(&item).unwrap(),
                created_at: "2024-01-01T00:00:00Z".to_owned(),
            })
        };

        // Turn 0: init batch + user "你是谁" + reply items.
        timeline.extend([
            msg(0, "system", "You are Slab, an AI agent."),
            msg(0, "developer", "<environment_context>…"),
            msg(0, "user", "你是谁"),
            item(
                "r0",
                0,
                TurnItem::Reasoning {
                    id: "r0".to_owned(),
                    summary: slab_agent::protocol::ReasoningText::one(""),
                    content: slab_agent::protocol::ReasoningText::one("thinking"),
                },
            ),
            item(
                "a0",
                0,
                TurnItem::AgentMessage { id: "a0".to_owned(), text: "我是 Slab".to_owned() },
            ),
        ]);
        // Turn 1: historical emit-drift re-append of the prior tail + the new
        // input + tool items (the permission-test turn).
        timeline.extend([
            msg(1, "developer", "<environment_context>…"),
            msg(1, "user", "你是谁"),
            msg(1, "user", "我现在需要进行权限测试，你随便执行一个命令"),
            item(
                "r1",
                1,
                TurnItem::Reasoning {
                    id: "r1".to_owned(),
                    summary: slab_agent::protocol::ReasoningText::one(""),
                    content: slab_agent::protocol::ReasoningText::one("need approval"),
                },
            ),
            item(
                "a1",
                1,
                TurnItem::AgentMessage {
                    id: "a1".to_owned(), text: "需要用户批准".to_owned()
                },
            ),
            item(
                "c1",
                1,
                TurnItem::CommandExecution {
                    id: "c1".to_owned(),
                    command: "echo hi".to_owned(),
                    cwd: "/".to_owned(),
                    process_id: None,
                    status: "completed".to_owned(),
                    aggregated_output: None,
                    exit_code: Some(0),
                    duration_ms: None,
                },
            ),
        ]);
        // Turn 2: snapshot-less (interrupted) turn — synthesized from messages.
        timeline.extend([msg(2, "user", "你能做什么")]);

        let thread = thread_from_timeline("hthread-1", &snapshot(), &[], &timeline);
        assert_eq!(thread.turns.len(), 3);

        // Turn 0: user prompt + items — no system/developer rendered, no bare
        // assistant block.
        let t0 = &thread.turns[0];
        assert_eq!(t0.items.len(), 3);
        assert!(matches!(&t0.items[0], TurnItem::UserMessage { content, .. }
            if matches!(&content[0], UserMessageContent::Text { text } if text == "你是谁")));
        assert!(matches!(&t0.items[1], TurnItem::Reasoning { .. }));
        assert!(matches!(&t0.items[2], TurnItem::AgentMessage { text, .. } if text == "我是 Slab"));

        // Turn 1: drifted dupes dropped; only the real input + items render.
        let t1 = &thread.turns[1];
        assert_eq!(t1.items.len(), 4);
        assert!(matches!(&t1.items[0], TurnItem::UserMessage { content, .. }
            if matches!(&content[0], UserMessageContent::Text { text }
                if text == "我现在需要进行权限测试，你随便执行一个命令")));
        assert!(matches!(&t1.items[1], TurnItem::Reasoning { .. }));
        assert!(matches!(
            &t1.items[3],
            TurnItem::CommandExecution { command, .. } if command == "echo hi"
        ));

        // Turn 2: synthesized user message.
        let t2 = &thread.turns[2];
        assert_eq!(t2.items.len(), 1);
        assert!(matches!(&t2.items[0], TurnItem::UserMessage { .. }));
    }

    #[test]
    fn turn_items_for_message_skips_init_context_roles() {
        // system/developer messages are LLM-visible only — never UI items.
        assert!(turn_items_for_message(&record("s1", 0, "system", "persona", "t")).is_empty());
        assert!(turn_items_for_message(&record("d1", 0, "developer", "<skills>", "t")).is_empty());
    }

    // Regression: the `slab_agents_md` init-context fragment rides the USER
    // role (with a fragment name tag), so it leaked through the user-message
    // projection and the workspace AGENTS.md body rendered as a user bubble.
    #[test]
    fn tagged_user_fragments_never_render_as_user_messages() {
        // Snapshot-less turn synthesis path.
        assert!(
            turn_items_for_message(&tagged_record(
                "g1",
                0,
                "user",
                "slab_agents_md",
                "<INSTRUCTIONS>…AGENTS.md body…</INSTRUCTIONS>"
            ))
            .is_empty()
        );
        // Full-fidelity turn user-prompt prefix path.
        assert!(
            user_message_item(&tagged_record(
                "g2",
                0,
                "user",
                "slab_agents_md",
                "<INSTRUCTIONS>…</INSTRUCTIONS>"
            ))
            .is_none()
        );
        // Untagged user messages still render.
        assert!(user_message_item(&record("u1", 0, "user", "real prompt", "t")).is_some());
    }

    #[test]
    fn thread_from_timeline_drops_tagged_agents_md_from_user_prompts() {
        // Turn 0 carries the injected init batch (tagged user fragment) ahead
        // of the real prompt, plus a persisted snapshot so the full-fidelity
        // path (user_message_item) runs.
        let mut timeline: Vec<slab_app_core::domain::services::TurnTimelineEntry> = vec![
            slab_app_core::domain::services::TurnTimelineEntry::Message(tagged_record(
                "g1",
                0,
                "user",
                "slab_agents_md",
                "<INSTRUCTIONS># AGENTS.md instructions…</INSTRUCTIONS>",
            )),
            slab_app_core::domain::services::TurnTimelineEntry::Message(record(
                "u1",
                0,
                "user",
                "hello",
                "2024-01-01T00:00:01Z",
            )),
            slab_app_core::domain::services::TurnTimelineEntry::Item(TurnItemRecord {
                id: "a1".to_owned(),
                thread_id: "t1".to_owned(),
                turn_index: 0,
                seq: 0,
                item_json: serde_json::to_string(&TurnItem::AgentMessage {
                    id: "a1".to_owned(),
                    text: "hi".to_owned(),
                })
                .unwrap(),
                created_at: "2024-01-01T00:00:02Z".to_owned(),
            }),
        ];
        // Snapshot-less turn 1: the tagged fragment must vanish there too.
        timeline.push(slab_app_core::domain::services::TurnTimelineEntry::Message(tagged_record(
            "g2",
            1,
            "user",
            "slab_agents_md",
            "<INSTRUCTIONS>…</INSTRUCTIONS>",
        )));
        timeline.push(slab_app_core::domain::services::TurnTimelineEntry::Message(record(
            "u2",
            1,
            "user",
            "again",
            "2024-01-01T00:00:03Z",
        )));

        let thread = thread_from_timeline("hthread-1", &snapshot(), &[], &timeline);
        assert_eq!(thread.turns.len(), 2);
        // Turn 0: only the real prompt + the item — no AGENTS.md bubble.
        assert_eq!(thread.turns[0].items.len(), 2);
        assert!(matches!(&thread.turns[0].items[0], TurnItem::UserMessage { content, .. }
            if matches!(&content[0], UserMessageContent::Text { text } if text == "hello")));
        // Turn 1 (synthesized): same.
        assert_eq!(thread.turns[1].items.len(), 1);
        assert!(matches!(&thread.turns[1].items[0], TurnItem::UserMessage { content, .. }
            if matches!(&content[0], UserMessageContent::Text { text } if text == "again")));
    }
}
