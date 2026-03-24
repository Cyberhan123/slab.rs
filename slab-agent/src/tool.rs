//! Tool handler trait and router registry.

use std::collections::HashMap;

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
}

/// The result produced by a tool handler.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    /// Plain-text (or JSON) content that will be fed back to the LLM.
    pub content: String,
    /// Optional structured metadata for logging / observability.
    pub metadata: Option<serde_json::Value>,
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

    /// Execute the tool with the given parsed arguments.
    async fn execute(
        &self,
        ctx: &ToolContext,
        arguments: &serde_json::Value,
    ) -> Result<ToolOutput, AgentError>;
}

// ── ToolRouter ───────────────────────────────────────────────────────────────

/// Registry of available tools for a given agent thread.
pub struct ToolRouter {
    handlers: HashMap<String, Box<dyn ToolHandler>>,
}

impl ToolRouter {
    /// Create an empty router.
    pub fn new() -> Self {
        Self { handlers: HashMap::new() }
    }

    /// Register a tool handler.  A handler with the same name replaces any
    /// previously registered handler.
    pub fn register(&mut self, handler: Box<dyn ToolHandler>) {
        self.handlers.insert(handler.name().to_owned(), handler);
    }

    /// Look up a handler by tool name.
    pub fn get(&self, name: &str) -> Option<&dyn ToolHandler> {
        self.handlers.get(name).map(|h| h.as_ref())
    }

    /// Return [`ToolSpec`] descriptors for all registered tools.
    pub fn tool_specs(&self) -> Vec<ToolSpec> {
        self.handlers
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
