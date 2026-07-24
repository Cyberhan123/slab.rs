//! Tool handler trait and router registry.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;

use crate::error::AgentError;
use crate::port::ToolSpec;

// ── Context & output types ───────────────────────────────────────────────────

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
    /// Durable plan scope associated with the thread, when plan-aware tools need it.
    pub plan: Option<PlanRef>,
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
            .field("plan", &self.plan)
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
            plan: None,
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
    plan: Option<PlanRef>,
    output: Option<Arc<dyn ToolOutputObserver>>,
}

impl std::fmt::Debug for ToolContextBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContextBuilder")
            .field("thread_id", &self.thread_id)
            .field("turn_index", &self.turn_index)
            .field("depth", &self.depth)
            .field("workspace", &self.workspace)
            .field("plan", &self.plan)
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

    pub fn plan(mut self, plan: PlanRef) -> Self {
        self.plan = Some(plan);
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
            plan: self.plan,
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

// ── ToolHandler trait ────────────────────────────────────────────────────────

/// An individual tool that can be invoked by an agent.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Canonical tool name, matched against LLM tool-call names.
    fn name(&self) -> &str;

    /// Human-readable description shown to the model in the tool list.
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's parameter object.
    fn parameters_schema(&self) -> serde_json::Value;

    /// Describe the operation this invocation performs, for the unified policy
    /// engine. Returning `None` (the default) lets the kernel infer the
    /// category from the tool name. Tools that carry a meaningful subject
    /// (command / path / query) should override this.
    fn describe_operation(
        &self,
        _arguments: &serde_json::Value,
    ) -> Option<slab_exec_policy::OperationDescriptor> {
        None
    }

    /// Coarse operation category used for *progressive tool exposure*: tools
    /// whose category the current permission behavior does not permit are
    /// hidden from the LLM's tool list (e.g. shell / file-write / network tools
    /// in read-only mode). Defaults to [`OperationCategory::ReadOnly`]; mutating
    /// tools override this to match their [`ToolHandler::describe_operation`]
    /// category (`Shell` / `FileEdit` / `Network`).
    fn category(&self) -> slab_exec_policy::OperationCategory {
        slab_exec_policy::OperationCategory::ReadOnly
    }

    /// Execute the tool with the given parsed arguments.
    async fn execute(
        &self,
        ctx: &ToolContext,
        arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError>;
}

// ── ToolRouter ───────────────────────────────────────────────────────────────

/// Registry of available tools for a given agent thread.
#[derive(Clone)]
pub struct ToolRouter {
    handlers: Arc<RwLock<HashMap<String, Arc<dyn ToolHandler>>>>,
}

impl ToolRouter {
    /// Create an empty router.
    pub fn new() -> Self {
        Self { handlers: Arc::new(RwLock::new(HashMap::new())) }
    }

    /// Register a tool handler.  A handler with the same name replaces any
    /// previously registered handler.
    pub fn register(&self, handler: Box<dyn ToolHandler>) {
        let handler: Arc<dyn ToolHandler> = handler.into();
        self.handlers
            .write()
            .expect("tool registry lock poisoned")
            .insert(handler.name().to_owned(), handler);
    }

    /// Remove a registered tool handler by name.
    pub fn unregister(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        self.handlers.write().expect("tool registry lock poisoned").remove(name)
    }

    /// Look up a handler by tool name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        self.handlers.read().expect("tool registry lock poisoned").get(name).cloned()
    }

    /// Return [`ToolSpec`] descriptors for all registered tools.
    pub fn tool_specs(&self) -> Vec<ToolSpec> {
        self.handlers
            .read()
            .expect("tool registry lock poisoned")
            .values()
            .map(|h| ToolSpec {
                name: h.name().to_owned(),
                description: h.description().to_owned(),
                parameters_schema: h.parameters_schema(),
            })
            .collect()
    }

    /// Map every registered tool name to its exposure category. Used by the
    /// turn loop to filter the tool list by the current permission behavior
    /// without leaking categories onto the LLM-facing [`ToolSpec`].
    pub fn categories(&self) -> HashMap<String, slab_exec_policy::OperationCategory> {
        self.handlers
            .read()
            .expect("tool registry lock poisoned")
            .iter()
            .map(|(name, handler)| (name.clone(), handler.category()))
            .collect()
    }
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}
