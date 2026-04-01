//! Agent configuration.

/// Runtime configuration for a single agent thread.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentConfig {
    /// The model identifier to use for chat completions.
    pub model: String,
    /// Optional system prompt injected as the first message.
    pub system_prompt: Option<String>,
    /// Maximum number of LLM turns before the thread is forcibly completed.
    pub max_turns: u32,
    /// Maximum nesting depth for child agents spawned by this thread.
    ///
    /// `AgentControl` enforces a global maximum at spawn time; this field lets
    /// individual threads apply a stricter per-thread limit on their children.
    pub max_depth: u32,
    /// Maximum number of concurrently active agent threads across the controller.
    ///
    /// `AgentControl` enforces a global maximum at spawn time; this field is
    /// preserved for serialisation fidelity and future per-session overrides.
    pub max_threads: u32,
    /// Sampling temperature passed to the LLM.
    pub temperature: f32,
    /// Maximum output tokens per LLM call.
    pub max_tokens: u32,
    /// Explicit allow-list of tool names available to this thread.
    /// An empty list means all registered tools are allowed.
    pub allowed_tools: Vec<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "default".to_owned(),
            system_prompt: None,
            max_turns: 10,
            max_depth: 3,
            max_threads: 8,
            temperature: 0.7,
            max_tokens: 2048,
            allowed_tools: vec![],
        }
    }
}
