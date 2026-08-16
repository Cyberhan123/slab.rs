//! The permissions-instruction fragment.
//!
//! Renders `<permissions_instructions>` describing the sandbox, the effective
//! file/network/shell permissions, and when the model must ask the user for
//! approval. It states POLICY only — it must not enumerate tools (the LLM
//! already receives tool descriptions via the request `tools` array, and the
//! tool-LIST exposure is enforced separately by the agent runtime).

use crate::fragment::ContextFragment;
use crate::helper::PERMISSIONS_TEMPLATE_NAME;
use crate::snapshots::PermissionSnapshot;

/// Renders `<permissions_instructions>` so the model can plan around the
/// sandbox instead of discovering its limits by failing.
#[derive(Debug, Clone)]
pub struct PermissionsInstructionFragment {
    pub snapshot: PermissionSnapshot,
}

impl ContextFragment for PermissionsInstructionFragment {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn template_name(&self) -> &'static str {
        PERMISSIONS_TEMPLATE_NAME
    }

    fn render_context(&self) -> serde_json::Value {
        serde_json::to_value(&self.snapshot).unwrap_or_else(|_| serde_json::json!({}))
    }
}
