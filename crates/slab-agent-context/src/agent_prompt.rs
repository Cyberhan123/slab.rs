//! Built-in agent system-prompt rendering (Slice 4 Phase F).
//!
//! Established the "built-in agent prompt = one jinja template + one render
//! line" paradigm: future built-in agents add a template under `templates/`
//! and a render function here. Rendered once at registration time and
//! materialized into [`slab_agent::AgentDefinition::system_prompt`], because
//! transient subagents bypass the per-turn [`crate::ContextInstructionHook`]
//! (see `hooks.rs`), so per-turn fragment injection cannot reach them.

use crate::error::ContextError;
use crate::helper::{PLAN_AGENT_TEMPLATE_NAME, build_environment};
use minijinja::Environment;

/// Render the read-only `plan` agent's system prompt from its jinja template.
///
/// The template is compile-time-embedded and variable-free, so rendering is
/// infallible in practice — a failure is a programming error (a broken
/// registration) caught by the unit tests. Returns `Result` to match the
/// crate's other render entry points; callers at infallible registration sites
/// may `.expect(...)`.
pub fn render_plan_agent_prompt() -> Result<String, ContextError> {
    render_with_env(&build_environment())
}

fn render_with_env(env: &Environment<'_>) -> Result<String, ContextError> {
    env.get_template(PLAN_AGENT_TEMPLATE_NAME)
        .map_err(|error| ContextError::Template(error.to_string()))?
        .render(serde_json::json!({}))
        .map_err(|error| ContextError::Template(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_agent_prompt_renders_read_only_directive() {
        let prompt = render_plan_agent_prompt().expect("plan agent prompt renders");
        assert!(prompt.contains("planning agent"));
        assert!(prompt.contains("plan tools"));
        assert!(prompt.contains("present_plan"));
        assert!(prompt.contains("CANNOT"));
    }

    #[test]
    fn plan_agent_prompt_registered_and_round_trips() {
        // The template is registered by build_environment and retrievable by name.
        let env = build_environment();
        assert!(env.get_template(PLAN_AGENT_TEMPLATE_NAME).is_ok());
        let rendered = render_with_env(&env).expect("renders via shared environment");
        assert!(!rendered.is_empty());
    }
}
