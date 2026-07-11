use std::future::Future;
use std::path::PathBuf;

use super::AnyStore;
use chrono::Utc;

/// Persistent mapping from a per-workspace rules filename (`hash-<workspace>.rules`)
/// to its absolute workspace path. Lets the exec-policy engine lazy-load the
/// right per-workspace rules file without scanning the whole rules directory.
pub trait ExecRuleWorkspaceStore: Send + Sync + 'static {
    fn remember_workspace(
        &self,
        rules_filename: &str,
        workspace_path: &std::path::Path,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    fn resolve_workspace(
        &self,
        rules_filename: &str,
    ) -> impl Future<Output = Result<Option<PathBuf>, sqlx::Error>> + Send;
}

impl ExecRuleWorkspaceStore for AnyStore {
    async fn remember_workspace(
        &self,
        rules_filename: &str,
        workspace_path: &std::path::Path,
    ) -> Result<(), sqlx::Error> {
        let workspace = workspace_path.to_string_lossy().to_string();
        let created_at = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR REPLACE INTO exec_rule_workspaces (rules_filename, workspace_path, created_at) \
             VALUES (?1, ?2, ?3)",
        )
        .bind(rules_filename)
        .bind(&workspace)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn resolve_workspace(
        &self,
        rules_filename: &str,
    ) -> Result<Option<PathBuf>, sqlx::Error> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT workspace_path FROM exec_rule_workspaces WHERE rules_filename = ?1",
        )
        .bind(rules_filename)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(path,)| PathBuf::from(path)))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{AnyStore, ExecRuleWorkspaceStore};
    use crate::test_support::migrated_test_store;

    #[tokio::test]
    async fn remember_and_resolve_workspace_round_trips() {
        let store: AnyStore = migrated_test_store().await;
        let filename = "hash-deadbeef.rules";
        let path = std::path::Path::new("/workspace/alpha");

        store.remember_workspace(filename, path).await.expect("remember workspace");

        let resolved = store.resolve_workspace(filename).await.expect("resolve workspace");
        assert_eq!(resolved, Some(PathBuf::from("/workspace/alpha")));

        // Unknown filename resolves to None.
        let missing = store.resolve_workspace("hash-unknown.rules").await.expect("resolve");
        assert_eq!(missing, None);
    }

    #[tokio::test]
    async fn remember_workspace_is_idempotent() {
        let store: AnyStore = migrated_test_store().await;
        let filename = "hash-cafe.rules";
        let path = std::path::Path::new("/workspace/beta");

        store.remember_workspace(filename, path).await.expect("first");
        store.remember_workspace(filename, path).await.expect("second (upsert)");

        let resolved = store.resolve_workspace(filename).await.expect("resolve");
        assert_eq!(resolved, Some(PathBuf::from("/workspace/beta")));
    }
}
