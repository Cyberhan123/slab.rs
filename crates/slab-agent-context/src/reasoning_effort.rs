//! The reasoning-effort fragment.
//!
//! Emits a model-agnostic rigor steer (deliberate/verify at `High`, direct at
//! `Low`) read from `AgentConfig.reasoning_effort` + `verbosity`. This is the
//! PROMPT half of thinking-strength; the sampling half is resolved separately
//! at the LLM call. It deliberately does NOT fake `<think>` blocks — non-native
//! models are no longer compat'd into reasoning, and native models steer their
//! own reasoning via the template/provider param.

use slab_types::{ChatReasoningEffort, ChatVerbosity};

use crate::fragment::ContextFragment;
use crate::helper::REASONING_EFFORT_TEMPLATE_NAME;

/// Renders `<reasoning_effort>` guidance for the requested effort/verbosity.
#[derive(Debug, Clone, Default)]
pub struct ReasoningEffortFragment {
    pub effort: Option<ChatReasoningEffort>,
    pub verbosity: Option<ChatVerbosity>,
}

impl ContextFragment for ReasoningEffortFragment {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn template_name(&self) -> &'static str {
        REASONING_EFFORT_TEMPLATE_NAME
    }

    fn render_context(&self) -> serde_json::Value {
        serde_json::json!({ "effort": self.effort, "verbosity": self.verbosity })
    }
}
