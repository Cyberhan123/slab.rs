//! `slab-agent` — Agent orchestration layer for slab.rs.
//!
//! This crate is a **pure library** that implements the agent control plane.
//! It has no dependency on `sqlx`, `axum`, `tonic`, `slab-proto`,
//! `slab-agent-rollout`, or `slab-app-core`.  All external capabilities (LLM
//! calls, persistence, notifications) are injected through the port traits
//! defined in [`port`].
//!
//! # Architecture
//!
//! ```text
//! slab-server
//!   └── AgentControl         (this crate)
//!         ├── LlmPort        ──► GrpcGateway → slab-runtime → slab-core
//!         ├── AgentStorePort ──► SQLx store (slab-server/infra)
//!         └── AgentNotifyPort──► SSE / WebSocket fan-out
//! ```
//!
//! # Typical usage
//!
//! 1. Implement the three port traits in `slab-server`.
//! 2. Build a [`ToolRouter`] and register your [`ToolHandler`] implementations.
//! 3. Construct an [`AgentControl`] with the port adapters and router.
//! 4. Call [`AgentControl::spawn`] to start a root agent.

pub mod agent;
pub mod compact;
pub mod config;
pub mod control;
pub mod error;
pub mod hook;
pub mod plan;
pub mod port;
pub mod protocol;
pub mod risk;
pub mod runtime;
pub mod thread;
pub mod tool;
pub mod tool_schema;

mod concurrency_gate;
mod llm_output;
mod repetition_guard;
mod state;
mod tool_validation;
mod turn;
mod turn_state;
mod turn_tool_call;
mod turn_tool_record;

#[cfg(test)]
mod tests;

pub use agent::{
    AgentDefinition, AgentRegistry, ModelPolicy, NoopAgentRegistry, ToolConstraint,
    filter_tools_for_agent,
};
pub use compact::{
    CompactContext, CompactOutcome, CompactPort, NoopCompactPort, SlidingWindowCompactPort,
    estimate_message_chars, estimate_message_tokens, estimate_tokens,
    remove_leading_orphan_tool_results, trailing_window, trim_to_target_after_system,
};
pub use config::{AgentConfig, AgentToolChoice};
pub use control::{AgentControl, AgentControlLimits, SendOutcome};
pub use error::{AgentError, ToolError, classify_llm_error};
pub use hook::{AgentHook, AgentHookRegistry, HookEffects, HookEvent, HookOutcome, HookToolAction};
pub use llm_output::{
    AgentStreamAssembler, AgentStreamCompletion, AgentStreamDelta, RenderedToolCallOutput,
    parse_rendered_tool_call_output,
};
pub use plan::{Plan, PlanCounts, PlanItem, PlanStatus};
pub use port::{
    AgentNotifyPort, AgentStorePort, ApprovalDecision, ApprovalPort, ExecPolicyPort, LlmPort,
    LlmResponse, MemoryPressure, MemoryPressurePort, NoopMemoryPressurePort, NoopPlanStore,
    PlanStorePort, PluginToolPort, ThreadStatus, ToolRiskAssessment, ToolRiskLevel,
    TurnStateRecord,
};
pub use risk::{BasicToolRiskAnalyzer, ToolApprovalDecision, ToolApprovalPolicy, ToolRiskAnalyzer};
pub use runtime::AgentRuntime;
pub use slab_exec_policy::{
    AllowAllExecPolicy, ApprovalScope, ExecDecision, ExecPolicyEngine, OperationCategory,
    OperationDescriptor, PermissionBaseline, PermissionMode, PermissionStateSnapshot, ToolExposure,
};
pub use tool::{
    AgentThreadContext, PlanRef, ToolApprovalRequest, ToolCallRender, ToolCapability, ToolContext,
    ToolContextBuilder, ToolDiscoveryState, ToolHandler, ToolName, ToolNamespace, ToolOutput,
    ToolOutputObserver, ToolOutputStream, ToolRouter, ToolVisibility, WorkspaceRef,
};
pub use turn::strip_think_blocks;
