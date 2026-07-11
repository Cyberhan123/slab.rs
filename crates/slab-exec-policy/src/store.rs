//! Rule persistence port + default filesystem store.
//!
//! Lazy loading (per the design): only `default.rules` (global) and the
//! current workspace's `hash-<workspace>.rules` are loaded — never the whole
//! rules directory. A DB-backed impl in `slab-app-core` wraps this to record
//! the `hash-<workspace>.rules → absolute workspace path` mapping.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use ring::digest;
use tokio::fs as afs;
use tracing::warn;

use crate::decision::ApprovalScope;
use crate::rule::{Rule, RuleError, RuleSet};

const DEFAULT_RULES_FILE: &str = "default.rules";
const HASH_PREFIX: &str = "hash-";

#[derive(Debug, thiserror::Error)]
pub enum RuleStoreError {
    #[error("rule I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("rule parse error: {0}")]
    Parse(#[from] RuleError),
    #[error("rule store error: {0}")]
    Other(String),
}

/// Persists and loads permission rules. The concrete DB-backed impl lives in
/// `slab-app-core`; [`FsRuleStore`] is the filesystem-only default.
#[async_trait]
pub trait RuleStore: Send + Sync {
    /// Load `default.rules` + the current workspace's `hash-<workspace>.rules`.
    async fn load(&self, workspace_root: Option<&Path>) -> Result<RuleSet, RuleStoreError>;

    /// Persist a rule according to `scope` (no-op for `RunOnce`/`Deny`).
    async fn store(
        &self,
        workspace_root: Option<&Path>,
        rule: &Rule,
        scope: ApprovalScope,
    ) -> Result<(), RuleStoreError>;

    /// Record the `hash-<workspace>.rules → absolute workspace path` mapping
    /// (DB-backed impls override; the filesystem store is a no-op).
    async fn remember_workspace(
        &self,
        _rules_filename: &str,
        _workspace_path: &Path,
    ) -> Result<(), RuleStoreError> {
        Ok(())
    }
}

/// Filesystem-only rule store. Rules live under `rules_dir`:
/// - `default.rules` — global rules (`Always` scope).
/// - `hash-<sha8(workspace)>.rules` — per-workspace rules (`AlwaysInWorkspace`).
pub struct FsRuleStore {
    pub rules_dir: PathBuf,
}

impl FsRuleStore {
    pub fn new(rules_dir: impl Into<PathBuf>) -> Self {
        Self { rules_dir: rules_dir.into() }
    }

    fn default_path(&self) -> PathBuf {
        self.rules_dir.join(DEFAULT_RULES_FILE)
    }

    fn workspace_path(&self, workspace_root: &Path) -> PathBuf {
        self.rules_dir.join(format!("{HASH_PREFIX}{}.rules", workspace_hash(workspace_root)))
    }
}

#[async_trait]
impl RuleStore for FsRuleStore {
    async fn load(&self, workspace_root: Option<&Path>) -> Result<RuleSet, RuleStoreError> {
        let mut rules = RuleSet::default();

        // Workspace-specific rules first (more specific wins on first-match).
        if let Some(root) = workspace_root {
            let path = self.workspace_path(root);
            if path.exists() {
                rules = RuleSet::from_file(&path)?;
            }
        }

        // Then global default rules.
        let default_path = self.default_path();
        if default_path.exists() {
            let default_rules = RuleSet::from_file(&default_path)?;
            for rule in default_rules.rules() {
                rules.append(rule.clone());
            }
        }

        Ok(rules)
    }

    async fn store(
        &self,
        workspace_root: Option<&Path>,
        rule: &Rule,
        scope: ApprovalScope,
    ) -> Result<(), RuleStoreError> {
        let target = match scope {
            ApprovalScope::Always => self.default_path(),
            ApprovalScope::AlwaysInWorkspace => match workspace_root {
                Some(root) => self.workspace_path(root),
                None => {
                    warn!(
                        "cannot persist AlwaysInWorkspace rule without a workspace root; falling back to default.rules"
                    );
                    self.default_path()
                }
            },
            ApprovalScope::RunOnce | ApprovalScope::Deny => return Ok(()),
        };

        ensure_dir(&self.rules_dir).await?;
        let line = format!("{}\n", rule.to_line());
        if target.exists() {
            let existing = afs::read_to_string(&target).await?;
            let mut content = existing;
            if !content.ends_with('\n') && !content.is_empty() {
                content.push('\n');
            }
            content.push_str(&line);
            afs::write(&target, content).await?;
        } else {
            afs::write(&target, line).await?;
        }
        Ok(())
    }
}

/// Stable short hash (first 8 hex chars of SHA-256) of the canonical workspace
/// path. Used as the per-workspace rules filename so a workspace maps to one
/// stable file across runs.
pub fn workspace_hash(workspace_root: &Path) -> String {
    let canonical =
        dunce::canonicalize(workspace_root).unwrap_or_else(|_| workspace_root.to_path_buf());
    let bytes = canonical.to_string_lossy().to_lowercase();
    let digest = digest::digest(&digest::SHA256, bytes.as_bytes());
    hex8(digest.as_ref())
}

fn hex8(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(8);
    for byte in bytes.iter().take(4) {
        out.push(hex_nibble(byte >> 4));
        out.push(hex_nibble(byte & 0x0f));
    }
    out
}

fn hex_nibble(n: u8) -> char {
    if n < 10 { (b'0' + n) as char } else { (b'a' + (n - 10)) as char }
}

async fn ensure_dir(dir: &Path) -> Result<(), RuleStoreError> {
    if !dir.exists() {
        afs::create_dir_all(dir).await?;
    }
    Ok(())
}

/// The rules filename a workspace would map to (exposed so the DB-backed store
/// can record the mapping before/after writing the file).
pub fn workspace_rules_filename(workspace_root: &Path) -> String {
    format!("{HASH_PREFIX}{}.rules", workspace_hash(workspace_root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::OperationCategory;
    use crate::rule::{RuleAction, RuleMatcher};

    #[tokio::test]
    async fn always_scope_writes_default_rules() {
        let dir = tempfile::tempdir().expect("dir");
        let store = FsRuleStore::new(dir.path());
        let rule = Rule::new(
            OperationCategory::Shell,
            RuleAction::Allow,
            RuleMatcher::Prefix,
            "cargo check",
        );
        store.store(None, &rule, ApprovalScope::Always).await.expect("store");

        let loaded = store.load(None).await.expect("load");
        assert_eq!(
            loaded.evaluate(OperationCategory::Shell, "cargo check -p x").map(|r| r.action),
            Some(RuleAction::Allow)
        );
        assert!(dir.path().join(DEFAULT_RULES_FILE).exists());
    }

    #[tokio::test]
    async fn always_in_workspace_writes_hashed_file_and_loads() {
        let dir = tempfile::tempdir().expect("dir");
        let workspace = dir.path().join("ws");
        std::fs::create_dir_all(&workspace).expect("ws");
        let store = FsRuleStore::new(dir.path().join("rules"));
        let rule = Rule::new(
            OperationCategory::Shell,
            RuleAction::Allow,
            RuleMatcher::Prefix,
            "cargo test",
        );
        store
            .store(Some(&workspace), &rule, ApprovalScope::AlwaysInWorkspace)
            .await
            .expect("store");

        let expected_name = workspace_rules_filename(&workspace);
        assert!(dir.path().join("rules").join(&expected_name).exists(), "hashed file should exist");

        let loaded = store.load(Some(&workspace)).await.expect("load");
        assert_eq!(
            loaded.evaluate(OperationCategory::Shell, "cargo test -p x").map(|r| r.action),
            Some(RuleAction::Allow)
        );
    }

    #[tokio::test]
    async fn run_once_writes_nothing() {
        let dir = tempfile::tempdir().expect("dir");
        let store = FsRuleStore::new(dir.path());
        let rule = Rule::new(
            OperationCategory::Shell,
            RuleAction::Allow,
            RuleMatcher::Prefix,
            "cargo check",
        );
        store.store(None, &rule, ApprovalScope::RunOnce).await.expect("store");
        assert!(!dir.path().join(DEFAULT_RULES_FILE).exists());
    }

    #[test]
    fn workspace_hash_is_stable_and_distinct() {
        let a = tempfile::tempdir().expect("a");
        let b = tempfile::tempdir().expect("b");
        let ha = workspace_hash(a.path());
        let hb = workspace_hash(b.path());
        assert_eq!(ha.len(), 8);
        assert_eq!(ha, workspace_hash(a.path()));
        assert_ne!(ha, hb);
    }
}
