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
#[derive(Debug, Clone, Default)]
pub struct SystemInstructionFragment {
    pub workspace_bound: bool,
}

impl ContextFragment for SystemInstructionFragment {
    fn role(&self) -> &'static str {
        "system"
    }

    fn template_name(&self) -> &'static str {
        SYSTEM_TEMPLATE_NAME
    }

    fn render_context(&self) -> serde_json::Value {
        serde_json::json!({ "workspace_bound": self.workspace_bound })
    }
}
