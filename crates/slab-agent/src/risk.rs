use async_trait::async_trait;

use crate::event::{ToolRiskAssessment, ToolRiskLevel};

/// Approval decision derived from a risk assessment + the configured policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolApprovalDecision {
    /// Run the tool without host approval.
    Allow,
    /// Pause and ask the host/user before running the tool.
    Ask,
}

/// Static, host-configurable policy mapping a tool's risk tier to an approval
/// decision. Plugins **cannot** self-report risk (ADR-008: local-first trust
/// model); the host/app-core supplies this policy.
///
/// Full sandbox execution (`Sandbox` tier for dangerous/external-network tools)
/// is a separate slice; until then such tools map to [`ToolApprovalDecision::Ask`].
#[derive(Debug, Clone, Copy)]
pub struct ToolApprovalPolicy {
    /// Risk at or above this tier requires approval (unless the tool supplies
    /// its own approval metadata). Defaults to [`ToolRiskLevel::Medium`] so
    /// workspace writes and external capabilities ask, while read-only tools
    /// and trusted surface openers are allowed.
    pub approval_threshold: ToolRiskLevel,
}

impl Default for ToolApprovalPolicy {
    fn default() -> Self {
        Self { approval_threshold: ToolRiskLevel::Medium }
    }
}

impl ToolApprovalPolicy {
    /// Create a policy that requires approval at or above `threshold`.
    pub fn new(approval_threshold: ToolRiskLevel) -> Self {
        Self { approval_threshold }
    }

    /// Map a risk level to an approval decision under this policy.
    pub fn decision(&self, level: ToolRiskLevel) -> ToolApprovalDecision {
        if level >= self.approval_threshold {
            ToolApprovalDecision::Ask
        } else {
            ToolApprovalDecision::Allow
        }
    }
}

/// Analyzes tool calls before execution so approval events can expose risk.
#[async_trait]
pub trait ToolRiskAnalyzer: Send + Sync {
    /// Return a risk assessment for a tool invocation.
    async fn analyze(&self, tool_name: &str, arguments: &serde_json::Value) -> ToolRiskAssessment;

    /// Map an assessment to an approval decision per the configured policy.
    fn approval_decision(&self, assessment: &ToolRiskAssessment) -> ToolApprovalDecision {
        ToolApprovalPolicy::default().decision(assessment.level)
    }
}

/// Default static risk analyzer with a configurable approval policy.
#[derive(Default)]
pub struct BasicToolRiskAnalyzer {
    policy: ToolApprovalPolicy,
}

impl BasicToolRiskAnalyzer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct with a custom approval policy (host/app-core override).
    pub fn new_with_policy(policy: ToolApprovalPolicy) -> Self {
        Self { policy }
    }
}

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
            // Read-only tools, deterministic control tools, and trusted a2u
            // surface openers are safe to allow without approval (ADR-008).
            "read_file" | "list_dir" | "file_glob" | "grep" | "web_search" | "mcp_list_tools"
            | "git_status" | "git_diff" | "fs_watch" | "plan_update" | "task.complete"
            | "verify" | "workspace.open" | "review.show" | "image.edit" | "hub.browse"
            | "plugin.launch" => {
                ToolRiskAssessment { level: ToolRiskLevel::Low, labels: Vec::new(), reason: None }
            }
            _ => ToolRiskAssessment {
                level: ToolRiskLevel::Medium,
                labels: vec!["unknown_tool".to_owned()],
                reason: Some("tool effect is not statically classified".to_owned()),
            },
        }
    }

    fn approval_decision(&self, assessment: &ToolRiskAssessment) -> ToolApprovalDecision {
        self.policy.decision(assessment.level)
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

    use super::{
        BasicToolRiskAnalyzer, ToolApprovalDecision, ToolApprovalPolicy, ToolRiskAnalyzer,
    };
    use crate::event::ToolRiskLevel;

    async fn analyze_shell(command: &str) -> crate::ToolRiskAssessment {
        BasicToolRiskAnalyzer::new().analyze("shell", &json!({ "command": command })).await
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
    async fn classifies_read_only_and_trusted_tools_as_low_risk() {
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
            "task.complete",
            "verify",
            "workspace.open",
            "review.show",
            "image.edit",
            "hub.browse",
        ] {
            let risk = BasicToolRiskAnalyzer::new().analyze(tool_name, &json!({})).await;

            assert_eq!(risk.level, ToolRiskLevel::Low, "{tool_name} should be low risk");
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
            let risk = BasicToolRiskAnalyzer::new().analyze(tool_name, &json!({})).await;

            assert_eq!(risk.level, ToolRiskLevel::Medium);
            assert!(risk.labels.contains(&expected_label.to_owned()));
            assert!(risk.reason.is_some());
        }
    }

    #[tokio::test]
    async fn classifies_git_commit_as_high_risk() {
        let risk =
            BasicToolRiskAnalyzer::new().analyze("git_commit", &json!({ "message": "ship" })).await;

        assert_eq!(risk.level, ToolRiskLevel::High);
        assert_eq!(risk.labels, ["git_write", "repository_mutation"]);
        assert_eq!(risk.reason.as_deref(), Some("tool creates a repository commit"));
    }

    #[tokio::test]
    async fn default_policy_asks_for_workspace_writes_but_allows_read_only() {
        // B-6 gap: workspace writes must require approval under the default policy.
        let analyzer = BasicToolRiskAnalyzer::new();
        for tool in ["write_file", "apply_patch", "mcp_call", "delegate_subagent"] {
            let risk = analyzer.analyze(tool, &json!({})).await;
            assert_eq!(
                analyzer.approval_decision(&risk),
                ToolApprovalDecision::Ask,
                "{tool} should require approval"
            );
        }
        for tool in ["read_file", "grep", "task.complete", "verify", "workspace.open"] {
            let risk = analyzer.analyze(tool, &json!({})).await;
            assert_eq!(
                analyzer.approval_decision(&risk),
                ToolApprovalDecision::Allow,
                "{tool} should be allowed without approval"
            );
        }
    }

    #[tokio::test]
    async fn policy_threshold_is_configurable() {
        // Host can raise the threshold so only High-risk tools ask.
        let analyzer =
            BasicToolRiskAnalyzer::new_with_policy(ToolApprovalPolicy::new(ToolRiskLevel::High));
        let write = analyzer.analyze("write_file", &json!({})).await;
        assert_eq!(analyzer.approval_decision(&write), ToolApprovalDecision::Allow);

        let commit = analyzer.analyze("git_commit", &json!({})).await;
        assert_eq!(analyzer.approval_decision(&commit), ToolApprovalDecision::Ask);

        // Or lower it so even Low-risk tools ask.
        let strict =
            BasicToolRiskAnalyzer::new_with_policy(ToolApprovalPolicy::new(ToolRiskLevel::Low));
        let read = strict.analyze("read_file", &json!({})).await;
        assert_eq!(strict.approval_decision(&read), ToolApprovalDecision::Ask);
    }
}
