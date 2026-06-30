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

/// Contextual information available to a tool handler during execution.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct ToolContextBuilder {
    thread_id: String,
    turn_index: u32,
    depth: u32,
    workspace: Option<WorkspaceRef>,
    plan: Option<PlanRef>,
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

    pub fn build(self) -> ToolContext {
        ToolContext {
            thread_id: self.thread_id,
            turn_index: self.turn_index,
            depth: self.depth,
            workspace: self.workspace,
            plan: self.plan,
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

/// Metadata returned by tools that require host approval before execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolApprovalRequest {
    pub command: String,
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

    /// Return approval metadata when this invocation requires host review.
    fn approval_request(&self, _arguments: &serde_json::Value) -> Option<ToolApprovalRequest> {
        None
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
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}
