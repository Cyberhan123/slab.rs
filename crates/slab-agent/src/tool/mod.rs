//! Tool handler trait and router registry.

mod capability;
mod context;
mod handler;
mod router;

pub use capability::{ToolCapability, ToolName, ToolNamespace, ToolVisibility};
pub use context::{
    AgentThreadContext, PlanRef, ToolApprovalRequest, ToolContext, ToolContextBuilder, ToolOutput,
    ToolOutputObserver, ToolOutputStream, WorkspaceRef, WorkspaceScopeRef,
};
pub use handler::{ToolCallRender, ToolHandler, default_tool_turn_item};
pub use router::{ToolDiscoveryState, ToolRegistration, ToolRouter};

#[cfg(test)]
mod tests;
