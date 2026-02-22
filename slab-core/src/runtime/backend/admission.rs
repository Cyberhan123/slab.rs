use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::runtime::types::RuntimeError;

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

/// Manages per-backend admission control via bounded semaphores.
///
/// Each backend is registered with a maximum concurrency (`capacity`).  Before
/// dispatching work to a backend the orchestrator calls [`Self::try_acquire`];
/// if no permit is available it receives [`RuntimeError::Busy`] immediately.
#[derive(Debug, Clone)]
pub struct ResourceManager {
    /// `backend_id` → semaphore pair.
    semaphores: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
}

impl ResourceManager {
    /// Create an empty `ResourceManager`.
    pub fn new() -> Self {
        Self {
            semaphores: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register (or replace) a backend with the given concurrency capacity.
    pub fn register_backend(&self, backend_id: impl Into<String>, capacity: usize) {
        let key = backend_id.into();
        let mut map = self.semaphores.lock().unwrap();
        map.insert(key, Arc::new(Semaphore::new(capacity)));
    }

    /// Try to acquire a permit for `backend_id`.
    ///
    /// Returns `Ok(Permit)` if a slot is available, or
    /// `Err(RuntimeError::Busy)` if all slots are taken.
    pub fn try_acquire(&self, backend_id: &str) -> Result<Permit, RuntimeError> {
        let semaphore = {
            let map = self.semaphores.lock().unwrap();
            map.get(backend_id).cloned().ok_or_else(|| RuntimeError::Busy {
                backend_id: backend_id.to_owned(),
            })?
        };

        semaphore
            .try_acquire_owned()
            .map(|permit| Permit { permit })
            .map_err(|_| RuntimeError::Busy {
                backend_id: backend_id.to_owned(),
            })
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}
