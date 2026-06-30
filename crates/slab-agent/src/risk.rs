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
        match tool_name {
            "shell" => analyze_shell(arguments),
            "write_file" | "apply_patch" => ToolRiskAssessment {
                level: ToolRiskLevel::Medium,
                labels: vec!["workspace_write".to_owned()],
                reason: Some("tool may modify workspace files".to_owned()),
            },
            "git_commit" => ToolRiskAssessment {
                level: ToolRiskLevel::High,
                labels: vec!["git_write".to_owned(), "repository_mutation".to_owned()],
                reason: Some("tool creates a repository commit".to_owned()),
            },
            "mcp_call" | "delegate_subagent" => ToolRiskAssessment {
                level: ToolRiskLevel::Medium,
                labels: vec!["external_capability".to_owned()],
                reason: Some("tool delegates work outside the current agent turn".to_owned()),
            },
            name if name.starts_with("mcp__") => ToolRiskAssessment {
                level: ToolRiskLevel::Medium,
                labels: vec!["external_capability".to_owned(), "mcp_proxy".to_owned()],
                reason: Some("tool calls a proxied MCP capability".to_owned()),
            },
            "read_file" | "list_dir" | "file_glob" | "grep" | "web_search" | "mcp_list_tools"
            | "git_status" | "git_diff" | "fs_watch" | "plan_update" => {
                ToolRiskAssessment { level: ToolRiskLevel::Low, labels: Vec::new(), reason: None }
            }
            _ => ToolRiskAssessment {
                level: ToolRiskLevel::Medium,
                labels: vec!["unknown_tool".to_owned()],
                reason: Some("tool effect is not statically classified".to_owned()),
            },
        }
    }
}

fn analyze_shell(arguments: &serde_json::Value) -> ToolRiskAssessment {
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

    ToolRiskAssessment {
        level: ToolRiskLevel::Medium,
        labels: vec!["shell".to_owned()],
        reason: Some("shell commands require host review".to_owned()),
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
    async fn classifies_read_only_tools_as_low_risk() {
        for tool_name in [
            "read_file",
            "list_dir",
            "file_glob",
            "grep",
            "web_search",
            "mcp_list_tools",
            "git_status",
            "git_diff",
            "fs_watch",
            "plan_update",
        ] {
            let risk = BasicToolRiskAnalyzer.analyze(tool_name, &json!({})).await;

            assert_eq!(risk.level, ToolRiskLevel::Low);
            assert!(risk.labels.is_empty());
            assert!(risk.reason.is_none());
        }
    }

    #[tokio::test]
    async fn classifies_workspace_writes_and_external_calls_as_medium_risk() {
        for (tool_name, expected_label) in [
            ("write_file", "workspace_write"),
            ("apply_patch", "workspace_write"),
            ("mcp_call", "external_capability"),
            ("delegate_subagent", "external_capability"),
            ("mcp__server__tool", "external_capability"),
            ("unknown_future_tool", "unknown_tool"),
        ] {
            let risk = BasicToolRiskAnalyzer.analyze(tool_name, &json!({})).await;

            assert_eq!(risk.level, ToolRiskLevel::Medium);
            assert!(risk.labels.contains(&expected_label.to_owned()));
            assert!(risk.reason.is_some());
        }
    }

    #[tokio::test]
    async fn classifies_git_commit_as_high_risk() {
        let risk = BasicToolRiskAnalyzer.analyze("git_commit", &json!({ "message": "ship" })).await;

        assert_eq!(risk.level, ToolRiskLevel::High);
        assert_eq!(risk.labels, ["git_write", "repository_mutation"]);
        assert_eq!(risk.reason.as_deref(), Some("tool creates a repository commit"));
    }
}
