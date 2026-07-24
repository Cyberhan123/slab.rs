//! The system-role instruction: one short English sentence identifying the
//! agent's role. Rendered through the bundled `system` template.

use crate::fragment::ContextFragment;
use crate::helper::SYSTEM_TEMPLATE_NAME;

#[derive(Debug, Clone, Default)]
pub struct SystemInstructionFragment;

impl ContextFragment for SystemInstructionFragment {
    fn role(&self) -> &'static str {
        "system"
    }

    fn template_name(&self) -> &'static str {
        SYSTEM_TEMPLATE_NAME
    }

    fn render_context(&self) -> serde_json::Value {
        serde_json::json!({})
    }
}
