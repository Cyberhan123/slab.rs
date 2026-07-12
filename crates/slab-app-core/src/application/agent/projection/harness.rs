//! Harness projection — pure conversion from slab-agent events to the
//! harness wire protocol (`slab_proto::harness`).
//!
//! This mirrors the OpenAI Responses projection (now in
//! `crate::domain::services::agent::response::projection`) in shape but emits
//! the slab-owned thread/turn/item model instead of OpenAI Responses events.
//! One inbound [`AgentEventEnvelope`] maps to zero or more [`EventMsg`]s; the
//! server dispatcher lifts each into a JSON-RPC notification.
//!
//! Boundary: pure conversion only. No `tokio`, `axum`, or agent-service calls.

use std::collections::HashSet;

use slab_agent::AgentEventKind;
use slab_agent::port::TurnEvent;
use slab_proto::harness::error::ErrorEvent;
use slab_proto::harness::event::{EventMsg, TurnAbortedParams};
use slab_proto::harness::item::TurnItem;
use slab_proto::harness::messages::Turn;
use slab_proto::harness::notification::*;

use crate::infra::agent::event_hub::AgentEventEnvelope;

/// Command-execution metadata captured at `item/started` and threaded through
/// to `item/completed`. `slab-agent` only repeats the command/cwd on the start
/// event; the completion event carries output only, so the projection must
/// retain this to avoid losing it on the wire.
#[derive(Debug, Clone, Default)]
struct CommandMeta {
    command: String,
    cwd: String,
}

/// Stateful projector that turns a stream of [`AgentEventEnvelope`]s into the
/// harness event model. Tracks which items have already been announced via
/// `item/started` so each item id emits exactly one start, and retains
/// per-item metadata (e.g. command/cwd) that upstream events only report once.
#[derive(Debug, Default)]
pub struct HarnessProjection {
    started_items: HashSet<String>,
    turn_started: bool,
    command_meta: std::collections::HashMap<String, CommandMeta>,
}

impl HarnessProjection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset per-turn state (call when a new turn begins on the same thread).
    pub fn reset(&mut self) {
        self.started_items.clear();
        self.turn_started = false;
        self.command_meta.clear();
    }

    /// Project one envelope into zero or more harness events.
    ///
    /// `thread_id` is the slab thread id; the harness `turn_id` is synthesized
    /// from the envelope's turn index (slab-agent identifies turns by index,
    /// not by string id).
    pub fn project(&mut self, thread_id: &str, envelope: &AgentEventEnvelope) -> Vec<EventMsg> {
        let TurnEvent::Response { turn_index, event } = &envelope.event;
        let turn_id = turn_index.map(|i| i.to_string()).unwrap_or_else(|| "current".to_owned());
        self.project_event(thread_id, &turn_id, event)
    }

    fn project_event(
        &mut self,
        thread_id: &str,
        turn_id: &str,
        event: &AgentEventKind,
    ) -> Vec<EventMsg> {
        let tid = thread_id.to_owned();
        match event {
            // ---- lifecycle ----
            AgentEventKind::ResponseQueued { .. } | AgentEventKind::ResponseInProgress { .. } => {
                if !self.turn_started {
                    self.turn_started = true;
                    vec![EventMsg::TurnStarted(TurnStartedParams {
                        thread_id: tid,
                        turn: Turn {
                            id: turn_id.to_owned(),
                            items: vec![],
                            status: "inProgress".to_owned(),
                            error: None,
                        },
                    })]
                } else {
                    vec![]
                }
            }
            AgentEventKind::ResponseCompleted { .. } => {
                vec![EventMsg::TurnCompleted(TurnCompletedParams {
                    thread_id: tid,
                    turn: Turn {
                        id: turn_id.to_owned(),
                        items: vec![],
                        status: "completed".to_owned(),
                        error: None,
                    },
                })]
            }
            AgentEventKind::ResponseCancelled { .. } => {
                vec![EventMsg::TurnAborted(TurnAbortedParams {
                    thread_id: tid,
                    turn: Turn {
                        id: turn_id.to_owned(),
                        items: vec![],
                        status: "interrupted".to_owned(),
                        error: None,
                    },
                })]
            }
            AgentEventKind::ResponseFailed { error, error_code, .. } => {
                let mut ev = ErrorEvent::new(error.clone());
                if let Some(code) = error_code {
                    ev = ev.with_code(code.clone());
                }
                vec![EventMsg::Error(ev)]
            }
            AgentEventKind::AgentStreamLagged => {
                vec![EventMsg::Error(
                    ErrorEvent::new("event stream lagged").with_code("stream_lagged"),
                )]
            }

            // ---- assistant text ----
            AgentEventKind::ResponseOutputTextDelta { item_id, delta, .. } => {
                let mut out = Vec::new();
                if self.started_items.insert(item_id.clone()) {
                    out.push(EventMsg::ItemStarted(ItemStartedParams {
                        item: TurnItem::AgentMessage { id: item_id.clone(), text: String::new() },
                        thread_id: tid.clone(),
                        turn_id: turn_id.to_owned(),
                    }));
                }
                out.push(EventMsg::AgentMessageDelta(AgentMessageDeltaParams {
                    thread_id: tid,
                    turn_id: turn_id.to_owned(),
                    item_id: item_id.clone(),
                    delta: delta.clone(),
                }));
                out
            }
            AgentEventKind::ResponseOutputTextDone { item_id, text, .. } => {
                vec![EventMsg::ItemCompleted(ItemCompletedParams {
                    item: TurnItem::AgentMessage { id: item_id.clone(), text: text.clone() },
                    thread_id: tid,
                    turn_id: turn_id.to_owned(),
                })]
            }

            // ---- reasoning ----
            AgentEventKind::ResponseReasoningTextDelta {
                item_id, content_index, delta, ..
            } => {
                vec![EventMsg::ReasoningTextDelta(ReasoningTextDeltaParams {
                    thread_id: tid,
                    turn_id: turn_id.to_owned(),
                    item_id: item_id.clone(),
                    content_index: (*content_index).max(0) as u32,
                    delta: delta.clone(),
                })]
            }
            AgentEventKind::ResponseReasoningTextDone { item_id, text, summary, .. } => {
                // `summary` is the model-authored recap of the reasoning; `text`
                // is the full trace. They are distinct fields on the wire, so
                // fall back to `text` only when the agent didn't provide a
                // summary rather than echoing the same string into both.
                let summary_text = summary.clone().unwrap_or_else(|| text.clone());
                vec![EventMsg::ItemCompleted(ItemCompletedParams {
                    item: TurnItem::Reasoning {
                        id: item_id.clone(),
                        summary: slab_proto::harness::item::ReasoningText::one(summary_text),
                        content: slab_proto::harness::item::ReasoningText::one(text.clone()),
                    },
                    thread_id: tid,
                    turn_id: turn_id.to_owned(),
                })]
            }

            // ---- shell / command execution ----
            AgentEventKind::ResponseShellCallOutputContentDelta { item_id, delta, .. } => {
                vec![EventMsg::CommandExecutionOutputDelta(CommandExecutionOutputDeltaParams {
                    thread_id: tid,
                    turn_id: turn_id.to_owned(),
                    item_id: item_id.clone(),
                    delta: delta.clone(),
                })]
            }
            AgentEventKind::ResponseShellCallOutputContentDone { item_id, outputs, .. } => {
                let meta = self.command_meta.remove(item_id).unwrap_or_default();
                vec![EventMsg::ItemCompleted(ItemCompletedParams {
                    item: TurnItem::CommandExecution {
                        id: item_id.clone(),
                        command: meta.command,
                        cwd: meta.cwd,
                        process_id: None,
                        status: "completed".to_owned(),
                        aggregated_output: Some(serde_json::to_string(outputs).unwrap_or_default()),
                        exit_code: None,
                        duration_ms: None,
                    },
                    thread_id: tid,
                    turn_id: turn_id.to_owned(),
                })]
            }
            AgentEventKind::ResponseLocalShellCallDone {
                item_id,
                command,
                working_directory,
                ..
            } => self.command_started(
                &tid,
                turn_id,
                item_id,
                command.join(" "),
                working_directory.clone().unwrap_or_default(),
            ),
            AgentEventKind::ResponseFunctionShellCallDone { item_id, commands, .. } => {
                // The function-shell variant doesn't report a working directory;
                // leave `cwd` empty rather than guessing.
                self.command_started(&tid, turn_id, item_id, commands.join(" "), String::new())
            }

            // ---- file changes ----
            AgentEventKind::ResponseApplyPatchCallDone {
                item_id,
                operation_type,
                path,
                diff,
                ..
            } => {
                vec![EventMsg::ItemCompleted(ItemCompletedParams {
                    item: TurnItem::FileChange {
                        id: item_id.clone(),
                        changes: vec![serde_json::json!({
                            "path": path,
                            "type": operation_type,
                            "diff": diff,
                        })],
                        status: "completed".to_owned(),
                    },
                    thread_id: tid,
                    turn_id: turn_id.to_owned(),
                })]
            }

            // ---- MCP / web-search / tool calls ----
            AgentEventKind::ResponseMcpCallDone {
                item_id,
                server_label,
                name,
                arguments,
                output,
                error,
                status,
                ..
            } => {
                let args = serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
                vec![EventMsg::ItemCompleted(ItemCompletedParams {
                    item: TurnItem::McpToolCall {
                        id: item_id.clone(),
                        server: server_label.clone(),
                        tool: name.clone(),
                        arguments: args,
                        status: status.clone().unwrap_or_else(|| "completed".to_owned()),
                        result: output.as_ref().and_then(|s| serde_json::from_str(s).ok()),
                        error: error.as_ref().and_then(|s| serde_json::from_str(s).ok()),
                        duration_ms: None,
                    },
                    thread_id: tid,
                    turn_id: turn_id.to_owned(),
                })]
            }
            AgentEventKind::ResponseWebSearchCallDone { item_id, action, .. } => {
                let query = action.get("query").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                vec![EventMsg::ItemCompleted(ItemCompletedParams {
                    item: TurnItem::WebSearch { id: item_id.clone(), query },
                    thread_id: tid,
                    turn_id: turn_id.to_owned(),
                })]
            }

            // ---- approvals ----
            AgentEventKind::ResponseToolCallApprovalRequired {
                item_id, command, category, ..
            } => {
                vec![EventMsg::CommandExecutionRequestApproval(
                    CommandExecutionRequestApprovalParams {
                        thread_id: tid,
                        turn_id: turn_id.to_owned(),
                        item_id: item_id.clone(),
                        command: command.clone(),
                        cwd: String::new(),
                        reason: None,
                        category: Some(*category),
                        allowed_scopes: default_allowed_scopes(),
                    },
                )]
            }

            // ---- dropped (no harness equivalent yet) ----
            // metrics, background, compaction, context.compact, tool_search,
            // image_generation, file_search, code_interpreter, mcp_list_tools,
            // mcp_approval_request, function_call/custom_tool_call deltas,
            // concurrency, validation_failed, approval_resolved, agent.status.
            _ => vec![],
        }
    }

    /// Emit an `item/started` for a command-execution item the first time its
    /// item id is seen on this turn, and retain its command/cwd so the later
    /// `item/completed` (which only carries output) can echo it back.
    fn command_started(
        &mut self,
        thread_id: &str,
        turn_id: &str,
        item_id: &str,
        command: String,
        cwd: String,
    ) -> Vec<EventMsg> {
        let meta = CommandMeta { command, cwd };
        self.command_meta.insert(item_id.to_owned(), meta.clone());
        if self.started_items.insert(item_id.to_owned()) {
            vec![EventMsg::ItemStarted(ItemStartedParams {
                item: TurnItem::CommandExecution {
                    id: item_id.to_owned(),
                    command: meta.command,
                    cwd: meta.cwd,
                    process_id: None,
                    status: "running".to_owned(),
                    aggregated_output: None,
                    exit_code: None,
                    duration_ms: None,
                },
                thread_id: thread_id.to_owned(),
                turn_id: turn_id.to_owned(),
            })]
        } else {
            vec![]
        }
    }
}

/// Lifting helper: convert an [`EventMsg`] into its wire [`ServerNotification`],
/// if any. `Error`, `Warning`, and `TurnAborted` return `None` — the dispatcher
/// adapts them (error needs correlation ids; aborted maps to a `turn/completed`
/// with interrupted status). This free function replaces the old
/// `EventMsg::into_notification` method, which could not live on the slab-agent
/// semantic type without dragging in the wire-envelope crate.
pub fn event_msg_to_notification(msg: EventMsg) -> Option<ServerNotification> {
    match msg {
        EventMsg::ThreadStarted(p) => Some(ServerNotification::ThreadStarted(p)),
        EventMsg::TurnStarted(p) => Some(ServerNotification::TurnStarted(p)),
        EventMsg::TurnCompleted(p) => Some(ServerNotification::TurnCompleted(p)),
        EventMsg::ItemStarted(p) => Some(ServerNotification::ItemStarted(p)),
        EventMsg::ItemCompleted(p) => Some(ServerNotification::ItemCompleted(p)),
        EventMsg::AgentMessageDelta(p) => Some(ServerNotification::AgentMessageDelta(p)),
        EventMsg::ReasoningTextDelta(p) => Some(ServerNotification::ReasoningTextDelta(p)),
        EventMsg::ReasoningSummaryTextDelta(p) => {
            Some(ServerNotification::ReasoningSummaryTextDelta(p))
        }
        EventMsg::CommandExecutionOutputDelta(p) => {
            Some(ServerNotification::CommandExecutionOutputDelta(p))
        }
        EventMsg::FileChangeOutputDelta(p) => Some(ServerNotification::FileChangeOutputDelta(p)),
        EventMsg::CommandExecutionRequestApproval(p) => {
            Some(ServerNotification::CommandExecutionRequestApproval(p))
        }
        EventMsg::FileChangeRequestApproval(p) => {
            Some(ServerNotification::FileChangeRequestApproval(p))
        }
        EventMsg::Error(_) | EventMsg::Warning(_) | EventMsg::TurnAborted(_) => None,
        // `EventMsg` is `#[non_exhaustive]`; future variants added in slab-agent
        // have no wire-notification mapping yet — drop them rather than failing
        // to compile the projection when slab-agent grows a new event.
        _ => None,
    }
}

/// The full set of persistence scopes a client may offer when approving. Uses
/// `slab_exec_policy::ApprovalScope` (re-exported by slab-agent) — the approval
/// param field type, wire-byte-identical to the old `slab-proto` mirror.
fn default_allowed_scopes() -> Vec<slab_agent::ApprovalScope> {
    use slab_agent::ApprovalScope;
    vec![
        ApprovalScope::RunOnce,
        ApprovalScope::AlwaysInWorkspace,
        ApprovalScope::Always,
        ApprovalScope::Deny,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_agent::AgentResponseRef;
    use slab_agent::port::{ThreadStatus, TurnEvent};
    use slab_proto::harness::item::ReasoningText;

    fn env(turn_index: Option<u32>, event: AgentEventKind) -> AgentEventEnvelope {
        AgentEventEnvelope { id: 0, event: TurnEvent::Response { turn_index, event } }
    }

    fn response_ref() -> AgentResponseRef {
        AgentResponseRef { id: "r".to_owned(), status: ThreadStatus::Pending }
    }

    #[test]
    fn text_turn_emits_started_then_deltas_then_completed() {
        let mut proj = HarnessProjection::new();
        let mut events = Vec::new();
        events.append(&mut proj.project(
            "t1",
            &env(Some(0), AgentEventKind::ResponseInProgress { response: response_ref() }),
        ));
        for d in ["hel", "lo"] {
            events.append(&mut proj.project(
                "t1",
                &env(
                    Some(0),
                    AgentEventKind::ResponseOutputTextDelta {
                        item_id: "i1".into(),
                        output_index: 0,
                        content_index: 0,
                        delta: d.into(),
                    },
                ),
            ));
        }
        events.append(&mut proj.project(
            "t1",
            &env(
                Some(0),
                AgentEventKind::ResponseOutputTextDone {
                    item_id: "i1".into(),
                    output_index: 0,
                    content_index: 0,
                    text: "hello".into(),
                    artifact_refs: vec![],
                    reason: None,
                    phase: None,
                },
            ),
        ));
        events.append(&mut proj.project(
            "t1",
            &env(Some(0), AgentEventKind::ResponseCompleted { response: response_ref() }),
        ));

        let kinds: Vec<String> = events.iter().map(|e| e.to_string()).collect();
        // turn_started, item_started, 2x agent_message_delta, item_completed, turn_completed
        assert_eq!(
            kinds,
            vec![
                "turn_started".to_string(),
                "item_started".to_string(),
                "agent_message_delta".to_string(),
                "agent_message_delta".to_string(),
                "item_completed".to_string(),
                "turn_completed".to_string(),
            ]
        );

        // The first delta's params carry the right correlation ids.
        let delta = events
            .iter()
            .find_map(|e| match e {
                EventMsg::AgentMessageDelta(p) => Some(p),
                _ => None,
            })
            .unwrap();
        assert_eq!(delta.thread_id, "t1");
        assert_eq!(delta.turn_id, "0");
        assert_eq!(delta.item_id, "i1");
    }

    #[test]
    fn failed_response_projects_to_error_event() {
        let mut proj = HarnessProjection::new();
        let events = proj.project(
            "t1",
            &env(
                Some(0),
                AgentEventKind::ResponseFailed {
                    response: response_ref(),
                    error: "boom".into(),
                    error_code: Some("insufficient_quota".into()),
                    error_type: None,
                },
            ),
        );
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], EventMsg::Error(_)));
    }

    #[test]
    fn command_execution_completed_echoes_command_and_cwd_from_started() {
        let mut proj = HarnessProjection::new();
        let started = proj.project(
            "t1",
            &env(
                Some(0),
                AgentEventKind::ResponseLocalShellCallDone {
                    item_id: "c1".into(),
                    call_id: "call-1".into(),
                    output_index: 0,
                    command: vec!["ls".into(), "-la".into()],
                    env: Default::default(),
                    working_directory: Some("/workspace".into()),
                },
            ),
        );
        let item = started
            .iter()
            .find_map(|e| match e {
                EventMsg::ItemStarted(p) => Some(&p.item),
                _ => None,
            })
            .unwrap();
        match item {
            TurnItem::CommandExecution { command, cwd, status, .. } => {
                assert_eq!(command, "ls -la");
                assert_eq!(cwd, "/workspace");
                assert_eq!(status, "running");
            }
            other => panic!("unexpected item: {other:?}"),
        }

        let completed = proj.project(
            "t1",
            &env(
                Some(0),
                AgentEventKind::ResponseShellCallOutputContentDone {
                    item_id: "c1".into(),
                    call_id: "call-1".into(),
                    output_index: 0,
                    outputs: vec![serde_json::json!({"type": "stdout", "text": "ok"})],
                },
            ),
        );
        let item = completed
            .iter()
            .find_map(|e| match e {
                EventMsg::ItemCompleted(p) => Some(&p.item),
                _ => None,
            })
            .unwrap();
        match item {
            TurnItem::CommandExecution { command, cwd, aggregated_output, .. } => {
                assert_eq!(command, "ls -la", "command must survive to completion");
                assert_eq!(cwd, "/workspace", "cwd must survive to completion");
                assert!(aggregated_output.is_some());
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn reasoning_completed_uses_distinct_summary_and_falls_back_to_text() {
        let mut proj = HarnessProjection::new();

        // With an explicit summary, `summary` and `content` must not collapse
        // to the same string.
        let events = proj.project(
            "t1",
            &env(
                Some(0),
                AgentEventKind::ResponseReasoningTextDone {
                    item_id: "r1".into(),
                    output_index: 0,
                    content_index: 0,
                    text: "full chain-of-thought trace".into(),
                    encrypted_content: None,
                    summary: Some("short recap".into()),
                },
            ),
        );
        let item = events
            .iter()
            .find_map(|e| match e {
                EventMsg::ItemCompleted(p) => Some(&p.item),
                _ => None,
            })
            .unwrap();
        match item {
            TurnItem::Reasoning { summary, content, .. } => {
                assert_eq!(summary, &ReasoningText::one("short recap"));
                assert_eq!(content, &ReasoningText::one("full chain-of-thought trace"));
            }
            other => panic!("unexpected item: {other:?}"),
        }

        // Without a summary, fall back to the full text rather than losing it.
        let events = proj.project(
            "t1",
            &env(
                Some(0),
                AgentEventKind::ResponseReasoningTextDone {
                    item_id: "r2".into(),
                    output_index: 0,
                    content_index: 0,
                    text: "only text available".into(),
                    encrypted_content: None,
                    summary: None,
                },
            ),
        );
        let item = events
            .iter()
            .find_map(|e| match e {
                EventMsg::ItemCompleted(p) => Some(&p.item),
                _ => None,
            })
            .unwrap();
        match item {
            TurnItem::Reasoning { summary, content, .. } => {
                assert_eq!(summary, &ReasoningText::one("only text available"));
                assert_eq!(content, &ReasoningText::one("only text available"));
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn delta_event_lifts_to_notification() {
        let event = EventMsg::AgentMessageDelta(AgentMessageDeltaParams {
            thread_id: "t".to_owned(),
            turn_id: "tu".to_owned(),
            item_id: "i".to_owned(),
            delta: "x".to_owned(),
        });
        let n = event_msg_to_notification(event).unwrap();
        assert_eq!(n.method(), "item/agentMessage/delta");
    }

    #[test]
    fn error_event_does_not_lift_to_notification() {
        let event = EventMsg::Error(ErrorEvent::new("boom"));
        assert!(event_msg_to_notification(event).is_none());
    }
}
