//! The tool handler trait.

use async_trait::async_trait;

use crate::error::AgentError;
use crate::port::ParsedToolCall;
use crate::protocol::TurnItem;

use super::capability::{ToolCapability, ToolNamespace, ToolVisibility};
use super::context::{ToolContext, ToolOutput};

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

/// The default [`TurnItem`] for a tool call: a `ToolCall` carrying the tool
/// name, its parsed arguments, and the completion output (as `error` when the
/// call failed). Every tool call is visible on the harness timeline this way —
/// tools with a richer render override [`ToolHandler::render_turn_item`].
pub fn default_tool_turn_item(r: &ToolCallRender<'_>) -> TurnItem {
    let failed = r.status == "failed";
    let outcome = r.output.map(|output| serde_json::Value::String(output.to_owned()));
    TurnItem::ToolCall {
        id: r.call.id.clone(),
        tool: r.call.name.clone(),
        arguments: r.args.clone(),
        status: r.status.to_owned(),
        result: if failed { None } else { outcome.clone() },
        error: if failed { outcome } else { None },
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
    /// renders a generic [`TurnItem::ToolCall`] (via
    /// [`default_tool_turn_item`]) so every tool call is visible on the timeline;
    /// tools with a richer representation (shell command, file change, web
    /// search, MCP call, …) override this. Render is purely a view over the
    /// call/result — it must not perform side effects.
    fn render_turn_item(&self, render: &ToolCallRender<'_>) -> TurnItem {
        default_tool_turn_item(render)
    }

    /// Release resources held by this tool (background tasks, subscriptions,
    /// caches). The registry calls this when the handler is replaced or
    /// removed — always OUTSIDE the registry locks, in reverse registration
    /// order for bulk removals. Dispose must not call back into the registry
    /// (`register`/`unregister`/projections): doing so would deadlock on the
    /// internal locks.
    ///
    /// Semantics: dispose releases resources but does NOT wait for in-flight
    /// [`execute`](Self::execute) calls — the handler stays memory-safe
    /// through its `Arc` and must tolerate being executed after dispose (the
    /// port layer then surfaces a clean error). The default is a no-op.
    fn dispose(&self) {}

    /// Execute the tool with the given parsed arguments.
    async fn execute(
        &self,
        ctx: &ToolContext,
        arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError>;
}
