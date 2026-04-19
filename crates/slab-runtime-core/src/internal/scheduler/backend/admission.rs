use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::sync::{
    OwnedRwLockReadGuard, OwnedRwLockWriteGuard, OwnedSemaphorePermit, Semaphore, broadcast, mpsc,
};

use crate::base::error::CoreError;
use crate::internal::scheduler::backend::protocol::{BackendRequest, WorkerCommand};
use crate::internal::scheduler::backend::runner::{SharedIngressRx, shared_ingress};

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
            next_seq: Arc::new(AtomicU64::new(1)),
        }
    }
}

/// Manages backend admission, queue handles, and per-backend management locks.
#[derive(Debug, Clone)]
pub struct ResourceManager {
    config: ResourceManagerConfig,
    backends: Arc<RwLock<HashMap<String, BackendHandle>>>,
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
        Self { config, backends: Arc::new(RwLock::new(HashMap::new())) }
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
        match self.backends.write() {
            Ok(mut backends) => {
                backends.insert(
                    key,
                    BackendHandle::new(
                        self.config.backend_capacity,
                        Some(ingress_tx),
                        Some(control_tx),
                    ),
                );
            }
            Err(_) => {
                tracing::error!("backend map poisoned during registration");
                // If the map is poisoned, the runtime is in an unrecoverable state.
                // We panic here to surface the issue immediately rather than continuing
                // with a broken state.
                panic!("backend map poisoned");
            }
        }
    }

    fn handle(&self, backend_id: &str) -> Result<BackendHandle, CoreError> {
        self.backends
            .read()
            .map_err(|_| CoreError::InternalPoisoned { lock_name: "backends".to_string() })?
            .get(backend_id)
            .cloned()
            .ok_or_else(|| CoreError::DriverNotRegistered { driver_id: backend_id.to_owned() })
    }

    /// List registered backends in deterministic order.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn backend_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = match self.backends.read() {
            Ok(backends) => backends.keys().cloned().collect(),
            Err(_) => {
                tracing::error!("backend map poisoned while listing backends");
                // Return empty list rather than crashing - the runtime may still be
                // able to recover if the poison is from a non-critical operation
                return Vec::new();
            }
        };
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
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{ResourceManager, ResourceManagerConfig};
    use crate::base::error::CoreError;

    #[tokio::test]
    async fn inference_lease_waits_for_available_capacity() {
        let mut manager = ResourceManager::with_config(ResourceManagerConfig {
            backend_capacity: 1,
            ..ResourceManagerConfig::default()
        });
        manager.register_backend("serial-backend", |_shared_rx, _control_tx| {});

        let lease = manager
            .acquire_inference_lease("serial-backend", std::time::Duration::from_secs(1))
            .await
            .expect("first lease should succeed");

        let clone = manager.clone();
        let waiter = tokio::spawn(async move {
            clone
                .acquire_inference_lease("serial-backend", std::time::Duration::from_secs(1))
                .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(lease);

        let second = waiter
            .await
            .expect("waiter task should not panic")
            .expect("second lease should succeed after the first is released");
        drop(second);
    }

    #[tokio::test]
    async fn unknown_backend_returns_driver_not_registered() {
        let manager = ResourceManager::new();
        let err = manager
            .acquire_inference_lease("missing-backend", std::time::Duration::from_millis(10))
            .await
            .expect_err("missing backend should fail");

        assert!(matches!(err, CoreError::DriverNotRegistered { .. }));
    }
}
