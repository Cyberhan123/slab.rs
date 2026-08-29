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

    /// A transient LLM transport/provider failure (connect reset, timeout,
    /// 429, 5xx) — eligible for the turn loop's bounded retry with backoff.
    /// Fatal errors (auth, bad request) stay [`AgentError::Llm`].
    #[error("llm transient error: {0}")]
    LlmTransient(String),

    /// The request exceeded the model's context window. Recoverable ONCE per
    /// run via a forced compaction + retry; a repeat fails the turn (death-
    /// spiral guard: compacting twice in a row cannot shrink further).
    #[error("llm context length exceeded: {0}")]
    LlmContextTooLong(String),

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

/// Classify a stringly LLM error into the typed taxonomy. Hosts call this when
/// mapping their chat-service errors onto [`AgentError`] — the turn loop's
/// recovery paths (retry / forced compaction) match on the variants.
///
/// Heuristics over the rendered message because providers surface transport
/// failures as formatted strings; false negatives (an unclassified transient)
/// simply fail the turn as before, never mis-retry a fatal error.
pub fn classify_llm_error(message: &str) -> AgentError {
    let lowered = message.to_ascii_lowercase();
    // Context-window exhaustion — checked FIRST (a 413 is also "transient-
    // looking" but needs compaction, not a blind retry).
    const CONTEXT_MARKERS: [&str; 8] = [
        "context length", // OpenAI: "maximum context length"
        "context window",
        "context_length_exceeded", // Anthropic code
        "prompt is too long",
        "input length and `max_tokens` exceed context limit",
        "request exceeds the available context", // local ggml/llama
        "too many tokens",
        "reduce the length of the messages",
    ];
    // A bare "413" substring also matches request ids and unrelated numbers
    // in the rendered message, misrouting a fatal error into the forced-
    // compaction recovery (which fails the turn anyway when compaction has
    // nothing left to shrink). Only anchored forms count.
    const CONTEXT_STATUS_MARKERS: [&str; 5] =
        ["status 413", "http 413", "code: 413", "error 413", "(413)"];
    if CONTEXT_STATUS_MARKERS.iter().any(|marker| lowered.contains(marker))
        || lowered.contains("request too large")
        || lowered.contains("payload too large")
        || CONTEXT_MARKERS.iter().any(|marker| lowered.contains(marker))
    {
        return AgentError::LlmContextTooLong(message.to_owned());
    }
    // Transient transport/provider failures.
    const TRANSIENT_MARKERS: [&str; 14] = [
        "429",
        "rate limit",
        "overloaded",
        "529",
        "500",
        "502",
        "503",
        "504",
        "bad gateway",
        "service unavailable",
        "gateway timeout",
        "connection reset",
        "connection closed",
        "timeout",
    ];
    if TRANSIENT_MARKERS.iter().any(|marker| lowered.contains(marker)) {
        return AgentError::LlmTransient(message.to_owned());
    }
    AgentError::Llm(message.to_owned())
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

    #[test]
    fn classify_llm_error_detects_context_overflow() {
        for message in [
            "This model's maximum context length is 8192 tokens",
            "prompt is too long: 123456 tokens > 8192 limit",
            "request exceeds the available context size",
            "Error code: 413 — payload too large",
        ] {
            assert!(
                matches!(classify_llm_error(message), AgentError::LlmContextTooLong(_)),
                "{message} should classify as context overflow"
            );
        }
    }

    #[test]
    fn classify_llm_error_detects_transient_failures() {
        for message in [
            "Error code: 429 — rate limit exceeded",
            "connection reset by peer",
            "HTTP 503 service unavailable",
            "gateway timeout after 30000ms",
        ] {
            assert!(
                matches!(classify_llm_error(message), AgentError::LlmTransient(_)),
                "{message} should classify as transient"
            );
        }
    }

    #[test]
    fn classify_llm_error_keeps_fatal_errors_plain() {
        for message in [
            "Error code: 401 — invalid api key",
            "Error code: 400 — invalid request body",
            "model not found: no-such-model",
        ] {
            assert!(
                matches!(classify_llm_error(message), AgentError::Llm(_)),
                "{message} must stay fatal (no retry)"
            );
        }
    }

    #[test]
    fn classify_llm_error_ignores_bare_413_in_request_ids() {
        // A bare "413" substring used to route these into the forced-
        // compaction recovery, which fails the turn when compaction has
        // nothing left to shrink.
        for message in
            ["request id 8f4139ab not found", "invalid param req-4137", "attempt 3 of 1413 failed"]
        {
            assert!(
                matches!(classify_llm_error(message), AgentError::Llm(_)),
                "{message} contains 413 but is not a context-overflow signal"
            );
        }
        // Anchored forms still classify as context overflow.
        for message in ["HTTP 413 request too large", "status code: 413"] {
            assert!(
                matches!(classify_llm_error(message), AgentError::LlmContextTooLong(_)),
                "{message} is a real 413"
            );
        }
    }
}
