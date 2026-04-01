use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::sync::{
    OwnedRwLockReadGuard, OwnedRwLockWriteGuard, OwnedSemaphorePermit, Semaphore, broadcast, mpsc,
};
use tracing::warn;

use crate::internal::scheduler::backend::protocol::{BackendRequest, WorkerCommand};
use crate::internal::scheduler::backend::runner::{SharedIngressRx, shared_ingress};
use crate::internal::scheduler::types::{BackendLifecycleState, CoreError, GlobalConsistencyState};

/// Inference lease: blocks management mutations and holds compute quota.
pub struct InferenceLease {
    #[allow(dead_code)]
    mgmt_guard: OwnedRwLockReadGuard<()>,
    #[allow(dead_code)]
    compute_permit: OwnedSemaphorePermit,
}

impl std::fmt::Debug for InferenceLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InferenceLease").finish()
    }
}

/// Management lease: exclusive configuration lock for initialize/load/unload.
pub struct ManagementLease {
    #[allow(dead_code)]
    mgmt_guard: OwnedRwLockWriteGuard<()>,
}

impl std::fmt::Debug for ManagementLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagementLease").finish()
    }
}

#[derive(Debug, Clone)]
struct BackendHandle {
    semaphore: Arc<Semaphore>,
    ingress_tx: Option<mpsc::Sender<BackendRequest>>,
    #[cfg_attr(not(test), allow(dead_code))]
    control_tx: Option<broadcast::Sender<WorkerCommand>>,
    management_lock: Arc<tokio::sync::RwLock<()>>,
    lifecycle: Arc<tokio::sync::RwLock<BackendLifecycleState>>,
    next_seq: Arc<AtomicU64>,
}

impl BackendHandle {
    fn new(
        capacity: usize,
        ingress_tx: Option<mpsc::Sender<BackendRequest>>,
        control_tx: Option<broadcast::Sender<WorkerCommand>>,
    ) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(capacity)),
            ingress_tx,
            control_tx,
            management_lock: Arc::new(tokio::sync::RwLock::new(())),
            lifecycle: Arc::new(tokio::sync::RwLock::new(BackendLifecycleState::Uninitialized)),
            next_seq: Arc::new(AtomicU64::new(1)),
        }
    }
}

/// Manages backend admission, queue handles, management locks and consistency state.
#[derive(Debug, Clone)]
pub struct ResourceManager {
    config: ResourceManagerConfig,
    backends: Arc<RwLock<HashMap<String, BackendHandle>>>,
    global_state: Arc<tokio::sync::RwLock<GlobalConsistencyState>>,
    #[cfg_attr(not(test), allow(dead_code))]
    next_global_op_id: Arc<AtomicU64>,
}

#[derive(Debug, Clone)]
pub struct ResourceManagerConfig {
    pub backend_capacity: usize,
    pub ingress_channel_capacity: usize,
    pub control_channel_capacity: usize,
}

impl Default for ResourceManagerConfig {
    fn default() -> Self {
        Self { backend_capacity: 4, ingress_channel_capacity: 128, control_channel_capacity: 16 }
    }
}

impl ResourceManager {
    /// Create an empty `ResourceManager`.
    pub fn new() -> Self {
        Self::with_config(ResourceManagerConfig::default())
    }

    pub fn with_config(config: ResourceManagerConfig) -> Self {
        Self {
            config,
            backends: Arc::new(RwLock::new(HashMap::new())),
            global_state: Arc::new(tokio::sync::RwLock::new(GlobalConsistencyState::Consistent)),
            next_global_op_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Register a backend using runtime-owned channels and bootstrap callback.
    pub fn register_backend<F>(&mut self, backend_id: impl Into<String>, spawn_backend: F)
    where
        F: FnOnce(SharedIngressRx, broadcast::Sender<WorkerCommand>),
    {
        let (ingress_tx, ingress_rx) =
            mpsc::channel::<BackendRequest>(self.config.ingress_channel_capacity);
        let (control_tx, _) =
            broadcast::channel::<WorkerCommand>(self.config.control_channel_capacity);
        let shared_ingress_rx = shared_ingress(ingress_rx);
        spawn_backend(Arc::clone(&shared_ingress_rx), control_tx.clone());

        let key = backend_id.into();
        self.backends.write().expect("backend map poisoned").insert(
            key,
            BackendHandle::new(self.config.backend_capacity, Some(ingress_tx), Some(control_tx)),
        );
    }

    fn handle(&self, backend_id: &str) -> Result<BackendHandle, CoreError> {
        self.backends
            .read()
            .expect("backend map poisoned")
            .get(backend_id)
            .cloned()
            .ok_or_else(|| CoreError::DriverNotRegistered { driver_id: backend_id.to_owned() })
    }

    /// List registered backends in deterministic order.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn backend_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> =
            self.backends.read().expect("backend map poisoned").keys().cloned().collect();
        ids.sort();
        ids
    }

    /// Clone backend ingress sender.
    pub fn ingress_tx(&self, backend_id: &str) -> Result<mpsc::Sender<BackendRequest>, CoreError> {
        let handle = self.handle(backend_id)?;
        handle.ingress_tx.ok_or_else(|| CoreError::Busy { backend_id: backend_id.to_owned() })
    }

    /// Clone backend control sender.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn control_tx(
        &self,
        backend_id: &str,
    ) -> Result<broadcast::Sender<WorkerCommand>, CoreError> {
        let handle = self.handle(backend_id)?;
        handle.control_tx.ok_or_else(|| CoreError::Busy { backend_id: backend_id.to_owned() })
    }

    /// Monotonic management sequence id per backend stream.
    pub fn next_seq(&self, backend_id: &str) -> Result<u64, CoreError> {
        let handle = self.handle(backend_id)?;
        Ok(handle.next_seq.fetch_add(1, Ordering::Relaxed))
    }

    async fn acquire_compute_permit(
        &self,
        handle: &BackendHandle,
        backend_id: &str,
        timeout: Duration,
    ) -> Result<OwnedSemaphorePermit, CoreError> {
        tokio::time::timeout(timeout, Arc::clone(&handle.semaphore).acquire_owned())
            .await
            .map_err(|_| CoreError::Timeout)?
            .map_err(|_| CoreError::Busy { backend_id: backend_id.to_owned() })
    }

    /// Acquire inference lease: read management lock + compute quota.
    pub async fn acquire_inference_lease(
        &self,
        backend_id: &str,
        timeout: Duration,
    ) -> Result<InferenceLease, CoreError> {
        self.ensure_inference_allowed().await?;
        let handle = self.handle(backend_id)?;
        let compute_permit = self.acquire_compute_permit(&handle, backend_id, timeout).await?;
        let mgmt_guard = Arc::clone(&handle.management_lock).read_owned().await;

        Ok(InferenceLease { mgmt_guard, compute_permit })
    }

    /// Acquire exclusive management lease for initialize/load/unload operations.
    pub async fn acquire_management_lease(
        &self,
        backend_id: &str,
    ) -> Result<ManagementLease, CoreError> {
        let handle = self.handle(backend_id)?;
        let mgmt_guard = Arc::clone(&handle.management_lock).write_owned().await;
        Ok(ManagementLease { mgmt_guard })
    }

    /// Set backend lifecycle state.
    pub async fn set_backend_state(
        &self,
        backend_id: &str,
        state: BackendLifecycleState,
    ) -> Result<(), CoreError> {
        let handle = self.handle(backend_id)?;
        *handle.lifecycle.write().await = state;
        Ok(())
    }

    /// If inconsistent, reject inference submission.
    pub async fn ensure_inference_allowed(&self) -> Result<(), CoreError> {
        let guard = self.global_state.read().await;
        if let GlobalConsistencyState::Inconsistent { op_id } = *guard {
            return Err(CoreError::GlobalStateInconsistent { op_id });
        }
        Ok(())
    }

    /// Transition to reconciling state and return operation id.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn begin_global_reconcile(&self) -> u64 {
        let op_id = self.next_global_op_id.fetch_add(1, Ordering::Relaxed);
        *self.global_state.write().await = GlobalConsistencyState::Reconciling;
        op_id
    }

    /// Mark system globally consistent.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn mark_global_consistent(&self) {
        *self.global_state.write().await = GlobalConsistencyState::Consistent;
    }

    /// Mark system inconsistent so test submissions are rejected.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn mark_global_inconsistent(&self, op_id: u64) {
        *self.global_state.write().await = GlobalConsistencyState::Inconsistent { op_id };
    }

    /// Operator override to recover from inconsistent state.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn manual_mark_consistent(&self, reason: &str) {
        warn!(%reason, "manual global consistency override requested");
        self.mark_global_consistent().await;
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}
