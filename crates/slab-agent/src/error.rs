//! Agent error types.

/// All errors that can be produced by the agent orchestration layer.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// A referenced thread does not exist.
    #[error("thread not found: {0}")]
    ThreadNotFound(String),

    /// A referenced tool is not registered in the router.
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    /// Spawning would exceed the configured concurrent-thread limit.
    #[error("thread limit exceeded: {current}/{max}")]
    ThreadLimitExceeded { current: usize, max: usize },

    /// Spawning is paused because the memory circuit breaker tripped
    /// (process RSS above the configured threshold). The host owns the breaker
    /// and clears it after a cooldown once pressure recedes (INFRA-05).
    #[error("memory pressure exceeded: {current_mb}/{threshold_mb} MB")]
    MemoryPressureExceeded { current_mb: u64, threshold_mb: u64 },

    /// A caller attempted to start another turn while the thread is still active.
    #[error("thread is busy: {0}")]
    ThreadBusy(String),

    /// A caller attempted to resume a terminal thread that cannot continue.
    #[error("thread cannot be resumed: {id} is {status}")]
    ThreadNotResumable { id: String, status: slab_types::agent::AgentThreadStatus },

    /// Spawning a child would exceed the configured nesting-depth limit.
    #[error("depth limit exceeded: {current}/{max}")]
    DepthLimitExceeded { current: u32, max: u32 },

    /// The underlying LLM call returned an error.
    #[error("llm error: {0}")]
    Llm(String),

    /// The current turn was interrupted by the caller.
    #[error("turn interrupted")]
    Interrupted,

    /// A state machine rejected an invalid lifecycle transition.
    #[error("invalid {entity} state transition: {from} -> {to}")]
    InvalidStateTransition { entity: &'static str, from: String, to: String },

    /// A persistence operation returned an error.
    #[error("store error: {0}")]
    Store(String),

    /// A tool handler returned an error.
    #[error("tool execution error: {0}")]
    ToolExecution(String),

    /// An unexpected internal error.
    #[error("{0}")]
    Internal(String),
}

/// Typed error produced by a tool handler. Replaces ad-hoc
/// `AgentError::ToolExecution(String)` for structured routing by the dispatch
/// loop, approval gate, and observability — callers can match on the variant
/// instead of parsing a string. Existing handlers may adopt it incrementally;
/// it converts to [`AgentError`] via `From`, so `?` works inside handlers that
/// still return `Result<_, AgentError>`.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// The arguments failed validation or could not be parsed.
    #[error("invalid tool arguments: {0}")]
    InvalidArgs(String),
    /// The operation was denied by policy / permissions.
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    /// A referenced resource (file, key, capability) was not found.
    #[error("not found: {0}")]
    NotFound(String),
    /// The operation did not complete in the allotted time.
    #[error("timeout: {0}")]
    Timeout(String),
    /// Any other tool-internal failure.
    #[error("{0}")]
    Internal(String),
}

impl ToolError {
    /// Whether this error reflects a client/argument problem (retryable by the
    /// model with corrected input) vs. an internal/environment problem.
    pub fn is_client_error(&self) -> bool {
        matches!(self, Self::InvalidArgs(_) | Self::PermissionDenied(_) | Self::NotFound(_))
    }
}

impl From<ToolError> for AgentError {
    fn from(error: ToolError) -> Self {
        Self::ToolExecution(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_error_client_vs_internal_classification() {
        assert!(ToolError::InvalidArgs("x".into()).is_client_error());
        assert!(ToolError::PermissionDenied("x".into()).is_client_error());
        assert!(ToolError::NotFound("x".into()).is_client_error());
        assert!(!ToolError::Timeout("x".into()).is_client_error());
        assert!(!ToolError::Internal("x".into()).is_client_error());
    }

    #[test]
    fn tool_error_converts_to_agent_error_tool_execution() {
        let err: AgentError = ToolError::InvalidArgs("bad".into()).into();
        assert!(matches!(err, AgentError::ToolExecution(_)));
        assert!(err.to_string().contains("invalid tool arguments"));
    }

    #[test]
    fn tool_error_display_is_human_readable() {
        assert_eq!(
            ToolError::PermissionDenied("sandbox".into()).to_string(),
            "permission denied: sandbox"
        );
    }
}
