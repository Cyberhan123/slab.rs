//! `slab-agent` — Agent orchestration layer for slab.rs.
//!
//! This crate is a **pure library** that implements the agent control plane.
//! It has no dependency on `sqlx`, `axum`, `tonic`, or `slab-core`.  All
//! external capabilities (LLM calls, persistence, notifications) are injected
//! through the port traits defined in [`port`].
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

pub mod config;
pub mod control;
pub mod error;
pub mod port;
pub mod thread;
pub mod tool;

mod turn;

#[cfg(test)]
mod tests;

pub use config::AgentConfig;
pub use control::AgentControl;
pub use error::AgentError;
pub use port::{AgentNotifyPort, AgentStorePort, LlmPort, LlmResponse, ThreadStatus};
pub use tool::{ToolContext, ToolHandler, ToolOutput, ToolRouter};
