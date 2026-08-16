//! Disk-backed per-thread plan store backing the plan agent.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use dashmap::DashMap;
use slab_agent::{AgentError, Plan, PlanStorePort};
use slab_utils::app_home::plans_dir;
use tracing::warn;

/// Disk-backed per-thread plan store.
///
/// Keeps an in-memory `DashMap` keyed by thread id as the hot copy for live
/// `current_plan` queries, and additionally persists each plan to
/// `<plans_dir>/<thread_id>-<plan_id>.json` so authored plans survive process
/// restarts. Disk writes are best-effort: an IO error is logged but never
/// fails the plan tool — the in-memory store remains the source of truth for
/// the live turn. `clear` drops only the in-memory entry; the durable plan
/// file is kept as a user deliverable.
pub struct DiskBackedPlanStore {
    plans: DashMap<String, Plan>,
    plans_dir: PathBuf,
}

impl DiskBackedPlanStore {
    /// Build a store rooted at the given plans directory.
    pub fn new(plans_dir: PathBuf) -> Self {
        Self { plans: DashMap::new(), plans_dir }
    }

    /// Write the plan JSON atomically to `<plans_dir>/<thread_id>-<plan_id>.json`.
    fn persist(&self, thread_id: &str, plan: &Plan) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(plan).map_err(std::io::Error::other)?;
        std::fs::create_dir_all(&self.plans_dir)?;
        let final_path = self.plans_dir.join(format!("{}-{}.json", thread_id, plan.plan_id));
        write_atomic(&final_path, &bytes)
    }
}

impl Default for DiskBackedPlanStore {
    fn default() -> Self {
        Self::new(plans_dir())
    }
}

#[async_trait]
impl PlanStorePort for DiskBackedPlanStore {
    async fn replace_plan(&self, thread_id: &str, plan: Plan) -> Result<(), AgentError> {
        if let Err(error) = self.persist(thread_id, &plan) {
            warn!(thread_id, error = %error, "failed to persist plan to disk");
        }
        self.plans.insert(thread_id.to_owned(), plan);
        Ok(())
    }

    async fn current_plan(&self, thread_id: &str) -> Option<Plan> {
        self.plans.get(thread_id).map(|entry| entry.clone())
    }

    async fn clear(&self, thread_id: &str) {
        // Drop the in-memory entry only; the durable plan file is a user
        // deliverable and intentionally kept on disk.
        self.plans.remove(thread_id);
    }
}

/// Best-effort atomic write: write to a sibling temp file then rename over the
/// target. `std::fs::rename` replaces an existing destination on both Unix and
/// Windows, so a crash mid-write never leaves a partially-written plan file.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "plan path has no parent dir")
    })?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("plan");
    let tmp = dir.join(format!(".{stem}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_agent::{PlanCounts, PlanItem, PlanStatus};

    fn sample_plan(plan_id: &str) -> Plan {
        Plan {
            plan_id: plan_id.to_owned(),
            summary: Some("demo".to_owned()),
            items: vec![PlanItem {
                step: "do thing".to_owned(),
                status: PlanStatus::Pending,
                depends_on: None,
                result_ref: None,
            }],
            counts: PlanCounts { pending: 1, in_progress: 0, completed: 0, blocked: 0 },
            current_step: None,
        }
    }

    #[tokio::test]
    async fn replace_plan_writes_durable_file_and_keeps_hot_copy() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = DiskBackedPlanStore::new(tmp.path().to_owned());

        store.replace_plan("thread-1", sample_plan("plan-1")).await.expect("replace");

        // Hot copy is queryable.
        let current = store.current_plan("thread-1").await.expect("plan present");
        assert_eq!(current.plan_id, "plan-1");

        // Durable file exists with the right content.
        let file = tmp.path().join("thread-1-plan-1.json");
        let bytes = std::fs::read(&file).expect("plan file written");
        let persisted: Plan = serde_json::from_slice(&bytes).expect("plan file parses");
        assert_eq!(persisted.plan_id, "plan-1");
        assert_eq!(persisted.items.len(), 1);
    }

    #[tokio::test]
    async fn clear_drops_hot_copy_but_keeps_durable_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = DiskBackedPlanStore::new(tmp.path().to_owned());
        store.replace_plan("thread-1", sample_plan("plan-1")).await.expect("replace");
        let file = tmp.path().join("thread-1-plan-1.json");
        assert!(file.exists());

        store.clear("thread-1").await;

        assert!(store.current_plan("thread-1").await.is_none(), "hot copy cleared");
        assert!(file.exists(), "durable plan file kept after clear");
    }

    #[tokio::test]
    async fn replace_plan_does_not_fail_when_disk_write_errors() {
        // Pointing plans_dir at a path under a file (not a dir) makes
        // create_dir_all fail — the store must still accept the plan in memory.
        let blocker = tempfile::tempdir().expect("tempdir");
        let blocker_file = blocker.path().join("not-a-dir");
        std::fs::write(&blocker_file, b"x").expect("blocker");
        let store = DiskBackedPlanStore::new(blocker_file.join("plans"));

        store
            .replace_plan("thread-1", sample_plan("plan-1"))
            .await
            .expect("replace must not fail on IO error");
        assert_eq!(store.current_plan("thread-1").await.expect("present").plan_id, "plan-1");
    }
}
