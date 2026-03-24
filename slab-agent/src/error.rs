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

    /// Spawning a child would exceed the configured nesting-depth limit.
    #[error("depth limit exceeded: {current}/{max}")]
    DepthLimitExceeded { current: u32, max: u32 },

    /// The underlying LLM call returned an error.
    #[error("llm error: {0}")]
    Llm(String),

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
