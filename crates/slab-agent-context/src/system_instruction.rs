//! The system-role instruction: identity, personality, and tool-use policy.
//! Rendered through the bundled `system` template.

use crate::fragment::ContextFragment;
use crate::helper::SYSTEM_TEMPLATE_NAME;

/// Renders the system prompt.
///
/// `workspace_bound` mirrors whether the host registered the workspace-bound
/// tools (`apply_patch` & friends register only when a workspace root exists),
/// so the tool-use guidance the model reads matches the tool list it actually
/// receives instead of describing tools that are not there.
///
/// `apply_patch_available` is stricter: whether `apply_patch` is actually
/// CALLABLE this run — registered AND exposed by the permission snapshot AND
/// allowed by the tool whitelist. The "prefer apply_patch" guidance is gated
/// on this flag alone: under a read-only exposure or a whitelist that excludes
/// it the tool is filtered out of the tool list, and a prompt still telling
/// the model to prefer it would reference a tool that does not exist.
#[derive(Debug, Clone, Default)]
pub struct SystemInstructionFragment {
    pub workspace_bound: bool,
    pub apply_patch_available: bool,
}

impl ContextFragment for SystemInstructionFragment {
    fn role(&self) -> &'static str {
        "system"
    }

    fn template_name(&self) -> &'static str {
        SYSTEM_TEMPLATE_NAME
    }

    fn render_context(&self) -> serde_json::Value {
        serde_json::json!({
            "workspace_bound": self.workspace_bound,
            "apply_patch_available": self.apply_patch_available,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragment::ContextFragment;

    #[test]
    fn render_context_carries_both_gates() {
        let context =
            SystemInstructionFragment { workspace_bound: true, apply_patch_available: false }
                .render_context();
        assert_eq!(context["workspace_bound"], true);
        assert_eq!(context["apply_patch_available"], false);
    }

    /// The context-budget guidance must reach the model: scoped search,
    /// truncation markers as a narrowing signal, and `delegate_subagent` for
    /// exhaustive sweeps.
    #[test]
    fn rendered_prompt_guides_context_budget_behavior() {
        let env = crate::helper::build_environment();
        let body =
            SystemInstructionFragment::default().render_body(&env).expect("render system template");
        assert!(body.contains("delegate_subagent"), "delegation guidance missing:\n{body}");
        assert!(body.contains("bytes omitted"), "truncation-marker guidance missing:\n{body}");
        assert!(body.contains("artifact"), "artifact-spill guidance missing:\n{body}");
    }
}
