//! The environment-context fragment: cwd / shell / os / start-time, rendered
//! through the bundled `environment` template into a `developer` message.

use crate::fragment::ContextFragment;
use crate::helper::ENVIRONMENT_TEMPLATE_NAME;
use crate::snapshots::EnvironmentSnapshot;

/// Renders `<environment_context>` so the model knows where it is working.
#[derive(Debug, Clone)]
pub struct EnvironmentContextFragment {
    pub snapshot: EnvironmentSnapshot,
}

impl ContextFragment for EnvironmentContextFragment {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn template_name(&self) -> &'static str {
        ENVIRONMENT_TEMPLATE_NAME
    }

    fn render_context(&self) -> serde_json::Value {
        serde_json::to_value(&self.snapshot).unwrap_or_else(|_| serde_json::json!({}))
    }
}
