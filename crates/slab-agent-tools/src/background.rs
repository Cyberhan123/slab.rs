//! Background task registry: resident processes started via
//! `shell background=true`.
//!
//! The registry owns the task lifecycle AROUND a
//! [`slab_sandboxing::BackgroundChild`]: id allocation, output-file placement
//! (`.slab/artifacts/<thread>/background/<task>/`), a watcher task that
//! records the exit code and emits lifecycle events, bounded capacity, and
//! explicit stop (tree kill). Turn/thread lifetime is deliberately DECOUPLED
//! — a background dev server must survive the turn that started it; the
//! registry lives with the tool router (server process) and dies with it
//! (the OS then closes every job/process-group handle, killing the trees).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use slab_agent::AgentError;
use slab_sandboxing::BackgroundChild;

/// How many tasks may be RUNNING at once. The model starts them; a bound
/// keeps a runaway loop from forking the machine to death.
const MAX_RUNNING_TASKS: usize = 8;
/// How many TERMINAL task slots to retain for `task_status` queries. Older
/// finished tasks are evicted (their output files stay on disk).
const MAX_RETAINED_TERMINAL_TASKS: usize = 32;
/// Default tail size for `task_output`.
pub const DEFAULT_OUTPUT_TAIL_BYTES: usize = 16 * 1024;
/// Hard cap for `task_output` tail reads (the file itself is unbounded).
const MAX_OUTPUT_TAIL_BYTES: usize = 256 * 1024;

/// Lifecycle status of a background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundTaskStatus {
    Running,
    Exited,
    Stopped,
    Failed,
    /// Non-process task (e.g. a delegated subagent) finished successfully.
    Completed,
}

impl BackgroundTaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Exited => "exited",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
            Self::Completed => "completed",
        }
    }

    fn is_terminal(self) -> bool {
        !matches!(self, Self::Running)
    }
}

/// What kind of work a registered task tracks. Shell tasks own a process
/// tree; subagent tasks track a detached child agent thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Shell,
    Subagent,
}

impl TaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::Subagent => "subagent",
        }
    }
}

/// What a detached task's `wait` future resolved with.
#[derive(Debug)]
pub enum DetachedTaskOutcome {
    /// Shell: the child process exited with this code (`None` = wait failed).
    ProcessExit(Option<i32>),
    /// Non-process task: the caller maps the awaited result to a TERMINAL
    /// status (Completed/Failed/Stopped) plus an optional result payload.
    Status { status: BackgroundTaskStatus, result: Option<String> },
}

/// Future that resolves when a detached task reaches its terminal state.
pub type DetachedWait =
    std::pin::Pin<Box<dyn std::future::Future<Output = DetachedTaskOutcome> + Send>>;
/// Fires the task's cancellation (process tree kill / thread interrupt).
/// MUST NOT be called while the registry lock is held.
pub type DetachedKill = Box<dyn FnOnce() + Send + 'static>;
/// Invoked once with the task's final snapshot after the terminal event has
/// been emitted (e.g. the subagent completion bridge notifying the parent).
pub type DetachedOnTerminal = Box<dyn FnOnce(BackgroundTaskSnapshot) + Send + 'static>;

/// Identity/binding fields of a non-process detached task.
pub struct DetachedTask {
    /// OWNING (parent) thread the lifecycle events are attributed to.
    pub thread_id: String,
    /// Human-readable task summary (shown in status listings).
    pub command: String,
    /// Workspace the task is bound to (workspace-scoped stop).
    pub workspace_root: Option<PathBuf>,
    /// Subagent tasks: the delegated child agent thread id.
    pub child_thread_id: Option<String>,
}

/// A task lifecycle event, delivered through [`BackgroundTaskEventSink`] (the
/// host bridges these to the harness `EventMsg` stream).
#[derive(Debug, Clone)]
pub struct BackgroundTaskEvent {
    pub task_id: String,
    pub thread_id: String,
    pub kind: TaskKind,
    pub status: BackgroundTaskStatus,
    pub exit_code: Option<i32>,
    pub pid: Option<u32>,
    pub command: Option<String>,
    /// Truncated result payload for non-process tasks (subagent completion).
    pub result_summary: Option<String>,
}

/// Receiver for background-task lifecycle events.
pub trait BackgroundTaskEventSink: Send + Sync {
    fn on_task_event(&self, event: BackgroundTaskEvent);
}

/// A no-op sink for hosts/tests that do not bridge events.
#[derive(Default)]
pub struct NoopBackgroundTaskEventSink;

impl BackgroundTaskEventSink for NoopBackgroundTaskEventSink {
    fn on_task_event(&self, _event: BackgroundTaskEvent) {}
}

/// Read-only view of a registered task.
#[derive(Debug, Clone)]
pub struct BackgroundTaskSnapshot {
    pub task_id: String,
    pub thread_id: String,
    pub kind: TaskKind,
    pub command: String,
    pub status: BackgroundTaskStatus,
    pub exit_code: Option<i32>,
    pub pid: Option<u32>,
    /// Forward-slash workspace-relative reference when a workspace is bound,
    /// else the absolute path (empty for non-process tasks).
    pub stdout_ref: String,
    pub stderr_ref: String,
    /// Subagent tasks: the delegated child agent thread id.
    pub child_thread_id: Option<String>,
    /// Subagent tasks: the terminal result payload once finished.
    pub result: Option<String>,
}

struct TaskSlot {
    thread_id: String,
    kind: TaskKind,
    child_thread_id: Option<String>,
    /// The workspace the task was started in (subagent workspace-scoped stop).
    workspace_root: Option<PathBuf>,
    command: String,
    pid: Option<u32>,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    stdout_ref: String,
    stderr_ref: String,
    status: Mutex<BackgroundTaskStatus>,
    exit_code: Mutex<Option<i32>>,
    /// Terminal result payload for non-process tasks (subagent completion).
    result: Mutex<Option<String>>,
    /// Fires the tree kill / thread interrupt; consumed exactly once.
    kill: Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>,
}

/// Registry of resident background tasks. Cheap to construct; the host
/// creates ONE and shares it with the `shell` tool and the `task_*` tools.
pub struct BackgroundTaskRegistry {
    tasks: Mutex<HashMap<String, TaskSlot>>,
    next_id: std::sync::atomic::AtomicU64,
    /// Per-registry (process) nonce embedded in every task id. The registry
    /// is in-memory only, so after a server restart the counter restarts at
    /// `bg-…-1` — without the nonce, a restarted server's first task would
    /// CREATE/TRUNCATE the log files of a pre-restart task in the same
    /// thread's artifact directory.
    id_nonce: String,
    event_sink: Option<Arc<dyn BackgroundTaskEventSink>>,
}

impl Default for BackgroundTaskRegistry {
    fn default() -> Self {
        Self::new(None)
    }
}

impl BackgroundTaskRegistry {
    pub fn new(event_sink: Option<Arc<dyn BackgroundTaskEventSink>>) -> Self {
        // Time + pid keep ids from colliding across restarts and concurrent
        // registries (tests); monotonicity is not required, uniqueness is.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let id_nonce = format!("{:x}{:x}", nanos, std::process::id());
        Self {
            tasks: Mutex::new(HashMap::new()),
            next_id: std::sync::atomic::AtomicU64::new(1),
            id_nonce,
            event_sink,
        }
    }

    /// Allocate the next task id (callers need it BEFORE spawning, to place
    /// the output files). Format: `bg-<nonce>-<n>` — see `Self::id_nonce`.
    pub fn alloc_task_id(&self) -> String {
        let n = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("bg-{}-{n}", self.id_nonce)
    }

    /// Where a task's output files live:
    /// `<workspace>/.slab/artifacts/<thread>/background/<task>/` (workspace
    /// convention) or `<app_home>/background-tasks/<thread>/<task>/` when no
    /// workspace is bound.
    pub fn output_dir(workspace_root: Option<&Path>, thread_id: &str, task_id: &str) -> PathBuf {
        match workspace_root {
            Some(root) => root
                .join(".slab")
                .join("artifacts")
                .join(thread_id)
                .join("background")
                .join(task_id),
            None => slab_utils::app_home::app_home_dir()
                .join("background-tasks")
                .join(thread_id)
                .join(task_id),
        }
    }

    /// Workspace-relative display reference for an output path (absolute when
    /// the path is outside the workspace).
    fn display_ref(workspace_root: Option<&Path>, path: &Path) -> String {
        match workspace_root {
            Some(root) => path
                .strip_prefix(root)
                .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/")),
            None => path.to_string_lossy().replace('\\', "/"),
        }
    }

    /// Register an already-spawned [`BackgroundChild`]: records the slot,
    /// spawns the exit watcher (records the exit code + emits the terminal
    /// event), and returns the initial snapshot. Rejects when the running
    /// capacity is exhausted.
    pub fn register(
        self: &Arc<Self>,
        task_id: String,
        thread_id: String,
        command: String,
        workspace_root: Option<&Path>,
        child: BackgroundChild,
    ) -> Result<BackgroundTaskSnapshot, AgentError> {
        let output_dir = Self::output_dir(workspace_root, &thread_id, &task_id);
        let stdout_path = output_dir.join("stdout.log");
        let stderr_path = output_dir.join("stderr.log");
        let slot = TaskSlot {
            thread_id: thread_id.clone(),
            kind: TaskKind::Shell,
            child_thread_id: None,
            workspace_root: workspace_root.map(Path::to_path_buf),
            command: command.clone(),
            pid: child.pid,
            stdout_ref: Self::display_ref(workspace_root, &stdout_path),
            stderr_ref: Self::display_ref(workspace_root, &stderr_path),
            stdout_path,
            stderr_path,
            status: Mutex::new(BackgroundTaskStatus::Running),
            exit_code: Mutex::new(None),
            result: Mutex::new(None),
            kill: Mutex::new(child.kill_tree),
        };
        self.register_slot(task_id.clone(), slot)?;

        // Exit watcher: records the exit code, flips the status (preserving
        // an explicit Stop), and emits the terminal event. Detached from the
        // turn — dropping the spawning future does not affect it.
        let wait: DetachedWait =
            Box::pin(async move { DetachedTaskOutcome::ProcessExit(child.wait.await.ok()) });
        self.spawn_watcher(task_id.clone(), wait, Box::new(|_| {}));

        self.snapshot(&task_id)
            .ok_or_else(|| AgentError::Internal("task vanished on register".into()))
    }

    /// Register a non-process detached task (e.g. a delegated subagent).
    /// Recorded as [`TaskKind::Subagent`]. `wait` resolves to the terminal
    /// outcome, `kill` cancels the underlying work, and `on_terminal` runs
    /// once with the final snapshot after the terminal event has been emitted
    /// (it fires even when the task was stopped, so stop semantics stay
    /// observable — receivers check the status themselves).
    pub fn register_detached(
        self: &Arc<Self>,
        task_id: String,
        task: DetachedTask,
        wait: DetachedWait,
        kill: DetachedKill,
        on_terminal: DetachedOnTerminal,
    ) -> Result<BackgroundTaskSnapshot, AgentError> {
        let slot = TaskSlot {
            thread_id: task.thread_id,
            kind: TaskKind::Subagent,
            child_thread_id: task.child_thread_id,
            workspace_root: task.workspace_root,
            command: task.command,
            pid: None,
            stdout_path: PathBuf::new(),
            stderr_path: PathBuf::new(),
            stdout_ref: String::new(),
            stderr_ref: String::new(),
            status: Mutex::new(BackgroundTaskStatus::Running),
            exit_code: Mutex::new(None),
            result: Mutex::new(None),
            kill: Mutex::new(Some(kill)),
        };
        self.register_slot(task_id.clone(), slot)?;
        self.spawn_watcher(task_id.clone(), wait, on_terminal);

        self.snapshot(&task_id)
            .ok_or_else(|| AgentError::Internal("task vanished on register".into()))
    }

    /// Capacity gate + slot insertion + terminal pruning (shared by
    /// `register` and `register_detached`).
    fn register_slot(&self, task_id: String, slot: TaskSlot) -> Result<(), AgentError> {
        let mut tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
        let running = tasks
            .values()
            .filter(|slot| {
                *slot.status.lock().unwrap_or_else(|p| p.into_inner())
                    == BackgroundTaskStatus::Running
            })
            .count();
        if running >= MAX_RUNNING_TASKS {
            return Err(AgentError::ToolExecution(format!(
                "background task limit reached ({MAX_RUNNING_TASKS} running); stop a task first"
            )));
        }
        tasks.insert(task_id, slot);
        Self::prune_terminal(&mut tasks);
        Ok(())
    }

    /// Terminal watcher shared by every registration path: resolves the wait
    /// future, records the terminal state, emits the event, then fires the
    /// caller's `on_terminal` callback with the final snapshot.
    fn spawn_watcher(
        self: &Arc<Self>,
        task_id: String,
        wait: DetachedWait,
        on_terminal: DetachedOnTerminal,
    ) {
        let registry = Arc::clone(self);
        tokio::spawn(async move {
            let outcome = wait.await;
            registry.finish_outcome(&task_id, outcome, on_terminal);
        });
    }

    /// Record a task's terminal state (watcher callback).
    fn finish_outcome(
        &self,
        task_id: &str,
        outcome: DetachedTaskOutcome,
        on_terminal: DetachedOnTerminal,
    ) {
        let snapshot = {
            let tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
            let Some(slot) = tasks.get(task_id) else { return };
            let mut status = slot.status.lock().unwrap_or_else(|p| p.into_inner());
            let (terminal, exit_code, result) = match outcome {
                DetachedTaskOutcome::ProcessExit(code) => {
                    let terminal = match *status {
                        BackgroundTaskStatus::Stopped => BackgroundTaskStatus::Stopped,
                        _ if code == Some(0) => BackgroundTaskStatus::Exited,
                        _ => BackgroundTaskStatus::Failed,
                    };
                    (terminal, code, None)
                }
                DetachedTaskOutcome::Status { status: mapped, result } => {
                    // Stopped-wins: an explicit stop must not be overwritten by
                    // the (racing) natural completion of the killed work — and
                    // the killed work's late payload is not a trustworthy
                    // result, so it is discarded too.
                    if *status == BackgroundTaskStatus::Stopped {
                        (BackgroundTaskStatus::Stopped, None, None)
                    } else {
                        (mapped, None, result)
                    }
                }
            };
            *status = terminal;
            *slot.exit_code.lock().unwrap_or_else(|p| p.into_inner()) = exit_code;
            *slot.result.lock().unwrap_or_else(|p| p.into_inner()) = result.clone();
            // The kill closure is moot once the work ended.
            *slot.kill.lock().unwrap_or_else(|p| p.into_inner()) = None;
            slot_snapshot(task_id, slot, terminal, exit_code, result)
        };
        let event = BackgroundTaskEvent {
            task_id: snapshot.task_id.clone(),
            thread_id: snapshot.thread_id.clone(),
            kind: snapshot.kind,
            status: snapshot.status,
            exit_code: snapshot.exit_code,
            pid: snapshot.pid,
            command: Some(snapshot.command.clone()),
            result_summary: snapshot.result.as_deref().map(truncate_summary),
        };
        self.emit(event);
        on_terminal(snapshot);
    }

    /// Emit a lifecycle event to the host bridge (best-effort).
    fn emit(&self, event: BackgroundTaskEvent) {
        if let Some(sink) = &self.event_sink {
            sink.on_task_event(event);
        }
    }

    /// Evict the OLDEST terminal slots beyond the retention bound (insertion
    /// order is approximated by the numeric id suffix after the nonce:
    /// `bg-<nonce>-<n>`).
    fn prune_terminal(tasks: &mut HashMap<String, TaskSlot>) {
        let terminal: Vec<(u64, String)> = tasks
            .iter()
            .filter(|(_, slot)| {
                *slot.status.lock().unwrap_or_else(|p| p.into_inner())
                    != BackgroundTaskStatus::Running
            })
            .filter_map(|(id, _)| {
                id.strip_prefix("bg-")
                    .and_then(|rest| rest.rsplit_once('-'))
                    .and_then(|(_, n)| n.parse::<u64>().ok())
                    .map(|n| (n, id.clone()))
            })
            .collect();
        if terminal.len() > MAX_RETAINED_TERMINAL_TASKS {
            let mut ordered = terminal;
            ordered.sort();
            let evict = ordered.len() - MAX_RETAINED_TERMINAL_TASKS;
            for (_, id) in ordered.into_iter().take(evict) {
                tasks.remove(&id);
            }
        }
    }

    /// Current snapshot of a task, if registered.
    pub fn snapshot(&self, task_id: &str) -> Option<BackgroundTaskSnapshot> {
        let tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
        let slot = tasks.get(task_id)?;
        Some(slot_snapshot(
            task_id,
            slot,
            *slot.status.lock().unwrap_or_else(|p| p.into_inner()),
            *slot.exit_code.lock().unwrap_or_else(|p| p.into_inner()),
            slot.result.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        ))
    }

    /// Snapshots of all retained tasks (running first, then newest terminal).
    pub fn list(&self) -> Vec<BackgroundTaskSnapshot> {
        let tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
        let mut snapshots: Vec<BackgroundTaskSnapshot> = tasks
            .keys()
            .filter_map(|id| {
                let slot = tasks.get(id)?;
                Some(slot_snapshot(
                    id,
                    slot,
                    *slot.status.lock().unwrap_or_else(|p| p.into_inner()),
                    *slot.exit_code.lock().unwrap_or_else(|p| p.into_inner()),
                    slot.result.lock().unwrap_or_else(|p| p.into_inner()).clone(),
                ))
            })
            .collect();
        snapshots.sort_by(|a, b| {
            let rank = |s: &BackgroundTaskSnapshot| {
                (s.status != BackgroundTaskStatus::Running, s.task_id.clone())
            };
            rank(a).cmp(&rank(b))
        });
        snapshots
    }

    /// Stop a running task: fires the tree kill and marks it Stopped (the
    /// watcher records the exit code). Stopping a terminal task is a no-op
    /// returning its snapshot.
    pub fn stop(&self, task_id: &str) -> Result<BackgroundTaskSnapshot, AgentError> {
        let (kill, event) = {
            let tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
            let slot = tasks.get(task_id).ok_or_else(|| {
                AgentError::ToolExecution(format!("unknown background task: {task_id}"))
            })?;
            // Short-lived inner locks only — never held across another lock
            // of the same mutex or across the kill closure.
            let current = *slot.status.lock().unwrap_or_else(|p| p.into_inner());
            let exit_code = *slot.exit_code.lock().unwrap_or_else(|p| p.into_inner());
            if current.is_terminal() {
                return Ok(slot_snapshot(
                    task_id,
                    slot,
                    current,
                    exit_code,
                    slot.result.lock().unwrap_or_else(|p| p.into_inner()).clone(),
                ));
            }
            // Take the kill closure now; fire it AFTER the locks release (a
            // kill closure must never run while the registry is locked).
            let kill = slot.kill.lock().unwrap_or_else(|p| p.into_inner()).take();
            *slot.status.lock().unwrap_or_else(|p| p.into_inner()) = BackgroundTaskStatus::Stopped;
            let kind = slot.kind;
            let event = BackgroundTaskEvent {
                task_id: task_id.to_owned(),
                thread_id: slot.thread_id.clone(),
                kind,
                status: BackgroundTaskStatus::Stopped,
                exit_code: None,
                pid: slot.pid,
                command: Some(slot.command.clone()),
                result_summary: None,
            };
            (kill, event)
        };
        if let Some(kill) = kill {
            kill();
        }
        self.emit(event);
        self.snapshot(task_id).ok_or_else(|| AgentError::Internal("task vanished on stop".into()))
    }

    /// Stop every RUNNING task that belongs to `root`'s workspace (workspace
    /// migration: no "ghost" tasks carry into the new workspace): shell tasks
    /// by output-file placement, subagent tasks by their recorded workspace
    /// root. Returns the stopped task ids.
    pub fn stop_all_for_workspace(&self, root: &Path) -> Vec<String> {
        // Mark + take the kill closures under the lock; fire after release
        // (a kill closure must never run while the registry is locked).
        let kills: Vec<(String, Box<dyn FnOnce() + Send + 'static>)> = {
            let mut tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
            let mut taken = Vec::new();
            for (id, slot) in tasks.iter_mut() {
                let mut status = slot.status.lock().unwrap_or_else(|p| p.into_inner());
                let in_workspace = match slot.kind {
                    TaskKind::Shell => {
                        slot.stdout_path.starts_with(root) || slot.stderr_path.starts_with(root)
                    }
                    TaskKind::Subagent => {
                        slot.workspace_root.as_deref().is_some_and(|ws| ws.starts_with(root))
                    }
                };
                if *status == BackgroundTaskStatus::Running
                    && in_workspace
                    && let Some(kill) = slot.kill.lock().unwrap_or_else(|p| p.into_inner()).take()
                {
                    *status = BackgroundTaskStatus::Stopped;
                    taken.push((id.clone(), kill));
                }
            }
            taken
        };
        let ids: Vec<String> = kills.iter().map(|(id, _)| id.clone()).collect();
        for (_, kill) in kills {
            kill();
        }
        ids
    }

    /// Stop every RUNNING subagent task OWNED by `thread_id` (parent-thread
    /// cascade on interrupt/shutdown). Deliberately scoped to
    /// [`TaskKind::Subagent`] — background SHELL tasks are meant to survive
    /// the run that started them. Returns the stopped task ids.
    pub fn stop_subagent_tasks_for_thread(&self, thread_id: &str) -> Vec<String> {
        // Mark + take the kill closures under the lock; fire after release
        // (a kill closure must never run while the registry is locked).
        let kills: Vec<(String, Box<dyn FnOnce() + Send + 'static>)> = {
            let mut tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
            let mut taken = Vec::new();
            for (id, slot) in tasks.iter_mut() {
                if slot.kind != TaskKind::Subagent || slot.thread_id != thread_id {
                    continue;
                }
                let mut status = slot.status.lock().unwrap_or_else(|p| p.into_inner());
                if *status == BackgroundTaskStatus::Running
                    && let Some(kill) = slot.kill.lock().unwrap_or_else(|p| p.into_inner()).take()
                {
                    *status = BackgroundTaskStatus::Stopped;
                    taken.push((id.clone(), kill));
                }
            }
            taken
        };
        let ids: Vec<String> = kills.iter().map(|(id, _)| id.clone()).collect();
        for (_, kill) in kills {
            kill();
        }
        ids
    }

    /// Read the TAIL of a task's stdout file (bounded; the file itself is the
    /// unbounded record). Subagent tasks have no log file — their slot-stored
    /// terminal result is returned instead (with its full byte length).
    pub async fn read_output(
        &self,
        task_id: &str,
        tail_bytes: Option<usize>,
    ) -> Result<(String, u64), AgentError> {
        // Block-scoped: the sync MutexGuard must not live across the await.
        let (kind, path, result) = {
            let tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
            let slot = tasks.get(task_id).ok_or_else(|| {
                AgentError::ToolExecution(format!("unknown background task: {task_id}"))
            })?;
            (
                slot.kind,
                slot.stdout_path.clone(),
                slot.result.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            )
        };
        if kind == TaskKind::Subagent {
            let total = result.as_deref().map(str::len).unwrap_or(0) as u64;
            return Ok((result.unwrap_or_default(), total));
        }

        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| crate::error::io_tool_error("read background task output", &path, &e))?;
        let total = bytes.len() as u64;
        let tail = tail_bytes.unwrap_or(DEFAULT_OUTPUT_TAIL_BYTES).min(MAX_OUTPUT_TAIL_BYTES);
        let start = bytes.len().saturating_sub(tail);
        // Cut at a UTF-8 boundary from the start offset.
        let mut start = start;
        while start < bytes.len() && !std::str::from_utf8(&bytes[start..start + 1]).is_ok() {
            start += 1;
        }
        let text = String::from_utf8_lossy(&bytes[start..]).into_owned();
        Ok((text, total))
    }
}

// ── task_* tools ─────────────────────────────────────────────────────────────

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use slab_agent::{ToolContext, ToolOutput, TypedTool};

/// Arguments for the `task_status` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskStatusArgs {
    /// Optional task id from the background spawn; omit to list all tasks.
    task_id: Option<String>,
}

/// Arguments for the `task_output` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskOutputArgs {
    /// Task id from the background spawn.
    task_id: String,
    /// How many trailing bytes to read (default 16384, max 262144).
    #[schemars(default = "default_output_tail_bytes", range(max = 262_144))]
    tail_bytes: Option<u64>,
}

/// Arguments for the `task_stop` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskStopArgs {
    /// Task id from the background spawn.
    task_id: String,
}

fn default_output_tail_bytes() -> u64 {
    DEFAULT_OUTPUT_TAIL_BYTES as u64
}

/// Build a snapshot view from a slot with the caller-held field values (the
/// per-field mutexes are already unlocked by the time we get here).
fn slot_snapshot(
    task_id: &str,
    slot: &TaskSlot,
    status: BackgroundTaskStatus,
    exit_code: Option<i32>,
    result: Option<String>,
) -> BackgroundTaskSnapshot {
    BackgroundTaskSnapshot {
        task_id: task_id.to_owned(),
        thread_id: slot.thread_id.clone(),
        kind: slot.kind,
        command: slot.command.clone(),
        status,
        exit_code,
        pid: slot.pid,
        stdout_ref: slot.stdout_ref.clone(),
        stderr_ref: slot.stderr_ref.clone(),
        child_thread_id: slot.child_thread_id.clone(),
        result,
    }
}

/// Cap a result payload for event summaries (~200 chars on a char boundary).
fn truncate_summary(text: &str) -> String {
    const MAX_CHARS: usize = 200;
    if text.chars().count() <= MAX_CHARS {
        return text.to_owned();
    }
    let truncated: String = text.chars().take(MAX_CHARS).collect();
    format!("{truncated}…")
}

fn snapshot_json(task: &BackgroundTaskSnapshot) -> serde_json::Value {
    let mut value = serde_json::json!({
        "task_id": task.task_id,
        "thread_id": task.thread_id,
        "kind": task.kind.as_str(),
        "command": task.command,
        "status": task.status.as_str(),
        "exit_code": task.exit_code,
        "pid": task.pid,
        "stdout_path": task.stdout_ref,
        "stderr_path": task.stderr_ref,
    });
    if let Some(child_thread_id) = &task.child_thread_id {
        value["child_thread_id"] = serde_json::json!(child_thread_id);
    }
    if let Some(result) = &task.result {
        value["result"] = serde_json::json!(result);
    }
    value
}

/// `task_status`: current state of one background task (or all of them when
/// `task_id` is omitted).
pub struct TaskStatusTool {
    registry: Arc<BackgroundTaskRegistry>,
}

impl TaskStatusTool {
    pub fn new(registry: Arc<BackgroundTaskRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl TypedTool for TaskStatusTool {
    type Input = TaskStatusArgs;
    fn name(&self) -> &str {
        "task_status"
    }

    fn description(&self) -> &str {
        "Report the status of a background SHELL task started with \
         `shell background=true` (running/exited/stopped/failed + exit code), \
         or list all background tasks when task_id is omitted. For subagent \
         delegations use subagent_status instead."
    }

    /// Read-only registry query.
    fn is_concurrency_safe(&self, _arguments: &serde_json::Value) -> bool {
        true
    }

    fn describe_operation(
        &self,
        arguments: &serde_json::Value,
    ) -> Option<slab_agent::OperationDescriptor> {
        let task_id = arguments.get("task_id").and_then(serde_json::Value::as_str)?;
        Some(slab_agent::OperationDescriptor::read_only(format!("task_status: {task_id}")))
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        args: TaskStatusArgs,
    ) -> Result<ToolOutput, AgentError> {
        let content = match args.task_id.as_deref() {
            Some(task_id) => {
                let task = self.registry.snapshot(task_id).ok_or_else(|| {
                    AgentError::ToolExecution(format!("unknown background task: {task_id}"))
                })?;
                serde_json::json!({ "task": snapshot_json(&task) })
            }
            None => serde_json::json!({
                "tasks": self.registry.list().iter().map(snapshot_json).collect::<Vec<_>>()
            }),
        };
        Ok(ToolOutput { content: content.to_string(), metadata: None })
    }
}

/// `task_output`: tail of a background task's streamed stdout file.
pub struct TaskOutputTool {
    registry: Arc<BackgroundTaskRegistry>,
}

impl TaskOutputTool {
    pub fn new(registry: Arc<BackgroundTaskRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl TypedTool for TaskOutputTool {
    type Input = TaskOutputArgs;
    fn name(&self) -> &str {
        "task_output"
    }

    fn description(&self) -> &str {
        "Read the TAIL of a background SHELL task's stdout log (default 16KB, \
         max 256KB). Returns the text plus the total log size so far. For a \
         subagent delegation's result use subagent_status."
    }

    /// Read-only file tail.
    fn is_concurrency_safe(&self, _arguments: &serde_json::Value) -> bool {
        true
    }

    fn describe_operation(
        &self,
        arguments: &serde_json::Value,
    ) -> Option<slab_agent::OperationDescriptor> {
        let task_id = arguments.get("task_id").and_then(serde_json::Value::as_str)?;
        Some(slab_agent::OperationDescriptor::read_only(format!("task_output: {task_id}")))
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        args: TaskOutputArgs,
    ) -> Result<ToolOutput, AgentError> {
        let tail_bytes = args.tail_bytes.map(|v| v as usize);
        let (output, total_bytes) = self.registry.read_output(&args.task_id, tail_bytes).await?;
        Ok(ToolOutput {
            content: serde_json::json!({
                "task_id": args.task_id,
                "output": output,
                "total_bytes": total_bytes,
            })
            .to_string(),
            metadata: None,
        })
    }
}

/// `task_stop`: stop a running background task (tree kill).
pub struct TaskStopTool {
    registry: Arc<BackgroundTaskRegistry>,
}

impl TaskStopTool {
    pub fn new(registry: Arc<BackgroundTaskRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl TypedTool for TaskStopTool {
    type Input = TaskStopArgs;
    fn name(&self) -> &str {
        "task_stop"
    }

    fn description(&self) -> &str {
        "Stop a background SHELL task started with `shell background=true`: \
         kills its whole process tree and reports the resulting status. To \
         cancel a subagent delegation use subagent_stop."
    }

    /// Touches only the task's own process tree — no shared workspace state.
    fn is_concurrency_safe(&self, _arguments: &serde_json::Value) -> bool {
        true
    }

    fn describe_operation(
        &self,
        arguments: &serde_json::Value,
    ) -> Option<slab_agent::OperationDescriptor> {
        let task_id = arguments.get("task_id").and_then(serde_json::Value::as_str)?;
        Some(slab_agent::OperationDescriptor::shell(format!("task_stop: {task_id}")))
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        args: TaskStopArgs,
    ) -> Result<ToolOutput, AgentError> {
        let task = self.registry.stop(&args.task_id)?;
        Ok(ToolOutput {
            content: serde_json::json!({ "stopped": snapshot_json(&task) }).to_string(),
            metadata: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: after a restart the counter restarts at 1, so ids must
    /// carry a per-registry nonce — otherwise a new task would CREATE/TRUNCATE
    /// a pre-restart task's log directory in the same thread.
    #[test]
    fn task_ids_carry_a_per_registry_nonce_and_increment() {
        let registry = BackgroundTaskRegistry::default();
        let first = registry.alloc_task_id();
        let second = registry.alloc_task_id();

        // `bg-<nonce>-<n>` shape with a shared nonce and incrementing counter.
        let nonce = first
            .strip_prefix("bg-")
            .and_then(|rest| rest.rsplit_once('-'))
            .map(|(nonce, _)| nonce)
            .expect("nonce segment");
        assert_eq!(second, format!("bg-{nonce}-2"), "{first} vs {second}");

        // A fresh registry (server restart) never reuses the id space.
        assert_ne!(first, BackgroundTaskRegistry::default().alloc_task_id());
    }

    // ---- detached (subagent) task support ----

    #[derive(Default)]
    struct RecordingSink {
        events: std::sync::Mutex<Vec<BackgroundTaskEvent>>,
    }

    impl BackgroundTaskEventSink for RecordingSink {
        fn on_task_event(&self, event: BackgroundTaskEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    /// A controllable detached wait: resolves when the sender fires (or to
    /// Failed if the sender is dropped).
    fn pending_outcome() -> (DetachedWait, tokio::sync::oneshot::Sender<DetachedTaskOutcome>) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let wait: DetachedWait = Box::pin(async move {
            rx.await.unwrap_or(DetachedTaskOutcome::Status {
                status: BackgroundTaskStatus::Failed,
                result: Some("wait channel dropped".to_owned()),
            })
        });
        (wait, tx)
    }

    fn register_pending_subagent(
        registry: &Arc<BackgroundTaskRegistry>,
        thread_id: &str,
        child_thread_id: &str,
    ) -> (String, tokio::sync::oneshot::Sender<DetachedTaskOutcome>) {
        let (wait, tx) = pending_outcome();
        let task_id = registry.alloc_task_id();
        registry
            .register_detached(
                task_id.clone(),
                DetachedTask {
                    thread_id: thread_id.to_owned(),
                    command: format!("delegated task for {child_thread_id}"),
                    workspace_root: None,
                    child_thread_id: Some(child_thread_id.to_owned()),
                },
                wait,
                Box::new(|| {}),
                Box::new(|_| {}),
            )
            .expect("register detached");
        (task_id, tx)
    }

    /// Poll until `check` holds (watcher tasks are asynchronous).
    async fn wait_for_condition(mut check: impl FnMut() -> bool) {
        for _ in 0..400 {
            if check() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        panic!("condition not met within deadline");
    }

    #[tokio::test]
    async fn register_detached_lifecycle_completes_and_emits() {
        let sink = Arc::new(RecordingSink::default());
        let registry = Arc::new(BackgroundTaskRegistry::new(Some(sink.clone())));
        let (task_id, tx) = register_pending_subagent(&registry, "parent", "child-1");

        let running = registry.snapshot(&task_id).expect("running snapshot");
        assert_eq!(running.status, BackgroundTaskStatus::Running);
        assert_eq!(running.kind, TaskKind::Subagent);
        assert_eq!(running.child_thread_id.as_deref(), Some("child-1"));
        assert_eq!(running.result, None);

        tx.send(DetachedTaskOutcome::Status {
            status: BackgroundTaskStatus::Completed,
            result: Some("child result".to_owned()),
        })
        .expect("send outcome");
        wait_for_condition(|| registry.snapshot(&task_id).is_some_and(|s| s.status.is_terminal()))
            .await;

        let final_snapshot = registry.snapshot(&task_id).expect("final snapshot");
        assert_eq!(final_snapshot.status, BackgroundTaskStatus::Completed);
        assert_eq!(final_snapshot.result.as_deref(), Some("child result"));

        let events = sink.events.lock().unwrap().clone();
        let terminal =
            events.iter().find(|event| event.task_id == task_id).expect("terminal event");
        assert_eq!(terminal.kind, TaskKind::Subagent);
        assert_eq!(terminal.status, BackgroundTaskStatus::Completed);
        assert_eq!(terminal.result_summary.as_deref(), Some("child result"));
    }

    #[tokio::test]
    async fn stop_wins_over_natural_completion_for_detached() {
        let registry = Arc::new(BackgroundTaskRegistry::default());
        let fired = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fired_flag = Arc::clone(&fired);

        let (wait, tx) = pending_outcome();
        let task_id = registry.alloc_task_id();
        registry
            .register_detached(
                task_id.clone(),
                DetachedTask {
                    thread_id: "parent".to_owned(),
                    command: "task".to_owned(),
                    workspace_root: None,
                    child_thread_id: Some("child-1".to_owned()),
                },
                wait,
                Box::new(|| {}),
                Box::new(move |_| {
                    fired_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                }),
            )
            .expect("register");

        let stopped = registry.stop(&task_id).expect("stop");
        assert_eq!(stopped.status, BackgroundTaskStatus::Stopped);

        // The killed work's late completion must not overwrite the Stop.
        tx.send(DetachedTaskOutcome::Status {
            status: BackgroundTaskStatus::Completed,
            result: Some("late".to_owned()),
        })
        .expect("send late outcome");
        wait_for_condition(|| fired.load(std::sync::atomic::Ordering::SeqCst)).await;
        let final_snapshot = registry.snapshot(&task_id).expect("final snapshot");
        assert_eq!(final_snapshot.status, BackgroundTaskStatus::Stopped);
        // The on_terminal callback still observed the terminal transition.
        assert_eq!(final_snapshot.result, None);
    }

    #[tokio::test]
    async fn stop_subagent_tasks_for_thread_scopes_by_owner() {
        let registry = Arc::new(BackgroundTaskRegistry::default());
        let kill_fired = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let register = |owner: &str, child: &str| {
            let (wait, _tx) = pending_outcome();
            let task_id = registry.alloc_task_id();
            let counter = Arc::clone(&kill_fired);
            registry
                .register_detached(
                    task_id.clone(),
                    DetachedTask {
                        thread_id: owner.to_owned(),
                        command: format!("task {child}"),
                        workspace_root: None,
                        child_thread_id: Some(child.to_owned()),
                    },
                    wait,
                    Box::new(move || {
                        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }),
                    Box::new(|_| {}),
                )
                .expect("register");
            task_id
        };
        let a1 = register("owner-a", "a1");
        let a2 = register("owner-a", "a2");
        let b1 = register("owner-b", "b1");

        let stopped = registry.stop_subagent_tasks_for_thread("owner-a");
        assert_eq!(stopped.len(), 2, "only owner-a tasks stop");
        assert_eq!(kill_fired.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(registry.snapshot(&a1).unwrap().status, BackgroundTaskStatus::Stopped);
        assert_eq!(registry.snapshot(&a2).unwrap().status, BackgroundTaskStatus::Stopped);
        assert_eq!(registry.snapshot(&b1).unwrap().status, BackgroundTaskStatus::Running);
    }

    #[tokio::test]
    async fn running_capacity_is_shared_across_kinds() {
        let registry = Arc::new(BackgroundTaskRegistry::default());
        for n in 0..MAX_RUNNING_TASKS {
            let (_task_id, tx) =
                register_pending_subagent(&registry, "parent", &format!("child-{n}"));
            std::mem::forget(tx);
        }

        let (wait, _tx) = pending_outcome();
        let overflow = registry.alloc_task_id();
        let error = registry
            .register_detached(
                overflow,
                DetachedTask {
                    thread_id: "parent".to_owned(),
                    command: "one too many".to_owned(),
                    workspace_root: None,
                    child_thread_id: None,
                },
                wait,
                Box::new(|| {}),
                Box::new(|_| {}),
            )
            .expect_err("capacity enforced");
        assert!(error.to_string().contains("limit reached"), "{error}");
    }

    #[tokio::test]
    async fn detached_result_reads_back_through_read_output() {
        let registry = Arc::new(BackgroundTaskRegistry::default());
        let (task_id, tx) = register_pending_subagent(&registry, "parent", "child-1");
        tx.send(DetachedTaskOutcome::Status {
            status: BackgroundTaskStatus::Completed,
            result: Some("child result".to_owned()),
        })
        .expect("send outcome");
        wait_for_condition(|| registry.snapshot(&task_id).is_some_and(|s| s.status.is_terminal()))
            .await;

        let (text, total) = registry.read_output(&task_id, None).await.expect("read result");
        assert_eq!(text, "child result");
        assert_eq!(total, "child result".len() as u64);
    }
}
