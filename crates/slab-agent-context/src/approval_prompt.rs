//! Approval-review prompt rendering for the "approve for me" model delegation.
//!
//! Same paradigm as [`crate::agent_prompt`]: one jinja template under
//! `templates/` plus one render function. The host-side reviewer
//! (`ModelApprovalReviewer` in app-core) renders this as the SYSTEM prompt of
//! its bounded single-shot review call and appends the user's custom policy
//! through the `custom_policy` variable.

use crate::error::ContextError;
use crate::helper::{APPROVAL_REVIEW_TEMPLATE_NAME, build_environment};
use minijinja::Environment;
use serde::Serialize;

/// Inputs to the built-in approval-review system prompt.
#[derive(Debug, Clone, Serialize)]
pub struct ApprovalReviewContext {
    /// Canonical tool name being gated (e.g. `run_shell`).
    pub tool_name: String,
    /// Unified policy category label (e.g. `shell`, `file_write`).
    pub category: String,
    /// Human-readable operation subject (command line, file path, query…).
    pub display: String,
    /// Effective JSON arguments of the tool call (caller-side truncation).
    pub arguments: String,
    /// Risk level label; use `not assessed` when no assessment exists.
    pub risk_level: String,
    /// Risk labels attached to the call (may be empty).
    pub risk_labels: Vec<String>,
    /// Workspace root, when known, to ground the workspace envelope rule.
    pub workspace_root: Option<String>,
    /// Extra policy instructions from the user (empty = built-in rules only).
    pub custom_policy: String,
}

/// Render the approval-review system prompt. The template is
/// compile-time-embedded and total over [`ApprovalReviewContext`], so a
/// failure is a programming error caught by the unit tests; `Result` matches
/// the crate's other render entry points.
pub fn render_approval_review_prompt(
    context: &ApprovalReviewContext,
) -> Result<String, ContextError> {
    render_with_env(&build_environment(), context)
}

fn render_with_env(
    env: &Environment<'_>,
    context: &ApprovalReviewContext,
) -> Result<String, ContextError> {
    env.get_template(APPROVAL_REVIEW_TEMPLATE_NAME)
        .map_err(|error| ContextError::Template(error.to_string()))?
        .render(context)
        .map_err(|error| ContextError::Template(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(custom_policy: &str) -> ApprovalReviewContext {
        ApprovalReviewContext {
            tool_name: "run_shell".to_owned(),
            category: "shell".to_owned(),
            display: "cargo test -p slab-agent".to_owned(),
            arguments: r#"{ "command": "cargo test -p slab-agent" }"#.to_owned(),
            risk_level: "medium".to_owned(),
            risk_labels: vec!["long_running".to_owned()],
            workspace_root: Some("C:/Users/dev/slab.rs".to_owned()),
            custom_policy: custom_policy.to_owned(),
        }
    }

    #[test]
    fn approval_review_prompt_carries_operation_and_contract() {
        let prompt = render_approval_review_prompt(&context("")).expect("approval prompt renders");
        assert!(prompt.contains("approval reviewer"));
        assert!(prompt.contains("Tool: run_shell (category: shell)"));
        assert!(prompt.contains("Subject: cargo test -p slab-agent"));
        assert!(prompt.contains("Workspace root: C:/Users/dev/slab.rs"));
        assert!(prompt.contains("Risk: medium (long_running)"));
        assert!(prompt.contains("APPROVE or DENY"));
        assert!(prompt.contains("max 200 characters"));
        // Built-in rules present.
        assert!(prompt.contains("Default to DENY when uncertain"));
        // No custom-policy block when empty.
        assert!(!prompt.contains("Custom policy from the user"));
    }

    #[test]
    fn approval_review_prompt_appends_custom_policy_when_set() {
        let prompt =
            render_approval_review_prompt(&context("Always deny git push. Tests may run freely."))
                .expect("approval prompt renders");
        assert!(prompt.contains("Custom policy from the user"));
        assert!(prompt.contains("Always deny git push. Tests may run freely."));
    }

    #[test]
    fn approval_review_prompt_omits_workspace_root_when_unknown() {
        let mut ctx = context("");
        ctx.workspace_root = None;
        let prompt = render_approval_review_prompt(&ctx).expect("approval prompt renders");
        assert!(!prompt.contains("Workspace root:"));
    }

    #[test]
    fn approval_review_prompt_registered_and_round_trips() {
        let env = build_environment();
        assert!(env.get_template(APPROVAL_REVIEW_TEMPLATE_NAME).is_ok());
        let rendered = render_with_env(&env, &context("")).expect("renders via shared environment");
        assert!(!rendered.is_empty());
    }
}
