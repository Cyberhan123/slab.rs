//! Bounded FIFO admission gate for agent thread spawns (INFRA-05 / ADR-013).
//!
//! [`ConcurrencyGate`] wraps a [`tokio::sync::Semaphore`] whose capacity is the
//! hard concurrency cap (`max_threads`). Tokio's semaphore releases permits to
//! waiters in FIFO order, so spawns that arrive while the cap is saturated wait
//! in arrival order rather than being rejected outright.
//!
//! `queue_capacity` bounds how many spawns may wait at once:
//! - `0` ⇒ legacy behavior: a missing immediate permit rejects the spawn with
//!   [`AgentError::ThreadLimitExceeded`] (no waiting).
//! - `> 0` ⇒ up to `queue_capacity` spawns may wait (FIFO); a spawn that would
//!   exceed the waiting bound is rejected immediately.
//!
//! The returned [`tokio::sync::OwnedSemaphorePermit`] is held for the thread's
//! lifetime and dropped when the task finishes or is aborted, releasing the slot
//! to the next FIFO waiter.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::error::AgentError;

pub(crate) struct ConcurrencyGate {
    running: Arc<Semaphore>,
    max_threads: usize,
    queue_capacity: usize,
    waiting: AtomicUsize,
}

impl ConcurrencyGate {
    pub(crate) fn new(max_threads: usize, queue_capacity: usize) -> Self {
        Self {
            running: Arc::new(Semaphore::new(max_threads.max(1))),
            max_threads: max_threads.max(1),
            queue_capacity,
            waiting: AtomicUsize::new(0),
        }
    }

    /// Number of spawns currently waiting for a permit (FIFO backlog).
    pub(crate) fn waiting_count(&self) -> usize {
        self.waiting.load(Ordering::SeqCst)
    }

    /// Total admission budget: concurrently running + queuable spawns.
    fn total_budget(&self) -> usize {
        self.max_threads + self.queue_capacity
    }

    /// Acquire a running permit, waiting in FIFO order when saturated.
    ///
    /// Returns [`AgentError::ThreadLimitExceeded`] when the wait queue is full
    /// (or immediately when `queue_capacity == 0` and no permit is free).
    pub(crate) async fn acquire(&self) -> Result<OwnedSemaphorePermit, AgentError> {
        if self.queue_capacity == 0 {
            // Legacy behavior: never wait — a missing permit is a hard rejection.
            return self.running.clone().try_acquire_owned().map_err(|_| {
                AgentError::ThreadLimitExceeded {
                    current: self.total_budget(),
                    max: self.total_budget(),
                }
            });
        }

        // Reserve a waiting slot first so the backlog is bounded.
        if self.waiting.fetch_add(1, Ordering::SeqCst) >= self.queue_capacity {
            self.waiting.fetch_sub(1, Ordering::SeqCst);
            return Err(AgentError::ThreadLimitExceeded {
                current: self.total_budget(),
                max: self.total_budget(),
            });
        }

        // Tokio's semaphore hands permits out in FIFO order.
        let permit = self
            .running
            .clone()
            .acquire_owned()
            .await
            .expect("agent concurrency semaphore is never closed");
        self.waiting.fetch_sub(1, Ordering::SeqCst);
        Ok(permit)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::ConcurrencyGate;

    #[tokio::test]
    async fn queue_capacity_zero_rejects_when_saturated() {
        let gate = ConcurrencyGate::new(1, 0);
        let _held = gate.acquire().await.expect("first acquire succeeds");
        // Semaphore exhausted and no waiting allowed ⇒ immediate rejection.
        let err = gate.acquire().await.expect_err("second acquire rejects");
        assert!(matches!(err, crate::error::AgentError::ThreadLimitExceeded { .. }));
        assert_eq!(gate.waiting_count(), 0);
    }

    #[tokio::test]
    async fn queue_capacity_positive_queues_then_admits_on_release() {
        let gate = Arc::new(ConcurrencyGate::new(1, 2));
        let held = gate.acquire().await.expect("first permit held");

        // Two waiters queue while the single permit is held.
        let g1 = Arc::clone(&gate);
        let w1 = tokio::spawn(async move { g1.acquire().await });
        let g2 = Arc::clone(&gate);
        let w2 = tokio::spawn(async move { g2.acquire().await });
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        assert_eq!(gate.waiting_count(), 2, "both waiters are queued");

        // Releasing the permit admits exactly one FIFO waiter; one remains.
        drop(held);
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        assert_eq!(gate.waiting_count(), 1, "one waiter admitted, one still queued");

        // Clean up the remaining (still-queued) waiter.
        w2.abort();
        let _ = w1.await;
    }

    #[tokio::test]
    async fn queue_capacity_rejects_beyond_bound() {
        let gate = Arc::new(ConcurrencyGate::new(1, 1));
        let held = gate.acquire().await.expect("permit held");
        // One waiter is allowed to queue.
        let g = Arc::clone(&gate);
        let waiter = tokio::spawn(async move { g.acquire().await });
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        assert_eq!(gate.waiting_count(), 1);
        // A second waiter exceeds queue_capacity(1) and is rejected immediately.
        let err = gate.acquire().await.expect_err("second waiter rejected");
        assert!(matches!(err, crate::error::AgentError::ThreadLimitExceeded { .. }));
        // Clean up the queued waiter.
        drop(held);
        let _ = waiter.await;
    }
}
