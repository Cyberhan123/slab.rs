//! The permission decision engine: the single owner of every
//! Allow/RequireApproval/Deny verdict, plus a permissive stub for tests.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use tokio::sync::RwLock;
use tracing::warn;

use crate::category::{OperationCategory, OperationDescriptor};
use crate::decision::{ApprovalScope, ExecDecision, PermissionBaseline, PermissionMode};
use crate::exposure::{PermissionStateSnapshot, ToolExposure};
use crate::rule::{RuleAction, RuleSet};
use crate::safety::{
    CommandSafetyChecker, SafetyDecision, is_sensitive_path, is_shell_autorun_safe,
};
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

    /// Snapshot the resolved permission state for a thread — its effective
    /// mode, the global baseline, and the derived tool exposure. Cheap (reads
    /// the in-memory mode map + baseline); used to drive progressive tool
    /// exposure and to render permission instructions to the LLM.
    fn permission_state_for(&self, thread_id: &str) -> PermissionStateSnapshot;
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
    /// acceptEdits (`ApproveForMe`): auto-allow what the active baseline
    /// permits, prompt the rest. The baseline is consulted in `evaluate` (via
    /// [`accept_edits_base`]); rules still override, so a `Block` rule denies
    /// and an `Allow` rule permits even an out-of-envelope operation.
    AcceptEdits,
}

fn resolve_behavior(mode: PermissionMode, baseline: PermissionBaseline) -> Behavior {
    match mode {
        PermissionMode::RequestApproval => Behavior::RequestApproval,
        PermissionMode::ApproveForMe => Behavior::AcceptEdits,
        PermissionMode::FullControl => Behavior::FullControl,
        PermissionMode::Custom => match baseline {
            PermissionBaseline::ReadOnly => Behavior::StrictReadOnly,
            PermissionBaseline::WorkspaceWrite => Behavior::RequestApproval,
            PermissionBaseline::FullAccess => Behavior::FullControl,
        },
    }
}

/// Map a resolved [`Behavior`] to the set of tool categories the agent may see.
/// `FullControl` exposes everything; `StrictReadOnly` exposes only reads;
/// `RequestApproval` and `AcceptEdits` expose all categories — the approval
/// popup (or the acceptEdits auto-allow) gates invocation, not visibility.
fn behavior_to_exposure(behavior: Behavior) -> ToolExposure {
    match behavior {
        Behavior::FullControl | Behavior::AcceptEdits => ToolExposure::all(),
        Behavior::RequestApproval => ToolExposure::read_only()
            .with(OperationCategory::FileEdit)
            .with(OperationCategory::Shell)
            .with(OperationCategory::Network),
        Behavior::StrictReadOnly => ToolExposure::read_only(),
    }
}

/// Concrete engine. `AgentControl` is a process-wide singleton in slab-server,
/// so per-session mode is keyed by `thread_id` to avoid multi-session races.
pub struct ExecPolicyEngine {
    modes: DashMap<String, PermissionMode>,
    baseline: PermissionBaseline,
    rules: RwLock<RuleSet>,
    /// Enterprise/policy rules — a separate, immutable partition consulted BEFORE
    /// `rules` so a policy `Block` cannot be overridden by user/workspace/global
    /// rules or by `remember` appends. Loaded once at construction.
    policy_rules: RuleSet,
    store: Arc<dyn RuleStore>,
}

impl ExecPolicyEngine {
    pub fn new(baseline: PermissionBaseline, rules: RuleSet, store: Arc<dyn RuleStore>) -> Self {
        Self {
            modes: DashMap::new(),
            baseline,
            rules: RwLock::new(rules),
            policy_rules: RuleSet::default(),
            store,
        }
    }

    /// Attach the enterprise/policy rule partition (highest precedence,
    /// read-first, immutable). Empty by default.
    #[must_use]
    pub fn with_policy_rules(mut self, policy_rules: RuleSet) -> Self {
        self.policy_rules = policy_rules;
        self
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

        // 2. Enterprise/policy rules: highest precedence, first-match-wins. A
        //    policy `Block` denies unconditionally (even under FullControl) and
        //    cannot be overridden by user/workspace/global rules or `remember`
        //    appends. Hard-deny safety above still wins over a policy `Allow`.
        if let Some(rule) =
            self.policy_rules.evaluate(category, subject, descriptor.tool_name.as_deref())
        {
            return match rule.action {
                RuleAction::Allow => ExecDecision::Allow,
                RuleAction::RequireApproval => ExecDecision::RequireApproval,
                RuleAction::Block => ExecDecision::Deny,
            };
        }

        // 3. FullControl: allow everything that survived hard-deny + policy.
        if behavior == Behavior::FullControl {
            return ExecDecision::Allow;
        }

        // 4. StrictReadOnly: mutations refused; reads allowed.
        if behavior == Behavior::StrictReadOnly {
            return match category {
                OperationCategory::ReadOnly => ExecDecision::Allow,
                _ => ExecDecision::Deny,
            };
        }

        // 5. RequestApproval / AcceptEdits share the rules-override step below;
        //    only the base differs. acceptEdits elevates the base to Allow for
        //    operations the active baseline already permits.
        let base = if behavior == Behavior::AcceptEdits {
            accept_edits_base(category, self.baseline, descriptor)
        } else {
            default_base(category)
        };

        // 6. Rules override (first-match-wins). A `Block` rule denies; `Allow`
        //    short-circuits to Allow; `RequireApproval` prompts. A remembered
        //    Allow rule is how repeat shell calls get silenced — and how an
        //    out-of-envelope op can still be auto-allowed under acceptEdits.
        let rules = self.rules.read().await;
        if let Some(rule) = rules.evaluate(category, subject, descriptor.tool_name.as_deref()) {
            return match rule.action {
                RuleAction::Allow => ExecDecision::Allow,
                RuleAction::RequireApproval => ExecDecision::RequireApproval,
                RuleAction::Block => ExecDecision::Deny,
            };
        }
        drop(rules);

        // 7. Fall back to the per-behavior base.
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

    fn permission_state_for(&self, thread_id: &str) -> PermissionStateSnapshot {
        let mode = self.mode_for(thread_id);
        let behavior = resolve_behavior(mode.effective(), self.baseline);
        let exposure = behavior_to_exposure(behavior);
        PermissionStateSnapshot { mode, baseline: self.baseline, exposure }
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

/// acceptEdits base decision: auto-allow operations the active baseline already
/// permits, prompt for the rest. Used only for `Behavior::AcceptEdits`; the
/// rules-override step still runs afterward so explicit rules still win.
fn accept_edits_base(
    category: OperationCategory,
    baseline: PermissionBaseline,
    descriptor: &OperationDescriptor,
) -> ExecDecision {
    match baseline {
        // FullAccess baseline ⇒ the envelope is everything.
        PermissionBaseline::FullAccess => ExecDecision::Allow,
        PermissionBaseline::ReadOnly => match category {
            OperationCategory::ReadOnly => ExecDecision::Allow,
            _ => ExecDecision::RequireApproval,
        },
        PermissionBaseline::WorkspaceWrite => match category {
            OperationCategory::ReadOnly => ExecDecision::Allow,
            // web_search and other network tools are outside the workspace-write
            // envelope.
            OperationCategory::Network => ExecDecision::RequireApproval,
            // Auto-allow edits scoped to the active workspace; prompt otherwise.
            OperationCategory::FileEdit => {
                if in_workspace(descriptor) {
                    ExecDecision::Allow
                } else {
                    ExecDecision::RequireApproval
                }
            }
            // Auto-allow non-destructive, non-network shell; prompt the rest.
            OperationCategory::Shell => {
                if is_shell_autorun_safe(&descriptor.subject) {
                    ExecDecision::Allow
                } else {
                    ExecDecision::RequireApproval
                }
            }
        },
    }
}

/// Whether the descriptor targets a path inside its own `workspace_root`. A
/// missing workspace, an unresolvable relative path, or a `..` traversal is
/// treated as out-of-workspace so the caller falls back to prompting (fail
/// safe). Symlink resolution is intentionally NOT performed — the sandbox owns
/// that; this is the permission-layer auto-allow heuristic.
fn in_workspace(descriptor: &OperationDescriptor) -> bool {
    let Some(root) = descriptor.workspace_root.as_deref() else {
        return false;
    };
    let root = Path::new(root);
    let subject = Path::new(&descriptor.subject);
    let absolute = if subject.is_absolute() { subject.to_path_buf() } else { root.join(subject) };
    lexical_normalize(&absolute).starts_with(lexical_normalize(root))
}

/// Lexically normalize a path (resolve `.`/`..` components) WITHOUT touching the
/// filesystem — so a not-yet-existing target (the common file-create case) still
/// resolves correctly. Leading/over-root `..` is preserved, which keeps escapes
/// like `/ws/../../etc` from matching a workspace prefix.
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut stack: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match stack.last() {
                // Pop a normal component; never pop a root/prefix.
                Some(Component::Normal(_)) => {
                    stack.pop();
                }
                // Preserve a `..` that would escape above the root so it cannot
                // match a workspace prefix.
                Some(Component::ParentDir) | None => stack.push(component),
                _ => {}
            },
            other => stack.push(other),
        }
    }
    stack.iter().map(|c| c.as_os_str()).collect::<PathBuf>()
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
    fn permission_state_for(&self, _thread_id: &str) -> PermissionStateSnapshot {
        // Consistent with "allow everything": report full control + full exposure
        // so the tool-list filter is a no-op when this stub is wired.
        PermissionStateSnapshot {
            mode: PermissionMode::FullControl,
            baseline: PermissionBaseline::FullAccess,
            exposure: ToolExposure::all(),
        }
    }
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

    /// Like [`engine_with_rules`] but also attaches the enterprise/policy
    /// partition (highest precedence, immutable).
    fn engine_with_policy(
        mode: PermissionMode,
        baseline: PermissionBaseline,
        rules: Vec<Rule>,
        policy_rules: Vec<Rule>,
        _workspace: Option<PathBuf>,
    ) -> ExecPolicyEngine {
        let dir = tempfile::tempdir().expect("dir");
        let store = Arc::new(FsRuleStore::new(dir.path().to_path_buf()));
        let engine = ExecPolicyEngine::new(baseline, RuleSet::from_rules(rules), store)
            .with_policy_rules(RuleSet::from_rules(policy_rules));
        engine.modes.insert("t1".to_owned(), mode);
        std::mem::forget(dir);
        engine
    }

    fn ws_path() -> PathBuf {
        tempfile::tempdir().expect("ws").keep()
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

    /// Security regression: a remembered prefix Allow must never silence a
    /// COMPOUND command. `cargo test && npm install x` (and the `&` background
    /// form) fall through the rule match to the base decision — the rule only
    /// ever auto-allows argument extensions of the approved command.
    #[tokio::test]
    async fn allow_rule_does_not_silence_compound_commands() {
        let engine = engine_with_rules(
            PermissionMode::RequestApproval,
            PermissionBaseline::WorkspaceWrite,
            vec![Rule::new(
                OperationCategory::Shell,
                RuleAction::Allow,
                crate::rule::RuleMatcher::Prefix,
                "cargo test",
            )],
            ws(),
        );
        for command in ["cargo test && npm install x", "cargo test & npm install x"] {
            let d = OperationDescriptor::shell(command);
            assert_eq!(
                engine.evaluate("t1", &d).await,
                ExecDecision::RequireApproval,
                "compound command must fall through to the base decision: {command}"
            );
        }
        // The exact approved command still auto-allows (argument extension).
        let d = OperationDescriptor::shell("cargo test -p slab-exec-policy");
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::Allow);
    }

    /// Security regression: a network-reaching compound is never auto-allowed
    /// under acceptEdits, even when a prefix rule covers its leading command.
    #[tokio::test]
    async fn accept_edits_never_auto_allows_network_compound_even_with_rule() {
        let engine = engine_with_rules(
            PermissionMode::ApproveForMe,
            PermissionBaseline::WorkspaceWrite,
            vec![Rule::new(
                OperationCategory::Shell,
                RuleAction::Allow,
                crate::rule::RuleMatcher::Prefix,
                "cargo test",
            )],
            ws(),
        );
        let d = OperationDescriptor::shell("cargo test && npm install x");
        // The rule does not match the compound (control chars in the suffix),
        // and the acceptEdits base refuses the network-reaching command.
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::RequireApproval);
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

    // ---- P3 Part A: ApproveForMe = acceptEdits ----

    #[tokio::test]
    async fn accept_edits_read_only_allows_reads_prompts_mutations() {
        let engine = engine_with_rules(
            PermissionMode::ApproveForMe,
            PermissionBaseline::ReadOnly,
            vec![],
            ws(),
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::read_only("/ws/a.txt")).await,
            ExecDecision::Allow
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("ls")).await,
            ExecDecision::RequireApproval
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::file_edit("/ws/a.txt")).await,
            ExecDecision::RequireApproval
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::network("query")).await,
            ExecDecision::RequireApproval
        );
    }

    #[tokio::test]
    async fn accept_edits_workspace_write_allows_in_workspace_edit() {
        let ws = ws_path();
        let engine = engine_with_rules(
            PermissionMode::ApproveForMe,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            None,
        );
        let d =
            OperationDescriptor::file_edit(ws.join("src/main.rs").to_string_lossy().to_string())
                .with_workspace(Some(ws));
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::Allow);
    }

    #[tokio::test]
    async fn accept_edits_workspace_write_prompts_out_of_workspace_edit() {
        let ws = ws_path();
        let outside = tempfile::tempdir().expect("out").keep();
        let engine = engine_with_rules(
            PermissionMode::ApproveForMe,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            None,
        );
        let d =
            OperationDescriptor::file_edit(outside.join("file.rs").to_string_lossy().to_string())
                .with_workspace(Some(ws));
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::RequireApproval);
    }

    #[tokio::test]
    async fn accept_edits_workspace_write_allows_safe_shell() {
        let engine = engine_with_rules(
            PermissionMode::ApproveForMe,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            ws(),
        );
        // Non-destructive, non-network shell auto-allowed.
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("git status")).await,
            ExecDecision::Allow
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("ls")).await,
            ExecDecision::Allow
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("cargo build")).await,
            ExecDecision::Allow
        );
    }

    #[tokio::test]
    async fn accept_edits_workspace_write_prompts_destructive_shell() {
        let engine = engine_with_rules(
            PermissionMode::ApproveForMe,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            ws(),
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("rm -rf target")).await,
            ExecDecision::RequireApproval
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("git reset --hard")).await,
            ExecDecision::RequireApproval
        );
    }

    #[tokio::test]
    async fn accept_edits_workspace_write_prompts_network_shell_and_category() {
        let engine = engine_with_rules(
            PermissionMode::ApproveForMe,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            ws(),
        );
        // Network-reaching shell prompts (outside the workspace-write envelope).
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("curl http://example.com")).await,
            ExecDecision::RequireApproval
        );
        // The Network category (web_search) also prompts.
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::network("rust async")).await,
            ExecDecision::RequireApproval
        );
    }

    #[tokio::test]
    async fn accept_edits_full_access_allows_all() {
        let engine = engine_with_rules(
            PermissionMode::ApproveForMe,
            PermissionBaseline::FullAccess,
            vec![],
            ws(),
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("cargo build")).await,
            ExecDecision::Allow
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::network("query")).await,
            ExecDecision::Allow
        );
    }

    #[tokio::test]
    async fn accept_edits_respects_block_rule() {
        // A Block rule denies even an operation the envelope would auto-allow.
        let engine = engine_with_rules(
            PermissionMode::ApproveForMe,
            PermissionBaseline::WorkspaceWrite,
            vec![Rule::new(
                OperationCategory::Shell,
                RuleAction::Block,
                crate::rule::RuleMatcher::Contains,
                "git",
            )],
            ws(),
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("git status")).await,
            ExecDecision::Deny
        );
    }

    #[tokio::test]
    async fn accept_edits_allow_rule_permits_out_of_envelope() {
        let ws = ws_path();
        let outside = tempfile::tempdir().expect("out").keep();
        let target = outside.join("file.rs").to_string_lossy().to_string();
        let engine = engine_with_rules(
            PermissionMode::ApproveForMe,
            PermissionBaseline::WorkspaceWrite,
            vec![Rule::new(
                OperationCategory::FileEdit,
                RuleAction::Allow,
                crate::rule::RuleMatcher::Exact,
                target.clone(),
            )],
            None,
        );
        let d = OperationDescriptor::file_edit(target).with_workspace(Some(ws));
        assert_eq!(engine.evaluate("t1", &d).await, ExecDecision::Allow);
    }

    #[tokio::test]
    async fn accept_edits_hard_deny_still_applies() {
        let engine = engine_with_rules(
            PermissionMode::ApproveForMe,
            PermissionBaseline::FullAccess,
            vec![],
            ws(),
        );
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("rm -rf /")).await,
            ExecDecision::Deny
        );
    }

    #[test]
    fn permission_state_accept_edits_exposes_all_categories() {
        let engine = engine_with_rules(
            PermissionMode::ApproveForMe,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            ws(),
        );
        // acceptEdits keeps every category visible (invocation is gated by
        // auto-allow/prompt, not visibility).
        assert_eq!(engine.permission_state_for("t1").exposure, ToolExposure::all());
    }

    // ---- P3 Part B: policy/enterprise scope (read-first, un-overridable) ----

    #[tokio::test]
    async fn policy_block_overrides_user_allow_rule() {
        let engine = engine_with_policy(
            PermissionMode::RequestApproval,
            PermissionBaseline::WorkspaceWrite,
            vec![Rule::new(
                OperationCategory::Shell,
                RuleAction::Allow,
                crate::rule::RuleMatcher::Prefix,
                "rm",
            )],
            vec![Rule::new(
                OperationCategory::Shell,
                RuleAction::Block,
                crate::rule::RuleMatcher::Contains,
                "rm",
            )],
            ws(),
        );
        // Policy Block wins over the user's Allow rule.
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("rm -rf target")).await,
            ExecDecision::Deny
        );
    }

    #[tokio::test]
    async fn policy_block_holds_under_full_control() {
        let engine = engine_with_policy(
            PermissionMode::FullControl,
            PermissionBaseline::FullAccess,
            vec![],
            vec![Rule::new(
                OperationCategory::Shell,
                RuleAction::Block,
                crate::rule::RuleMatcher::Contains,
                "secret",
            )],
            ws(),
        );
        // Without policy, FullControl would Allow; policy Block still denies.
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("cat secret/file")).await,
            ExecDecision::Deny
        );
    }

    #[tokio::test]
    async fn policy_allow_authoritative_under_strict_read_only() {
        let engine = engine_with_policy(
            PermissionMode::Custom,
            PermissionBaseline::ReadOnly,
            vec![],
            vec![Rule::new(
                OperationCategory::Shell,
                RuleAction::Allow,
                crate::rule::RuleMatcher::Prefix,
                "git status",
            )],
            ws(),
        );
        // Without policy, StrictReadOnly would Deny shell; policy Allow wins.
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("git status")).await,
            ExecDecision::Allow
        );
    }

    #[tokio::test]
    async fn policy_no_match_falls_through_to_mode_behavior() {
        let engine = engine_with_policy(
            PermissionMode::RequestApproval,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            vec![Rule::new(
                OperationCategory::Shell,
                RuleAction::Block,
                crate::rule::RuleMatcher::Contains,
                "forbidden",
            )],
            ws(),
        );
        // Non-matching policy rule ⇒ normal RequestApproval behavior (prompt).
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("git status")).await,
            ExecDecision::RequireApproval
        );
    }

    #[tokio::test]
    async fn remembered_allow_cannot_override_policy_block() {
        // A persisted (Always) Allow rule lands in self.rules; the immutable
        // policy partition is consulted first and still wins.
        let engine = engine_with_policy(
            PermissionMode::RequestApproval,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            vec![Rule::new(
                OperationCategory::Shell,
                RuleAction::Block,
                crate::rule::RuleMatcher::Contains,
                "rm",
            )],
            ws(),
        );
        engine
            .remember("t1", &OperationDescriptor::shell("rm -rf target"), ApprovalScope::Always)
            .await;
        assert_eq!(
            engine.evaluate("t1", &OperationDescriptor::shell("rm -rf target")).await,
            ExecDecision::Deny
        );
    }

    #[test]
    fn permission_state_read_only_hides_mutations() {
        let engine =
            engine_with_rules(PermissionMode::Custom, PermissionBaseline::ReadOnly, vec![], ws());
        let snapshot = engine.permission_state_for("t1");
        assert_eq!(snapshot.mode, PermissionMode::Custom);
        assert_eq!(snapshot.baseline, PermissionBaseline::ReadOnly);
        assert_eq!(snapshot.exposure, ToolExposure::read_only());
        assert!(!snapshot.exposure.contains(OperationCategory::Shell));
        assert!(!snapshot.exposure.contains(OperationCategory::FileEdit));
        assert!(!snapshot.exposure.contains(OperationCategory::Network));
        assert!(snapshot.exposure.contains(OperationCategory::ReadOnly));
    }

    #[test]
    fn permission_state_request_approval_exposes_everything() {
        let engine = engine_with_rules(
            PermissionMode::RequestApproval,
            PermissionBaseline::WorkspaceWrite,
            vec![],
            ws(),
        );
        // RequestApproval gates invocation via the approval popup, not
        // visibility, so every category stays exposed.
        assert_eq!(engine.permission_state_for("t1").exposure, ToolExposure::all());
    }

    #[test]
    fn permission_state_full_control_exposes_everything() {
        let engine = engine_with_rules(
            PermissionMode::FullControl,
            PermissionBaseline::FullAccess,
            vec![],
            ws(),
        );
        assert_eq!(engine.permission_state_for("t1").exposure, ToolExposure::all());
    }
}
