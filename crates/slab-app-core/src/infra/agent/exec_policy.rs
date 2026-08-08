//! Concrete `slab-exec-policy` wiring for the app host: a DB-backed rule store
//! (filesystem rules + a SQLite `hash-<workspace>.rules → path` mapping) and a
//! builder that constructs the [`ExecPolicyEngine`] injected into `AgentControl`.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use slab_exec_policy::{
    ApprovalScope, ExecPolicyEngine, ExecPolicyPort, FsRuleStore, PermissionBaseline, Rule,
    RuleSet, RuleStore, RuleStoreError,
};
use tracing::warn;

use crate::infra::db::repository::{AnyStore, ExecRuleWorkspaceStore};

/// DB-backed rule store: filesystem rules (`default.rules` +
/// `hash-<workspace>.rules`) plus a SQLite mapping from per-workspace rules
/// filenames to absolute workspace paths.
pub struct DbRuleStore {
    fs: FsRuleStore,
    db: Arc<AnyStore>,
}

impl DbRuleStore {
    pub fn new(rules_dir: PathBuf, db: Arc<AnyStore>) -> Self {
        Self { fs: FsRuleStore::new(rules_dir), db }
    }
}

#[async_trait]
impl RuleStore for DbRuleStore {
    async fn load(
        &self,
        workspace_root: Option<&std::path::Path>,
    ) -> Result<RuleSet, RuleStoreError> {
        self.fs.load(workspace_root).await
    }

    async fn store(
        &self,
        workspace_root: Option<&std::path::Path>,
        rule: &Rule,
        scope: ApprovalScope,
    ) -> Result<(), RuleStoreError> {
        self.fs.store(workspace_root, rule, scope).await?;
        // Record the workspace mapping so the engine can resolve the hashed
        // rules file back to an absolute workspace path later.
        if scope == ApprovalScope::AlwaysInWorkspace
            && let Some(root) = workspace_root
        {
            let filename = slab_exec_policy::workspace_rules_filename(root);
            if let Err(error) = self.db.remember_workspace(&filename, root).await {
                warn!(error = %error, "failed to record workspace rule mapping");
            }
        }
        Ok(())
    }

    async fn remember_workspace(
        &self,
        rules_filename: &str,
        workspace_path: &std::path::Path,
    ) -> Result<(), RuleStoreError> {
        self.db
            .remember_workspace(rules_filename, workspace_path)
            .await
            .map_err(|error| RuleStoreError::Other(error.to_string()))
    }
}

/// Convert the config-side baseline enum into the runtime policy baseline.
pub fn baseline_from_config(baseline: slab_config::AgentPermissionBaseline) -> PermissionBaseline {
    match baseline {
        slab_config::AgentPermissionBaseline::ReadOnly => PermissionBaseline::ReadOnly,
        slab_config::AgentPermissionBaseline::WorkspaceWrite => PermissionBaseline::WorkspaceWrite,
        slab_config::AgentPermissionBaseline::FullAccess => PermissionBaseline::FullAccess,
    }
}

/// Convert the wire `ApprovalScope` (from `slab-proto`) into the runtime type.
pub fn approval_scope_from_proto(scope: slab_proto::harness::ApprovalScope) -> ApprovalScope {
    match scope {
        slab_proto::harness::ApprovalScope::RunOnce => ApprovalScope::RunOnce,
        slab_proto::harness::ApprovalScope::AlwaysInWorkspace => ApprovalScope::AlwaysInWorkspace,
        slab_proto::harness::ApprovalScope::Always => ApprovalScope::Always,
        slab_proto::harness::ApprovalScope::Deny => ApprovalScope::Deny,
    }
}

/// Convert the wire `PermissionMode` (from `slab-proto`) into the runtime type.
pub fn permission_mode_from_proto(
    mode: slab_proto::harness::PermissionMode,
) -> slab_exec_policy::PermissionMode {
    match mode {
        slab_proto::harness::PermissionMode::RequestApproval => {
            slab_exec_policy::PermissionMode::RequestApproval
        }
        slab_proto::harness::PermissionMode::ApproveForMe => {
            slab_exec_policy::PermissionMode::ApproveForMe
        }
        slab_proto::harness::PermissionMode::FullControl => {
            slab_exec_policy::PermissionMode::FullControl
        }
        slab_proto::harness::PermissionMode::Custom => slab_exec_policy::PermissionMode::Custom,
    }
}

/// Build the [`ExecPolicyEngine`] wired with the DB-backed rule store and the
/// current workspace's rules. Loads `default.rules` + the workspace's
/// `hash-<workspace>.rules` lazily (never the whole directory). Synchronous
/// because the app-context constructor that calls bootstrap is synchronous.
pub fn build_exec_policy_engine(
    baseline: PermissionBaseline,
    rules_dir: PathBuf,
    db: Arc<AnyStore>,
    workspace_root: Option<PathBuf>,
) -> Arc<dyn ExecPolicyPort> {
    let store = Arc::new(DbRuleStore::new(rules_dir.clone(), db));
    let rules = load_rules_sync(&rules_dir, workspace_root.as_deref());
    Arc::new(ExecPolicyEngine::new(baseline, rules, store))
}

/// Synchronously load `default.rules` + the current workspace's
/// `hash-<workspace>.rules` (workspace rules first, so the more specific set
/// wins on first-match). Missing files are treated as empty.
fn load_rules_sync(
    rules_dir: &std::path::Path,
    workspace_root: Option<&std::path::Path>,
) -> RuleSet {
    let mut rules = RuleSet::default();

    if let Some(root) = workspace_root {
        let path = rules_dir.join(slab_exec_policy::workspace_rules_filename(root));
        if path.exists() {
            match RuleSet::from_file(&path) {
                Ok(ws_rules) => rules = ws_rules,
                Err(error) => warn!(error = %error, ?path, "failed to load workspace rules"),
            }
        }
    }

    let default_path = rules_dir.join("default.rules");
    if default_path.exists() {
        match RuleSet::from_file(&default_path) {
            Ok(default_rules) => {
                for rule in default_rules.rules() {
                    rules.append(rule.clone());
                }
            }
            Err(error) => warn!(error = %error, ?default_path, "failed to load default rules"),
        }
    }

    rules
}
