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
}

impl BackgroundTaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Exited => "exited",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }
}

/// A task lifecycle event, delivered through [`BackgroundTaskEventSink`] (the
/// host bridges these to the harness `EventMsg` stream).
#[derive(Debug, Clone)]
pub struct BackgroundTaskEvent {
    pub task_id: String,
    pub thread_id: String,
    pub status: BackgroundTaskStatus,
    pub exit_code: Option<i32>,
    pub pid: Option<u32>,
    pub command: Option<String>,
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
    pub command: String,
    pub status: BackgroundTaskStatus,
    pub exit_code: Option<i32>,
    pub pid: Option<u32>,
    /// Forward-slash workspace-relative reference when a workspace is bound,
    /// else the absolute path.
    pub stdout_ref: String,
    pub stderr_ref: String,
}

struct TaskSlot {
    thread_id: String,
    command: String,
    pid: Option<u32>,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    stdout_ref: String,
    stderr_ref: String,
    status: Mutex<BackgroundTaskStatus>,
    exit_code: Mutex<Option<i32>>,
    /// Fires the tree kill; consumed exactly once.
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
            command: command.clone(),
            pid: child.pid,
            stdout_ref: Self::display_ref(workspace_root, &stdout_path),
            stderr_ref: Self::display_ref(workspace_root, &stderr_path),
            stdout_path,
            stderr_path,
            status: Mutex::new(BackgroundTaskStatus::Running),
            exit_code: Mutex::new(None),
            kill: Mutex::new(child.kill_tree),
        };

        {
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
                    "background task limit reached ({MAX_RUNNING_TASKS} running); stop a task with task_stop first"
                )));
            }
            tasks.insert(task_id.clone(), slot);
            Self::prune_terminal(&mut tasks);
        }

        // Exit watcher: records the exit code, flips the status (preserving
        // an explicit Stop), and emits the terminal event. Detached from the
        // turn — dropping the spawning future does not affect it.
        let registry = Arc::clone(self);
        let watch_task_id = task_id.clone();
        tokio::spawn(async move {
            let code = child.wait.await.ok();
            registry.finish(&watch_task_id, code);
        });

        self.snapshot(&task_id)
            .ok_or_else(|| AgentError::Internal("task vanished on register".into()))
    }

    /// Record a task's terminal state (exit watcher callback).
    fn finish(&self, task_id: &str, exit_code: Option<i32>) {
        let event = {
            let tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
            let Some(slot) = tasks.get(task_id) else { return };
            let mut status = slot.status.lock().unwrap_or_else(|p| p.into_inner());
            let terminal = match *status {
                BackgroundTaskStatus::Stopped => BackgroundTaskStatus::Stopped,
                _ => match exit_code {
                    Some(0) => BackgroundTaskStatus::Exited,
                    Some(_) => BackgroundTaskStatus::Failed,
                    None => BackgroundTaskStatus::Failed,
                },
            };
            *status = terminal;
            *slot.exit_code.lock().unwrap_or_else(|p| p.into_inner()) = exit_code;
            // The kill closure is moot once the process ended.
            *slot.kill.lock().unwrap_or_else(|p| p.into_inner()) = None;
            BackgroundTaskEvent {
                task_id: task_id.to_owned(),
                thread_id: slot.thread_id.clone(),
                status: terminal,
                exit_code,
                pid: slot.pid,
                command: Some(slot.command.clone()),
            }
        };
        self.emit(event);
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
        Some(BackgroundTaskSnapshot {
            task_id: task_id.to_owned(),
            thread_id: slot.thread_id.clone(),
            command: slot.command.clone(),
            status: *slot.status.lock().unwrap_or_else(|p| p.into_inner()),
            exit_code: *slot.exit_code.lock().unwrap_or_else(|p| p.into_inner()),
            pid: slot.pid,
            stdout_ref: slot.stdout_ref.clone(),
            stderr_ref: slot.stderr_ref.clone(),
        })
    }

    /// Snapshots of all retained tasks (running first, then newest terminal).
    pub fn list(&self) -> Vec<BackgroundTaskSnapshot> {
        let tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
        let mut snapshots: Vec<BackgroundTaskSnapshot> = tasks
            .keys()
            .filter_map(|id| {
                let slot = tasks.get(id)?;
                Some(BackgroundTaskSnapshot {
                    task_id: id.clone(),
                    thread_id: slot.thread_id.clone(),
                    command: slot.command.clone(),
                    status: *slot.status.lock().unwrap_or_else(|p| p.into_inner()),
                    exit_code: *slot.exit_code.lock().unwrap_or_else(|p| p.into_inner()),
                    pid: slot.pid,
                    stdout_ref: slot.stdout_ref.clone(),
                    stderr_ref: slot.stderr_ref.clone(),
                })
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
            if current != BackgroundTaskStatus::Running {
                return Ok(BackgroundTaskSnapshot {
                    task_id: task_id.to_owned(),
                    thread_id: slot.thread_id.clone(),
                    command: slot.command.clone(),
                    status: current,
                    exit_code,
                    pid: slot.pid,
                    stdout_ref: slot.stdout_ref.clone(),
                    stderr_ref: slot.stderr_ref.clone(),
                });
            }
            // Take the kill closure now; fire it AFTER the locks release (a
            // kill closure must never run while the registry is locked).
            let kill = slot.kill.lock().unwrap_or_else(|p| p.into_inner()).take();
            *slot.status.lock().unwrap_or_else(|p| p.into_inner()) = BackgroundTaskStatus::Stopped;
            let event = BackgroundTaskEvent {
                task_id: task_id.to_owned(),
                thread_id: slot.thread_id.clone(),
                status: BackgroundTaskStatus::Stopped,
                exit_code: None,
                pid: slot.pid,
                command: Some(slot.command.clone()),
            };
            (kill, event)
        };
        if let Some(kill) = kill {
            kill();
        }
        self.emit(event);
        self.snapshot(task_id).ok_or_else(|| AgentError::Internal("task vanished on stop".into()))
    }

    /// Stop every RUNNING task whose output lives under `root`'s workspace
    /// (workspace migration: no "ghost" tasks carry into the new workspace).
    /// Returns the stopped task ids.
    pub fn stop_all_for_workspace(&self, root: &Path) -> Vec<String> {
        // Mark + take the kill closures under the lock; fire after release
        // (a kill closure must never run while the registry is locked).
        let kills: Vec<(String, Box<dyn FnOnce() + Send + 'static>)> = {
            let mut tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
            let mut taken = Vec::new();
            for (id, slot) in tasks.iter_mut() {
                let mut status = slot.status.lock().unwrap_or_else(|p| p.into_inner());
                if *status == BackgroundTaskStatus::Running
                    && (slot.stdout_path.starts_with(root) || slot.stderr_path.starts_with(root))
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
    /// unbounded record).
    pub async fn read_output(
        &self,
        task_id: &str,
        tail_bytes: Option<usize>,
    ) -> Result<(String, u64), AgentError> {
        // Block-scoped: the sync MutexGuard must not live across the await.
        let path = {
            let tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
            let slot = tasks.get(task_id).ok_or_else(|| {
                AgentError::ToolExecution(format!("unknown background task: {task_id}"))
            })?;
            slot.stdout_path.clone()
        };

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
use slab_agent::{ToolContext, ToolHandler, ToolOutput};

fn snapshot_json(task: &BackgroundTaskSnapshot) -> serde_json::Value {
    serde_json::json!({
        "task_id": task.task_id,
        "thread_id": task.thread_id,
        "command": task.command,
        "status": task.status.as_str(),
        "exit_code": task.exit_code,
        "pid": task.pid,
        "stdout_path": task.stdout_ref,
        "stderr_path": task.stderr_ref,
    })
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
impl ToolHandler for TaskStatusTool {
    fn name(&self) -> &str {
        "task_status"
    }

    fn description(&self) -> &str {
        "Report the status of a background task started with \
         `shell background=true` (running/exited/stopped/failed + exit code), \
         or list all background tasks when task_id is omitted."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "Optional task id from the background spawn; omit to list all tasks."
                }
            }
        })
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
        arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        let content = match arguments.get("task_id").and_then(serde_json::Value::as_str) {
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
impl ToolHandler for TaskOutputTool {
    fn name(&self) -> &str {
        "task_output"
    }

    fn description(&self) -> &str {
        "Read the TAIL of a background task's stdout log (default 16KB, max \
         256KB). Returns the text plus the total log size so far."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string", "description": "Task id from the background spawn." },
                "tail_bytes": {
                    "type": "integer",
                    "description": "How many trailing bytes to read (default 16384, max 262144).",
                    "default": DEFAULT_OUTPUT_TAIL_BYTES,
                    "maximum": MAX_OUTPUT_TAIL_BYTES
                }
            },
            "required": ["task_id"]
        })
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
        arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        let task_id = arguments
            .get("task_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'task_id' argument".into()))?;
        let tail_bytes =
            arguments.get("tail_bytes").and_then(serde_json::Value::as_u64).map(|v| v as usize);
        let (output, total_bytes) = self.registry.read_output(task_id, tail_bytes).await?;
        Ok(ToolOutput {
            content: serde_json::json!({
                "task_id": task_id,
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
impl ToolHandler for TaskStopTool {
    fn name(&self) -> &str {
        "task_stop"
    }

    fn description(&self) -> &str {
        "Stop a background task started with `shell background=true`: kills \
         its whole process tree and reports the resulting status."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string", "description": "Task id from the background spawn." }
            },
            "required": ["task_id"]
        })
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
        arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError> {
        let task_id = arguments
            .get("task_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'task_id' argument".into()))?;
        let task = self.registry.stop(task_id)?;
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
}
