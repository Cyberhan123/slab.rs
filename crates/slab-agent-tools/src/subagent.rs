use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slab_agent::{
    AgentConfig, AgentControl, AgentError, ModelPolicy, ToolContext, ToolOutput, TypedTool,
};
use slab_types::{ConversationMessage, ConversationMessageContent};

use crate::background::{
    BackgroundTaskRegistry, BackgroundTaskSnapshot, BackgroundTaskStatus, DetachedKill,
    DetachedOnTerminal, DetachedTask, DetachedTaskOutcome, DetachedWait,
};

/// Default turn budget for a delegation when the caller does not set
/// `max_turns`. 24 (3× the historical 8): subagent tasks routinely need more
/// than a handful of tool rounds, and an exhausted child now delivers its
/// confirmed-so-far findings instead of dropping all work — but a wider
/// default still buys the child room to finish properly. Callers are told
/// about the default via a note in the result envelope.
const DEFAULT_SUBAGENT_TURNS: u32 = 24;

/// Hard upper bound for a child's `max_turns`: a delegation is a focused
/// task, not a competing main agent — 100 turns already buys an unbounded
/// run from the parent's perspective, and an unclamped model-supplied value
/// (the schema range is advisory only) would burn tokens long after any
/// interest in the result is gone.
const MAX_SUBAGENT_TURNS_CAP: u32 = 100;

/// Cap for inlining a child result alongside its artifact. Results at or under
/// this bound flow into the parent notification / registry summary verbatim;
/// larger results live in the artifact alone. The notification renderer in
/// slab-app-core truncates at the same bound, so the two crates share this
/// single constant (app-core imports it — the dependency only runs that way).
pub const MAX_NOTIFICATION_RESULT_CHARS: usize = 8_000;

/// A subagent was spawned (both background and inline delegations report
/// this — the host uses it to attach rollout persistence to the child).
pub struct SubagentSpawnedEvent {
    pub parent_thread_id: String,
    pub child_thread_id: String,
    /// Whether the parent auto-resume is suppressed for this delegation.
    pub no_resume: bool,
}

/// A subagent reached a natural terminal state (explicit stops are NOT
/// reported — the stopper already knows).
pub struct SubagentFinishedEvent {
    pub parent_thread_id: String,
    pub child_thread_id: String,
    pub task_id: String,
    /// One-line task summary (same text the registry status listing shows).
    pub task_summary: String,
    pub status: BackgroundTaskStatus,
    pub completion_text: Option<String>,
    pub artifact_refs: Vec<String>,
    /// Whether the parent auto-resume was suppressed for this delegation
    /// (mirrors the spawned event; the host skips the follow-up delivery).
    pub no_resume: bool,
}

/// Host seam for subagent lifecycle events. Sync on purpose — called from
/// the tool execution and the detached watcher task; hosts spawn their own
/// async work.
pub trait SubagentTaskSink: Send + Sync {
    fn on_subagent_spawned(&self, event: SubagentSpawnedEvent);
    fn on_subagent_finished(&self, event: SubagentFinishedEvent);
}

/// No-op sink for hosts/tests that do not bridge subagent events.
#[derive(Default)]
pub struct NoopSubagentTaskSink;

impl SubagentTaskSink for NoopSubagentTaskSink {
    fn on_subagent_spawned(&self, _event: SubagentSpawnedEvent) {}
    fn on_subagent_finished(&self, _event: SubagentFinishedEvent) {}
}

/// Terminal data shared between the watcher future and the inline wait.
#[derive(Debug, Clone)]
struct SubagentTerminalData {
    child_thread_id: String,
    status: slab_types::AgentThreadStatus,
    completion_text: Option<String>,
    artifact_refs: Vec<String>,
}

/// Map an agent-thread terminal status onto the registry task status.
///
/// A max-turns interruption whose completion text carries the partial-findings
/// prefix is a RESULT-carrying exit: it maps to `Completed` so the parent
/// notification fires and the findings flow through the normal delivery path.
/// Every genuine user stop stays `Stopped` — the registry pre-marks stopped
/// slots before interrupting (Stopped-wins), and the only direct-interrupt
/// bypass (workspace migration) writes a plain "interrupted" completion that
/// keeps mapping to `Stopped` and stays suppressed.
fn map_registry_status(
    status: slab_types::AgentThreadStatus,
    completion_text: Option<&str>,
) -> BackgroundTaskStatus {
    use slab_types::AgentThreadStatus as ThreadStatus;
    match status {
        ThreadStatus::Completed => BackgroundTaskStatus::Completed,
        ThreadStatus::Errored => BackgroundTaskStatus::Failed,
        ThreadStatus::Interrupted
            if completion_text
                .is_some_and(|text| text.starts_with(slab_agent::MAX_TURNS_PARTIAL_PREFIX)) =>
        {
            BackgroundTaskStatus::Completed
        }
        ThreadStatus::Interrupted | ThreadStatus::Shutdown => BackgroundTaskStatus::Stopped,
        // Non-terminal statuses cannot reach the watcher; treat defensively.
        _ => BackgroundTaskStatus::Failed,
    }
}

pub struct DelegateSubagentTool {
    control: Arc<AgentControl>,
    registry: Arc<BackgroundTaskRegistry>,
    sink: Arc<dyn SubagentTaskSink>,
}

impl DelegateSubagentTool {
    pub fn new(
        control: Arc<AgentControl>,
        registry: Arc<BackgroundTaskRegistry>,
        sink: Arc<dyn SubagentTaskSink>,
    ) -> Self {
        Self { control, registry, sink }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DelegateSubagentArgs {
    /// The focused task for the child agent.
    task: String,
    /// Optional built-in agent type (e.g. "plan"). Resolves a tool constraint and system prompt from the agent registry; the call fails if the type is unknown.
    agent_type: Option<String>,
    /// Optional model override for the child agent.
    model: Option<String>,
    /// Optional child-agent system prompt.
    system_prompt: Option<String>,
    /// Optional tool allow-list for the child agent.
    allowed_tools: Option<Vec<String>>,
    /// Optional child-agent turn limit.
    #[schemars(range(min = 1))]
    max_turns: Option<u32>,
    /// Optional requested output format for the child result.
    output_format: Option<String>,
    /// Optional workspace-relative path that bounds the delegated work.
    workspace_scope: Option<String>,
    /// Run the delegation in the background (default `true`): the call
    /// returns immediately with a task id and the result is delivered to
    /// this agent as a follow-up message when the subagent finishes.
    #[serde(default)]
    #[schemars(default = "default_background")]
    background: Option<bool>,
    /// Suppress the parent auto-resume when this delegation finishes (the
    /// result stays queryable via subagent_status / the artifact). Orthogonal
    /// to `background`.
    #[serde(default)]
    #[schemars(default = "default_no_resume")]
    no_resume: Option<bool>,
}

/// Schema-only default for `background`: absence means `true` at runtime
/// (`unwrap_or(true)`), so the schema advertises that default without
/// changing deserialization.
fn default_background() -> Option<bool> {
    Some(true)
}

/// Schema-only default for `no_resume`: absence means `false` at runtime
/// (`unwrap_or(false)`).
fn default_no_resume() -> Option<bool> {
    Some(false)
}

#[async_trait]
impl TypedTool for DelegateSubagentTool {
    type Input = DelegateSubagentArgs;
    fn name(&self) -> &str {
        "delegate_subagent"
    }

    fn description(&self) -> &str {
        "Delegate a focused task to an isolated child agent. By default the \
         call returns IMMEDIATELY with a task_id and does not block this \
         agent: the result is delivered as a follow-up message when the \
         subagent finishes. Track it with subagent_status, steer it with \
         subagent_message, cancel it with subagent_stop. Pass \
         background=false only when the next step strictly needs the inline \
         result before this agent can continue."
    }

    /// Background delegation is a quick spawn + registry bookkeeping — safe
    /// to run alongside other calls in the same batch. Inline delegation
    /// parks this agent for the whole child run, so it stays exclusive.
    fn is_concurrency_safe(&self, arguments: &Value) -> bool {
        arguments.get("background").and_then(Value::as_bool).unwrap_or(true)
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        args: DelegateSubagentArgs,
    ) -> Result<ToolOutput, AgentError> {
        if args.task.trim().is_empty() {
            return Err(AgentError::ToolExecution("subagent task must not be blank".to_owned()));
        }
        let output_format =
            args.output_format.as_deref().map(str::trim).filter(|value| !value.is_empty());
        let workspace_scope = resolve_workspace_scope(
            ctx.workspace.as_ref().map(|workspace| workspace.root.as_path()),
            args.workspace_scope.as_deref(),
        )?;
        // A scoped agent delegating an explicitly OUT-OF-SCOPE child scope is
        // a hard error (a scoped parent must not widen the boundary). A
        // scoped parent delegating WITHOUT a child scope stays allowed — the
        // child then runs unscoped (declared gap; nesting is bounded by
        // max_depth).
        if let (Some(parent_scope), Some(child_scope)) =
            (ctx.workspace_scope.as_ref(), workspace_scope.as_ref())
            && !child_scope.root.starts_with(&parent_scope.root)
        {
            return Err(AgentError::ToolExecution(format!(
                "workspace_scope must stay inside this agent's own delegated scope '{}'",
                parent_scope.relative
            )));
        }

        let parent = self
            .control
            .thread_snapshot(&ctx.thread_id)
            .await?
            .ok_or_else(|| AgentError::ThreadNotFound(ctx.thread_id.clone()))?;
        let mut child_config =
            serde_json::from_str::<AgentConfig>(&parent.config_json).map_err(|error| {
                AgentError::ToolExecution(format!("invalid parent agent config: {error}"))
            })?;
        // Slice 4: resolve a built-in agent_type (if any) BEFORE applying caller
        // overrides so an explicit caller value still wins. A named type that is
        // absent from the registry is a hard error — the model asked for an agent
        // that does not exist.
        let definition =
            match args.agent_type.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
                Some(agent_type) => {
                    let registry = self.control.agent_registry();
                    Some(registry.get(agent_type).ok_or_else(|| {
                        AgentError::ToolExecution(format!("unknown agent_type: {agent_type}"))
                    })?)
                }
                None => None,
            };
        if let Some(definition) = &definition {
            child_config.agent_type = Some(definition.agent_type.clone());
        }
        if let Some(model) = args.model.filter(|value| !value.trim().is_empty()) {
            child_config.model = model;
        } else if let Some(definition) = &definition
            && let ModelPolicy::Fixed(model) = &definition.model
        {
            child_config.model = model.clone();
        }
        child_config.system_prompt = Some(match args.system_prompt {
            Some(prompt) => prompt,
            None => definition
                .as_ref()
                .map(|definition| definition.system_prompt.clone())
                .unwrap_or_else(default_system_prompt),
        });
        if let Some(allowed_tools) = args.allowed_tools {
            let requested: Vec<String> =
                allowed_tools.into_iter().filter(|tool| !tool.trim().is_empty()).collect();
            // The caller-supplied allow-list INTERSECTS the parent's: a
            // restricted parent (e.g. a read-only consolidation agent) must not
            // be able to delegate a child wielding tools it cannot use itself.
            // An EMPTY parent list means unrestricted (`AgentConfig`
            // semantics), so the request passes through unchanged.
            if child_config.allowed_tools.is_empty() {
                child_config.allowed_tools = requested;
            } else {
                let parent_list = child_config.allowed_tools.clone();
                let filtered: Vec<String> =
                    requested.iter().filter(|tool| parent_list.contains(tool)).cloned().collect();
                if filtered.is_empty() {
                    return Err(AgentError::ToolExecution(format!(
                        "delegate_subagent: none of the requested tools {requested:?} are in the \
                         parent agent's allow-list {parent_list:?}; a child cannot receive tools \
                         the parent itself cannot use"
                    )));
                }
                child_config.allowed_tools = filtered;
            }
        }
        // Clamp the model-supplied turn budget to the cap; the clamp is
        // echoed in the tool result so the model knows the effective bound.
        // A missing budget gets the (wider) default, also echoed as a note.
        let max_turns_clamped = args.max_turns.is_some_and(|turns| turns > MAX_SUBAGENT_TURNS_CAP);
        let max_turns_defaulted = args.max_turns.is_none();
        child_config.max_turns =
            args.max_turns.unwrap_or(DEFAULT_SUBAGENT_TURNS).clamp(1, MAX_SUBAGENT_TURNS_CAP);
        child_config.transient = true;
        // The validated scope rides on the config: the kernel resolves it
        // into every ToolContext of the child thread and the file tools
        // enforce it (see resolve_scoped_agent_path).
        child_config.workspace_scope = workspace_scope.as_ref().map(|scope| scope.relative.clone());

        let messages = vec![ConversationMessage {
            role: "user".to_owned(),
            content: ConversationMessageContent::Text(render_child_task(
                args.task.trim(),
                output_format,
                workspace_scope.as_ref(),
            )),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }];
        let child_thread_id =
            self.control.spawn_child_for_parent(&ctx.thread_id, child_config, messages).await?;

        // Both modes report the spawn (rollout persistence attach) and go
        // through the registry (status visibility + cascade stop) — the
        // inline mode just additionally parks on the watcher's oneshot.
        let no_resume = args.no_resume.unwrap_or(false);
        self.sink.on_subagent_spawned(SubagentSpawnedEvent {
            parent_thread_id: ctx.thread_id.clone(),
            child_thread_id: child_thread_id.clone(),
            no_resume,
        });

        let workspace_root: Option<PathBuf> =
            ctx.workspace.as_ref().map(|workspace| workspace.root.clone());
        let task_id = self.registry.alloc_task_id();
        let (filled_tx, filled_rx) = tokio::sync::oneshot::channel::<SubagentTerminalData>();
        let terminal_data: Arc<std::sync::Mutex<Option<SubagentTerminalData>>> =
            Arc::new(std::sync::Mutex::new(None));

        // Watcher future: waits for the child's terminal snapshot, strips the
        // LLM-grade reasoning, spills the artifact, then publishes the
        // terminal outcome. Runs detached from the parent turn — an inline
        // caller that is dropped (parent interrupt) does not affect it.
        //
        // The `kill_failed` select arm is the stop-failure escape: when the
        // kill closure could not interrupt the child, the snapshot wait would
        // park forever (the child keeps running) — converge the watcher on a
        // synthetic Errored outcome instead, which maps to `Failed` below.
        let kill_failed = Arc::new(tokio::sync::Notify::new());
        let wait: DetachedWait = {
            let control = Arc::clone(&self.control);
            let child_id = child_thread_id.clone();
            let artifact_root = workspace_root.clone();
            let shared = Arc::clone(&terminal_data);
            let kill_failed = Arc::clone(&kill_failed);
            Box::pin(async move {
                let terminal = tokio::select! {
                    snapshot = control.wait_for_terminal_snapshot(&child_id) => snapshot,
                    _ = kill_failed.notified() => Err(AgentError::ToolExecution(format!(
                        "stop failed: the interrupt did not reach subagent {child_id}; \
                         the child may still be running"
                    ))),
                };
                let data = match terminal {
                    Ok(snapshot) => {
                        // Diagnostic anchor: a NON-terminal status here (e.g.
                        // `Interrupting` from the bounded persisted-snapshot
                        // wait) is defensively mapped to `Failed` by
                        // `map_registry_status` — the registry outcome and the
                        // notify decision key off this line.
                        tracing::info!(
                            child_thread_id = %snapshot.id,
                            status = ?snapshot.status,
                            "subagent watcher resolved terminal snapshot"
                        );
                        // The snapshot's completion_text is LLM-grade (reasoning
                        // embedded as `<think>` blocks for the next chat-template
                        // round); the parent conversation and the persisted
                        // artifact only want the final answer.
                        let completion_text =
                            snapshot.completion_text.as_deref().map(slab_agent::strip_think_blocks);
                        let artifact_refs = write_subagent_artifact(
                            artifact_root.as_deref(),
                            &snapshot.id,
                            &completion_text,
                        )
                        .await
                        .unwrap_or_else(|error| {
                            tracing::warn!(%error, "failed to write subagent artifact");
                            Vec::new()
                        });
                        // The artifact is the durable record, but a bounded
                        // result is ALSO inlined so the parent notification,
                        // the registry summary, and `subagent_status` all
                        // carry it without a follow-up file read. Only a
                        // runaway result is dropped to the artifact alone.
                        // Applied BEFORE `SubagentTerminalData` is built so
                        // the inline output, the registry result, and the
                        // sink event all see the same value.
                        let completion_text = completion_text.filter(|text| {
                            artifact_refs.is_empty()
                                || text.chars().count() <= MAX_NOTIFICATION_RESULT_CHARS
                        });
                        // Without an artifact there is nowhere for an oversized
                        // result to live — truncate to the inline bound with an
                        // explicit marker instead of inlining an unbounded child
                        // output into the parent context.
                        let completion_text = if artifact_refs.is_empty() {
                            completion_text.map(|text| {
                                if text.chars().count() <= MAX_NOTIFICATION_RESULT_CHARS {
                                    text
                                } else {
                                    let truncated: String =
                                        text.chars().take(MAX_NOTIFICATION_RESULT_CHARS).collect();
                                    format!(
                                        "{truncated}\n(result truncated at {} chars: no \
                                         workspace artifact is available to hold the full output)",
                                        MAX_NOTIFICATION_RESULT_CHARS
                                    )
                                }
                            })
                        } else {
                            completion_text
                        };
                        SubagentTerminalData {
                            child_thread_id: snapshot.id,
                            status: snapshot.status,
                            completion_text,
                            artifact_refs,
                        }
                    }
                    // The wait itself failed (store/registry error) — surface
                    // the error text as the task result.
                    Err(error) => SubagentTerminalData {
                        child_thread_id: child_id.clone(),
                        status: slab_types::AgentThreadStatus::Errored,
                        completion_text: Some(error.to_string()),
                        artifact_refs: Vec::new(),
                    },
                };
                let outcome = DetachedTaskOutcome::Status {
                    status: map_registry_status(data.status, data.completion_text.as_deref()),
                    result: data.completion_text.clone(),
                };
                *shared.lock().unwrap_or_else(|p| p.into_inner()) = Some(data.clone());
                // Inline waiter may be gone (background mode / dropped turn):
                // the registry is the source of truth either way.
                let _ = filled_tx.send(data);
                outcome
            })
        };

        let kill: DetachedKill = {
            let control = Arc::clone(&self.control);
            let registry = Arc::clone(&self.registry);
            let child_id = child_thread_id.clone();
            let task_id = task_id.clone();
            let kill_failed = Arc::clone(&kill_failed);
            Box::new(move || {
                tokio::spawn(async move {
                    // Grandchildren FIRST: the child-owned delegations must be
                    // cascade-stopped before the child itself is interrupted.
                    let stopped = registry.stop_subagent_tasks_for_thread(&child_id);
                    if !stopped.is_empty() {
                        tracing::debug!(child = %child_id, count = stopped.len(),
                            "cascade-stopped grandchild delegations");
                    }
                    match control.interrupt(&child_id).await {
                        Ok(()) => {}
                        // The child is already gone or terminal — the natural
                        // watcher path resolves on the persisted snapshot, so
                        // there is nothing to roll back.
                        Err(AgentError::ThreadNotFound(_))
                        | Err(AgentError::InvalidStateTransition { .. }) => {}
                        Err(error) => {
                            tracing::warn!(
                                %error,
                                "failed to interrupt subagent {child_id}; rolling the task back to Failed"
                            );
                            // The registry already flipped to Stopped inside
                            // `stop()`; a still-running child must not read as
                            // "stopped". The rollback re-emits the Failed
                            // lifecycle event, and the notify un-parks the
                            // watcher so the failure reaches the parent.
                            if let Err(rollback) = registry
                                .mark_stop_failed(&task_id, &format!("stop failed: {error}"))
                            {
                                tracing::warn!(
                                    %rollback,
                                    "failed to roll back subagent task {task_id} after a failed kill"
                                );
                            }
                            kill_failed.notify_one();
                        }
                    }
                });
            })
        };

        let sink = Arc::clone(&self.sink);
        let notify_parent = ctx.thread_id.clone();
        let notify_child = child_thread_id.clone();
        let notify_task = task_id.clone();
        let notify_summary = summarize_task_for_registry(args.task.trim());
        let shared_terminal = Arc::clone(&terminal_data);
        let on_terminal: DetachedOnTerminal = Box::new(move |snapshot: BackgroundTaskSnapshot| {
            // Explicit stops are not reported — the stopper already knows and
            // the parent asked for the cancellation.
            if snapshot.status == BackgroundTaskStatus::Stopped {
                return;
            }
            let data = shared_terminal.lock().unwrap_or_else(|p| p.into_inner()).clone();
            let (completion_text, artifact_refs) = data
                .map(|data| (data.completion_text, data.artifact_refs))
                .unwrap_or((None, Vec::new()));
            sink.on_subagent_finished(SubagentFinishedEvent {
                parent_thread_id: notify_parent,
                child_thread_id: notify_child,
                task_id: notify_task,
                task_summary: notify_summary,
                status: snapshot.status,
                completion_text,
                artifact_refs,
                no_resume,
            });
        });

        if let Err(register_error) = self.registry.register_detached(
            task_id.clone(),
            DetachedTask {
                thread_id: ctx.thread_id.clone(),
                command: summarize_task_for_registry(args.task.trim()),
                workspace_root,
                child_thread_id: Some(child_thread_id.clone()),
            },
            wait,
            kill,
            on_terminal,
        ) {
            // The child is ALREADY running (spawn happened above) and the
            // dropped registration took its kill handle with it — without this
            // interrupt the child would leak with no stop path anywhere.
            tracing::warn!(
                %register_error,
                child_thread_id = %child_thread_id,
                "subagent registration failed; requesting an interrupt for the running child"
            );
            if let Err(interrupt_error) = self.control.interrupt(&child_thread_id).await {
                tracing::warn!(
                    %interrupt_error,
                    "failed to interrupt the unregistered subagent {child_thread_id}"
                );
            }
            return Err(AgentError::ToolExecution(format!(
                "subagent started but could not be registered ({register_error}); \
                 an interrupt was requested for the child thread"
            )));
        }

        if args.background.unwrap_or(true) {
            let mut value = serde_json::json!({
                "background": true,
                "task_id": task_id,
                "child_thread_id": child_thread_id,
                "status": "running",
                "hint": "Delegated in the background. The result will arrive as a follow-up message when the subagent finishes; use subagent_status to check progress, subagent_message to steer it, or subagent_stop to cancel."
            });
            if let Some(scope) = workspace_scope.as_ref() {
                value["workspace_scope"] = serde_json::json!(scope.relative);
            }
            if max_turns_clamped {
                value["max_turns_note"] = format!(
                    "requested max_turns exceeded the cap; clamped to {MAX_SUBAGENT_TURNS_CAP}"
                )
                .into();
            } else if max_turns_defaulted {
                value["max_turns_note"] = format!(
                    "max_turns was not set; defaulted to {DEFAULT_SUBAGENT_TURNS} turns for this \
                     delegation (set max_turns explicitly if the task needs a different budget)"
                )
                .into();
            }
            return Ok(ToolOutput { content: value.to_string(), metadata: None });
        }

        // Inline mode: park until the watcher published the terminal data.
        let data = filled_rx.await.map_err(|_| {
            AgentError::ToolExecution("subagent watcher terminated without a result".to_owned())
        })?;
        let mut value = serde_json::json!({
            "child_thread_id": data.child_thread_id,
            "status": data.status,
            "completion_text": data.completion_text,
            "artifact_refs": data.artifact_refs,
        });
        if let Some(scope) = workspace_scope.as_ref() {
            value["workspace_scope"] = serde_json::json!(scope.relative);
        }
        if max_turns_clamped {
            value["max_turns_note"] = format!(
                "requested max_turns exceeded the cap; clamped to {MAX_SUBAGENT_TURNS_CAP}"
            )
            .into();
        } else if max_turns_defaulted {
            value["max_turns_note"] = format!(
                "max_turns was not set; defaulted to {DEFAULT_SUBAGENT_TURNS} turns for this \
                 delegation (set max_turns explicitly if the task needs a different budget)"
            )
            .into();
        }
        Ok(ToolOutput { content: value.to_string(), metadata: None })
    }
}

/// One-line task summary for the registry status listing (single line, ~80
/// chars — it is display metadata, not the full prompt).
pub(crate) fn summarize_task_for_registry(task: &str) -> String {
    let first_line = task.lines().next().unwrap_or_default().trim();
    let mut summary: String = first_line.chars().take(80).collect();
    if first_line.chars().count() > 80 {
        summary.push('…');
    }
    summary
}

fn default_system_prompt() -> String {
    // The running-summary instruction makes the deterministic max-turns
    // partial-findings synthesis distill-quality: the trailing narrations it
    // scrapes already state what has been confirmed.
    "You are a focused subagent. Work only on the delegated task, use the allowed tools, and return a concise result for the parent agent. Keep a running summary of your confirmed findings so far in each response; if the turn budget runs out, that summary is delivered to the parent as your result.".to_owned()
}

fn render_child_task(
    task: &str,
    output_format: Option<&str>,
    workspace_scope: Option<&WorkspaceScope>,
) -> String {
    let mut prompt =
        format!("Objective:\n{task}\n\nConstraints:\n- Work only on this delegated task.");
    if let Some(scope) = workspace_scope {
        prompt.push_str("\n- Limit workspace file operations to this workspace-relative scope: ");
        prompt.push_str(&scope.relative);
        prompt.push_str(" (enforced: file tool calls outside this scope are rejected)");
    }
    if let Some(output_format) = output_format {
        prompt.push_str("\n\nRequired output format:\n");
        prompt.push_str(output_format);
    }
    prompt
}

#[derive(Debug, Clone)]
struct WorkspaceScope {
    /// Original workspace-relative path (normalized separators), used for the
    /// child prompt, the tool result echo, and AgentConfig persistence.
    relative: String,
    /// Canonical absolute root of the scope (the scope directory may not
    /// exist yet), used for the grandchild containment check.
    root: PathBuf,
}

/// Resolve and validate the caller-supplied `workspace_scope` argument.
///
/// Semantics: the scope is ALWAYS relative to the workspace root — a
/// grandchild delegation resolves against the workspace too, never against
/// the delegating parent's scope; explicit escapes are caught separately in
/// `execute` via the parent's own scope.
fn resolve_workspace_scope(
    workspace_root: Option<&Path>,
    workspace_scope: Option<&str>,
) -> Result<Option<WorkspaceScope>, AgentError> {
    let Some(scope) = workspace_scope.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let Some(workspace_root) = workspace_root else {
        return Err(AgentError::ToolExecution(
            "workspace_scope requires a workspace context".to_owned(),
        ));
    };
    let scope_path = Path::new(scope);
    if scope_path.components().any(|component| {
        matches!(
            component,
            std::path::Component::Prefix(_)
                | std::path::Component::RootDir
                | std::path::Component::ParentDir
        )
    }) {
        return Err(AgentError::ToolExecution(
            "workspace_scope must stay inside the workspace".to_owned(),
        ));
    }
    let root = normalize_path(workspace_root);
    let resolved = normalize_path(root.join(scope_path));
    if !resolved.starts_with(&root) {
        return Err(AgentError::ToolExecution(
            "workspace_scope must stay inside the workspace".to_owned(),
        ));
    }
    // Canonical containment: the lexical check above is blind to symlinks —
    // an existing segment of the scope path pointing outside the workspace
    // would pass it. Resolve through the existing ancestors and re-check.
    let canonical_root = slab_utils::fs::existing_ancestor(workspace_root).map_err(|error| {
        AgentError::ToolExecution(format!("workspace_scope could not be resolved: {error}"))
    })?;
    let canonical_scope = slab_utils::fs::canonicalize_with_existing_ancestor(
        &workspace_root.join(normalize_relative_scope(scope_path)),
    )
    .map_err(|error| {
        AgentError::ToolExecution(format!("workspace_scope could not be resolved: {error}"))
    })?;
    if !canonical_scope.starts_with(&canonical_root) {
        return Err(AgentError::ToolExecution(
            "workspace_scope must stay inside the workspace".to_owned(),
        ));
    }
    Ok(Some(WorkspaceScope {
        relative: normalize_relative_scope(scope_path),
        root: canonical_scope,
    }))
}

fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.as_ref().components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn normalize_relative_scope(path: &Path) -> String {
    let normalized = normalize_path(path);
    normalized.to_string_lossy().replace('\\', "/")
}

async fn write_subagent_artifact(
    workspace_root: Option<&Path>,
    child_thread_id: &str,
    completion_text: &Option<String>,
) -> Result<Vec<String>, AgentError> {
    let Some(workspace_root) = workspace_root else {
        return Ok(Vec::new());
    };
    let content = serde_json::json!({
        "child_thread_id": child_thread_id,
        "completion_text": completion_text,
    });
    let bytes = serde_json::to_vec_pretty(&content)
        .map_err(|error| AgentError::ToolExecution(error.to_string()))?;
    // Thin wrapper over the shared spill helper (same result.json layout as
    // before the generalization).
    match crate::artifact::write_tool_artifact(
        Some(workspace_root),
        child_thread_id,
        "result.json",
        &bytes,
    )
    .await
    {
        Some(artifact_ref) => Ok(vec![artifact_ref]),
        None => Err(AgentError::ToolExecution("failed to write subagent artifact".to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use slab_agent::ToolHandler;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use slab_agent::port::{
        AgentNotifyPort, AgentStorePort, ApprovalDecision, ApprovalPort, LlmPort, LlmResponse,
        ParsedToolCall, ThreadMessageRecord, ThreadSnapshot, ThreadStatus, ToolSpec,
    };
    use slab_agent::{
        AgentControlLimits, AgentDefinition, AgentRegistry, ToolConstraint, ToolContext,
        ToolRouter, WorkspaceRef,
    };
    use slab_agent_tracing::AgentTraceContext;
    use slab_types::ConversationMessage;

    use super::*;

    fn delegate_tool(control: Arc<AgentControl>) -> DelegateSubagentTool {
        DelegateSubagentTool::new(
            control,
            Arc::new(BackgroundTaskRegistry::default()),
            Arc::new(NoopSubagentTaskSink),
        )
    }

    /// Sink that records subagent lifecycle events (spawned/finished).
    #[derive(Default)]
    struct RecordingSink {
        spawned: Mutex<Vec<SubagentSpawnedEvent>>,
        finished: Mutex<Vec<SubagentFinishedEvent>>,
    }

    impl SubagentTaskSink for RecordingSink {
        fn on_subagent_spawned(&self, event: SubagentSpawnedEvent) {
            self.spawned.lock().unwrap().push(event);
        }
        fn on_subagent_finished(&self, event: SubagentFinishedEvent) {
            self.finished.lock().unwrap().push(event);
        }
    }

    async fn wait_until(mut check: impl FnMut() -> bool) {
        for _ in 0..400 {
            if check() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        panic!("condition not met within deadline");
    }

    struct FinalLlm;

    #[async_trait]
    impl LlmPort for FinalLlm {
        async fn chat_completion(
            &self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _config: &AgentConfig,
            _trace_context: &AgentTraceContext,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                content: Some("child result".to_owned()),
                content_already_streamed: false,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".to_owned()),
                usage: None,
            })
        }
    }

    #[derive(Default)]
    struct MemoryStore {
        threads: Mutex<HashMap<String, ThreadSnapshot>>,
        // The slab-agent `insert_thread_message` trait method is gone;
        // retained for direct-push seeding in tests. Unread — tests verify
        // emission via `RecordingNotify`.
        #[allow(dead_code)]
        messages: Mutex<Vec<ThreadMessageRecord>>,
    }

    impl MemoryStore {
        fn insert_parent(&self, max_depth: u32) {
            self.insert_parent_with_config(AgentConfig {
                model: "mock".into(),
                max_depth,
                ..AgentConfig::default()
            });
        }

        /// A RESTRICTED parent: an explicit `allowed_tools` list (empty means
        /// unrestricted — `AgentConfig` semantics).
        fn insert_restricted_parent(&self, allowed_tools: &[&str]) {
            self.insert_parent_with_config(AgentConfig {
                model: "mock".into(),
                max_depth: 4,
                allowed_tools: allowed_tools.iter().map(|tool| tool.to_string()).collect(),
                ..AgentConfig::default()
            });
        }

        fn insert_parent_with_config(&self, config: AgentConfig) {
            let now = "2026-01-01T00:00:00Z".to_owned();
            self.threads.lock().unwrap().insert(
                "parent".to_owned(),
                ThreadSnapshot {
                    id: "parent".to_owned(),
                    session_id: "session".to_owned(),
                    parent_id: None,
                    depth: 0,
                    status: ThreadStatus::Completed,
                    role_name: None,
                    config_json: serde_json::to_string(&config).expect("config"),
                    completion_text: Some("parent".to_owned()),
                    created_at: now.clone(),
                    updated_at: now,
                    archived_at: None,
                },
            );
        }
    }

    #[async_trait]
    impl AgentStorePort for MemoryStore {
        async fn upsert_thread(&self, snapshot: &ThreadSnapshot) -> Result<(), AgentError> {
            self.threads.lock().unwrap().insert(snapshot.id.clone(), snapshot.clone());
            Ok(())
        }

        async fn get_thread(&self, id: &str) -> Result<Option<ThreadSnapshot>, AgentError> {
            Ok(self.threads.lock().unwrap().get(id).cloned())
        }

        async fn list_session_threads(
            &self,
            _session_id: &str,
        ) -> Result<Vec<ThreadSnapshot>, AgentError> {
            Ok(Vec::new())
        }

        async fn update_thread_status(
            &self,
            id: &str,
            status: ThreadStatus,
            completion_text: Option<&str>,
        ) -> Result<(), AgentError> {
            let mut threads = self.threads.lock().unwrap();
            let snapshot =
                threads.get_mut(id).ok_or_else(|| AgentError::ThreadNotFound(id.to_owned()))?;
            snapshot.status = status;
            snapshot.completion_text = completion_text.map(str::to_owned);
            Ok(())
        }
    }

    struct NoopNotify;

    #[async_trait]
    impl AgentNotifyPort for NoopNotify {
        async fn on_status_change(&self, _thread_id: &str, _status: ThreadStatus) {}
    }

    /// A notify port that records emitted `EventMsg`s so tests can
    /// verify emission (slab-agent no longer writes conversation data to the
    /// store — it emits `MessageAppended` / `TurnStateChanged` events).
    #[derive(Default)]
    struct RecordingNotify {
        events: std::sync::Mutex<Vec<slab_agent::protocol::EventMsg>>,
    }

    #[async_trait]
    impl AgentNotifyPort for RecordingNotify {
        async fn on_status_change(&self, _thread_id: &str, _status: ThreadStatus) {}

        async fn on_event_msg(&self, _thread_id: &str, msg: &slab_agent::protocol::EventMsg) {
            self.events.lock().unwrap().push(msg.clone());
        }
    }

    #[async_trait]
    impl ApprovalPort for RecordingNotify {
        async fn request_approval(
            &self,
            _thread_id: &str,
            _call_id: &str,
            _tool_name: &str,
            _descriptor: &slab_agent::OperationDescriptor,
            _risk: Option<slab_agent::ToolRiskAssessment>,
        ) -> ApprovalDecision {
            ApprovalDecision::Approved(slab_agent::ApprovalScope::RunOnce)
        }
    }

    impl RecordingNotify {
        /// Emitted `MessageAppended` conversation messages for a thread.
        fn emitted_messages(&self, thread_id: &str) -> Vec<slab_types::ConversationMessage> {
            use slab_agent::protocol::EventMsg;
            self.events
                .lock()
                .unwrap()
                .iter()
                .filter_map(|event| match event {
                    EventMsg::MessageAppended(p) if p.thread_id == thread_id => {
                        Some(p.message.clone())
                    }
                    _ => None,
                })
                .collect()
        }
    }

    #[async_trait]
    impl ApprovalPort for NoopNotify {
        async fn request_approval(
            &self,
            _thread_id: &str,
            _call_id: &str,
            _tool_name: &str,
            _descriptor: &slab_agent::OperationDescriptor,
            _risk: Option<slab_agent::ToolRiskAssessment>,
        ) -> ApprovalDecision {
            ApprovalDecision::Approved(slab_agent::ApprovalScope::RunOnce)
        }
    }

    /// LLM double whose final answer embeds a `<think>` reasoning block
    /// (LLM-grade text, as the chat-template round actually produces).
    struct ThinkingLlm;

    #[async_trait]
    impl LlmPort for ThinkingLlm {
        async fn chat_completion(
            &self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _config: &AgentConfig,
            _trace_context: &AgentTraceContext,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                content: Some(
                    "<think status=\"done\">plan the summary privately</think>child result"
                        .to_owned(),
                ),
                content_already_streamed: false,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".to_owned()),
                usage: None,
            })
        }
    }

    #[tokio::test]
    async fn delegate_subagent_strips_think_blocks_from_completion() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(ThinkingLlm),
            store.clone(),
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);

        // No workspace: the stripped completion flows into the parent tool
        // output verbatim.
        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "summarize", "max_turns": 1, "background": false }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        assert_eq!(value["status"], "completed");
        assert_eq!(
            value["completion_text"], "child result",
            "think blocks must not leak into the parent conversation"
        );

        // Workspace variant: the persisted artifact carries the stripped text.
        let temp_dir = std::env::temp_dir()
            .join(format!("slab-agent-tools-subagent-think-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        tokio::fs::create_dir_all(&temp_dir).await.expect("temp workspace");
        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent")
                .workspace(WorkspaceRef { root: temp_dir.clone(), session_id: None })
                .build(),
            &serde_json::json!({ "task": "summarize", "max_turns": 1, "background": false }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let artifact_ref = value["artifact_refs"][0].as_str().expect("artifact ref");
        let artifact =
            tokio::fs::read_to_string(temp_dir.join(artifact_ref)).await.expect("artifact content");
        let artifact: serde_json::Value = serde_json::from_str(&artifact).expect("artifact json");
        assert_eq!(artifact["completion_text"], "child result");

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn delegate_subagent_spawns_transient_child_and_returns_result() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store.clone(),
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({
                "task": "summarize",
                "allowed_tools": ["read_file"],
                "max_turns": 1,
                "background": false
            }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let child_id = value["child_thread_id"].as_str().expect("child id");
        assert_eq!(value["status"], "completed");
        assert_eq!(value["completion_text"], "child result");

        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        assert_eq!(child.parent_id.as_deref(), Some("parent"));
        assert_eq!(child.depth, 1);
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        assert!(child_config.transient);
        assert_eq!(child_config.allowed_tools, vec!["read_file"]);
        assert_eq!(child_config.max_turns, 1);
    }

    /// The caller-supplied allow-list INTERSECTS a restricted parent's list —
    /// a read-only parent cannot delegate a child with `shell`/`write_file`.
    #[tokio::test]
    async fn delegate_subagent_intersects_allowed_tools_with_parent_restriction() {
        let store = Arc::new(MemoryStore::default());
        store.insert_restricted_parent(&["read_file", "grep"]);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store.clone(),
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({
                "task": "summarize",
                "allowed_tools": ["shell", "read_file", "write_file"],
                "max_turns": 1,
                "background": false
            }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let child_id = value["child_thread_id"].as_str().expect("child id");
        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        // Requested order preserved; only the parent-permitted entry survives.
        assert_eq!(child_config.allowed_tools, vec!["read_file"]);
    }

    /// A request whose every entry is outside the parent's allow-list is a
    /// hard error — an empty intersection would otherwise mean "no tools" and
    /// silently waste a child run.
    #[tokio::test]
    async fn delegate_subagent_rejects_fully_blocked_tool_request() {
        let store = Arc::new(MemoryStore::default());
        store.insert_restricted_parent(&["read_file"]);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store.clone(),
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);

        let error = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({
                "task": "summarize",
                "allowed_tools": ["shell", "write_file"],
                "max_turns": 1,
                "background": false
            }),
        )
        .await
        .expect_err("fully-blocked request must error");
        match error {
            AgentError::ToolExecution(message) => {
                assert!(
                    message.contains("allow-list"),
                    "error explains the intersection: {message}"
                );
            }
            other => panic!("expected ToolExecution, got: {other:?}"),
        }
    }

    /// Registration failure no longer leaks the already-spawned child: the
    /// capacity gate rejects the 9th concurrent task, and the tool requests an
    /// interrupt for the running child before failing the call loudly.
    #[tokio::test]
    async fn registration_failure_interrupts_the_spawned_child() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store.clone(),
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 16, max_depth: 4 },
            Vec::new(),
        ));

        // Pre-fill the registry to its running-task capacity so the delegation
        // below hits the gate AFTER its child has already spawned.
        let registry = Arc::new(BackgroundTaskRegistry::default());
        for index in 0..8 {
            let task_id = registry.alloc_task_id();
            registry
                .register_detached(
                    task_id,
                    DetachedTask {
                        thread_id: "parent".to_owned(),
                        command: format!("filler {index}"),
                        workspace_root: None,
                        child_thread_id: Some(format!("filler-child-{index}")),
                    },
                    Box::pin(std::future::pending()),
                    Box::new(|| {}),
                    Box::new(|_| {}),
                )
                .expect("fill the capacity gate");
        }

        let tool = DelegateSubagentTool::new(control, registry, Arc::new(NoopSubagentTaskSink));
        let error = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({
                "task": "summarize",
                "max_turns": 1,
                "background": false
            }),
        )
        .await
        .expect_err("registration must fail at the capacity gate");
        match error {
            AgentError::ToolExecution(message) => {
                assert!(
                    message.contains("could not be registered"),
                    "the error explains the spawn-then-register failure: {message}"
                );
                assert!(
                    message.contains("background task limit reached"),
                    "the underlying gate reason is carried through: {message}"
                );
            }
            other => panic!("expected ToolExecution, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn delegate_subagent_writes_workspace_artifact_and_returns_reference() {
        let temp_dir =
            std::env::temp_dir().join(format!("slab-agent-tools-subagent-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        tokio::fs::create_dir_all(&temp_dir).await.expect("temp workspace");

        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(RecordingNotify::default());
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store.clone(),
            notify.clone(),
            notify.clone(),
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent")
                .workspace(WorkspaceRef { root: temp_dir.clone(), session_id: None })
                .build(),
            &serde_json::json!({
                "task": "summarize",
                "workspace_scope": "src",
                "output_format": "Return JSON with a summary field.",
                "max_turns": 1,
                "background": false
            }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let artifact_ref = value["artifact_refs"][0].as_str().expect("artifact ref");

        // A bounded result is inlined ALONGSIDE the artifact — the parent
        // (and every summary surface) sees the text without a file read.
        assert_eq!(value["completion_text"], "child result");
        assert!(artifact_ref.starts_with(".slab/artifacts/"));
        assert!(artifact_ref.ends_with("/result.json"));

        let artifact_path = temp_dir.join(artifact_ref);
        let artifact = tokio::fs::read_to_string(&artifact_path).await.expect("artifact content");
        let artifact: serde_json::Value = serde_json::from_str(&artifact).expect("artifact json");
        assert_eq!(artifact["completion_text"], "child result");

        let child_id = value["child_thread_id"].as_str().expect("child id");
        // slab-agent emits `MessageAppended` (no store writes); read
        // the emitted child-prompt message from the recording notify.
        let child_prompt = notify
            .emitted_messages(child_id)
            .iter()
            .find(|message| message.role == "user")
            .expect("emitted child prompt")
            .rendered_text();
        assert!(child_prompt.contains("Objective:\nsummarize"));
        assert!(child_prompt.contains("workspace-relative scope: src"));
        // The prompt now also says the scope is enforced, and the scope rides
        // on the persisted child config (consumed by the kernel into every
        // child ToolContext).
        assert!(child_prompt.contains("(enforced:"), "{child_prompt}");
        assert_eq!(value["workspace_scope"], "src");
        let child_id = value["child_thread_id"].as_str().expect("child id");
        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        assert_eq!(child_config.workspace_scope.as_deref(), Some("src"));

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    /// LLM double whose final answer exceeds the notification inline bound.
    struct RunawayLlm;

    #[async_trait]
    impl LlmPort for RunawayLlm {
        async fn chat_completion(
            &self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _config: &AgentConfig,
            _trace_context: &AgentTraceContext,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                content: Some("x".repeat(MAX_NOTIFICATION_RESULT_CHARS + 100)),
                content_already_streamed: false,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".to_owned()),
                usage: None,
            })
        }
    }

    #[tokio::test]
    async fn delegate_subagent_drops_runaway_result_to_artifact_alone() {
        let temp_dir = std::env::temp_dir()
            .join(format!("slab-agent-tools-subagent-runaway-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        tokio::fs::create_dir_all(&temp_dir).await.expect("temp workspace");

        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(RunawayLlm),
            store,
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent")
                .workspace(WorkspaceRef { root: temp_dir.clone(), session_id: None })
                .build(),
            &serde_json::json!({ "task": "summarize", "max_turns": 1, "background": false }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");

        // Over the bound the text is dropped: the artifact is the only
        // carrier, keeping the parent's context bounded by design.
        assert_eq!(value["completion_text"], serde_json::Value::Null);
        let artifact_ref = value["artifact_refs"][0].as_str().expect("artifact ref");
        let artifact =
            tokio::fs::read_to_string(temp_dir.join(artifact_ref)).await.expect("artifact content");
        let artifact: serde_json::Value = serde_json::from_str(&artifact).expect("artifact json");
        assert_eq!(
            artifact["completion_text"].as_str().map(str::len),
            Some(MAX_NOTIFICATION_RESULT_CHARS + 100)
        );

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    /// Without a workspace there is no artifact to hold an oversized result:
    /// the inline copy is truncated at the bound with an explicit marker
    /// instead of flowing into the parent context unbounded.
    #[tokio::test]
    async fn runaway_result_without_workspace_is_truncated() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(RunawayLlm),
            store.clone(),
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);

        // NOTE: no `.workspace(...)` on the tool context — no artifact path.
        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "summarize", "max_turns": 1, "background": false }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");

        let text = value["completion_text"].as_str().expect("truncated copy is inlined");
        assert!(
            text.contains("result truncated"),
            "the truncation is explicit, not silent: {text:?}"
        );
        assert!(
            text.chars().count()
                <= MAX_NOTIFICATION_RESULT_CHARS
                    + "\n(result truncated at 0000 chars: no workspace artifact is available to hold the full output)".chars().count(),
            "the inline copy stays at the bound: {}",
            text.chars().count()
        );
    }

    /// The model-supplied `max_turns` is clamped to the cap and the clamp is
    /// echoed in the tool result.
    #[tokio::test]
    async fn max_turns_is_clamped_to_the_cap() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store.clone(),
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({
                "task": "summarize",
                "max_turns": 100_000,
                "background": false
            }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        assert!(
            value["max_turns_note"].as_str().is_some_and(|note| note.contains("clamped to 100")),
            "the clamp is echoed to the model: {}",
            value["max_turns_note"]
        );

        let child_id = value["child_thread_id"].as_str().expect("child id");
        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        assert_eq!(child_config.max_turns, 100);
    }

    /// LLM double that never finals: every turn narrates its confirmed
    /// findings and requests another tool call. A child running against it
    /// exhausts its turn budget mid-work — the max-turns partial-findings path.
    struct ToolLoopNarratingLlm;

    #[async_trait]
    impl LlmPort for ToolLoopNarratingLlm {
        async fn chat_completion(
            &self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _config: &AgentConfig,
            _trace_context: &AgentTraceContext,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                content: Some("Confirmed so far: the parser bug is at src/lex.rs:42.".to_owned()),
                content_already_streamed: false,
                tool_calls: vec![ParsedToolCall {
                    id: "call-loop".to_owned(),
                    name: "read_file".to_owned(),
                    arguments: "{\"path\": \"src/lex.rs\"}".to_owned(),
                }],
                finish_reason: Some("tool_calls".to_owned()),
                usage: None,
            })
        }
    }

    fn narrating_control(store: Arc<MemoryStore>) -> Arc<AgentControl> {
        let notify = Arc::new(NoopNotify);
        Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(ToolLoopNarratingLlm),
            store,
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ))
    }

    /// LLM double: the FIRST call requests a `write_file` tool call for the
    /// configured path; every later call finalizes. Drives the end-to-end
    /// scope-enforcement round (the child sees its tool result and finishes).
    struct WriteThenFinalLlm {
        path: String,
        calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait]
    impl LlmPort for WriteThenFinalLlm {
        async fn chat_completion(
            &self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _config: &AgentConfig,
            _trace_context: &AgentTraceContext,
        ) -> Result<LlmResponse, AgentError> {
            let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if call == 0 {
                Ok(LlmResponse {
                    content: None,
                    content_already_streamed: false,
                    tool_calls: vec![ParsedToolCall {
                        id: "call-write".to_owned(),
                        name: "write_file".to_owned(),
                        arguments: serde_json::json!({
                            "path": self.path,
                            "content": "payload"
                        })
                        .to_string(),
                    }],
                    finish_reason: Some("tool_calls".to_owned()),
                    usage: None,
                })
            } else {
                Ok(LlmResponse {
                    content: Some("done".to_owned()),
                    content_already_streamed: false,
                    tool_calls: Vec::new(),
                    finish_reason: Some("stop".to_owned()),
                    usage: None,
                })
            }
        }
    }

    /// Inline delegation whose child runs out of turns: the synthesized
    /// partial findings flow back as the result instead of a bare
    /// "max_turns_reached" — the raw thread status stays honestly
    /// "interrupted".
    #[tokio::test]
    async fn max_turns_child_delivers_partial_findings_inline() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let control = narrating_control(store.clone());
        let tool = delegate_tool(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "find the bug", "max_turns": 1, "background": false }),
        )
        .await
        .expect("delegate");

        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        assert_eq!(value["status"], "interrupted", "the thread status stays honest");
        let text = value["completion_text"].as_str().expect("partial findings are inlined");
        assert!(
            text.starts_with(slab_agent::MAX_TURNS_PARTIAL_PREFIX),
            "the partial-findings prefix drives registry routing: {text:?}"
        );
        assert!(text.contains("src/lex.rs:42"), "the confirmed finding survives: {text:?}");
        assert_ne!(text, "max_turns_reached", "not the bare pre-fix reason string");
    }

    /// Background delegation whose child runs out of turns: the
    /// partial-carrying interruption maps to `Completed` (not `Stopped`), so
    /// the parent notification fires and the registry keeps the result.
    #[tokio::test]
    async fn max_turns_child_notifies_parent_with_partial_findings() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let control = narrating_control(store.clone());
        let registry = Arc::new(BackgroundTaskRegistry::default());
        let sink = Arc::new(RecordingSink::default());
        let tool =
            DelegateSubagentTool::new(Arc::clone(&control), Arc::clone(&registry), sink.clone());

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "find the bug", "max_turns": 1, "background": true }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let task_id = value["task_id"].as_str().expect("task id").to_owned();

        wait_until(|| !sink.finished.lock().unwrap().is_empty()).await;
        {
            let finished = sink.finished.lock().unwrap();
            assert_eq!(finished[0].status, BackgroundTaskStatus::Completed);
            let text = finished[0].completion_text.as_deref().expect("partial findings");
            assert!(text.starts_with(slab_agent::MAX_TURNS_PARTIAL_PREFIX));
        }

        // The registry result survives for subagent_status reads.
        wait_until(|| {
            registry.list().iter().any(|task| {
                task.task_id == task_id && task.status == BackgroundTaskStatus::Completed
            })
        })
        .await;
        let snapshot = registry
            .list()
            .into_iter()
            .find(|task| task.task_id == task_id)
            .expect("registered task");
        assert!(snapshot.result.is_some_and(|result| result.contains("src/lex.rs:42")));
    }

    /// A genuine user stop stays suppressed: the registry pre-marks the slot
    /// Stopped, and the plain "interrupted" completion never maps to
    /// Completed.
    #[tokio::test]
    async fn explicit_stop_still_suppresses_the_parent_notification() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let control = narrating_control(store.clone());
        let registry = Arc::new(BackgroundTaskRegistry::default());
        let sink = Arc::new(RecordingSink::default());
        let tool =
            DelegateSubagentTool::new(Arc::clone(&control), Arc::clone(&registry), sink.clone());

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "find the bug", "max_turns": 100, "background": true }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let task_id = value["task_id"].as_str().expect("task id").to_owned();

        // Stop immediately: the delegation is registered Running by the time
        // the background result returns, and the narrating loop burns through
        // its turns quickly — a sleep here would race the child to terminal.
        let stopped = registry.stop(&task_id).expect("stop task");
        assert_eq!(stopped.status, BackgroundTaskStatus::Stopped);

        // Give the watcher a moment: no finished event may fire.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(
            sink.finished.lock().unwrap().is_empty(),
            "explicit stops are not re-reported to the parent"
        );
    }

    /// Omitting `max_turns` uses the wider default and tells the caller.
    #[tokio::test]
    async fn omitted_max_turns_defaults_wider_and_notes_the_caller() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let control = narrating_control(store.clone());
        let tool = delegate_tool(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "find the bug", "background": false }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        assert!(
            value["max_turns_note"].as_str().is_some_and(|note| note.contains("defaulted to")),
            "the default is echoed to the model: {}",
            value["max_turns_note"]
        );

        let child_id = value["child_thread_id"].as_str().expect("child id");
        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        assert_eq!(child_config.max_turns, DEFAULT_SUBAGENT_TURNS);
    }

    #[tokio::test]
    async fn delegate_subagent_rejects_workspace_scope_escape() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store,
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);

        let result = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent")
                .workspace(WorkspaceRef {
                    root: PathBuf::from("C:/workspace/demo"),
                    session_id: None,
                })
                .build(),
            &serde_json::json!({"task": "summarize", "workspace_scope": "../outside"}),
        )
        .await;

        let error = result.expect_err("scope escape rejected").to_string();
        assert!(error.contains("workspace_scope must stay inside the workspace"));
    }

    /// The lexical escape check is blind to symlinks: a scope directory that
    /// exists as a symlink pointing OUTSIDE the workspace must be rejected by
    /// the canonical containment check.
    #[tokio::test]
    async fn delegate_subagent_rejects_symlinked_workspace_scope() {
        let temp_dir = std::env::temp_dir()
            .join(format!("slab-agent-tools-subagent-symlink-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        tokio::fs::create_dir_all(&temp_dir).await.expect("temp workspace");
        let outside = temp_dir.join("outside");
        tokio::fs::create_dir_all(&outside).await.expect("outside dir");
        let link = temp_dir.join("src");
        #[cfg(unix)]
        let symlink_result = std::os::unix::fs::symlink(&outside, &link);
        #[cfg(windows)]
        let symlink_result = std::os::windows::fs::symlink_dir(&outside, &link);
        if symlink_result.is_err() {
            // Symlink creation needs privileges on some hosts; skip silently.
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            return;
        }

        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store,
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);

        let result = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent")
                .workspace(WorkspaceRef { root: temp_dir.clone(), session_id: None })
                .build(),
            &serde_json::json!({"task": "summarize", "workspace_scope": "src"}),
        )
        .await;

        let error = result.expect_err("symlinked scope rejected").to_string();
        assert!(error.contains("workspace_scope must stay inside the workspace"), "{error}");

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    /// A scoped agent delegating an explicitly out-of-scope child scope is a
    /// hard error. Delegating WITHOUT a child scope stays allowed (declared
    /// gap — nesting is bounded by max_depth instead).
    #[tokio::test]
    async fn delegate_subagent_rejects_child_scope_outside_parent_scope() {
        let temp_dir = std::env::temp_dir()
            .join(format!("slab-agent-tools-subagent-parent-scope-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        tokio::fs::create_dir_all(temp_dir.join("src")).await.expect("scope dir");

        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store,
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);

        let parent_scope = slab_agent::WorkspaceScopeRef {
            root: temp_dir.join("src").canonicalize().expect("canonical scope"),
            relative: "src".to_owned(),
        };
        let result = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent")
                .workspace(WorkspaceRef { root: temp_dir.clone(), session_id: None })
                .workspace_scope(parent_scope)
                .build(),
            &serde_json::json!({"task": "summarize", "workspace_scope": "docs"}),
        )
        .await;

        let error = result.expect_err("widening scope rejected").to_string();
        assert!(
            error.contains(
                "workspace_scope must stay inside this agent's own delegated scope \
                            'src'"
            ),
            "{error}"
        );

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    /// End-to-end: the scope on the persisted child config reaches the child's
    /// ToolContext and the REAL write_file tool rejects out-of-scope writes
    /// while allowing in-scope ones.
    #[tokio::test]
    async fn delegate_subagent_enforces_workspace_scope_end_to_end() {
        async fn round(write_path: &str) -> (bool, bool) {
            let temp_dir = std::env::temp_dir()
                .join(format!("slab-agent-tools-subagent-e2e-scope-{}", std::process::id()));
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            tokio::fs::create_dir_all(&temp_dir).await.expect("temp workspace");

            let router = ToolRouter::new();
            router.register(Box::new(crate::fs::WriteFileTool::new(Some(temp_dir.clone()))));
            let store = Arc::new(MemoryStore::default());
            store.insert_parent(1);
            let llm = Arc::new(WriteThenFinalLlm {
                path: write_path.to_owned(),
                calls: std::sync::atomic::AtomicUsize::new(0),
            });
            let control = Arc::new(
                slab_agent::AgentControl::new_with_hooks(
                    llm,
                    store,
                    Arc::new(NoopNotify),
                    Arc::new(NoopNotify),
                    Arc::new(router),
                    AgentControlLimits { max_threads: 4, max_depth: 4 },
                    Vec::new(),
                )
                .with_thread_context(
                    slab_agent::AgentThreadContext::new()
                        .with_workspace(WorkspaceRef { root: temp_dir.clone(), session_id: None }),
                ),
            );
            let tool = delegate_tool(control);

            let output = ToolHandler::execute(
                &tool,
                &ToolContext::for_thread("parent")
                    .workspace(WorkspaceRef { root: temp_dir.clone(), session_id: None })
                    .build(),
                &serde_json::json!({
                    "task": "write the file",
                    "workspace_scope": "src",
                    "max_turns": 3,
                    "background": false
                }),
            )
            .await
            .expect("delegate");
            let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
            assert_eq!(value["status"], "completed", "child still finishes: {value}");

            let outside_created = temp_dir.join("outside.txt").exists();
            let inside_created = temp_dir.join("src").join("inside.txt").exists();
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            (outside_created, inside_created)
        }

        // Out-of-scope write: rejected, file never lands.
        let (outside_created, _) = round("outside.txt").await;
        assert!(!outside_created, "out-of-scope write must not land");

        // In-scope write: allowed.
        let (_, inside_created) = round("src/inside.txt").await;
        assert!(inside_created, "in-scope write must land");
    }

    #[tokio::test]
    async fn delegate_subagent_respects_parent_depth_limit() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(0);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store,
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);

        let result = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({"task": "summarize"}),
        )
        .await;

        assert!(matches!(result, Err(AgentError::DepthLimitExceeded { current: 1, max: 0 })));
    }

    // ---- Slice 4: agent_type integration ----

    const PLAN_PROMPT: &str = "You are a read-only planning agent.";

    /// LLM that records the tool list presented each call, so tests can verify
    /// the agent tool constraint reached the model-facing projection.
    #[derive(Default)]
    struct RecordingLlm {
        captured_tools: Mutex<Vec<Vec<ToolSpec>>>,
    }

    #[async_trait]
    impl LlmPort for RecordingLlm {
        async fn chat_completion(
            &self,
            _model: &str,
            _messages: &[ConversationMessage],
            tools: &[ToolSpec],
            _config: &AgentConfig,
            _trace_context: &AgentTraceContext,
        ) -> Result<LlmResponse, AgentError> {
            self.captured_tools.lock().unwrap().push(tools.to_vec());
            Ok(LlmResponse {
                content: Some("child result".to_owned()),
                content_already_streamed: false,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".to_owned()),
                usage: None,
            })
        }
    }

    /// Minimal named tool handler used only to populate a router with specs.
    struct StubTool {
        tool_name: String,
    }

    impl StubTool {
        fn new(name: &str) -> Self {
            Self { tool_name: name.to_owned() }
        }
    }

    #[async_trait]
    impl TypedTool for StubTool {
        type Input = serde_json::Value;
        fn name(&self) -> &str {
            &self.tool_name
        }

        fn description(&self) -> &str {
            "stub"
        }

        async fn execute(
            &self,
            _ctx: &ToolContext,
            _arguments: serde_json::Value,
        ) -> Result<ToolOutput, AgentError> {
            Ok(ToolOutput { content: "stub".to_owned(), metadata: None })
        }
    }

    /// HashMap-backed agent registry for tests.
    #[derive(Default)]
    struct MockRegistry {
        agents: Vec<AgentDefinition>,
    }

    impl MockRegistry {
        /// A "plan" agent that denies `shell` with a fixed system prompt.
        fn plan() -> Self {
            Self {
                agents: vec![AgentDefinition {
                    agent_type: "plan".to_owned(),
                    description: "test plan agent".to_owned(),
                    tools: ToolConstraint::Denylist(vec!["shell".to_owned()]),
                    system_prompt: PLAN_PROMPT.to_owned(),
                    model: ModelPolicy::Inherit,
                }],
            }
        }

        /// A "plan" agent that also pins a model (for caller-overrides tests).
        fn plan_with_fixed_model(model: &str) -> Self {
            let mut registry = Self::plan();
            if let Some(def) = registry.agents.get_mut(0) {
                def.model = ModelPolicy::Fixed(model.to_owned());
            }
            registry
        }
    }

    impl AgentRegistry for MockRegistry {
        fn get(&self, agent_type: &str) -> Option<AgentDefinition> {
            self.agents.iter().find(|def| def.agent_type == agent_type).cloned()
        }
        fn list(&self) -> Vec<AgentDefinition> {
            self.agents.clone()
        }
    }

    fn build_control(
        llm: Arc<dyn LlmPort>,
        store: Arc<MemoryStore>,
        router: Arc<ToolRouter>,
        registry: Arc<dyn AgentRegistry>,
    ) -> Arc<AgentControl> {
        let notify = Arc::new(NoopNotify);
        Arc::new(
            slab_agent::AgentControl::new_with_hooks(
                llm,
                store,
                notify.clone(),
                notify,
                router,
                AgentControlLimits { max_threads: 4, max_depth: 4 },
                Vec::new(),
            )
            .with_agent_registry(registry),
        )
    }

    #[tokio::test]
    async fn delegate_subagent_with_agent_type_sets_config_and_system_prompt() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let control = build_control(
            Arc::new(FinalLlm),
            store.clone(),
            Arc::new(ToolRouter::new()),
            Arc::new(MockRegistry::plan()),
        );
        let tool = delegate_tool(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "plan it", "agent_type": "plan", "background": false }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let child_id = value["child_thread_id"].as_str().expect("child id");

        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        assert_eq!(child_config.agent_type.as_deref(), Some("plan"));
        assert_eq!(child_config.system_prompt.as_deref(), Some(PLAN_PROMPT));
        assert!(child_config.transient);
    }

    #[tokio::test]
    async fn delegate_subagent_agent_type_enforces_tool_constraint_end_to_end() {
        let router = ToolRouter::new();
        router.register(Box::new(StubTool::new("shell")));
        router.register(Box::new(StubTool::new("read_file")));
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let llm = Arc::new(RecordingLlm::default());
        let control = build_control(
            Arc::clone(&llm) as Arc<dyn LlmPort>,
            store.clone(),
            Arc::new(router),
            Arc::new(MockRegistry::plan()),
        );
        let tool = delegate_tool(control);

        ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "plan it", "agent_type": "plan", "max_turns": 1, "background": false }),
        )
        .await
        .expect("delegate");

        let captured = llm.captured_tools.lock().unwrap().clone();
        let names: Vec<String> = captured.iter().flatten().map(|spec| spec.name.clone()).collect();
        assert!(
            !names.contains(&"shell".to_owned()),
            "plan agent must not see the denied `shell` tool: {names:?}"
        );
        assert!(
            names.contains(&"read_file".to_owned()),
            "plan agent should still see `read_file`: {names:?}"
        );
    }

    #[tokio::test]
    async fn delegate_subagent_agent_type_not_in_registry_errors() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        // Default control carries a NoopAgentRegistry — "missing" is unknown.
        let control = build_control(
            Arc::new(FinalLlm),
            store.clone(),
            Arc::new(ToolRouter::new()),
            Arc::new(MockRegistry::default()),
        );
        let tool = delegate_tool(control);

        let result = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "plan it", "agent_type": "missing" }),
        )
        .await;

        let error = result.expect_err("unknown agent_type rejected").to_string();
        assert!(error.contains("unknown agent_type: missing"), "{error}");
    }

    #[tokio::test]
    async fn delegate_subagent_explicit_system_prompt_overrides_definition() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let control = build_control(
            Arc::new(FinalLlm),
            store.clone(),
            Arc::new(ToolRouter::new()),
            Arc::new(MockRegistry::plan()),
        );
        let tool = delegate_tool(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({
                "task": "plan it",
                "agent_type": "plan",
                "system_prompt": "custom prompt",
                "background": false
            }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let child_id = value["child_thread_id"].as_str().expect("child id");

        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        assert_eq!(child_config.system_prompt.as_deref(), Some("custom prompt"));
        assert_eq!(child_config.agent_type.as_deref(), Some("plan"));
    }

    #[tokio::test]
    async fn delegate_subagent_explicit_model_overrides_definition() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let control = build_control(
            Arc::new(FinalLlm),
            store.clone(),
            Arc::new(ToolRouter::new()),
            Arc::new(MockRegistry::plan_with_fixed_model("plan-model")),
        );
        let tool = delegate_tool(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({
                "task": "plan it",
                "agent_type": "plan",
                "model": "caller-model",
                "background": false
            }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let child_id = value["child_thread_id"].as_str().expect("child id");

        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        // Caller wins over ModelPolicy::Fixed.
        assert_eq!(child_config.model, "caller-model");
        assert_eq!(child_config.agent_type.as_deref(), Some("plan"));
    }

    #[tokio::test]
    async fn delegate_subagent_definition_model_applies_when_caller_omits() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let control = build_control(
            Arc::new(FinalLlm),
            store.clone(),
            Arc::new(ToolRouter::new()),
            Arc::new(MockRegistry::plan_with_fixed_model("plan-model")),
        );
        let tool = delegate_tool(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "plan it", "agent_type": "plan", "background": false }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let child_id = value["child_thread_id"].as_str().expect("child id");

        let child = store.get_thread(child_id).await.expect("thread").expect("child");
        let child_config: AgentConfig =
            serde_json::from_str(&child.config_json).expect("child config");
        // No caller model → definition's Fixed policy applies.
        assert_eq!(child_config.model, "plan-model");
    }

    // ---- background (async) delegation ----

    #[tokio::test]
    async fn delegate_background_returns_immediately_and_registry_tracks() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store.clone(),
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let registry = Arc::new(BackgroundTaskRegistry::default());
        let sink = Arc::new(RecordingSink::default());
        let tool = DelegateSubagentTool::new(control, Arc::clone(&registry), sink.clone());

        // Default (background omitted) = async: the call must NOT wait for
        // the child. The child here finishes nearly instantly — the contract
        // under test is the immediate return SHAPE plus eventual tracking.
        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "summarize", "max_turns": 1 }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        assert_eq!(value["background"], true);
        assert_eq!(value["status"], "running");
        let task_id = value["task_id"].as_str().expect("task id").to_owned();
        let child_id = value["child_thread_id"].as_str().expect("child id").to_owned();
        assert!(value["hint"].as_str().is_some_and(|hint| !hint.is_empty()));

        wait_until(|| {
            registry.snapshot(&task_id).is_some_and(|task| {
                task.status == crate::background::BackgroundTaskStatus::Completed
            })
        })
        .await;

        let spawned = sink.spawned.lock().unwrap();
        assert_eq!(spawned.len(), 1);
        assert_eq!(spawned[0].parent_thread_id, "parent");
        assert_eq!(spawned[0].child_thread_id, child_id);

        let finished = sink.finished.lock().unwrap();
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].task_id, task_id);
        assert_eq!(finished[0].status, crate::background::BackgroundTaskStatus::Completed);
        assert_eq!(finished[0].completion_text.as_deref(), Some("child result"));
    }

    #[tokio::test]
    async fn delegate_background_false_keeps_legacy_result_shape() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store.clone(),
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "summarize", "background": false }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        // Inline shape: the legacy fields, no background envelope.
        assert!(value.get("background").is_none());
        assert_eq!(value["status"], "completed");
        assert_eq!(value["completion_text"], "child result");
        assert!(value["artifact_refs"].as_array().is_some_and(Vec::is_empty));
    }

    #[test]
    fn delegate_is_concurrency_safe_only_for_background() {
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            Arc::new(MemoryStore::default()),
            Arc::new(NoopNotify),
            Arc::new(NoopNotify),
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let tool = delegate_tool(control);
        // Default (omitted) is background → parallel-safe.
        assert!(ToolHandler::is_concurrency_safe(&tool, &serde_json::json!({ "task": "x" })));
        assert!(ToolHandler::is_concurrency_safe(
            &tool,
            &serde_json::json!({ "task": "x", "background": true })
        ));
        assert!(!ToolHandler::is_concurrency_safe(
            &tool,
            &serde_json::json!({ "task": "x", "background": false })
        ));
    }

    // ---- grandchild cascade + no_resume (B2/B3) ----

    /// LLM double for the grandchild-cascade test: the child's first turn
    /// delegates a grandchild in the background; every later call (the
    /// child's follow-up turn AND the grandchild's own turn) parks forever,
    /// so both threads stay Running until an interrupt cancels the parked
    /// LLM future (the turn loop selects on the cancellation token).
    struct DelegatingLlm {
        delegated: std::sync::atomic::AtomicBool,
    }

    #[async_trait]
    impl LlmPort for DelegatingLlm {
        async fn chat_completion(
            &self,
            _model: &str,
            messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _config: &AgentConfig,
            _trace_context: &AgentTraceContext,
        ) -> Result<LlmResponse, AgentError> {
            if messages.iter().any(|message| message.rendered_text().contains("grandchild work")) {
                // Grandchild turn: park until the cascade interrupt cancels us.
                std::future::pending::<()>().await;
                return Err(AgentError::Interrupted);
            }
            if !self.delegated.swap(true, std::sync::atomic::Ordering::SeqCst) {
                // Child turn 1: delegate the grandchild in the background.
                return Ok(LlmResponse {
                    content: None,
                    content_already_streamed: false,
                    tool_calls: vec![ParsedToolCall {
                        id: "call-grandchild".to_owned(),
                        name: "delegate_subagent".to_owned(),
                        arguments: serde_json::json!({ "task": "grandchild work", "max_turns": 1 })
                            .to_string(),
                    }],
                    finish_reason: Some("tool_calls".to_owned()),
                    usage: None,
                });
            }
            // Child follow-up turn: park until the cascade interrupt cancels us.
            std::future::pending::<()>().await;
            Err(AgentError::Interrupted)
        }
    }

    #[tokio::test]
    async fn stopping_a_child_task_cascades_to_grandchildren() {
        let store = Arc::new(MemoryStore::default());
        // Depth headroom: parent (0) → child (1) → grandchild (2).
        store.insert_parent(2);
        let notify = Arc::new(NoopNotify);
        let llm = Arc::new(DelegatingLlm { delegated: std::sync::atomic::AtomicBool::new(false) });
        let registry = Arc::new(BackgroundTaskRegistry::default());
        let router = Arc::new(ToolRouter::new());
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::clone(&llm) as Arc<dyn LlmPort>,
            store.clone(),
            notify.clone(),
            notify,
            Arc::clone(&router),
            AgentControlLimits { max_threads: 8, max_depth: 4 },
            Vec::new(),
        ));
        // The child delegates THROUGH the router — same control, same
        // registry — so the grandchild lands in the shared registry as a
        // task owned by the child thread.
        router.register(Box::new(DelegateSubagentTool::new(
            Arc::clone(&control),
            Arc::clone(&registry),
            Arc::new(NoopSubagentTaskSink),
        )));
        let tool = DelegateSubagentTool::new(
            Arc::clone(&control),
            Arc::clone(&registry),
            Arc::new(NoopSubagentTaskSink),
        );

        let output = ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "child work" }),
        )
        .await
        .expect("delegate");
        let value: serde_json::Value = serde_json::from_str(&output.content).expect("json");
        let child_task_id = value["task_id"].as_str().expect("task id").to_owned();
        let child_thread_id = value["child_thread_id"].as_str().expect("child id").to_owned();

        // Wait for the child's own delegation: a second Subagent task owned
        // by the child thread, pointing at a grandchild thread.
        wait_until(|| {
            registry
                .list()
                .iter()
                .any(|task| task.thread_id == child_thread_id && task.child_thread_id.is_some())
        })
        .await;
        let grandchild_task = registry
            .list()
            .into_iter()
            .find(|task| task.thread_id == child_thread_id && task.child_thread_id.is_some())
            .expect("grandchild task");
        let grandchild_task_id = grandchild_task.task_id.clone();
        let grandchild_thread_id =
            grandchild_task.child_thread_id.clone().expect("grandchild thread id");

        // Stop the CHILD task: its kill closure must cascade-stop the
        // grandchild delegation before interrupting the child itself.
        let stopped_child = registry.stop(&child_task_id).expect("stop child task");
        assert_eq!(stopped_child.status, BackgroundTaskStatus::Stopped);

        wait_until(|| {
            registry
                .snapshot(&grandchild_task_id)
                .is_some_and(|task| task.status == BackgroundTaskStatus::Stopped)
        })
        .await;
        wait_until(|| {
            matches!(
                store
                    .threads
                    .lock()
                    .unwrap()
                    .get(&grandchild_thread_id)
                    .map(|thread| thread.status),
                Some(ThreadStatus::Interrupted)
            )
        })
        .await;
        // The child itself was interrupted too.
        wait_until(|| {
            matches!(
                store.threads.lock().unwrap().get(&child_thread_id).map(|thread| thread.status),
                Some(ThreadStatus::Interrupted)
            )
        })
        .await;
    }

    #[tokio::test]
    async fn delegate_no_resume_flags_both_lifecycle_events() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store.clone(),
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let registry = Arc::new(BackgroundTaskRegistry::default());
        let sink = Arc::new(RecordingSink::default());
        let tool = DelegateSubagentTool::new(control, Arc::clone(&registry), sink.clone());

        ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "summarize", "background": false, "no_resume": true }),
        )
        .await
        .expect("delegate");

        wait_until(|| sink.finished.lock().unwrap().len() == 1).await;
        let finished = sink.finished.lock().unwrap();
        assert!(finished[0].no_resume);
        let spawned = sink.spawned.lock().unwrap();
        assert_eq!(spawned.len(), 1);
        assert!(spawned[0].no_resume);
    }

    #[tokio::test]
    async fn delegate_without_no_resume_defaults_to_resuming() {
        let store = Arc::new(MemoryStore::default());
        store.insert_parent(1);
        let notify = Arc::new(NoopNotify);
        let control = Arc::new(slab_agent::AgentControl::new_with_hooks(
            Arc::new(FinalLlm),
            store.clone(),
            notify.clone(),
            notify,
            Arc::new(ToolRouter::new()),
            AgentControlLimits { max_threads: 4, max_depth: 4 },
            Vec::new(),
        ));
        let registry = Arc::new(BackgroundTaskRegistry::default());
        let sink = Arc::new(RecordingSink::default());
        let tool = DelegateSubagentTool::new(control, Arc::clone(&registry), sink.clone());

        ToolHandler::execute(
            &tool,
            &ToolContext::for_thread("parent").build(),
            &serde_json::json!({ "task": "summarize", "background": false }),
        )
        .await
        .expect("delegate");

        wait_until(|| sink.finished.lock().unwrap().len() == 1).await;
        let finished = sink.finished.lock().unwrap();
        assert!(!finished[0].no_resume);
        let spawned = sink.spawned.lock().unwrap();
        assert_eq!(spawned.len(), 1);
        assert!(!spawned[0].no_resume);
    }
}
