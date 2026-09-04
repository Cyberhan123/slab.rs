//! `tool_search` — discover Deferred tools (plugins/MCP) by keyword.
//!
//! The model calls `tool_search` with a query; the agent dispatch layer (in
//! `slab-agent::turn_tool_call`) intercepts the call *before* execution,
//! matches the query against the registry's Deferred tool specs, injects the
//! hits into the per-thread discovery state (so they become visible/callable in
//! subsequent turns), and returns the matched specs to the model.
//!
//! This handler's own [`ToolHandler::execute`] is therefore never reached on the
//! normal agent path; it exists mainly to contribute its spec to the
//! model-facing tool list. It is `Direct`/`ReadOnly` so it is always visible and
//! never approval-gated (discovery is read-only metadata).

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput, typed_input_schema};

/// Tool name. Mirrored as a literal in `slab-agent::turn_tool_call` (the
/// dependency direction is reversed, so slab-agent cannot import this const).
pub const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";

/// Arguments for the `tool_search` tool.
///
/// Parsed by the dispatch layer's `handle_tool_search` (in
/// `slab-agent::turn_tool_call`), which intercepts the call before this
/// crate's handler runs; the struct exists to declare the schema.
#[derive(Debug, Deserialize, JsonSchema)]
#[allow(dead_code)] // schema-only; the dispatch layer parses the raw arguments
struct ToolSearchArgs {
    /// Keyword(s) matched (case-insensitive) against tool names and descriptions. Empty lists all discoverable tools.
    query: String,
    /// Optional namespace filter, e.g. "mcp" or "plugin".
    namespace: Option<String>,
}

/// Discover Deferred tools by keyword so they can be called this thread.
pub struct ToolSearchTool;

impl ToolSearchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ToolSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolHandler for ToolSearchTool {
    fn name(&self) -> &str {
        TOOL_SEARCH_TOOL_NAME
    }

    fn description(&self) -> &str {
        "Search for tools that are not in the base tool list (e.g. plugin or MCP tools) by \
         keyword. Matching tools become available to call in subsequent turns. Pass an empty \
         query to list all discoverable tools."
    }

    fn parameters_schema(&self) -> Value {
        typed_input_schema::<ToolSearchArgs>()
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        _arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        // Intercepted by the dispatch layer (`handle_tool_search` in
        // `slab-agent::turn_tool_call`) before this runs. Reaching here means
        // the intercept was bypassed (the tool called outside the agent turn
        // loop); surface a clear error rather than silently no-op'ing.
        Err(AgentError::Internal(
            "tool_search is handled by the agent dispatch layer and must not be executed directly"
                .into(),
        ))
    }
}
