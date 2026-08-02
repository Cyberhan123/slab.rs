//! L3 conversation reducer — folds a trace bundle's many inferences into the
//! LINEAR conversation the model was shown.
//!
//! This is the L3 semantic layer: the rollout (L1) records WHAT happened; the
//! reducer knows WHAT THE MODEL SAW. It is an OFFLINE diagnostic — it does NOT
//! run on the agent hot path and is invoked on demand against a finished (or
//! in-progress) trace bundle.
//!
//! ## Three folding modes
//!
//! Reading the bundle's `trace.jsonl` events in order, each `InferenceStarted`
//! references a request payload and each `InferenceCompleted` a response
//! payload. The reducer folds these into one cumulative conversation:
//!
//! 1. **AppendOnly** (previous_response_id incremental backfill): when a
//!    request payload carries a `previous_response_id` pointing at a prior
//!    response *in the current lineage*, the reducer does NOT re-emit the full
//!    history — it appends only the delta (the new turn input) VERBATIM and
//!    reuses the already-built prefix. Delta messages are appended
//!    unconditionally: a legitimately repeated delta (the same user text twice,
//!    or an identical tool result twice) is NEW input the model received this
//!    inference and MUST be kept — fingerprint dedup would silently drop it and
//!    produce a conversation that does NOT match what the model saw. The
//!    previous_response_id chain is walked implicitly because events are
//!    processed in order: each referenced response was already recorded when its
//!    own `InferenceCompleted` was processed. An unresolved
//!    `previous_response_id` (never recorded, OR replaced out of the lineage by
//!    a later FullSnapshot) falls back to FullSnapshot so the reducer never
//!    panics on a partial bundle and never reconstructs a stale lineage.
//! 2. **FullSnapshot**: a request with NO `previous_response_id` (or an
//!    unresolved one) is a full request. Its input messages become the
//!    conversation verbatim, reusing item ids where fingerprints match (no
//!    duplication) and dropping items no longer present (a divergent snapshot
//!    replaces the prior history). The replace INVALIDATES the response_index —
//!    a response id from before the snapshot is no longer in the lineage, so a
//!    later AppendOnly request referencing it falls back to FullSnapshot.
//! 3. **Post-compaction snapshot**: after a `ContextCompacted` event, the next
//!    full request's input is the POST-compaction history (summary + recent
//!    window). A FullSnapshot replace already does the right thing here — it
//!    drops the pre-compaction items that compaction replaced (validated by the
//!    `post_compaction_full_request_replaces_pre_compaction_history` test), so
//!    no separate compaction flag is tracked.
//!
//! `InferenceCompleted` appends the assistant message verbatim. Deduplication
//! happens ONLY at the explicit prefix-reuse point — the FullSnapshot replace —
//! never on the delta / new-message append paths. This is what surfaces the
//! trailing assistant reply that no later full request would otherwise include.
//!
//! ## Best-effort payload parsing
//!
//! `previous_response_id`, response `id`, and the message lists are parsed
//! best-effort from each payload's JSON [`serde_json::Value`]. A payload
//! missing them is not an error — the inference falls back to FullSnapshot
//! semantics. The reducer never panics on a partial payload; only I/O or
//! JSON-shape failures that prevent any reconstruction surface as a
//! [`ReduceError`].
//!
//! ## state.json cache
//!
//! After a successful reduction the reconstructed conversation is cached to the
//! bundle's `state.json` (the path reserved in Slice 9). A re-run reads the
//! cache first and reuses it only when it covers EXACTLY every current
//! `trace.jsonl` event — a truncated (shrunk) trace forces a re-derive, symmetric
//! with the appended (grown) case. See [`reduce_conversation_cached`].

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::bundle::TraceBundle;
use crate::event::{RawPayloadRef, RawTraceEvent, RawTraceEventPayload};

/// Schema version of the [`ReducerState`] cache. Bumped on breaking cache
/// layout changes; a mismatch forces a re-derive.
const REDUCER_STATE_VERSION: u32 = 1;

/// A single reconstructed conversation message — what the model was shown.
///
/// This is a local L3 projection type (not `slab_types::ConversationMessage`)
/// so the reducer stays self-contained and decoupled from the conversation
/// model's evolution. `content` is the raw JSON content exactly as it appeared
/// in the request/response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConversationMessage {
    pub role: String,
    pub content: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Best-effort stable id carried from the payload (e.g. a response item id).
    /// Used for dedup/reuse across full snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Errors raised by the reducer. A payload missing an expected field is NOT an
/// error (it falls back to FullSnapshot); only I/O or JSON-shape failures that
/// prevent any reconstruction are surfaced.
#[derive(Debug)]
pub enum ReduceError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for ReduceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "trace bundle io error: {error}"),
            Self::Json(error) => write!(f, "trace bundle json error: {error}"),
        }
    }
}

impl std::error::Error for ReduceError {}

impl From<std::io::Error> for ReduceError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ReduceError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

/// Cached reducer output, written to `state.json` after a successful reduction.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReducerState {
    /// Schema version of this cache.
    version: u32,
    /// Number of `trace.jsonl` events covered by this cache. A re-run reuses
    /// the cache only when it covers every current line.
    events_processed: usize,
    /// The reconstructed conversation at the time of caching.
    conversation: Vec<ConversationMessage>,
}

/// One folded entry: the message plus a stable fingerprint used ONLY by the
/// FullSnapshot replace to detect reuse (id preservation) vs replacement across
/// snapshots. The delta / new-message append paths never consult the
/// fingerprint — they append verbatim.
struct ReconstructedItem {
    message: ConversationMessage,
    fingerprint: String,
}

/// Mutable fold state accumulated while iterating the bundle's events.
struct FoldState {
    /// The reconstructed conversation, in order.
    items: Vec<ReconstructedItem>,
    /// Set of response ids (from response payloads' `id` field) that belong to
    /// the CURRENT conversation lineage — i.e. responses whose
    /// `InferenceCompleted` was processed against the current item set. Only
    /// MEMBERSHIP is consulted (an AppendOnly `previous_response_id` is resolved
    /// by `contains`); the index of the appended message is never needed because
    /// AppendOnly always appends at the tail. A FullSnapshot replace clears this
    /// so a response id from before the snapshot (no longer in the lineage)
    /// fails the membership check and falls back to FullSnapshot.
    response_index: HashSet<String>,
}

impl FoldState {
    fn fold_request(&mut self, req: &Value) {
        let prev_id = req.get("previous_response_id").and_then(|v| v.as_str());
        let messages = extract_request_messages(req);

        if let Some(prev_id) = prev_id
            && self.response_index.contains(prev_id)
        {
            // AppendOnly: the prefix (through the referenced response's
            // assistant message) is already built; append ONLY the new delta,
            // VERBATIM. These are NEW input messages the model received this
            // inference — a legitimately repeated delta (the same user text
            // twice, or an identical tool result twice) MUST be kept, so NO
            // fingerprint dedup is applied here. Deduplication happens only at
            // the explicit prefix-reuse point (FullSnapshot replace).
            for msg in messages {
                self.append_verbatim(msg);
            }
            return;
        }
        // Either no previous_response_id (FullSnapshot) or an UNRESOLVED
        // previous_response_id (the referenced response was never recorded OR
        // was replaced out of the lineage by a later FullSnapshot — a
        // partial/cold/diverged bundle). Both fall through to FullSnapshot so
        // the reducer never panics on a partial bundle and never reconstructs a
        // stale lineage.

        // FullSnapshot: the input messages become the conversation verbatim.
        // replace_preserving_ids reuses ids where fingerprints match (no
        // duplication) and drops items no longer present, then INVALIDATES
        // response_index so a pre-snapshot response id is no longer resolvable.
        // This is also exactly the post-compaction semantics: a ContextCompacted
        // event means the next full request carries the post-compaction history,
        // and the replace drops the pre-compaction items compaction replaced.
        self.replace_preserving_ids(&messages);
    }

    fn fold_response(&mut self, resp: &Value) {
        let resp_id = resp.get("id").and_then(|v| v.as_str()).map(String::from);
        if let Some(mut msg) = extract_assistant_message(resp) {
            // Stamp the response id onto the message when the payload did not
            // carry its own item id, so a later full snapshot can reuse it.
            if let Some(id) = &resp_id
                && msg.id.is_none()
            {
                msg.id = Some(id.clone());
            }
            // The assistant reply is NEW content the model produced; append it
            // verbatim. Dedup happens only at the FullSnapshot prefix-reuse
            // point, never here (two identical assistant replies are two real
            // turns the model produced).
            self.append_verbatim(msg);
            if let Some(id) = resp_id {
                self.response_index.insert(id);
            }
        }
    }

    /// Append `msg` verbatim at the tail — NO fingerprint dedup. Used for every
    /// delta / new-message append (AppendOnly deltas and assistant replies),
    /// where the content is genuine new input/output the model saw or produced.
    fn append_verbatim(&mut self, msg: ConversationMessage) {
        let fp = fingerprint(&msg);
        self.items.push(ReconstructedItem { message: msg, fingerprint: fp });
    }

    /// Replace the conversation with `messages` verbatim, reusing the id from
    /// any prior item whose fingerprint matches (so a re-sent identical message
    /// keeps a stable id) — this is the ONLY dedup point, the explicit
    /// prefix-reuse case. Items whose fingerprint is absent from the new list
    /// are dropped (the compaction / divergence case). INVALIDATES
    /// `response_index` so a response id from before the snapshot (no longer in
    /// the lineage) cannot be resolved by a later AppendOnly request.
    fn replace_preserving_ids(&mut self, messages: &[ConversationMessage]) {
        let old_id_by_fp: std::collections::HashMap<String, Option<String>> =
            self.items.iter().map(|it| (it.fingerprint.clone(), it.message.id.clone())).collect();
        self.items = messages
            .iter()
            .map(|msg| {
                let fp = fingerprint(msg);
                let mut msg = msg.clone();
                if msg.id.is_none() {
                    msg.id = old_id_by_fp.get(&fp).cloned().flatten();
                }
                ReconstructedItem { message: msg, fingerprint: fp }
            })
            .collect();
        // J2: the rebuilt items are a NEW lineage. response ids from before the
        // replace no longer correspond to any item position in the current
        // conversation, so drop them — a later AppendOnly lookup for a
        // pre-snapshot id will fail and fall back to FullSnapshot (correct,
        // because that response was replaced out of the lineage).
        self.response_index.clear();
    }
}

/// Extract the message list from a request payload. Handles both
/// chat-completions (`messages`) and responses-style (`input`) envelopes,
/// best-effort. Returns an empty vec when neither is present / parseable.
fn extract_request_messages(req: &Value) -> Vec<ConversationMessage> {
    if let Some(messages) = req.get("messages").and_then(|v| v.as_array()) {
        return messages.iter().filter_map(parse_message).collect();
    }
    if let Some(input) = req.get("input") {
        // responses-style: `input` may be a string or an array of items.
        if let Some(arr) = input.as_array() {
            return arr.iter().filter_map(parse_message).collect();
        }
        if let Some(s) = input.as_str() {
            return vec![ConversationMessage {
                role: "user".to_owned(),
                content: Value::String(s.to_owned()),
                name: None,
                tool_call_id: None,
                id: None,
            }];
        }
    }
    Vec::new()
}

/// Parse a single message object from a payload. Returns None when the object
/// has no `role` (the minimum identifying field).
fn parse_message(value: &Value) -> Option<ConversationMessage> {
    let obj = value.as_object()?;
    let role = obj.get("role").and_then(|v| v.as_str())?.to_owned();
    let content = obj.get("content").cloned().unwrap_or(Value::Null);
    let name = obj.get("name").and_then(|v| v.as_str()).map(String::from);
    let tool_call_id = obj.get("tool_call_id").and_then(|v| v.as_str()).map(String::from);
    let id = obj.get("id").and_then(|v| v.as_str()).map(String::from);
    Some(ConversationMessage { role, content, name, tool_call_id, id })
}

/// Extract the assistant message from a response payload. Handles
/// chat-completions (`choices[0].message`) and responses-style (`output`)
/// envelopes, best-effort. Returns None when no assistant message is found.
fn extract_assistant_message(resp: &Value) -> Option<ConversationMessage> {
    // chat-completions: choices[0].message
    if let Some(choice) = resp.get("choices").and_then(|v| v.as_array()).and_then(|a| a.first())
        && let Some(msg) = parse_message(&choice["message"])
    {
        return Some(msg);
    }
    // responses-style: output[] items whose role is assistant / message.
    if let Some(output) = resp.get("output").and_then(|v| v.as_array()) {
        for item in output {
            if let Some(msg) = parse_message(item)
                && matches!(msg.role.as_str(), "assistant" | "message")
            {
                return Some(msg);
            }
        }
    }
    None
}

/// Canonical identity of a message for dedup: role + content + name +
/// tool_call_id. The `id` is intentionally excluded — ids vary across
/// snapshots of the same turn.
fn fingerprint(msg: &ConversationMessage) -> String {
    serde_json::to_string(&serde_json::json!({
        "role": msg.role,
        "content": msg.content,
        "name": msg.name,
        "tool_call_id": msg.tool_call_id,
    }))
    .unwrap_or_default()
}

// ── Public entry points ──────────────────────────────────────────────────────

/// Reconstruct the linear conversation the model was shown across all
/// inferences in the bundle. Reads `trace.jsonl` and every referenced payload.
///
/// Does NOT read or write the `state.json` cache — see
/// [`reduce_conversation_cached`] for the cached variant.
pub fn reduce_conversation(bundle: &TraceBundle) -> Result<Vec<ConversationMessage>, ReduceError> {
    let events = read_trace_events(bundle)?;
    reduce_from_events(bundle, &events)
}

/// Same as [`reduce_conversation`] but consults and updates the bundle's
/// `state.json` cache. Reuses the cache only when it covers EXACTLY every
/// current `trace.jsonl` line (and the schema version matches) — a truncated
/// (shrunk) trace forces a re-derive, symmetric with the appended (grown)
/// case; otherwise re-derives and overwrites. Reads `trace.jsonl` ONCE (the
/// cached path shares the single read with the reduction).
pub fn reduce_conversation_cached(
    bundle: &TraceBundle,
) -> Result<Vec<ConversationMessage>, ReduceError> {
    let events = read_trace_events(bundle)?;
    let total_events = events.len();
    if let Some(cached) = read_state(bundle)?
        && cached.version == REDUCER_STATE_VERSION
        && cached.events_processed == total_events
    {
        return Ok(cached.conversation);
    }
    let conversation = reduce_from_events(bundle, &events)?;
    write_state(
        bundle,
        &ReducerState {
            version: REDUCER_STATE_VERSION,
            events_processed: total_events,
            conversation: conversation.clone(),
        },
    )?;
    Ok(conversation)
}

/// Fold a pre-read event slice into the linear conversation. Factored out of
/// [`reduce_conversation`] so the cached entry point can read `trace.jsonl`
/// once and share the single read with the reduction (avoiding a double read).
fn reduce_from_events(
    bundle: &TraceBundle,
    events: &[RawTraceEvent],
) -> Result<Vec<ConversationMessage>, ReduceError> {
    let mut state = FoldState { items: Vec::new(), response_index: HashSet::new() };
    for event in events {
        match &event.event {
            RawTraceEventPayload::InferenceStarted { request_payload } => {
                if let Some(reff) = request_payload
                    && let Ok(req) = read_payload(bundle, reff)
                {
                    state.fold_request(&req);
                }
            }
            RawTraceEventPayload::InferenceCompleted { response_payload } => {
                if let Some(reff) = response_payload
                    && let Ok(resp) = read_payload(bundle, reff)
                {
                    state.fold_response(&resp);
                }
            }
            RawTraceEventPayload::ContextCompacted => {
                // No state to track: the next full request's FullSnapshot
                // replace drops the pre-compaction items compaction replaced
                // (validated by the post-compaction test). Kept as an explicit
                // arm (not folded into the `_` catch-all) to document that the
                // event is part of the fold's vocabulary.
            }
            // ToolCall*, Turn*, Other — not part of the linear conversation fold.
            _ => {}
        }
    }
    Ok(state.items.into_iter().map(|it| it.message).collect())
}

// ── Bundle I/O helpers ───────────────────────────────────────────────────────

/// Read and parse every event line in `trace.jsonl`. Corrupt lines are skipped
/// (tolerant parse — mirrors the rollout reader); they never abort a reduction.
fn read_trace_events(bundle: &TraceBundle) -> Result<Vec<RawTraceEvent>, ReduceError> {
    let trace_path = bundle.trace_path();
    if !trace_path.is_file() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(&trace_path)?;
    let mut events = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<RawTraceEvent>(trimmed) {
            Ok(event) => events.push(event),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "trace.jsonl: skipping unparseable line during reduction"
                );
            }
        }
    }
    Ok(events)
}

/// Read and parse a referenced payload file.
fn read_payload(bundle: &TraceBundle, reff: &RawPayloadRef) -> Result<Value, ReduceError> {
    let abs = bundle.dir().join(&reff.path);
    let body = std::fs::read_to_string(&abs)?;
    Ok(serde_json::from_str(body.trim())?)
}

/// Read the cached reducer state from `state.json`. A missing or corrupt file
/// yields `None` (the reducer re-derives).
fn read_state(bundle: &TraceBundle) -> Result<Option<ReducerState>, ReduceError> {
    let path = bundle.state_path();
    if !path.is_file() {
        return Ok(None);
    }
    let body = std::fs::read_to_string(&path)?;
    match serde_json::from_str::<ReducerState>(body.trim()) {
        Ok(state) => Ok(Some(state)),
        Err(error) => {
            tracing::warn!(error = %error, "state.json: ignoring unparseable reducer cache");
            Ok(None)
        }
    }
}

/// Write the reducer state to `state.json` atomically (temp sibling + rename),
/// falling back to a direct write if the temp write fails.
fn write_state(bundle: &TraceBundle, state: &ReducerState) -> Result<(), ReduceError> {
    let path = bundle.state_path();
    let json = serde_json::to_string_pretty(state)?;
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, &json).is_ok() && std::fs::rename(&tmp, &path).is_ok() {
        return Ok(());
    }
    // Fallback: direct write (the rename may fail across volumes / on some
    // Windows configurations; the cache is advisory, durability is best-effort).
    std::fs::write(&path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{BundleStart, ThreadTraceContext, TraceBundle};
    use crate::event::RawPayloadKind;
    use crate::writer::TraceWriter;
    use serde_json::json;

    fn make_bundle(temp: &tempfile::TempDir) -> TraceBundle {
        TraceBundle::create_at(
            temp.path().to_path_buf(),
            BundleStart::new("root-thread-1", "trace-uuid-1", None),
        )
        .expect("create bundle")
    }

    fn writer(bundle: &TraceBundle) -> TraceWriter {
        let ctx = ThreadTraceContext::new(bundle.dir(), "root-thread-1", None, None);
        TraceWriter::for_thread(&ctx).expect("open writer")
    }

    /// Drive a full inference pair (request payload + InferenceStarted, then
    /// response payload + InferenceCompleted) and flush.
    fn infer(writer: &TraceWriter, request: serde_json::Value, response: serde_json::Value) {
        let req_ref = writer
            .write_json_payload(RawPayloadKind::Request, &request)
            .expect("write request payload");
        writer
            .append(RawTraceEventPayload::InferenceStarted { request_payload: Some(req_ref) })
            .expect("append inference started");
        let resp_ref = writer
            .write_json_payload(RawPayloadKind::Response, &response)
            .expect("write response payload");
        writer
            .append(RawTraceEventPayload::InferenceCompleted { response_payload: Some(resp_ref) })
            .expect("append inference completed");
        writer.flush().expect("flush");
    }

    fn roles(conv: &[ConversationMessage]) -> Vec<String> {
        conv.iter().map(|m| m.role.clone()).collect()
    }

    // (1) Two full-snapshot inferences with matching history: ids reused, no dup.
    #[test]
    fn full_snapshot_reuses_ids_and_does_not_duplicate() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let w = writer(&bundle);

        // Turn 1 full request + response.
        infer(
            &w,
            json!({ "messages": [{ "role": "user", "content": "hi" }] }),
            json!({ "id": "r1", "choices": [{ "message": { "role": "assistant", "content": "hello" } }] }),
        );
        // Turn 2 full request REPEATS the prior history + adds a new user msg.
        infer(
            &w,
            json!({
                "messages": [
                    { "role": "user", "content": "hi" },
                    { "role": "assistant", "content": "hello" },
                    { "role": "user", "content": "again" }
                ]
            }),
            json!({ "id": "r2", "choices": [{ "message": { "role": "assistant", "content": "hi again" } }] }),
        );

        let conv = reduce_conversation(&bundle).expect("reduce");
        // Linear conversation: u hi, a hello, u again, a "hi again" — NO dup of
        // hi/hello even though turn 2's request re-sent them.
        assert_eq!(
            conv.iter().map(|m| m.content.as_str().unwrap_or("")).collect::<Vec<_>>(),
            vec!["hi", "hello", "again", "hi again"],
            "no duplication across full snapshots"
        );
        assert_eq!(roles(&conv), vec!["user", "assistant", "user", "assistant"]);

        // The assistant id from r1 is reused on turn 2's snapshot of "hello".
        let hello = conv.iter().find(|m| m.content == "hello").expect("hello present");
        assert_eq!(hello.id.as_deref(), Some("r1"), "response id reused across snapshots");
    }

    // (2) AppendOnly chain: a request with previous_response_id emits only the
    // delta; the prefix is reused (not re-emitted).
    #[test]
    fn append_only_emits_only_delta_reusing_prefix() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let w = writer(&bundle);

        // Turn 1: full request (no previous_response_id) + response r1.
        infer(
            &w,
            json!({ "messages": [{ "role": "user", "content": "hi" }] }),
            json!({ "id": "r1", "choices": [{ "message": { "role": "assistant", "content": "hello" } }] }),
        );
        // Turn 2: AppendOnly — previous_response_id=r1, input is ONLY the delta.
        infer(
            &w,
            json!({ "previous_response_id": "r1", "input": [{ "role": "user", "content": "more" }] }),
            json!({ "id": "r2", "choices": [{ "message": { "role": "assistant", "content": "sure" } }] }),
        );

        let conv = reduce_conversation(&bundle).expect("reduce");
        // u hi, a hello (prefix from turn 1), u more (delta), a sure (turn 2 reply).
        assert_eq!(
            conv.iter().map(|m| m.content.as_str().unwrap_or("")).collect::<Vec<_>>(),
            vec!["hi", "hello", "more", "sure"],
            "append-only emits only the delta, reusing the prefix"
        );
    }

    // (2b) An unresolved previous_response_id falls back to FullSnapshot
    // (the reducer must not panic on a partial bundle).
    #[test]
    fn unresolved_previous_response_id_falls_back_to_full_snapshot() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let w = writer(&bundle);

        // A request claiming previous_response_id=never-seen, but carrying a
        // FULL messages list. The reducer treats it as a full snapshot.
        infer(
            &w,
            json!({
                "previous_response_id": "never-seen",
                "messages": [{ "role": "user", "content": "cold" }]
            }),
            json!({ "id": "rc", "choices": [{ "message": { "role": "assistant", "content": "start" } }] }),
        );

        let conv = reduce_conversation(&bundle).expect("reduce");
        assert_eq!(
            conv.iter().map(|m| m.content.as_str().unwrap_or("")).collect::<Vec<_>>(),
            vec!["cold", "start"],
            "unresolved previous_response_id → full snapshot fallback"
        );
    }

    // (3) ContextCompacted followed by a full request: post-compaction items
    // win, pre-compaction replaced items are dropped.
    #[test]
    fn post_compaction_full_request_replaces_pre_compaction_history() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let w = writer(&bundle);

        // Pre-compaction: u1, a1, u2, a2 (a long conversation).
        infer(
            &w,
            json!({ "messages": [
                { "role": "user", "content": "u1" },
                { "role": "assistant", "content": "a1" },
                { "role": "user", "content": "u2" }
            ]}),
            json!({ "id": "rp", "choices": [{ "message": { "role": "assistant", "content": "a2" } }] }),
        );

        // Compaction happens.
        w.append(RawTraceEventPayload::ContextCompacted).expect("append compacted");
        w.flush().expect("flush");

        // Post-compaction full request: summary + recent window (u2, a2).
        // Crucially does NOT include u1/a1 (they were summarized away).
        infer(
            &w,
            json!({ "messages": [
                { "role": "system", "content": "summary of earlier" },
                { "role": "user", "content": "u2" },
                { "role": "assistant", "content": "a2" },
                { "role": "user", "content": "u3" }
            ]}),
            json!({ "id": "rq", "choices": [{ "message": { "role": "assistant", "content": "a3" } }] }),
        );

        let conv = reduce_conversation(&bundle).expect("reduce");
        // u1/a1 are DROPPED (compaction replaced them); the post-compaction
        // history (summary, u2, a2) + the new u3/a3 win.
        assert_eq!(
            conv.iter().map(|m| m.content.as_str().unwrap_or("")).collect::<Vec<_>>(),
            vec!["summary of earlier", "u2", "a2", "u3", "a3"],
            "post-compaction items win; pre-compaction u1/a1 dropped"
        );
        // u1 is gone entirely.
        assert!(
            !conv.iter().any(|m| m.content.as_str() == Some("u1")),
            "pre-compaction u1 must be dropped after compaction"
        );
        assert!(
            !conv.iter().any(|m| m.content.as_str() == Some("a1")),
            "pre-compaction a1 must be dropped after compaction"
        );
    }

    // (4) state.json cache: a second cached run reuses the cache when it covers
    // all current events; a stale cache is re-derived.
    #[test]
    fn cached_run_reuses_state_then_rederives_when_stale() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let w = writer(&bundle);

        infer(
            &w,
            json!({ "messages": [{ "role": "user", "content": "one" }] }),
            json!({ "id": "r1", "choices": [{ "message": { "role": "assistant", "content": "two" } }] }),
        );

        // First cached reduction writes state.json.
        let first = reduce_conversation_cached(&bundle).expect("first reduce");
        assert_eq!(
            first.iter().map(|m| m.content.as_str().unwrap_or("")).collect::<Vec<_>>(),
            vec!["one", "two"]
        );
        assert!(bundle.state_path().is_file(), "state.json written");

        // Tamper with the on-disk conversation to prove the cache is reused
        // when it covers all current events. Turn 1 wrote exactly 2 events
        // (InferenceStarted + InferenceCompleted), so a cache claiming 2 covers.
        let tampered = serde_json::json!({
            "version": REDUCER_STATE_VERSION,
            "events_processed": 2,
            "conversation": [{ "role": "sentinel", "content": "from-cache" }]
        });
        std::fs::write(bundle.state_path(), serde_json::to_string(&tampered).unwrap()).unwrap();

        let reused = reduce_conversation_cached(&bundle).expect("cached reduce");
        assert_eq!(reused[0].role, "sentinel", "covering cache is reused verbatim");
        assert_eq!(reused[0].content.as_str().unwrap(), "from-cache");

        // Append a NEW inference → the cache no longer covers all events →
        // re-derive and overwrite.
        infer(
            &w,
            json!({ "messages": [{ "role": "user", "content": "one" }, { "role": "assistant", "content": "two" }, { "role": "user", "content": "three" }] }),
            json!({ "id": "r2", "choices": [{ "message": { "role": "assistant", "content": "four" } }] }),
        );
        let rederived = reduce_conversation_cached(&bundle).expect("rederive");
        assert_eq!(
            rederived.iter().map(|m| m.content.as_str().unwrap_or("")).collect::<Vec<_>>(),
            vec!["one", "two", "three", "four"],
            "stale cache re-derived"
        );
    }

    // (5) Best-effort: a bundle with empty / odd payloads reduces to empty
    // without panicking.
    #[test]
    fn empty_or_odd_payloads_reduce_without_panicking() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let w = writer(&bundle);

        // InferenceStarted with no recognizable fields.
        let req_ref =
            w.write_json_payload(RawPayloadKind::Request, &json!({ "model": "x" })).expect("write");
        w.append(RawTraceEventPayload::InferenceStarted { request_payload: Some(req_ref) })
            .expect("append");
        // InferenceCompleted with no choices / output.
        let resp_ref =
            w.write_json_payload(RawPayloadKind::Response, &json!({ "id": "rx" })).expect("write");
        w.append(RawTraceEventPayload::InferenceCompleted { response_payload: Some(resp_ref) })
            .expect("append");
        w.flush().expect("flush");

        let conv = reduce_conversation(&bundle).expect("reduce");
        assert!(conv.is_empty(), "odd payloads → empty conversation, no panic");
    }

    // (6) responses-style output[] assistant extraction.
    #[test]
    fn responses_style_output_is_extracted() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let w = writer(&bundle);

        infer(
            &w,
            json!({ "input": [{ "role": "user", "content": "q" }] }),
            json!({
                "id": "resp_1",
                "output": [
                    { "type": "message", "role": "assistant", "content": "ans" }
                ]
            }),
        );
        let conv = reduce_conversation(&bundle).expect("reduce");
        assert_eq!(
            conv.iter().map(|m| m.content.as_str().unwrap_or("")).collect::<Vec<_>>(),
            vec!["q", "ans"]
        );
    }

    // (7) J1: an AppendOnly delta whose content is IDENTICAL to an earlier
    // message is NEW input the model received — it MUST be kept. The pre-fix
    // fingerprint-dedup path silently dropped it, producing a conversation that
    // did NOT match what the model saw. This test fails on the pre-fix code.
    #[test]
    fn append_only_keeps_repeated_duplicate_delta() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let w = writer(&bundle);

        // Turn 1: full request with user "hello" + response r1.
        infer(
            &w,
            json!({ "messages": [{ "role": "user", "content": "hello" }] }),
            json!({ "id": "r1", "choices": [{ "message": { "role": "assistant", "content": "hi" } }] }),
        );
        // Turn 2: AppendOnly (previous_response_id=r1) — the user sends the
        // SAME text "hello" again. The model SAW both.
        infer(
            &w,
            json!({
                "previous_response_id": "r1",
                "input": [{ "role": "user", "content": "hello" }]
            }),
            json!({ "id": "r2", "choices": [{ "message": { "role": "assistant", "content": "bye" } }] }),
        );

        let conv = reduce_conversation(&bundle).expect("reduce");
        // BOTH user "hello" messages must survive (the prefix one and the
        // repeated delta one). The pre-fix fingerprint dedup dropped the second.
        let hello_count =
            conv.iter().filter(|m| m.role == "user" && m.content.as_str() == Some("hello")).count();
        assert_eq!(hello_count, 2, "a legitimately repeated delta is kept, not deduped");
        // Full linear conversation: u hello, a hi, u hello (repeated), a bye.
        assert_eq!(
            conv.iter().map(|m| m.content.as_str().unwrap_or("")).collect::<Vec<_>>(),
            vec!["hello", "hi", "hello", "bye"],
            "repeated AppendOnly delta appended verbatim"
        );
    }

    // (8) J2: a FullSnapshot replace INVALIDATES response_index. A later
    // AppendOnly request referencing a pre-snapshot response id (replaced out of
    // the lineage) must NOT resolve it — it falls back to FullSnapshot instead
    // of reconstructing a stale lineage. This test fails on the pre-fix code
    // (stale response_index let the AppendOnly resolve and wrongly reuse the
    // post-snapshot items as a prefix).
    #[test]
    fn full_snapshot_invalidates_response_index_so_stale_appendonly_falls_back() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let w = writer(&bundle);

        // Turn 1: full request → response rA ("replyA").
        infer(
            &w,
            json!({ "messages": [{ "role": "user", "content": "first" }] }),
            json!({ "id": "rA", "choices": [{ "message": { "role": "assistant", "content": "replyA" } }] }),
        );
        // Turn 2: a DIVERGENT full snapshot replaces the history — rA's lineage
        // ("first"/"replyA") is dropped. rA is no longer in the conversation.
        infer(
            &w,
            json!({ "messages": [{ "role": "user", "content": "second" }] }),
            json!({ "id": "rB", "choices": [{ "message": { "role": "assistant", "content": "replyB" } }] }),
        );
        // Turn 3: AppendOnly claiming previous_response_id=rA (a pre-snapshot
        // id no longer in the lineage), carrying its own full messages list.
        // The reducer must treat the unresolved prev_id as a FullSnapshot
        // fallback, NOT append the delta onto the (unrelated) post-snapshot
        // items.
        infer(
            &w,
            json!({
                "previous_response_id": "rA",
                "messages": [{ "role": "user", "content": "third" }]
            }),
            json!({ "id": "rC", "choices": [{ "message": { "role": "assistant", "content": "replyC" } }] }),
        );

        let conv = reduce_conversation(&bundle).expect("reduce");
        // rA's lineage was replaced out — neither "first" nor "replyA" survives.
        assert!(
            !conv.iter().any(|m| m.content.as_str() == Some("replyA")),
            "replaced-out rA lineage must not be reconstructed"
        );
        // The post-snapshot lineage ("second"/"replyB") must NOT be reused as a
        // prefix for turn 3's delta — turn 3 fell back to FullSnapshot, so only
        // turn 3's own messages + reply survive.
        assert!(
            !conv.iter().any(|m| m.content.as_str() == Some("second")),
            "stale AppendOnly must not reuse the unrelated post-snapshot items as a prefix"
        );
        assert_eq!(
            conv.iter().map(|m| m.content.as_str().unwrap_or("")).collect::<Vec<_>>(),
            vec!["third", "replyC"],
            "unresolved (replaced-out) previous_response_id → FullSnapshot fallback"
        );
    }

    // (9) J3a: a truncated (shrunk) trace.jsonl forces a re-derive. The cache
    // claims more events than now exist; `==` (not `>=`) detects the shrinkage
    // symmetrically with the appended/grow case. This test fails on the pre-fix
    // `>=` code (which returned the stale covering cache).
    #[test]
    fn cached_run_rederives_when_trace_truncated() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let w = writer(&bundle);

        // Two inference pairs → 4 trace events.
        infer(
            &w,
            json!({ "messages": [{ "role": "user", "content": "a" }] }),
            json!({ "id": "r1", "choices": [{ "message": { "role": "assistant", "content": "b" } }] }),
        );
        infer(
            &w,
            json!({
                "messages": [
                    { "role": "user", "content": "a" },
                    { "role": "assistant", "content": "b" },
                    { "role": "user", "content": "c" }
                ]
            }),
            json!({ "id": "r2", "choices": [{ "message": { "role": "assistant", "content": "d" } }] }),
        );

        // First cached reduction writes state.json claiming 4 events.
        let first = reduce_conversation_cached(&bundle).expect("first");
        assert_eq!(
            first.iter().map(|m| m.content.as_str().unwrap_or("")).collect::<Vec<_>>(),
            vec!["a", "b", "c", "d"]
        );

        // Tamper the cache to a sentinel, still claiming 4 events (a covering
        // cache for the un-truncated trace) so a stale return is detectable.
        let tampered = serde_json::json!({
            "version": REDUCER_STATE_VERSION,
            "events_processed": 4,
            "conversation": [{ "role": "sentinel", "content": "from-cache" }]
        });
        std::fs::write(bundle.state_path(), serde_json::to_string(&tampered).unwrap()).unwrap();

        // Truncate trace.jsonl to just its FIRST line — the bundle shrank.
        let trace = std::fs::read_to_string(bundle.trace_path()).expect("read trace");
        let first_line = trace.lines().next().expect("at least one line").to_owned();
        std::fs::write(bundle.trace_path(), first_line).expect("truncate trace");

        let rederived = reduce_conversation_cached(&bundle).expect("rederive");
        // `4 == 1` is false → re-derive; the sentinel must be gone. (Pre-fix
        // `4 >= 1` was true → returned the stale sentinel.)
        assert!(
            !rederived.iter().any(|m| m.role == "sentinel"),
            "truncated trace must force a re-derive, not return the stale covering cache"
        );
    }
}
