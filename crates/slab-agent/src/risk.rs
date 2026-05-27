use async_trait::async_trait;

use crate::event::{ToolRiskAssessment, ToolRiskLevel};

/// Analyzes tool calls before execution so approval events can expose risk.
#[async_trait]
pub trait ToolRiskAnalyzer: Send + Sync {
    /// Return a risk assessment for a tool invocation.
    async fn analyze(&self, tool_name: &str, arguments: &serde_json::Value) -> ToolRiskAssessment;
}

#[derive(Default)]
pub struct BasicToolRiskAnalyzer;

#[async_trait]
impl ToolRiskAnalyzer for BasicToolRiskAnalyzer {
    async fn analyze(&self, tool_name: &str, arguments: &serde_json::Value) -> ToolRiskAssessment {
        if tool_name == "shell" {
            let command = arguments
                .get("command")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            if command.contains("rm ")
                || command.contains("remove-item")
                || command.contains("git reset")
                || command.contains("del ")
            {
                return ToolRiskAssessment {
                    level: ToolRiskLevel::High,
                    labels: vec!["destructive_command".to_owned(), "shell".to_owned()],
                    reason: Some("shell command may modify or delete files".to_owned()),
                };
            }

            return ToolRiskAssessment {
                level: ToolRiskLevel::Medium,
                labels: vec!["shell".to_owned()],
                reason: Some("shell commands require host review".to_owned()),
            };
        }

        ToolRiskAssessment { level: ToolRiskLevel::Low, labels: Vec::new(), reason: None }
    }
}
