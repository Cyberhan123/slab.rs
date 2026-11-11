//! `/v1/agents/rollouts` — read-only rollout debug viewer endpoints.
//!
//! The rollout JSONL is the conversation's true source: LLM-grade messages
//! (embedded `<think>` blocks, name-tagged injection fragments), per-turn
//! `TurnState` prompt snapshots, `Compacted` baselines, and (under Extended
//! persistence) raw events. These endpoints expose it for the "Agent 调试追踪"
//! viewer — gated on `agent.debug` (404 when off, so the diagnostic surface
//! does not even reveal existence), and strictly read-only.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::error::ServerError;
use slab_agent_rollout::read_rollout_lines;
use slab_app_core::context::AppState;

use super::harness::thread_from_timeline;

/// Default / maximum page size for raw line reads.
const LINES_DEFAULT_LIMIT: usize = 1000;
const LINES_MAX_LIMIT: usize = 5000;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_rollout_sessions,
        read_rollout_lines_endpoint,
        read_rollout_timeline,
        read_rollout_trace
    ),
    components(schemas(
        RolloutSessionEntry,
        RolloutLineEntry,
        RolloutLinesResponse,
        RolloutTimelineResponse,
        RolloutTraceResponse
    ))
)]
pub struct RolloutsApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/agents/rollouts", get(list_rollout_sessions))
        .route("/agents/rollouts/{thread_id}/lines", get(read_rollout_lines_endpoint))
        .route("/agents/rollouts/{thread_id}/timeline", get(read_rollout_timeline))
        .route("/agents/rollouts/{thread_id}/trace", get(read_rollout_trace))
}

/// 404 unless the "Agent 调试追踪" (`agent.debug`) setting is on — the
/// diagnostic surface must not exist (let alone leak data) when debugging is
/// disabled.
fn debug_gate(state: &AppState) -> Result<(), ServerError> {
    let settings = state.context.pmid.config();
    if settings.agent.debug {
        Ok(())
    } else {
        Err(ServerError::NotFound("agent debug tracing is disabled".to_owned()))
    }
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
struct ThreadIdPath {
    thread_id: String,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
struct LinesQuery {
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
struct TraceQuery {
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

/// One rollout session (thread) discovered on disk.
#[derive(Debug, Serialize, ToSchema)]
pub struct RolloutSessionEntry {
    pub thread_id: String,
    pub session_id: String,
    pub started_at: String,
    #[schema(nullable = true)]
    pub role_name: Option<String>,
    /// Whether the rollout links to a trace bundle (`SessionMeta.trace_path`).
    pub has_trace: bool,
    pub file_name: String,
    pub size_bytes: u64,
}

/// One raw rollout line, item JSON passed through verbatim.
#[derive(Debug, Serialize, ToSchema)]
pub struct RolloutLineEntry {
    /// 0-based position of the line in the file.
    pub index: usize,
    pub timestamp: String,
    /// The `rolloutType` discriminant (`sessionMeta` / `turnItem` / `eventMsg`
    /// / `compacted` / `turnContext`).
    pub rollout_type: String,
    #[schema(value_type = Object)]
    pub item: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RolloutLinesResponse {
    pub lines: Vec<RolloutLineEntry>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    /// Whether more lines exist past the returned window.
    pub truncated: bool,
}

/// The rollout timeline projected into the harness `Thread` wire type (same
/// projection `thread/resume` restores history with) plus every turn's final
/// `TurnState` input messages — the exact prompt the model was sent.
#[derive(Debug, Serialize, ToSchema)]
pub struct RolloutTimelineResponse {
    #[schema(value_type = Object)]
    pub thread: serde_json::Value,
    /// Per turn (indexed): the last `Sampling` turn-state's input messages —
    /// the exact final prompt the model was sent — or `null` when the turn has
    /// no sampling snapshot.
    pub turn_prompts: Vec<Option<serde_json::Value>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RolloutTraceResponse {
    #[schema(value_type = Object)]
    pub manifest: serde_json::Value,
    /// Raw `trace.jsonl` events (paged, JSON passed through verbatim).
    pub events: Vec<serde_json::Value>,
    pub total_events: usize,
    /// The reducer's reconstruction of the conversation the model actually
    /// saw (L3 semantic replay). Empty on a reduction failure.
    pub conversation: Vec<serde_json::Value>,
}

#[utoipa::path(
    get,
    path = "/v1/agents/rollouts",
    tag = "agents",
    responses(
        (status = 200, description = "Rollout sessions on disk (debug viewer)", body = [RolloutSessionEntry]),
        (status = 404, description = "Agent debug tracing disabled"),
    )
)]
async fn list_rollout_sessions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<RolloutSessionEntry>>, ServerError> {
    debug_gate(&state)?;
    let service = state.services.harness.clone();
    let store = service.rollout_store();

    let mut entries = Vec::new();
    for meta in store.list_all_session_metas() {
        let path = store.resolve_path(&meta.thread_id);
        let (size_bytes, file_name) = match std::fs::metadata(&path) {
            Ok(metadata) => (
                metadata.len(),
                path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            ),
            Err(_) => (0, String::new()),
        };
        entries.push(RolloutSessionEntry {
            thread_id: meta.thread_id,
            session_id: meta.session_id,
            started_at: meta.started_at,
            role_name: meta.role_name,
            has_trace: meta.trace_path.is_some(),
            file_name,
            size_bytes,
        });
    }
    // Newest first — the viewer's rail lists recent sessions on top.
    entries.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(Json(entries))
}

#[utoipa::path(
    get,
    path = "/v1/agents/rollouts/{thread_id}/lines",
    tag = "agents",
    params(
        ("thread_id" = String, Path, description = "Real slab thread id"),
        ("offset" = Option<usize>, Query, description = "First line index (0-based, default 0)"),
        ("limit" = Option<usize>, Query, description = "Page size (default 1000, max 5000)")
    ),
    responses(
        (status = 200, description = "Raw rollout lines (item JSON verbatim)", body = RolloutLinesResponse),
        (status = 404, description = "Thread rollout not found, or debug tracing disabled"),
    )
)]
async fn read_rollout_lines_endpoint(
    State(state): State<Arc<AppState>>,
    Path(params): Path<ThreadIdPath>,
    Query(query): Query<LinesQuery>,
) -> Result<Json<RolloutLinesResponse>, ServerError> {
    debug_gate(&state)?;
    let service = state.services.harness.clone();
    let path = service.rollout_store().resolve_path(&params.thread_id);
    if !path.exists() {
        return Err(ServerError::NotFound(format!("no rollout for thread {}", params.thread_id)));
    }

    let limit = query.limit.unwrap_or(LINES_DEFAULT_LIMIT).clamp(1, LINES_MAX_LIMIT);
    let offset = query.offset.unwrap_or(0);
    let all = read_rollout_lines(&path);
    let total = all.len();
    let window = all
        .into_iter()
        .enumerate()
        .skip(offset)
        .take(limit)
        .filter_map(|(index, line)| {
            let value = serde_json::to_value(&line).ok()?;
            let item = value.get("item").cloned()?;
            Some(RolloutLineEntry {
                index,
                rollout_type: value
                    .get("rolloutType")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                timestamp: line.timestamp,
                item,
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(RolloutLinesResponse {
        truncated: offset + window.len() < total,
        total,
        offset,
        limit,
        lines: window,
    }))
}

#[utoipa::path(
    get,
    path = "/v1/agents/rollouts/{thread_id}/timeline",
    tag = "agents",
    params(("thread_id" = String, Path, description = "Real slab thread id")),
    responses(
        (status = 200, description = "Rollout timeline projected as a harness Thread + per-turn final prompts", body = RolloutTimelineResponse),
        (status = 404, description = "Thread rollout not found, or debug tracing disabled"),
    )
)]
async fn read_rollout_timeline(
    State(state): State<Arc<AppState>>,
    Path(params): Path<ThreadIdPath>,
) -> Result<Json<RolloutTimelineResponse>, ServerError> {
    debug_gate(&state)?;
    let service = state.services.harness.clone();
    let snapshot = service.thread_snapshot(&params.thread_id).await?.ok_or_else(|| {
        ServerError::NotFound(format!("no rollout for thread {}", params.thread_id))
    })?;
    let turn_states = service.list_turn_states(&params.thread_id).await?;
    let timeline = service.list_turn_timeline(&params.thread_id).await?;

    let thread = thread_from_timeline(&snapshot.id, &snapshot, &turn_states, &timeline);
    let turn_count = thread.turns.len();
    let thread_value =
        serde_json::to_value(&thread).map_err(|error| ServerError::Internal(error.to_string()))?;
    let turn_prompts: Vec<Option<serde_json::Value>> =
        (0..turn_count as u32).map(|index| turn_prompt_for_turn(&turn_states, index)).collect();

    Ok(Json(RolloutTimelineResponse { thread: thread_value, turn_prompts }))
}

/// The per-turn timeline prompt: the LAST `Sampling` turn-state's input — the
/// exact prompt the model was sent on that turn's final iteration — or `None`
/// when the turn has no sampling snapshot.
///
/// Matching on the typed variant (not just "the last state of the turn") is
/// load-bearing: a turn also emits `Phase` lines and a terminal `Finished`
/// state, which carry no input — the old "take the last row" rule surfaced
/// those input-less rows as null prompts once terminal states stopped copying
/// the input snapshot.
fn turn_prompt_for_turn(
    turn_states: &[slab_agent::port::TurnStateRecord],
    turn_index: u32,
) -> Option<serde_json::Value> {
    turn_states
        .iter()
        .rev()
        .filter(|state| state.turn_index == turn_index)
        .find_map(|state| state.state.sampling_input())
        .and_then(|messages| serde_json::to_value(messages).ok())
}

#[utoipa::path(
    get,
    path = "/v1/agents/rollouts/{thread_id}/trace",
    tag = "agents",
    params(
        ("thread_id" = String, Path, description = "Real slab thread id"),
        ("offset" = Option<usize>, Query, description = "First event index (0-based, default 0)"),
        ("limit" = Option<usize>, Query, description = "Event page size (default 1000, max 5000)")
    ),
    responses(
        (status = 200, description = "Trace bundle manifest + events + the reduced conversation the model saw", body = RolloutTraceResponse),
        (status = 404, description = "No trace bundle for the thread, or debug tracing disabled"),
    )
)]
async fn read_rollout_trace(
    State(state): State<Arc<AppState>>,
    Path(params): Path<ThreadIdPath>,
    Query(query): Query<TraceQuery>,
) -> Result<Json<RolloutTraceResponse>, ServerError> {
    debug_gate(&state)?;
    let service = state.services.harness.clone();
    let bundle = service.trace_bundle_for(&params.thread_id).await?.ok_or_else(|| {
        ServerError::NotFound(format!("no trace bundle for thread {}", params.thread_id))
    })?;

    // Raw events: parse each trace.jsonl line as JSON (unparseable lines are
    // skipped — the bundle may still be receiving appends).
    let raw = std::fs::read_to_string(bundle.trace_path())
        .map_err(|error| ServerError::Internal(error.to_string()))?;
    let all: Vec<serde_json::Value> = raw
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let total_events = all.len();
    let limit = query.limit.unwrap_or(LINES_DEFAULT_LIMIT).clamp(1, LINES_MAX_LIMIT);
    let offset = query.offset.unwrap_or(0);
    let events: Vec<serde_json::Value> = all.into_iter().skip(offset).take(limit).collect();

    // L3 semantic replay: what the conversation actually looked like to the
    // model across this bundle's events (cached in the bundle's state.json).
    let conversation =
        slab_agent_tracing::reducer::conversation::reduce_conversation_cached(&bundle)
            .map_err(|error| ServerError::Internal(error.to_string()))?
            .iter()
            .filter_map(|message| serde_json::to_value(message).ok())
            .collect();

    let manifest = serde_json::to_value(bundle.manifest())
        .map_err(|error| ServerError::Internal(error.to_string()))?;
    Ok(Json(RolloutTraceResponse { manifest, events, total_events, conversation }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_agent::port::TurnStateRecord;
    use slab_agent::protocol::TurnStatePayload;
    use slab_types::ConversationMessage;

    fn user_msg(text: &str) -> ConversationMessage {
        ConversationMessage {
            role: "user".to_owned(),
            content: slab_types::ConversationMessageContent::Text(text.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    fn state_record(turn_index: u32, status: &str, state: TurnStatePayload) -> TurnStateRecord {
        TurnStateRecord {
            thread_id: "t".to_owned(),
            turn_index,
            status: status.to_owned(),
            state,
            started_at: "2026-01-01T00:00:00Z".to_owned(),
        }
    }

    fn sampling(turn_index: u32, texts: &[&str]) -> TurnStateRecord {
        state_record(
            turn_index,
            "sampling",
            TurnStatePayload::Sampling {
                input_messages: texts.iter().map(|t| user_msg(t)).collect(),
                tool_specs_json: None,
            },
        )
    }

    // Regression for the old `.rev().find(last row of the turn)` rule: a turn
    // ends with `Phase` lines and a terminal `Finished` state, which carry NO
    // input — the prompt lookup must skip them (typed variant match) and take
    // the LAST Sampling snapshot, i.e. the final iteration's actual prompt.
    #[test]
    fn turn_prompt_takes_last_sampling_snapshot_past_phase_and_finished_rows() {
        let states = vec![
            sampling(0, &["iter-1 prompt"]),
            state_record(0, "executing_tools", TurnStatePayload::Phase),
            sampling(0, &["iter-1 prompt", "tool result", "iter-2 prompt"]),
            state_record(0, "executing_tools", TurnStatePayload::Phase),
            state_record(
                0,
                "completed",
                TurnStatePayload::Finished {
                    error: None,
                    completed_at: Some("2026-01-01T00:00:09Z".to_owned()),
                },
            ),
        ];
        let prompt = turn_prompt_for_turn(&states, 0).expect("sampling snapshot present");
        let texts: Vec<String> = prompt
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["content"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            texts,
            vec!["iter-1 prompt", "tool result", "iter-2 prompt"],
            "the FINAL iteration's snapshot wins, not the terminal/phase rows"
        );
    }

    // A turn with only phase/terminal rows (no sampling snapshot) has no
    // prompt — null, not an empty array.
    #[test]
    fn turn_prompt_is_none_without_sampling_snapshot() {
        let states = vec![
            state_record(1, "awaiting_approval", TurnStatePayload::Phase),
            state_record(
                1,
                "interrupted",
                TurnStatePayload::Finished {
                    error: Some("interrupted".to_owned()),
                    completed_at: Some("2026-01-01T00:00:09Z".to_owned()),
                },
            ),
        ];
        assert!(turn_prompt_for_turn(&states, 1).is_none());
        assert!(turn_prompt_for_turn(&states, 9).is_none(), "absent turn → null");
    }
}
