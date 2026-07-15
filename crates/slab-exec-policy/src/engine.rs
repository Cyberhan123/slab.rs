//! The permission decision engine: the single owner of every
//! Allow/RequireApproval/Deny verdict, plus a permissive stub for tests.

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::RwLock;
use tracing::warn;

use crate::category::{OperationCategory, OperationDescriptor};
use crate::decision::{ApprovalScope, ExecDecision, PermissionBaseline, PermissionMode};
use crate::rule::{RuleAction, RuleSet};
use crate::safety::{CommandSafetyChecker, SafetyDecision, is_sensitive_path};
use crate::store::{RuleStore, workspace_rules_filename};

/// Decision engine the agent kernel calls. The SINGLE owner of the
/// Allow/RequireApproval/Deny verdict — the shell policy, risk analyzer, and
/// sandbox are all demoted to inputs feeding this engine.
#[async_trait]
pub trait ExecPolicyPort: Send + Sync {
    /// Decide whether the operation may run, must be approved, or is refused.
    async fn evaluate(&self, thread_id: &str, descriptor: &OperationDescriptor) -> ExecDecision;

    /// Persist a user-chosen scope as a rule (no-op for `RunOnce`/`Deny`).
    async fn remember(
        &self,
        thread_id: &str,
        descriptor: &OperationDescriptor,
        scope: ApprovalScope,
    );

    /// Set the per-session mode for a thread (flows from `ThreadStartParams`).
    async fn set_thread_mode(&self, thread_id: &str, mode: PermissionMode);

    /// Drop per-thread state when the thread ends.
    async fn clear_thread(&self, thread_id: &str);
}

/// Internal behavior after resolving `PermissionMode` (+ baseline for Custom).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Behavior {
    /// Prompt for shell/file-edit/network. Every shell call prompts unless a
    /// remembered Allow rule matches (acceptForSession / AlwaysInWorkspace).
    RequestApproval,
    /// Allow everything (hard-deny safety patterns still apply).
    FullControl,
    /// Read-only: mutations are refused.
    StrictReadOnly,
}

fn resolve_behavior(mode: PermissionMode, baseline: PermissionBaseline) -> Behavior {
    match mode {
        PermissionMode::RequestApproval | PermissionMode::ApproveForMe => Behavior::RequestApproval,
        PermissionMode::FullControl => Behavior::FullControl,
        PermissionMode::Custom => match baseline {
            PermissionBaseline::ReadOnly => Behavior::StrictReadOnly,
            PermissionBaseline::WorkspaceWrite => Behavior::RequestApproval,
            PermissionBaseline::FullAccess => Behavior::FullControl,
        },
    }
}

/// Concrete engine. `AgentControl` is a process-wide singleton in slab-server,
/// so per-session mode is keyed by `thread_id` to avoid multi-session races.
pub struct ExecPolicyEngine {
    modes: DashMap<String, PermissionMode>,
    baseline: PermissionBaseline,
    rules: RwLock<RuleSet>,
    store: Arc<dyn RuleStore>,
}

impl ExecPolicyEngine {
    pub fn new(baseline: PermissionBaseline, rules: RuleSet, store: Arc<dyn RuleStore>) -> Self {
        Self { modes: DashMap::new(), baseline, rules: RwLock::new(rules), store }
    }

    fn mode_for(&self, thread_id: &str) -> PermissionMode {
        self.modes.get(thread_id).map(|m| *m).unwrap_or_default()
    }
}

#[async_trait]
impl ExecPolicyPort for ExecPolicyEngine {
    async fn evaluate(&self, thread_id: &str, descriptor: &OperationDescriptor) -> ExecDecision {
        let behavior = resolve_behavior(self.mode_for(thread_id).effective(), self.baseline);
        let category = descriptor.category;
        let subject = descriptor.subject.as_str();

        // 1. Hard-deny safety (always, even FullControl).
        match category {
            OperationCategory::Shell => {
                if let SafetyDecision::Dangerous(reason) = CommandSafetyChecker::check(subject) {
                    warn!(command = subject, reason, "hard-denied destructive shell command");
                    return ExecDecision::Deny;
                }
            }
            OperationCategory::FileEdit | OperationCategory::ReadOnly => {
                if is_sensitive_path(subject) {
                    return ExecDecision::RequireApproval;
                }
            }
            OperationCategory::Network => {}
        }

        // 2. FullControl: allow everything that survived hard-deny.
        if behavior == Behavior::FullControl {
            return ExecDecision::Allow;
        }

        // 3. StrictReadOnly: mutations refused; reads allowed.
        if behavior == Behavior::StrictReadOnly {
            return match category {
                OperationCategory::ReadOnly => ExecDecision::Allow,
                _ => ExecDecision::Deny,
            };
        }

        // 4. RequestApproval: per-category base.
        let base = default_base(category);

        // 5. Rules override (first-match-wins). A `Block` rule denies; `Allow`
        //    short-circuits to Allow; `RequireApproval` prompts. A remembered
        //    Allow rule (acceptForSession / AlwaysInWorkspace) is how repeat
        //    shell calls get silenced.
        let rules = self.rules.read().await;
        if let Some(rule) = rules.evaluate(category, subject) {
            return match rule.action {
                RuleAction::Allow => ExecDecision::Allow,
                RuleAction::RequireApproval => ExecDecision::RequireApproval,
                RuleAction::Block => ExecDecision::Deny,
            };
        }
        drop(rules);

        // 6. Shell commands prompt by default (`base == RequireApproval`) — the
        //    sandbox safe-auto-allow was removed so every shell call surfaces an
        //    approval decision. FileEdit/Network likewise use their `base`.
        base
    }

    async fn remember(
        &self,
        _thread_id: &str,
        descriptor: &OperationDescriptor,
        scope: ApprovalScope,
    ) {
        if !scope.persists() {
            return;
        }
        let rule = crate::rule::Rule::new(
            descriptor.category,
            RuleAction::Allow,
            best_matcher_for(descriptor),
            descriptor.subject.clone(),
        );
        if let Err(error) =
            self.store.store(descriptor.workspace_root.as_deref(), &rule, scope).await
        {
            warn!(error = %error, "failed to persist approval rule");
        } else {
            // Record the workspace mapping for AlwaysInWorkspace (DB-backed
            // stores override `remember_workspace`).
            if scope == ApprovalScope::AlwaysInWorkspace
                && let Some(root) = descriptor.workspace_root.as_deref()
            {
                let filename = workspace_rules_filename(root);
                if let Err(error) = self.store.remember_workspace(&filename, root).await {
                    warn!(error = %error, "failed to record workspace mapping");
                }
            }
            self.rules.write().await.append(rule);
        }
    }

    async fn set_thread_mode(&self, thread_id: &str, mode: PermissionMode) {
        self.modes.insert(thread_id.to_owned(), mode);
    }

    async fn clear_thread(&self, thread_id: &str) {
        self.modes.remove(thread_id);
    }
}

fn default_base(category: OperationCategory) -> ExecDecision {
    match category {
        OperationCategory::ReadOnly => ExecDecision::Allow,
        OperationCategory::Shell | OperationCategory::FileEdit | OperationCategory::Network => {
            ExecDecision::RequireApproval
        }
    }
}

/// Pick a matcher for a persisted rule. Commands and queries use `prefix`
/// (the common "allow cargo check" intent); file paths use `exact` so a
/// persisted allow targets the specific path.
fn best_matcher_for(descriptor: &OperationDescriptor) -> crate::rule::RuleMatcher {
    match descriptor.category {
        OperationCategory::FileEdit => crate::rule::RuleMatcher::Exact,
        _ => crate::rule::RuleMatcher::Prefix,
    }
}

/// Permissive stub: allows everything, persists nothing. Used as the default
/// when no concrete engine is wired (e.g. tests) so the kernel never blocks.
#[derive(Debug, Default, Clone, Copy)]
pub struct AllowAllExecPolicy;

#[async_trait]
impl ExecPolicyPort for AllowAllExecPolicy {
    async fn evaluate(&self, _thread_id: &str, _descriptor: &OperationDescriptor) -> ExecDecision {
        ExecDecision::Allow
    }
    async fn remember(
        &self,
        _thread_id: &str,
        _descriptor: &OperationDescriptor,
        _scope: ApprovalScope,
    ) {
    }
    async fn set_thread_mode(&self, _thread_id: &str, _mode: PermissionMode) {}
    async fn clear_thread(&self, _thread_id: &str) {}
}

// Re-export so callers don't need a separate `use` for the error type.
pub use crate::store::RuleStoreError as StoreError;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::category::OperationDescriptor;
    use crate::rule::{Rule, RuleSet};
    use crate::store::FsRuleStore;

    fn engine_with_rules(
        mode: PermissionMode,
        baseline: PermissionBaseline,
        rules: Vec<Rule>,
        _workspace: Option<PathBuf>,
    ) -> ExecPolicyEngine {
        let dir = tempfile::tempdir().expect("dir");
        let store = Arc::new(FsRuleStore::new(dir.path().to_path_buf()));
        let engine = ExecPolicyEngine::new(baseline, RuleSet::from_rules(rules), store);
        engine.modes.insert("t1".to_owned(), mode);
        // Leak the tempdir for the test lifetime (rules persist to it).
        std::mem::forget(dir);
        engine
    }

    fn ws() -> Option<PathBuf> {
        Some(tempfile::tempdir().expect("ws").keep())
    }

    #[tokio::test]
    async fn request_approval_shell_prompts_by_default() {
        let engine = engine_with_rules(
            PermissionMode::RequestApproval,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            ws(),
        );
        // Safe shell commands prompt too (safe-auto-allow removed); only a
        // remembered Allow rule (acceptForSession) silences them.
        let d = OperationDescriptor::shell("git status");
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::RequireApproval);
    }

    #[tokio::test]
    async fn request_approval_network_shell_prompts() {
        let engine = engine_with_rules(
            PermissionMode::RequestApproval,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            ws(),
        );
        let d = OperationDescriptor::shell("curl http://example.com");
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::RequireApproval);
    }

    #[tokio::test]
    async fn request_approval_destructive_shell_prompts() {
        let engine = engine_with_rules(
            PermissionMode::RequestApproval,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            ws(),
        );
        let d = OperationDescriptor::shell("rm -rf target");
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::RequireApproval);
    }

    #[tokio::test]
    async fn hard_deny_blocks_rm_rf_root_even_under_full_control() {
        let engine = engine_with_rules(
            PermissionMode::FullControl,
            PermissionBaseline::FullAccess,
            vec![],
            ws(),
        );
        let d = OperationDescriptor::shell("rm -rf /");
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::Deny);
    }

    #[tokio::test]
    async fn full_control_allows_normal_shell() {
        let engine = engine_with_rules(
            PermissionMode::FullControl,
            PermissionBaseline::FullAccess,
            vec![],
            ws(),
        );
        let d = OperationDescriptor::shell("cargo build");
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::Allow);
    }

    #[tokio::test]
    async fn allow_rule_short_circuits() {
        let engine = engine_with_rules(
            PermissionMode::RequestApproval,
            PermissionBaseline::WorkspaceWrite,
            vec![Rule::new(
                OperationCategory::Shell,
                RuleAction::Allow,
                crate::rule::RuleMatcher::Prefix,
                "cargo check",
            )],
            ws(),
        );
        let d = OperationDescriptor::shell("cargo check -p slab-agent");
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::Allow);
    }

    #[tokio::test]
    async fn block_rule_denies() {
        let engine = engine_with_rules(
            PermissionMode::RequestApproval,
            PermissionBaseline::WorkspaceWrite,
            vec![Rule::new(
                OperationCategory::Shell,
                RuleAction::Block,
                crate::rule::RuleMatcher::Contains,
                "Remove-Item",
            )],
            ws(),
        );
        let d = OperationDescriptor::shell("Remove-Item file.txt");
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::Deny);
    }

    #[tokio::test]
    async fn file_edit_prompts_by_default() {
        let engine = engine_with_rules(
            PermissionMode::RequestApproval,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            ws(),
        );
        let d = OperationDescriptor::file_edit("/workspace/src/main.rs");
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::RequireApproval);
    }

    #[tokio::test]
    async fn sensitive_path_prompts() {
        let engine = engine_with_rules(
            PermissionMode::RequestApproval,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            ws(),
        );
        let d = OperationDescriptor::read_only("/workspace/.env");
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::RequireApproval);
    }

    #[tokio::test]
    async fn custom_read_only_denies_mutations() {
        let engine =
            engine_with_rules(PermissionMode::Custom, PermissionBaseline::ReadOnly, vec![], ws());
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("ls")).await,
            ExecDecision::Deny
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::file_edit("/x")).await,
            ExecDecision::Deny
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::read_only("/x")).await,
            ExecDecision::Allow
        );
    }

    #[tokio::test]
    async fn custom_full_access_acts_like_full_control() {
        let engine =
            engine_with_rules(PermissionMode::Custom, PermissionBaseline::FullAccess, vec![], ws());
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("cargo build")).await,
            ExecDecision::Allow
        );
    }

    #[tokio::test]
    async fn network_category_prompts_by_default() {
        let engine = engine_with_rules(
            PermissionMode::RequestApproval,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            ws(),
        );
        let d = OperationDescriptor::network("rust async");
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::RequireApproval);
    }

    #[tokio::test]
    async fn approve_for_me_behaves_like_request_approval() {
        let engine = engine_with_rules(
            PermissionMode::ApproveForMe,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            ws(),
        );
        // ApproveForMe resolves to RequestApproval; shell prompts there too now.
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("git status")).await,
            ExecDecision::RequireApproval
        );
    }
}
