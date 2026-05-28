//! Agent configuration.

use slab_types::chat::{ChatReasoningEffort, ChatVerbosity};

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
    /// Maximum output tokens per LLM call.
    pub max_tokens: Option<u32>,
    /// Sampling temperature passed to the LLM.
    pub temperature: Option<f32>,
    /// Nucleus sampling threshold.
    pub top_p: Option<f32>,
    /// Top-k sampling limit for local llama backends.
    pub top_k: Option<i32>,
    /// Min-p sampling threshold for local llama backends.
    pub min_p: Option<f32>,
    /// Presence penalty for local llama backends.
    pub presence_penalty: Option<f32>,
    /// Repetition penalty for local llama backends.
    pub repetition_penalty: Option<f32>,
    /// Provider reasoning effort override.
    pub reasoning_effort: Option<ChatReasoningEffort>,
    /// Provider verbosity override.
    pub verbosity: Option<ChatVerbosity>,
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
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            min_p: None,
            presence_penalty: None,
            repetition_penalty: None,
            reasoning_effort: None,
            verbosity: None,
            allowed_tools: vec![],
        }
    }
}
