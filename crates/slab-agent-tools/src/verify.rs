//! Deterministic workspace verification tool.
//!
//! `verify` runs a deterministic check against the current workspace and
//! reports a structured pass/fail with a `result_ref` that plan nodes can
//! attach to mark a step as objectively done (the Anthropic lesson: prefer
//! deterministic verification over the model's self-assessment).
//!
//! The LLM picks a `target` (`workspace_build` / `lint` / `diff`); the command
//! each target maps to is fixed by the host and cannot be overridden by the
//! model, so the result is deterministic for a given workspace state. The
//! actual command execution lives behind the [`WorkspaceVerifier`] trait so the
//! tool stays unit-testable without spawning real `cargo`/`git` subprocesses.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use slab_agent::{AgentError, ToolContext, ToolHandler, ToolOutput};

/// Deterministic verification target. The command mapped to each variant is
/// fixed by the host; the model only chooses the variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyTarget {
    WorkspaceBuild,
    Lint,
    Diff,
}

impl VerifyTarget {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WorkspaceBuild => "workspace_build",
            Self::Lint => "lint",
            Self::Diff => "diff",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "workspace_build" | "build" => Some(Self::WorkspaceBuild),
            "lint" => Some(Self::Lint),
            "diff" => Some(Self::Diff),
            _ => None,
        }
    }
}

/// Outcome of a deterministic verification run.
#[derive(Debug, Clone)]
pub struct VerifyOutcome {
    pub target: VerifyTarget,
    pub passed: bool,
    pub exit_code: Option<i32>,
    /// Truncated stdout+stderr captured from the verification command.
    pub summary: String,
}

impl VerifyOutcome {
    /// Deterministic marker a plan node can store as `result_ref`.
    pub fn result_ref(&self) -> String {
        let status = if self.passed { "passed" } else { "failed" };
        format!("verify:{}:{}", self.target.as_str(), status)
    }
}

/// Runs a verification command for a workspace root. Production implementation
/// shells out to fixed host commands; tests inject a fake.
pub trait WorkspaceVerifier: Send + Sync {
    fn verify(&self, root: &Path, target: VerifyTarget) -> VerifyOutcome;
}

/// Default verifier that maps each target to a fixed host command.
#[derive(Default)]
pub struct CommandWorkspaceVerifier;

impl CommandWorkspaceVerifier {
    pub fn new() -> Self {
        Self
    }

    fn command_for(target: VerifyTarget) -> Vec<&'static str> {
        match target {
            // `cargo check` is the deterministic "does it build" signal.
            VerifyTarget::WorkspaceBuild => vec!["cargo", "check", "--quiet", "--workspace"],
            // `cargo fmt --check` is a deterministic formatting/lint signal.
            VerifyTarget::Lint => vec!["cargo", "fmt", "--check"],
            // Empty porcelain output means a clean working tree.
            VerifyTarget::Diff => vec!["git", "status", "--porcelain"],
        }
    }
}

impl WorkspaceVerifier for CommandWorkspaceVerifier {
    fn verify(&self, root: &Path, target: VerifyTarget) -> VerifyOutcome {
        let cmd = Self::command_for(target);
        let output = match Command::new(cmd[0]).args(&cmd[1..]).current_dir(root).output() {
            Ok(output) => output,
            Err(error) => {
                return VerifyOutcome {
                    target,
                    passed: false,
                    exit_code: None,
                    summary: format!("failed to run {}: {error}", cmd.join(" ")),
                };
            }
        };

        let mut summary = String::new();
        if !output.stdout.is_empty() {
            summary.push_str(&String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            if !summary.is_empty() {
                summary.push('\n');
            }
            summary.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        truncate(&mut summary, 2048);

        let passed = match target {
            // git status --porcelain: pass iff there is no dirty output.
            VerifyTarget::Diff => output.status.success() && summary.trim().is_empty(),
            // cargo check / cargo fmt --check: pass on exit code 0.
            _ => output.status.success(),
        };

        VerifyOutcome { target, passed, exit_code: output.status.code(), summary }
    }
}

fn truncate(value: &mut String, max_chars: usize) {
    if value.chars().count() <= max_chars {
        return;
    }
    let cut = value.char_indices().nth(max_chars).map(|(idx, _)| idx).unwrap_or(value.len());
    value.truncate(cut);
    value.push('…');
}

/// Agent-facing deterministic verification tool.
pub struct VerifyTool {
    verifier: Arc<dyn WorkspaceVerifier>,
}

impl VerifyTool {
    pub fn new() -> Self {
        Self { verifier: Arc::new(CommandWorkspaceVerifier::new()) }
    }

    /// Construct with a custom verifier (for tests / host overrides).
    pub fn new_with_verifier(verifier: Arc<dyn WorkspaceVerifier>) -> Self {
        Self { verifier }
    }
}

impl Default for VerifyTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct VerifyArgs {
    target: String,
    #[serde(default)]
    path: Option<String>,
}

#[async_trait]
impl ToolHandler for VerifyTool {
    fn name(&self) -> &str {
        "verify"
    }

    fn description(&self) -> &str {
        "Run a deterministic workspace check (workspace_build / lint / diff) and return pass/fail with a result_ref for plan nodes."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "enum": ["workspace_build", "lint", "diff"],
                    "description": "Which deterministic check to run."
                },
                "path": {
                    "type": "string",
                    "description": "Optional workspace-relative scope hint (informational)."
                }
            },
            "required": ["target"]
        })
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let args: VerifyArgs = serde_json::from_value(arguments.clone())
            .map_err(|error| AgentError::ToolExecution(format!("invalid verify args: {error}")))?;
        let target = VerifyTarget::from_str(&args.target).ok_or_else(|| {
            AgentError::ToolExecution(format!(
                "verify target must be one of: workspace_build, lint, diff (got '{}')",
                args.target
            ))
        })?;

        let root =
            ctx.workspace.as_ref().map(|workspace| workspace.root.clone()).ok_or_else(|| {
                AgentError::ToolExecution("verify requires an active workspace".to_owned())
            })?;

        let scope = args.path.as_deref().map(str::trim).filter(|value| !value.is_empty());
        let verifier = Arc::clone(&self.verifier);
        let outcome = tokio::task::spawn_blocking(move || verifier.verify(&root, target))
            .await
            .map_err(|error| AgentError::ToolExecution(format!("verify task failed: {error}")))?;

        let content = json!({
            "target": outcome.target.as_str(),
            "passed": outcome.passed,
            "exit_code": outcome.exit_code,
            "summary": outcome.summary,
            "result_ref": outcome.result_ref(),
            "scope": scope,
        });

        Ok(ToolOutput {
            content: serde_json::to_string(&content)
                .map_err(|error| AgentError::ToolExecution(error.to_string()))?,
            metadata: Some(content),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use serde_json::{Value, json};
    use slab_agent::{AgentError, ToolContext, ToolHandler, WorkspaceRef};

    use super::*;

    fn ctx_with_workspace() -> ToolContext {
        ToolContext::for_thread("thread")
            .workspace(WorkspaceRef { root: PathBuf::from("/tmp/ws"), session_id: None })
            .build()
    }

    fn ctx_without_workspace() -> ToolContext {
        ToolContext::for_thread("thread").build()
    }

    /// Fake verifier that records its calls and returns a configured outcome.
    /// `seen` is shared via an inner `Arc<Mutex<..>>` so a clone keeps the same
    /// call log (lets us move the trait object into the tool while still
    /// asserting from the test).
    #[derive(Clone)]
    struct FakeVerifier {
        outcome: VerifyOutcome,
        seen: Arc<Mutex<Vec<(PathBuf, VerifyTarget)>>>,
    }

    impl FakeVerifier {
        fn new(outcome: VerifyOutcome) -> Self {
            Self { outcome, seen: Arc::new(Mutex::new(Vec::new())) }
        }
    }

    impl WorkspaceVerifier for FakeVerifier {
        fn verify(&self, root: &Path, target: VerifyTarget) -> VerifyOutcome {
            self.seen.lock().expect("lock").push((root.to_path_buf(), target));
            self.outcome.clone()
        }
    }

    #[tokio::test]
    async fn verify_runs_configured_target_in_workspace_root() {
        let fake = FakeVerifier::new(VerifyOutcome {
            target: VerifyTarget::Lint,
            passed: true,
            exit_code: Some(0),
            summary: "all good".to_owned(),
        });
        let verifier: Arc<dyn WorkspaceVerifier> = Arc::new(fake.clone());
        let tool = VerifyTool::new_with_verifier(verifier);
        let output = tool
            .execute(&ctx_with_workspace(), &json!({ "target": "lint" }))
            .await
            .expect("verify executes");

        let seen = fake.seen.lock().expect("lock").clone();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, PathBuf::from("/tmp/ws"));
        assert_eq!(seen[0].1, VerifyTarget::Lint);

        let value: Value = serde_json::from_str(&output.content).unwrap();
        assert_eq!(value["target"], "lint");
        assert_eq!(value["passed"], true);
        assert_eq!(value["result_ref"], "verify:lint:passed");
        assert_eq!(value["summary"], "all good");
    }

    #[tokio::test]
    async fn verify_reports_failure_result_ref() {
        let verifier: Arc<dyn WorkspaceVerifier> = Arc::new(FakeVerifier::new(VerifyOutcome {
            target: VerifyTarget::WorkspaceBuild,
            passed: false,
            exit_code: Some(101),
            summary: "error[E0599]: ...".to_owned(),
        }));
        let tool = VerifyTool::new_with_verifier(verifier);
        let output = tool
            .execute(&ctx_with_workspace(), &json!({ "target": "workspace_build" }))
            .await
            .expect("verify executes");

        let value: Value = serde_json::from_str(&output.content).unwrap();
        assert_eq!(value["passed"], false);
        assert_eq!(value["result_ref"], "verify:workspace_build:failed");
    }

    #[tokio::test]
    async fn verify_requires_workspace() {
        let tool = VerifyTool::new_with_verifier(Arc::new(FakeVerifier::new(VerifyOutcome {
            target: VerifyTarget::Diff,
            passed: true,
            exit_code: Some(0),
            summary: String::new(),
        })));
        let error = tool
            .execute(&ctx_without_workspace(), &json!({ "target": "diff" }))
            .await
            .expect_err("workspace required");

        assert!(matches!(error, AgentError::ToolExecution(_)));
        assert!(error.to_string().contains("workspace"));
    }

    #[tokio::test]
    async fn verify_rejects_unknown_target() {
        let tool = VerifyTool::new_with_verifier(Arc::new(FakeVerifier::new(VerifyOutcome {
            target: VerifyTarget::Lint,
            passed: true,
            exit_code: Some(0),
            summary: String::new(),
        })));
        let error = tool
            .execute(&ctx_with_workspace(), &json!({ "target": "rubify" }))
            .await
            .expect_err("unknown target rejected");

        assert!(error.to_string().contains("workspace_build, lint, diff"));
    }

    #[test]
    fn verify_target_parses_aliases_case_insensitively() {
        assert_eq!(VerifyTarget::from_str("DIFF"), Some(VerifyTarget::Diff));
        assert_eq!(VerifyTarget::from_str("build"), Some(VerifyTarget::WorkspaceBuild));
        assert_eq!(VerifyTarget::from_str("nope"), None);
    }

    #[test]
    fn verify_command_mapping_is_fixed() {
        // Lint maps to cargo fmt --check (deterministic), not an arbitrary command.
        let cmd = CommandWorkspaceVerifier::command_for(VerifyTarget::Lint);
        assert_eq!(cmd, vec!["cargo", "fmt", "--check"]);
    }
}
