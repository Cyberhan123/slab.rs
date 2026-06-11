//! Agent configuration.

use slab_types::chat::{ChatReasoningEffort, ChatVerbosity, StructuredOutput};

pub const DEFAULT_TOOL_CONCURRENCY: u8 = 1;
pub const MAX_TOOL_CONCURRENCY: u8 = 4;
pub const DEFAULT_INVALID_TOOL_CALL_RETRIES: u8 = 1;
pub const MAX_INVALID_TOOL_CALL_RETRIES: u8 = 3;

/// Tool-call mode requested for an agent thread.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentToolChoice {
    Auto,
    None,
    Required,
    Tool { name: String },
}

impl Default for AgentToolChoice {
    fn default() -> Self {
        Self::Auto
    }
}

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
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Tool-call mode requested for this thread.
    #[serde(default)]
    pub tool_choice: AgentToolChoice,
    /// Maximum number of tool calls from one assistant turn to execute concurrently.
    #[serde(default = "default_tool_concurrency")]
    pub tool_concurrency: u8,
    /// Number of invalid tool-call feedback turns allowed before the thread errors.
    #[serde(default = "default_invalid_tool_call_retries")]
    pub invalid_tool_call_retries: u8,
    /// Optional structured-output request forwarded to the chat backend.
    #[serde(default)]
    pub structured_output: Option<StructuredOutput>,
    /// True for short-lived sessions that should skip root-start background work.
    #[serde(default)]
    pub transient: bool,
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
            tool_choice: AgentToolChoice::Auto,
            tool_concurrency: DEFAULT_TOOL_CONCURRENCY,
            invalid_tool_call_retries: DEFAULT_INVALID_TOOL_CALL_RETRIES,
            structured_output: None,
            transient: false,
        }
    }
}

impl AgentConfig {
    pub fn effective_tool_concurrency(&self) -> usize {
        self.tool_concurrency.clamp(1, MAX_TOOL_CONCURRENCY) as usize
    }

    pub fn effective_invalid_tool_call_retries(&self) -> u8 {
        self.invalid_tool_call_retries.clamp(0, MAX_INVALID_TOOL_CALL_RETRIES)
    }
}

fn default_tool_concurrency() -> u8 {
    DEFAULT_TOOL_CONCURRENCY
}

fn default_invalid_tool_call_retries() -> u8 {
    DEFAULT_INVALID_TOOL_CALL_RETRIES
}
