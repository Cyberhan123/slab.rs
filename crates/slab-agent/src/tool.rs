//! Tool handler trait and router registry.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;

use crate::error::AgentError;
use crate::port::{NoopPlanStore, ParsedToolCall, PlanStorePort, ToolSpec};
use crate::protocol::TurnItem;

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

// ── Tool capability metadata ─────────────────────────────────────────────────

/// When/how a tool appears in the model-facing tool list. Orthogonal to the
/// category-based exposure filter driven by permission/interaction mode:
/// visibility governs *whether* a tool is a candidate at all, exposure governs
/// *which categories* are permitted this turn. Together they let the registry
/// scale to many tools (plugins/MCP) without bloating the LLM tool list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolVisibility {
    /// Always a candidate for the model-facing tool list, subject to category
    /// exposure. The default for built-in tools.
    #[default]
    Direct,
    /// Not shown to the model until `tool_search` injects it for the current
    /// turn. The default for plugin/MCP tools — keeps the base tool list small
    /// and the model discovers them on demand.
    Deferred,
    /// Never shown to the model, but still dispatchable via the registry.
    /// Used for internal/helper tools invoked only by other tools.
    Hidden,
}

/// Namespace a tool belongs to (e.g. `builtin`, `mcp:<server>`, `plugin:<id>`).
///
/// Used for namespaced dispatch (the `namespace__name` wire form) and capability
/// metadata. The default for built-in tools is [`ToolNamespace::builtin`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolNamespace(pub String);

impl ToolNamespace {
    /// The namespace all built-in tools belong to.
    pub const BUILTIN: &'static str = "builtin";

    /// Create a namespace from a string.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The `builtin` namespace.
    pub fn builtin() -> Self {
        Self::new(Self::BUILTIN)
    }

    /// View the namespace string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ToolNamespace {
    fn default() -> Self {
        Self::builtin()
    }
}

/// A structured tool identity: a [`ToolNamespace`] plus a name within it.
///
/// The canonical wire form is `namespace__name` for namespaced tools and a bare
/// `name` for built-in tools (e.g. `shell`, `write_file`). MCP proxy names like
/// `mcp__server__tool` parse to namespace `mcp` / name `server__tool` and
/// round-trip losslessly via [`ToolName::to_wire`].
///
/// `ToolName` is a parse/classify helper only — tool structs keep returning a
/// cached wire-form `&str` from [`ToolHandler::name`] (the trait requires `&str`,
/// not `String`); `ToolName` is used where structured namespace reasoning is
/// needed (e.g. deciding whether a name is built-in vs. namespaced).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolName {
    /// Namespace the tool belongs to.
    pub namespace: ToolNamespace,
    /// Name within the namespace. For namespaced tools this may itself contain
    /// `__` (e.g. the `server__tool` part of an MCP proxy name).
    pub name: String,
}

impl ToolName {
    /// Create a namespaced tool name.
    pub fn new(namespace: ToolNamespace, name: impl Into<String>) -> Self {
        Self { namespace, name: name.into() }
    }

    /// Create a built-in tool name (namespace = `builtin`).
    pub fn builtin(name: impl Into<String>) -> Self {
        Self { namespace: ToolNamespace::builtin(), name: name.into() }
    }

    /// Whether this is a built-in (un-namespaced) tool.
    pub fn is_builtin(&self) -> bool {
        self.namespace.as_str() == ToolNamespace::BUILTIN
    }

    /// Render the canonical wire form. Built-in tools render as their bare name;
    /// namespaced tools render as `namespace__name`.
    pub fn to_wire(&self) -> String {
        if self.is_builtin() {
            self.name.clone()
        } else {
            format!("{}__{}", self.namespace.as_str(), self.name)
        }
    }

    /// Parse a wire-form name. A string with no `__` (or an empty side) is
    /// treated as a built-in bare name; otherwise the first `__` splits
    /// namespace from the (possibly multi-segment) name.
    pub fn parse_wire(value: &str) -> Self {
        match value.split_once("__") {
            Some((ns, name)) if !ns.is_empty() && !name.is_empty() => {
                Self::new(ToolNamespace::new(ns), name)
            }
            _ => Self::builtin(value),
        }
    }
}

/// Static capability metadata for a tool — the single source of truth consumed
/// by tool-exposure filtering, approval/risk routing, and (future) per-agent
/// tool constraints.
///
/// Per-call operation descriptors (which depend on the invocation arguments)
/// stay on [`ToolHandler::describe_operation`]; this struct captures the static
/// metadata declared once per tool and cached by the registry.
#[derive(Debug, Clone)]
pub struct ToolCapability {
    /// Coarse operation category — drives category-based exposure filtering
    /// (read-only / shell / file-edit / network).
    pub category: slab_exec_policy::OperationCategory,
    /// Whether/when the tool appears in the model-facing tool list.
    pub visibility: ToolVisibility,
    /// Namespace the tool belongs to.
    pub namespace: ToolNamespace,
    /// Optional static risk hint. `None` (the default) defers to the runtime
    /// [`ToolRiskAnalyzer`](crate::ToolRiskAnalyzer); tools with a known static
    /// risk may declare it here so the analyzer need not infer it by name.
    pub risk_level: Option<crate::port::ToolRiskLevel>,
}

impl ToolCapability {
    /// Build a capability from a category, defaulting visibility to
    /// [`ToolVisibility::Direct`] and namespace to builtin.
    pub fn new(category: slab_exec_policy::OperationCategory) -> Self {
        Self {
            category,
            visibility: ToolVisibility::Direct,
            namespace: ToolNamespace::builtin(),
            risk_level: None,
        }
    }
}

impl Default for ToolCapability {
    fn default() -> Self {
        Self::new(slab_exec_policy::OperationCategory::ReadOnly)
    }
}

// ── ToolHandler trait ────────────────────────────────────────────────────────

/// Inputs to [`ToolHandler::render_turn_item`]: everything a tool needs to build
/// its harness [`TurnItem`] for a given call. Bundled into a struct so the
/// render method signature stays small (and overriding tools pick out only the
/// fields they care about).
pub struct ToolCallRender<'a> {
    /// The parsed tool call (id + name + raw arguments string).
    pub call: &'a ParsedToolCall,
    /// Parsed arguments object.
    pub args: &'a serde_json::Value,
    /// `"running"` for `ItemStarted`, `"completed"`/`"failed"` for `ItemCompleted`.
    pub status: &'a str,
    /// Tool result text, filled only on completion.
    pub output: Option<&'a str>,
    /// Workspace root for `CommandExecution.cwd`, or `None` when unbound.
    pub workspace_root: Option<&'a str>,
    /// Shell exit code, surfaced only for completed `shell` calls.
    pub exit_code: Option<i64>,
    /// Elapsed milliseconds, surfaced only on completion.
    pub duration_ms: Option<u64>,
}

/// The default [`TurnItem`] for a tool call: a `CommandExecution` whose
/// `command` is the tool name. Every tool call is visible on the harness
/// timeline this way — tools with a richer render override [`ToolHandler::render_turn_item`].
pub fn default_tool_turn_item(r: &ToolCallRender<'_>) -> TurnItem {
    TurnItem::CommandExecution {
        id: r.call.id.clone(),
        command: r.call.name.clone(),
        cwd: String::new(),
        process_id: None,
        status: r.status.to_owned(),
        aggregated_output: r.output.map(str::to_owned),
        exit_code: r.exit_code,
        duration_ms: r.duration_ms,
    }
}

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
    /// in read-only mode). Defaults to [`slab_exec_policy::OperationCategory::ReadOnly`]; mutating
    /// tools override this to match their [`ToolHandler::describe_operation`]
    /// category (`Shell` / `FileEdit` / `Network`).
    fn category(&self) -> slab_exec_policy::OperationCategory {
        slab_exec_policy::OperationCategory::ReadOnly
    }

    /// Whether THIS invocation may run concurrently with other
    /// concurrency-safe invocations in the same assistant tool batch. Pure
    /// read-only tools (read_file / grep / glob / list_dir, read-only git
    /// subcommands, web_search) override to `true`; everything else keeps the
    /// conservative `false` so mutating calls stay strictly serialized. The
    /// dispatch loop partitions a batch into runs of safe calls (executed in
    /// parallel, bounded by `tool_concurrency`) interleaved with serial
    /// single-call batches.
    fn is_concurrency_safe(&self, _arguments: &serde_json::Value) -> bool {
        false
    }

    /// When/how the tool appears in the model-facing tool list. Defaults to
    /// [`ToolVisibility::Direct`] (always a candidate, subject to category
    /// exposure). Plugin/MCP tools override to [`ToolVisibility::Deferred`] so
    /// they stay out of the base list until `tool_search` injects them.
    fn visibility(&self) -> ToolVisibility {
        ToolVisibility::Direct
    }

    /// Namespace the tool belongs to. Defaults to [`ToolNamespace::builtin`];
    /// plugin/MCP proxies override with `mcp:<server>` / `plugin:<id>`.
    fn namespace(&self) -> ToolNamespace {
        ToolNamespace::builtin()
    }

    /// Static capability metadata — the single source of truth for exposure,
    /// approval routing, and (future) agent tool constraints. The default
    /// derives from [`category`](Self::category) + [`visibility`](Self::visibility)
    /// + [`namespace`](Self::namespace); tools may override to add a static
    /// risk hint via [`ToolCapability::risk_level`].
    fn capability(&self) -> ToolCapability {
        ToolCapability {
            category: self.category(),
            visibility: self.visibility(),
            namespace: self.namespace(),
            risk_level: None,
        }
    }

    /// Build the harness [`TurnItem`] for a call to this tool. The default
    /// renders a generic [`TurnItem::CommandExecution`] (via
    /// [`default_tool_turn_item`]) so every tool is visible on the timeline;
    /// tools with a richer representation (shell command, file change, web
    /// search, MCP call, …) override this. Render is purely a view over the
    /// call/result — it must not perform side effects.
    fn render_turn_item(&self, render: &ToolCallRender<'_>) -> TurnItem {
        default_tool_turn_item(render)
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
///
/// Acts as both the dispatch table (name → [`ToolHandler`]) and the source of
/// the model-facing spec projection. It caches each tool's static
/// [`ToolCapability`] (category / visibility / namespace / risk) at
/// registration so the per-turn projection never re-queries handlers. A future
/// refactor may physically split dispatch (`ToolRegistry`) from projection
/// (`ToolSpecProvider`); until then both live here behind a stable facade.
#[derive(Clone)]
pub struct ToolRouter {
    handlers: Arc<RwLock<HashMap<String, Arc<dyn ToolHandler>>>>,
    capabilities: Arc<RwLock<HashMap<String, ToolCapability>>>,
}

impl ToolRouter {
    /// Create an empty router.
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            capabilities: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a tool handler.  A handler with the same name replaces any
    /// previously registered handler. Its [`ToolCapability`] is cached once so
    /// the projection need not re-query the handler each turn.
    pub fn register(&self, handler: Box<dyn ToolHandler>) {
        let handler: Arc<dyn ToolHandler> = handler.into();
        let name = handler.name().to_owned();
        let capability = handler.capability();
        let mut handlers = self.handlers.write().expect("tool registry lock poisoned");
        handlers.insert(name.clone(), handler);
        self.capabilities.write().expect("tool registry lock poisoned").insert(name, capability);
    }

    /// Remove a registered tool handler by name.
    pub fn unregister(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        self.capabilities.write().expect("tool registry lock poisoned").remove(name);
        self.handlers.write().expect("tool registry lock poisoned").remove(name)
    }

    /// Look up a handler by tool name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        self.handlers.read().expect("tool registry lock poisoned").get(name).cloned()
    }

    /// Return [`ToolSpec`] descriptors for all registered tools (regardless of
    /// visibility/exposure). Use [`Self::visible_tool_specs`] for the
    /// model-facing projection.
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

    /// The cached [`ToolCapability`] for a registered tool, if any.
    pub fn capability_of(&self, name: &str) -> Option<ToolCapability> {
        self.capabilities.read().expect("tool registry lock poisoned").get(name).cloned()
    }

    /// Map every registered tool name to its exposure category. Used by the
    /// turn loop to filter the tool list by the current permission behavior
    /// without leaking categories onto the LLM-facing [`ToolSpec`].
    pub fn categories(&self) -> HashMap<String, slab_exec_policy::OperationCategory> {
        self.capabilities
            .read()
            .expect("tool registry lock poisoned")
            .iter()
            .map(|(name, cap)| (name.clone(), cap.category))
            .collect()
    }

    /// Project the model-facing tool list for a turn: applies both the
    /// per-tool [`ToolVisibility`] tri-state and the category-based
    /// `exposure` filter (driven by permission/interaction mode).
    ///
    /// - [`ToolVisibility::Direct`]   → included iff its category is exposed.
    /// - [`ToolVisibility::Deferred`]  → included iff its name is in
    ///   `injected_deferred` (populated by `tool_search`) AND its category is
    ///   exposed. Keeps plugin/MCP tools out of the base list until discovered.
    /// - [`ToolVisibility::Hidden`]    → never included, but still dispatchable
    ///   via [`Self::get`].
    ///
    /// Under `ToolExposure::all()` (FullControl / RequestApproval) every
    /// category passes, so the result is the visibility filter alone.
    pub fn visible_tool_specs(
        &self,
        exposure: slab_exec_policy::ToolExposure,
        injected_deferred: &HashSet<String>,
    ) -> Vec<ToolSpec> {
        let handlers = self.handlers.read().expect("tool registry lock poisoned");
        let caps = self.capabilities.read().expect("tool registry lock poisoned");
        let all_exposed = exposure == slab_exec_policy::ToolExposure::all();
        handlers
            .values()
            .filter_map(|handler| {
                let name = handler.name();
                let visibility =
                    caps.get(name).map(|cap| cap.visibility).unwrap_or(ToolVisibility::Direct);
                let category = caps
                    .get(name)
                    .map(|cap| cap.category)
                    .unwrap_or(slab_exec_policy::OperationCategory::ReadOnly);
                let exposed = all_exposed || exposure.contains(category);
                let include = match visibility {
                    ToolVisibility::Hidden => false,
                    ToolVisibility::Deferred => injected_deferred.contains(name) && exposed,
                    ToolVisibility::Direct => exposed,
                };
                include.then(|| ToolSpec {
                    name: handler.name().to_owned(),
                    description: handler.description().to_owned(),
                    parameters_schema: handler.parameters_schema(),
                })
            })
            .collect()
    }

    /// Specs for every registered [`ToolVisibility::Deferred`] tool, regardless
    /// of category exposure. These are the candidates `tool_search` matches
    /// against; whether a hit becomes callable still depends on exposure (see
    /// [`Self::visible_tool_specs`]).
    pub fn deferred_tool_specs(&self) -> Vec<ToolSpec> {
        let handlers = self.handlers.read().expect("tool registry lock poisoned");
        let caps = self.capabilities.read().expect("tool registry lock poisoned");
        handlers
            .values()
            .filter_map(|handler| {
                let name = handler.name();
                let visibility =
                    caps.get(name).map(|cap| cap.visibility).unwrap_or(ToolVisibility::Direct);
                (visibility == ToolVisibility::Deferred).then(|| ToolSpec {
                    name: handler.name().to_owned(),
                    description: handler.description().to_owned(),
                    parameters_schema: handler.parameters_schema(),
                })
            })
            .collect()
    }
}

/// Per-thread state tracking which `Deferred` tools `tool_search` has injected
/// for the current thread. Lives on the thread runtime (not the process-wide
/// [`ToolRouter`]) so discovery is isolated per thread and cleaned up when the
/// thread ends — no manual `clear` needed.
#[derive(Debug, Default)]
pub struct ToolDiscoveryState {
    injected: std::sync::Mutex<HashSet<String>>,
}

impl ToolDiscoveryState {
    /// Create an empty discovery state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a Deferred tool as injected (visible/callable for subsequent turns
    /// of this thread, subject to category exposure).
    pub fn inject(&self, name: &str) {
        self.injected.lock().expect("tool discovery state lock poisoned").insert(name.to_owned());
    }

    /// Snapshot the set of injected tool names (wire form), for the per-turn
    /// [`ToolRouter::visible_tool_specs`] projection.
    pub fn snapshot(&self) -> HashSet<String> {
        self.injected.lock().expect("tool discovery state lock poisoned").clone()
    }
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal handler stub to exercise the default trait methods.
    struct StubTool;

    #[async_trait]
    impl ToolHandler for StubTool {
        fn name(&self) -> &str {
            "stub"
        }
        fn description(&self) -> &str {
            "stub"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn execute(
            &self,
            _ctx: &ToolContext,
            _arguments: &serde_json::Value,
        ) -> Result<ToolOutput, crate::error::AgentError> {
            Ok(ToolOutput { content: String::new(), metadata: None })
        }
    }

    #[test]
    fn tool_visibility_default_is_direct() {
        assert_eq!(ToolVisibility::default(), ToolVisibility::Direct);
    }

    #[test]
    fn tool_namespace_builtin_and_default() {
        assert_eq!(ToolNamespace::builtin().as_str(), "builtin");
        assert_eq!(ToolNamespace::default().as_str(), "builtin");
        assert_eq!(ToolNamespace::new("mcp:foo").as_str(), "mcp:foo");
        assert_eq!(ToolNamespace(ToolNamespace::BUILTIN.to_owned()).as_str(), "builtin");
    }

    #[test]
    fn tool_name_builtin_to_wire_is_bare() {
        let n = ToolName::builtin("shell");
        assert!(n.is_builtin());
        assert_eq!(n.to_wire(), "shell");
        assert_eq!(n.namespace.as_str(), "builtin");
        assert_eq!(n.name, "shell");
    }

    #[test]
    fn parse_wire_treats_bare_name_as_builtin() {
        assert!(ToolName::parse_wire("write_file").is_builtin());
        assert!(ToolName::parse_wire("task.complete").is_builtin());
        // No "__" at all → builtin.
        assert_eq!(ToolName::parse_wire("grep").to_wire(), "grep");
    }

    #[test]
    fn tool_name_namespaced_round_trip() {
        let n = ToolName::parse_wire("mcp__server__tool");
        assert!(!n.is_builtin());
        assert_eq!(n.namespace.as_str(), "mcp");
        assert_eq!(n.name, "server__tool");
        assert_eq!(n.to_wire(), "mcp__server__tool");
    }

    #[test]
    fn mcp_proxy_name_parses_back() {
        let wire = "mcp__team_server__search_web_v1";
        assert_eq!(ToolName::parse_wire(wire).to_wire(), wire);
        // A single-segment namespaced name round-trips too.
        let single = "plugin__my_tool";
        let parsed = ToolName::parse_wire(single);
        assert_eq!(parsed.namespace.as_str(), "plugin");
        assert_eq!(parsed.name, "my_tool");
        assert_eq!(parsed.to_wire(), single);
    }

    #[test]
    fn tool_capability_new_defaults() {
        let cap = ToolCapability::new(slab_exec_policy::OperationCategory::FileEdit);
        assert_eq!(cap.category, slab_exec_policy::OperationCategory::FileEdit);
        assert_eq!(cap.visibility, ToolVisibility::Direct);
        assert_eq!(cap.namespace.as_str(), "builtin");
        assert_eq!(cap.risk_level, None);
    }

    #[test]
    fn tool_capability_default_is_read_only_direct() {
        let cap = ToolCapability::default();
        assert_eq!(cap.category, slab_exec_policy::OperationCategory::ReadOnly);
        assert_eq!(cap.visibility, ToolVisibility::Direct);
    }

    #[test]
    fn handler_default_capability_derives_from_category_and_visibility() {
        let tool = StubTool;
        // StubTool uses the default category() (ReadOnly) + default visibility (Direct).
        let cap = tool.capability();
        assert_eq!(cap.category, slab_exec_policy::OperationCategory::ReadOnly);
        assert_eq!(cap.visibility, ToolVisibility::Direct);
        assert_eq!(cap.namespace.as_str(), "builtin");
        assert_eq!(cap.risk_level, None);
    }

    #[test]
    fn handler_visibility_and_namespace_defaults() {
        let tool = StubTool;
        assert_eq!(tool.visibility(), ToolVisibility::Direct);
        assert_eq!(tool.namespace().as_str(), "builtin");
    }

    // ── visible_tool_specs projection ──────────────────────────────────────────

    fn noop_output() -> Result<ToolOutput, crate::error::AgentError> {
        Ok(ToolOutput { content: String::new(), metadata: None })
    }

    struct ReadOnlyDirectTool;
    #[async_trait]
    impl ToolHandler for ReadOnlyDirectTool {
        fn name(&self) -> &str {
            "read_direct"
        }
        fn description(&self) -> &str {
            "read direct"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn execute(
            &self,
            _: &ToolContext,
            _: &serde_json::Value,
        ) -> Result<ToolOutput, crate::error::AgentError> {
            noop_output()
        }
    }

    struct ShellDirectTool;
    #[async_trait]
    impl ToolHandler for ShellDirectTool {
        fn name(&self) -> &str {
            "shell_direct"
        }
        fn description(&self) -> &str {
            "shell direct"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        fn category(&self) -> slab_exec_policy::OperationCategory {
            slab_exec_policy::OperationCategory::Shell
        }
        async fn execute(
            &self,
            _: &ToolContext,
            _: &serde_json::Value,
        ) -> Result<ToolOutput, crate::error::AgentError> {
            noop_output()
        }
    }

    struct DeferredTool;
    #[async_trait]
    impl ToolHandler for DeferredTool {
        fn name(&self) -> &str {
            "deferred_read"
        }
        fn description(&self) -> &str {
            "deferred read"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        fn visibility(&self) -> ToolVisibility {
            ToolVisibility::Deferred
        }
        async fn execute(
            &self,
            _: &ToolContext,
            _: &serde_json::Value,
        ) -> Result<ToolOutput, crate::error::AgentError> {
            noop_output()
        }
    }

    struct HiddenTool;
    #[async_trait]
    impl ToolHandler for HiddenTool {
        fn name(&self) -> &str {
            "hidden_helper"
        }
        fn description(&self) -> &str {
            "hidden helper"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        fn visibility(&self) -> ToolVisibility {
            ToolVisibility::Hidden
        }
        async fn execute(
            &self,
            _: &ToolContext,
            _: &serde_json::Value,
        ) -> Result<ToolOutput, crate::error::AgentError> {
            noop_output()
        }
    }

    fn router_with_all_visibilities() -> ToolRouter {
        let router = ToolRouter::new();
        router.register(Box::new(ReadOnlyDirectTool));
        router.register(Box::new(ShellDirectTool));
        router.register(Box::new(DeferredTool));
        router.register(Box::new(HiddenTool));
        router
    }

    fn names(specs: &[ToolSpec]) -> Vec<&str> {
        let mut names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
        names.sort_unstable();
        names
    }

    #[test]
    fn visible_tool_specs_shows_direct_and_hides_hidden_and_deferred_by_default() {
        let router = router_with_all_visibilities();
        // Full exposure: visibility is the only filter.
        let specs =
            router.visible_tool_specs(slab_exec_policy::ToolExposure::all(), &HashSet::new());
        assert_eq!(names(&specs), ["read_direct", "shell_direct"]);
    }

    #[test]
    fn visible_tool_specs_injects_deferred_only_when_named() {
        let router = router_with_all_visibilities();
        let mut injected = HashSet::new();
        injected.insert("deferred_read".to_owned());
        let specs = router.visible_tool_specs(slab_exec_policy::ToolExposure::all(), &injected);
        assert_eq!(names(&specs), ["deferred_read", "read_direct", "shell_direct"]);
    }

    #[test]
    fn visible_tool_specs_respects_category_exposure() {
        let router = router_with_all_visibilities();
        // read-only exposure hides shell_direct (Shell category).
        let specs =
            router.visible_tool_specs(slab_exec_policy::ToolExposure::read_only(), &HashSet::new());
        assert_eq!(names(&specs), ["read_direct"]);
    }

    #[test]
    fn visible_tool_specs_hidden_tool_still_dispatchable_but_never_visible() {
        let router = router_with_all_visibilities();
        assert!(router.get("hidden_helper").is_some());
        let specs =
            router.visible_tool_specs(slab_exec_policy::ToolExposure::all(), &HashSet::new());
        assert!(specs.iter().all(|s| s.name != "hidden_helper"));
    }

    #[test]
    fn deferred_tool_specs_lists_only_deferred_tools() {
        let router = router_with_all_visibilities();
        // Only the Deferred tool surfaces as a search candidate; Direct/Hidden
        // tools never appear here (Direct are already in the base list, Hidden
        // are internal helpers).
        let specs = router.deferred_tool_specs();
        assert_eq!(names(&specs), ["deferred_read"]);
    }

    #[test]
    fn discovery_state_inject_and_snapshot_round_trip() {
        let state = ToolDiscoveryState::new();
        assert!(state.snapshot().is_empty());
        state.inject("mcp__srv__tool");
        state.inject("plugin__p__cap");
        let snap = state.snapshot();
        assert_eq!(snap.len(), 2);
        assert!(snap.contains("mcp__srv__tool"));
        // Two independent states don't share injected sets (per-thread isolation).
        let other = ToolDiscoveryState::new();
        assert!(other.snapshot().is_empty());
    }

    #[test]
    fn capability_of_returns_cached_metadata() {
        let router = router_with_all_visibilities();
        let cap = router.capability_of("deferred_read").expect("cached capability");
        assert_eq!(cap.visibility, ToolVisibility::Deferred);
        assert_eq!(cap.category, slab_exec_policy::OperationCategory::ReadOnly);
        let shell_cap = router.capability_of("shell_direct").expect("cached capability");
        assert_eq!(shell_cap.category, slab_exec_policy::OperationCategory::Shell);
        assert!(router.capability_of("nonexistent").is_none());
    }

    #[test]
    fn unregister_drops_capability_cache() {
        let router = router_with_all_visibilities();
        assert!(router.capability_of("deferred_read").is_some());
        router.unregister("deferred_read");
        assert!(router.capability_of("deferred_read").is_none());
        assert!(router.get("deferred_read").is_none());
    }
}
