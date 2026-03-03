use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::sync::{
    broadcast, mpsc, Mutex, OwnedMutexGuard, OwnedRwLockReadGuard, OwnedRwLockWriteGuard,
    OwnedSemaphorePermit, Semaphore,
};
use tracing::warn;

use crate::runtime::backend::protocol::{BackendRequest, WorkerCommand};
use crate::runtime::types::{
    BackendLifecycleState, FailedGlobalOperation, GlobalConsistencyState, RuntimeError,
};

/// RAII guard that releases a semaphore slot when dropped.
///
/// Callers must hold this until the corresponding backend request completes.
pub struct Permit {
    /// Owned permit; dropping this struct releases it back to the semaphore.
    #[allow(dead_code)]
    permit: OwnedSemaphorePermit,
}

impl std::fmt::Debug for Permit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Permit").finish()
    }
}

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
    backends: Arc<RwLock<HashMap<String, BackendHandle>>>,
    global_state: Arc<tokio::sync::RwLock<GlobalConsistencyState>>,
    failed_global: Arc<tokio::sync::RwLock<Option<FailedGlobalOperation>>>,
    global_management: Arc<Mutex<()>>,
    generation: Arc<AtomicU64>,
    next_global_op_id: Arc<AtomicU64>,
}

impl ResourceManager {
    /// Create an empty `ResourceManager`.
    pub fn new() -> Self {
        Self {
            backends: Arc::new(RwLock::new(HashMap::new())),
            global_state: Arc::new(tokio::sync::RwLock::new(GlobalConsistencyState::Consistent {
                generation: 0,
            })),
            failed_global: Arc::new(tokio::sync::RwLock::new(None)),
            global_management: Arc::new(Mutex::new(())),
            generation: Arc::new(AtomicU64::new(0)),
            next_global_op_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Register backend compute resources only.
    ///
    /// This keeps compatibility with existing tests that only assert semaphore behavior.
    pub fn register_backend(&mut self, backend_id: impl Into<String>, capacity: usize) {
        let key = backend_id.into();
        self.backends
            .write()
            .expect("backend map poisoned")
            .insert(key, BackendHandle::new(capacity, None, None));
    }

    /// Register backend resources and queue/control channels managed by runtime.
    pub fn register_backend_runtime(
        &mut self,
        backend_id: impl Into<String>,
        capacity: usize,
        ingress_tx: mpsc::Sender<BackendRequest>,
        control_tx: Option<broadcast::Sender<WorkerCommand>>,
    ) {
        let key = backend_id.into();
        self.backends
            .write()
            .expect("backend map poisoned")
            .insert(
                key,
                BackendHandle::new(capacity, Some(ingress_tx), control_tx),
            );
    }

    fn handle(&self, backend_id: &str) -> Result<BackendHandle, RuntimeError> {
        self.backends
            .read()
            .expect("backend map poisoned")
            .get(backend_id)
            .cloned()
            .ok_or_else(|| RuntimeError::Busy {
                backend_id: backend_id.to_owned(),
            })
    }

    /// List registered backends in deterministic order.
    pub fn backend_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .backends
            .read()
            .expect("backend map poisoned")
            .keys()
            .cloned()
            .collect();
        ids.sort();
        ids
    }

    /// Clone backend ingress sender.
    pub fn ingress_tx(&self, backend_id: &str) -> Result<mpsc::Sender<BackendRequest>, RuntimeError> {
        let handle = self.handle(backend_id)?;
        handle.ingress_tx.ok_or_else(|| RuntimeError::Busy {
            backend_id: backend_id.to_owned(),
        })
    }

    /// Clone backend control sender.
    pub fn control_tx(
        &self,
        backend_id: &str,
    ) -> Result<broadcast::Sender<WorkerCommand>, RuntimeError> {
        let handle = self.handle(backend_id)?;
        handle.control_tx.ok_or_else(|| RuntimeError::Busy {
            backend_id: backend_id.to_owned(),
        })
    }

    /// Monotonic management sequence id per backend stream.
    pub fn next_seq(&self, backend_id: &str) -> Result<u64, RuntimeError> {
        let handle = self.handle(backend_id)?;
        Ok(handle.next_seq.fetch_add(1, Ordering::Relaxed))
    }

    /// Try to acquire compute permit for `backend_id`.
    pub fn try_acquire(&self, backend_id: &str) -> Result<Permit, RuntimeError> {
        let handle = self.handle(backend_id)?;
        handle
            .semaphore
            .try_acquire_owned()
            .map(|permit| Permit { permit })
            .map_err(|_| RuntimeError::Busy {
                backend_id: backend_id.to_owned(),
            })
    }

    /// Acquire compute permit with timeout.
    pub async fn acquire_with_timeout(
        &self,
        backend_id: &str,
        timeout: Duration,
    ) -> Result<Permit, RuntimeError> {
        let handle = self.handle(backend_id)?;
        tokio::time::timeout(timeout, handle.semaphore.acquire_owned())
            .await
            .map_err(|_| RuntimeError::Timeout)?
            .map(|permit| Permit { permit })
            .map_err(|_| RuntimeError::Busy {
                backend_id: backend_id.to_owned(),
            })
    }

    /// Acquire inference lease: read management lock + compute quota.
    pub async fn acquire_inference_lease(
        &self,
        backend_id: &str,
        timeout: Duration,
    ) -> Result<InferenceLease, RuntimeError> {
        self.ensure_inference_allowed().await?;
        let handle = self.handle(backend_id)?;

        let compute_permit = tokio::time::timeout(timeout, handle.semaphore.acquire_owned())
            .await
            .map_err(|_| RuntimeError::Timeout)?
            .map_err(|_| RuntimeError::Busy {
                backend_id: backend_id.to_owned(),
            })?;
        let mgmt_guard = Arc::clone(&handle.management_lock).read_owned().await;

        Ok(InferenceLease {
            mgmt_guard,
            compute_permit,
        })
    }

    /// Acquire exclusive management lease for initialize/load/unload operations.
    pub async fn acquire_management_lease(&self, backend_id: &str) -> Result<ManagementLease, RuntimeError> {
        let handle = self.handle(backend_id)?;
        let mgmt_guard = Arc::clone(&handle.management_lock).write_owned().await;
        Ok(ManagementLease { mgmt_guard })
    }

    /// Lock global management pipeline (serialize global ops).
    pub async fn lock_global_management(&self) -> OwnedMutexGuard<()> {
        Arc::clone(&self.global_management).lock_owned().await
    }

    /// Return backend lifecycle state.
    pub async fn backend_state(&self, backend_id: &str) -> Result<BackendLifecycleState, RuntimeError> {
        let handle = self.handle(backend_id)?;
        let state = handle.lifecycle.read().await.clone();
        Ok(state)
    }

    /// Set backend lifecycle state.
    pub async fn set_backend_state(
        &self,
        backend_id: &str,
        state: BackendLifecycleState,
    ) -> Result<(), RuntimeError> {
        let handle = self.handle(backend_id)?;
        *handle.lifecycle.write().await = state;
        Ok(())
    }

    /// Current global consistency state.
    pub async fn global_state(&self) -> GlobalConsistencyState {
        self.global_state.read().await.clone()
    }

    /// If inconsistent, reject inference submission.
    pub async fn ensure_inference_allowed(&self) -> Result<(), RuntimeError> {
        let guard = self.global_state.read().await;
        if let GlobalConsistencyState::Inconsistent { op_id, .. } = *guard {
            return Err(RuntimeError::GlobalStateInconsistent { op_id });
        }
        Ok(())
    }

    /// Transition to reconciling state and return operation id.
    pub async fn begin_global_reconcile(&self) -> u64 {
        let op_id = self.next_global_op_id.fetch_add(1, Ordering::Relaxed);
        *self.global_state.write().await = GlobalConsistencyState::Reconciling {
            op_id,
            started_at: std::time::SystemTime::now(),
        };
        op_id
    }

    /// Mark system globally consistent and clear failed operation snapshot.
    pub async fn mark_global_consistent(&self) {
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        *self.global_state.write().await = GlobalConsistencyState::Consistent { generation };
        *self.failed_global.write().await = None;
    }

    /// Mark system inconsistent and store failed operation snapshot.
    pub async fn mark_global_inconsistent(
        &self,
        op_id: u64,
        failed_backends: Vec<String>,
        cleanup_report: Vec<String>,
        failed_op: FailedGlobalOperation,
    ) {
        *self.global_state.write().await = GlobalConsistencyState::Inconsistent {
            op_id,
            failed_backends,
            cleanup_report,
            since: std::time::SystemTime::now(),
        };
        *self.failed_global.write().await = Some(failed_op);
    }

    /// Retrieve failed global operation snapshot for retry.
    pub async fn failed_global_operation(&self) -> Option<FailedGlobalOperation> {
        self.failed_global.read().await.clone()
    }

    /// Operator override to recover from inconsistent state.
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
