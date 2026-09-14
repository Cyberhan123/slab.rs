//! Tool execution context, output types, and host-provided scopes.

use std::{path::PathBuf, sync::Arc};

use crate::port::{NoopPlanStore, PlanStorePort};

/// Which process stream a tool output delta came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolOutputStream {
    Stdout,
    Stderr,
}

/// Receiver for incremental tool output (e.g. live shell stdout/stderr). The
/// agent forwards each delta to the harness display while the tool runs; the
/// tool still returns its finalized result via [`ToolOutput`]. Default `None` —
/// tools opt in by reading [`ToolContext::output`].
pub trait ToolOutputObserver: Send + Sync {
    fn on_output(&self, stream: ToolOutputStream, delta: &str);
}

/// Contextual information available to a tool handler during execution.
#[derive(Clone)]
pub struct ToolContext {
    /// ID of the agent thread invoking the tool.
    pub thread_id: String,
    /// Zero-based index of the current LLM turn within the thread.
    pub turn_index: u32,
    /// Nesting depth of the agent thread (0 = root).
    pub depth: u32,
    /// Workspace scope associated with the thread, when the host provided one.
    pub workspace: Option<WorkspaceRef>,
    /// Delegated workspace boundary (from `AgentConfig::workspace_scope`),
    /// when this thread is a scoped child. Enforced by the file tools.
    pub workspace_scope: Option<WorkspaceScopeRef>,
    /// Durable plan scope associated with the thread, when plan-aware tools need it.
    pub plan: Option<PlanRef>,
    /// Per-thread plan store backing Plan interaction mode. Defaults to a no-op
    /// store; the agent wires the host-provided store per call so the `plan` /
    /// `update_plan` / `present_plan` tools can read and persist the durable plan.
    pub plan_store: Arc<dyn PlanStorePort>,
    /// Optional live-output observer. Set per-call by the agent for tools that
    /// stream output (e.g. `shell`); `None` by default.
    pub output: Option<Arc<dyn ToolOutputObserver>>,
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("thread_id", &self.thread_id)
            .field("turn_index", &self.turn_index)
            .field("depth", &self.depth)
            .field("workspace", &self.workspace)
            .field("workspace_scope", &self.workspace_scope)
            .field("plan", &self.plan)
            .field("plan_store", &"<port>")
            .field("output", &self.output.as_ref().map(|_| "<observer>"))
            .finish()
    }
}

impl ToolContext {
    /// Start building a tool context for the given thread.
    pub fn for_thread(thread_id: impl Into<String>) -> ToolContextBuilder {
        ToolContextBuilder {
            thread_id: thread_id.into(),
            turn_index: 0,
            depth: 0,
            workspace: None,
            workspace_scope: None,
            plan: None,
            plan_store: Arc::new(NoopPlanStore),
            output: None,
        }
    }
}

/// Host-provided scope applied to tools executed by an agent thread.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentThreadContext {
    /// Workspace scope inherited by tools, when the host has a workspace root.
    pub workspace: Option<WorkspaceRef>,
    /// Optional durable plan identifier. The concrete [`PlanRef`] is resolved per thread.
    pub plan_id: Option<String>,
    /// Offline degradation flag (INFRA-07): when true the agent's tool list is
    /// narrowed to drop tools that need external network/provider reachability
    /// (`web_search`, `mcp_call`, `mcp_list_tools`, `mcp__*`). Set by the host
    /// after probing provider reachability.
    pub offline: bool,
}

impl AgentThreadContext {
    /// Create an empty thread context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach workspace scope to the context.
    pub fn with_workspace(mut self, workspace: WorkspaceRef) -> Self {
        self.workspace = Some(workspace);
        self
    }

    /// Attach a durable plan identifier to the context.
    pub fn with_plan_id(mut self, plan_id: impl Into<String>) -> Self {
        let plan_id = plan_id.into();
        if !plan_id.trim().is_empty() {
            self.plan_id = Some(plan_id);
        }
        self
    }

    /// Mark the thread as running in offline mode (INFRA-07).
    pub fn with_offline(mut self, offline: bool) -> Self {
        self.offline = offline;
        self
    }
}

/// Workspace identity made available to workspace-scoped tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRef {
    /// Canonical or host-resolved workspace root.
    pub root: PathBuf,
    /// Optional session scope associated with this workspace.
    pub session_id: Option<String>,
}

/// Delegated workspace boundary for a child agent thread, set from
/// [`crate::AgentConfig::workspace_scope`] by the kernel. Transport only:
/// the file tools in `slab-agent-tools` enforce it. NOT a security boundary
/// for shell/verify (cwd pinned to the workspace root), git tools, or
/// MCP/plugin tools; a scoped parent spawning an unscoped child also escapes
/// it — nesting depth is bounded by `max_depth` instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceScopeRef {
    /// Canonical workspace-relative root of the delegated scope (resolved via
    /// `slab_utils::fs::canonicalize_with_existing_ancestor`, so the scope
    /// directory may not exist yet).
    pub root: PathBuf,
    /// Original workspace-relative path, kept for error messages and telemetry.
    pub relative: String,
}

/// Reference to durable plan state for plan-aware tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRef {
    /// Thread that owns the current plan.
    pub thread_id: String,
    /// Optional host-defined plan identifier.
    pub plan_id: Option<String>,
}

/// Builder for [`ToolContext`].
#[derive(Clone)]
pub struct ToolContextBuilder {
    thread_id: String,
    turn_index: u32,
    depth: u32,
    workspace: Option<WorkspaceRef>,
    workspace_scope: Option<WorkspaceScopeRef>,
    plan: Option<PlanRef>,
    plan_store: Arc<dyn PlanStorePort>,
    output: Option<Arc<dyn ToolOutputObserver>>,
}

impl std::fmt::Debug for ToolContextBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContextBuilder")
            .field("thread_id", &self.thread_id)
            .field("turn_index", &self.turn_index)
            .field("depth", &self.depth)
            .field("workspace", &self.workspace)
            .field("workspace_scope", &self.workspace_scope)
            .field("plan", &self.plan)
            .field("plan_store", &"<port>")
            .field("output", &self.output.as_ref().map(|_| "<observer>"))
            .finish()
    }
}

impl ToolContextBuilder {
    pub fn turn_index(mut self, turn_index: u32) -> Self {
        self.turn_index = turn_index;
        self
    }

    pub fn depth(mut self, depth: u32) -> Self {
        self.depth = depth;
        self
    }

    pub fn workspace(mut self, workspace: WorkspaceRef) -> Self {
        self.workspace = Some(workspace);
        self
    }

    /// Attach the delegated workspace boundary (from a scoped child config).
    pub fn workspace_scope(mut self, scope: WorkspaceScopeRef) -> Self {
        self.workspace_scope = Some(scope);
        self
    }

    pub fn plan(mut self, plan: PlanRef) -> Self {
        self.plan = Some(plan);
        self
    }

    /// Attach the host-provided plan store (wired by the agent per call so the
    /// plan tools can persist/query the durable plan). Defaults to a no-op store.
    pub fn plan_store(mut self, plan_store: Arc<dyn PlanStorePort>) -> Self {
        self.plan_store = plan_store;
        self
    }

    /// Attach a live-output observer (used by streaming tools like `shell`).
    pub fn output(mut self, output: Arc<dyn ToolOutputObserver>) -> Self {
        self.output = Some(output);
        self
    }

    pub fn build(self) -> ToolContext {
        ToolContext {
            thread_id: self.thread_id,
            turn_index: self.turn_index,
            depth: self.depth,
            workspace: self.workspace,
            workspace_scope: self.workspace_scope,
            plan: self.plan,
            plan_store: self.plan_store,
            output: self.output,
        }
    }
}

/// The result produced by a tool handler.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    /// Plain-text (or JSON) content that will be fed back to the LLM.
    pub content: String,
    /// Optional structured metadata for logging / observability.
    pub metadata: Option<serde_json::Value>,
}

/// Metadata returned by the policy engine when an invocation requires host
/// approval before execution.
///
/// `descriptor` carries the operation category + subject (so the approval UI
/// can render category-appropriate choices and the engine can persist a rule);
/// `display` is the human-readable summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolApprovalRequest {
    pub descriptor: slab_exec_policy::OperationDescriptor,
    pub display: String,
}
