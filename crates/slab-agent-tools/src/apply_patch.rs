//! Unified-diff patch application tool.

use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};

pub struct ApplyPatchTool {
    workspace_root: PathBuf,
}

impl ApplyPatchTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl ToolHandler for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }

    fn description(&self) -> &str {
        "Apply a unified diff patch inside the configured workspace root."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "patch": {
                    "type": "string",
                    "description": "Unified diff patch text."
                }
            },
            "required": ["patch"]
        })
    }

    async fn execute(
        &self,
        _ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let patch = arguments
            .get("patch")
            .and_then(Value::as_str)
            .ok_or_else(|| AgentError::ToolExecution("missing 'patch' argument".into()))?;

        let result = match slab_file::apply_unified_patch(&self.workspace_root, patch) {
            Ok(result) => result,
            Err(error) => slab_file::PatchApplyResult {
                applied_files: Vec::new(),
                result: "error".to_string(),
                error_message: Some(error.to_string()),
            },
        };

        Ok(ToolOutput {
            content: serde_json::to_string(&result)
                .map_err(|error| AgentError::ToolExecution(error.to_string()))?,
            metadata: None,
        })
    }
}
