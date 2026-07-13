//! Agent application-layer response models.
//!
//! These types keep `/v1/agents/responses` use-case semantics out of the HTTP
//! transport while avoiding dependencies on axum, WebSocket, SSE, or OpenAPI
//! schema types.

use slab_agent::port::{ThreadMessageRecord, ThreadSnapshot};

/// Transport-neutral session restoration payload.
#[derive(Debug)]
pub struct AgentSessionSnapshot {
    pub session_id: String,
    pub thread: Option<ThreadSnapshot>,
    pub messages: Vec<ThreadMessageRecord>,
    pub responses: Vec<serde_json::Value>,
}
