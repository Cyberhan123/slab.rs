//! In-memory per-thread plan store backing Plan interaction mode.

use async_trait::async_trait;
use dashmap::DashMap;
use slab_agent::{AgentError, Plan, PlanStorePort};

/// In-memory per-thread plan store.
///
/// Keyed by thread id so plans are isolated per thread. Lives for the process
/// lifetime; per-thread entries are cleared on thread teardown via
/// `AgentControl::clear_thread_mode` (which calls `clear`). This is the
/// concrete host adapter for [`PlanStorePort`] — the durable source of truth a
/// thread authors with the `plan` / `update_plan` tools and presents via
/// `present_plan`.
#[derive(Default)]
pub struct InMemoryPlanStore {
    plans: DashMap<String, Plan>,
}

#[async_trait]
impl PlanStorePort for InMemoryPlanStore {
    async fn replace_plan(&self, thread_id: &str, plan: Plan) -> Result<(), AgentError> {
        self.plans.insert(thread_id.to_owned(), plan);
        Ok(())
    }

    async fn current_plan(&self, thread_id: &str) -> Option<Plan> {
        self.plans.get(thread_id).map(|entry| entry.clone())
    }

    async fn clear(&self, thread_id: &str) {
        self.plans.remove(thread_id);
    }
}
