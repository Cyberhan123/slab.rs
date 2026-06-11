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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::{Value, json};
    use slab_agent::{ToolContext, ToolHandler};

    use super::*;

    fn ctx() -> ToolContext {
        ToolContext { thread_id: "thread".into(), turn_index: 0, depth: 0 }
    }

    #[tokio::test]
    async fn apply_patch_tool_reports_success_and_patch_errors_as_json() {
        let root = temp_root("apply_patch");
        fs::write(root.join("a.txt"), "one\ntwo\n").expect("seed file");
        let tool = ApplyPatchTool::new(root.clone());
        let patch = "\
--- a/a.txt
+++ b/a.txt
@@ -1,2 +1,2 @@
 one
-two
+three
";

        let output = tool.execute(&ctx(), &json!({ "patch": patch })).await.expect("patch output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        assert_eq!(value["result"], "ok");
        assert_eq!(value["applied_files"], json!(["a.txt"]));
        assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "one\nthree\n");

        let output = tool.execute(&ctx(), &json!({ "patch": patch })).await.expect("patch output");
        let value: Value = serde_json::from_str(&output.content).expect("json output");
        assert_eq!(value["result"], "error");
        assert!(value["error_message"].as_str().expect("error").contains("patch does not apply"));

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn apply_patch_tool_requires_patch_argument() {
        let root = temp_root("apply_patch_missing");
        let tool = ApplyPatchTool::new(root.clone());

        let error = tool.execute(&ctx(), &json!({})).await.expect_err("missing patch rejected");

        assert_eq!(error.to_string(), "tool execution error: missing 'patch' argument");
        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "slab_agent_tools_patch_{name}_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }
}
