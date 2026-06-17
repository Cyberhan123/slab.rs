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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{BasicToolRiskAnalyzer, ToolRiskAnalyzer};
    use crate::event::ToolRiskLevel;

    async fn analyze_shell(command: &str) -> crate::ToolRiskAssessment {
        BasicToolRiskAnalyzer.analyze("shell", &json!({ "command": command })).await
    }

    #[tokio::test]
    async fn flags_destructive_shell_commands_as_high_risk() {
        for command in
            ["rm -rf target", "Remove-Item -Recurse .", "git reset --hard", "DEL output.log"]
        {
            let risk = analyze_shell(command).await;

            assert_eq!(risk.level, ToolRiskLevel::High);
            assert_eq!(risk.labels, ["destructive_command", "shell"]);
            assert_eq!(risk.reason.as_deref(), Some("shell command may modify or delete files"));
        }
    }

    #[tokio::test]
    async fn classifies_other_shell_commands_as_medium_risk() {
        for command in ["echo hello", "git status", "rm", "del"] {
            let risk = analyze_shell(command).await;

            assert_eq!(risk.level, ToolRiskLevel::Medium);
            assert_eq!(risk.labels, ["shell"]);
            assert_eq!(risk.reason.as_deref(), Some("shell commands require host review"));
        }
    }

    #[tokio::test]
    async fn classifies_non_shell_tools_as_low_risk() {
        let risk = BasicToolRiskAnalyzer.analyze("web_search", &json!({ "query": "slab" })).await;

        assert_eq!(risk.level, ToolRiskLevel::Low);
        assert!(risk.labels.is_empty());
        assert!(risk.reason.is_none());
    }
}
