//! `slab-exec-policy` — the unified permission decision engine for slab agent
//! tools.
//!
//! This crate is the single owner of every Allow/RequireApproval/Deny verdict.
//! The shell policy, the kernel risk analyzer, and the sandbox are all demoted
//! to inputs feeding [`engine::ExecPolicyPort`]. It also owns the global,
//! per-category rule system (migrated + generalized from `slab-shell-command`)
//! and the per-session permission mode + popup persistence scope vocabulary.
//!
//! Dependency position: `slab-agent → slab-exec-policy → slab-sandboxing`.

pub mod category;
pub mod compat;
pub mod decision;
pub mod engine;
pub mod exposure;
pub mod rule;
pub mod safety;
pub mod store;

pub use category::{OperationCategory, OperationDescriptor};
pub use compat::ShellPolicy;
pub use decision::{
    ApprovalScope, ExecDecision, InteractionMode, PermissionBaseline, PermissionMode,
};
pub use engine::{AllowAllExecPolicy, ExecPolicyEngine, ExecPolicyPort};
pub use exposure::{PermissionStateSnapshot, ToolExposure, interaction_constraint};
pub use rule::{Rule, RuleAction, RuleError, RuleMatcher, RuleSet, RuleSource};
pub use safety::{CommandSafetyChecker, SafetyDecision, is_destructive_command, is_sensitive_path};
pub use store::{FsRuleStore, RuleStore, RuleStoreError, workspace_hash, workspace_rules_filename};
