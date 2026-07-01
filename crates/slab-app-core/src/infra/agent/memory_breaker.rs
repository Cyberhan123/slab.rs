//! Memory circuit breaker for agent spawn admission (INFRA-05 / ADR-013).
//!
//! [`MemoryCircuitBreaker`] tracks the host process RSS and trips when it
//! exceeds a configured threshold, pausing new agent spawns (the agent control
//! plane checks it via [`crate`]'s `MemoryPressurePort` before admitting a
//! thread). Once tripped the breaker stays tripped for a cooldown window even if
//! RSS drops back below the threshold (hysteresis), then probes again — this
//! prevents flapping under oscillating memory pressure.
//!
//! The trip/clear logic is pure and clock-injected so it is fully testable
//! without touching `sysinfo`; the real RSS sampler ([`spawn_memory_sampler`])
//! samples the current process and is wired only in the running host.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use slab_agent::{MemoryPressure, MemoryPressurePort};

/// Hysteresis circuit breaker over a sampled RSS value (MB).
pub(crate) struct MemoryCircuitBreaker {
    threshold_mb: u64,
    cooldown_ms: u128,
    current_mb: AtomicU64,
    /// `Some(ts_ms)` once the breaker has tripped; cleared after the cooldown
    /// elapses with RSS back under the threshold.
    tripped_at_ms: Mutex<Option<u64>>,
    now_ms: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl MemoryCircuitBreaker {
    pub(crate) fn new(threshold_mb: u64, cooldown: Duration) -> Self {
        let start = Instant::now();
        let now_ms: Arc<dyn Fn() -> u64 + Send + Sync> =
            Arc::new(move || start.elapsed().as_millis() as u64);
        Self::new_with_clock(threshold_mb, cooldown, now_ms)
    }

    fn new_with_clock(
        threshold_mb: u64,
        cooldown: Duration,
        now_ms: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        Self {
            threshold_mb,
            cooldown_ms: cooldown.as_millis(),
            current_mb: AtomicU64::new(0),
            tripped_at_ms: Mutex::new(None),
            now_ms,
        }
    }

    /// Record the latest RSS sample (MB). Called by the background sampler or,
    /// in tests, injected directly.
    pub(crate) fn record_sample(&self, rss_mb: u64) {
        self.current_mb.store(rss_mb, Ordering::SeqCst);
    }
}

impl MemoryPressurePort for MemoryCircuitBreaker {
    fn check(&self) -> MemoryPressure {
        let now = (self.now_ms)();
        let current = self.current_mb.load(Ordering::SeqCst);
        let mut tripped_at_ms = self.tripped_at_ms.lock().expect("breaker state lock");

        if current > self.threshold_mb {
            // Over threshold: ensure the breaker is tripped (record the first
            // crossing time; leave an existing timestamp in place so a sustained
            // spike measures the cooldown from the original trip).
            if tripped_at_ms.is_none() {
                *tripped_at_ms = Some(now);
            }
            return MemoryPressure::Tripped {
                current_mb: current,
                threshold_mb: self.threshold_mb,
            };
        }

        // Under threshold: only clear after the cooldown window has elapsed.
        if let Some(tripped_at) = *tripped_at_ms {
            let elapsed_ms = (now as u128).saturating_sub(tripped_at as u128);
            if elapsed_ms >= self.cooldown_ms {
                *tripped_at_ms = None;
                return MemoryPressure::Nominal;
            }
            return MemoryPressure::Tripped {
                current_mb: current,
                threshold_mb: self.threshold_mb,
            };
        }
        MemoryPressure::Nominal
    }
}

/// [`MemoryPressurePort`] adapter wrapping a shared breaker (so the breaker can
/// also be sampled/inspected by diagnostics independently of the agent).
pub(crate) struct BreakerPressurePort {
    breaker: Arc<MemoryCircuitBreaker>,
}

impl BreakerPressurePort {
    pub(crate) fn new(breaker: Arc<MemoryCircuitBreaker>) -> Self {
        Self { breaker }
    }
}

impl MemoryPressurePort for BreakerPressurePort {
    fn check(&self) -> MemoryPressure {
        self.breaker.check()
    }
}

/// Spawn a background task that samples the current process RSS every few
/// seconds and feeds it to the breaker. No-op when no Tokio runtime is active
/// (e.g. unit tests that inject samples directly).
pub(crate) fn spawn_memory_sampler(breaker: Arc<MemoryCircuitBreaker>) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        tracing::warn!("memory circuit breaker sampler skipped: no Tokio runtime");
        return;
    };
    handle.spawn(async move {
        let pid = sysinfo::Pid::from_u32(std::process::id());
        let mut system = sysinfo::System::new();
        loop {
            system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            if let Some(process) = system.process(pid) {
                // sysinfo 0.30+ reports RSS in bytes.
                let rss_mb = process.memory() / (1024 * 1024);
                breaker.record_sample(rss_mb.max(1));
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;
    use std::time::Duration;

    use slab_agent::{MemoryPressure, MemoryPressurePort};

    use super::MemoryCircuitBreaker;

    /// A controllable clock backed by an atomic counter, so the cooldown logic
    /// is deterministic without waiting on real time.
    fn fake_clock() -> (Arc<AtomicU64>, Arc<dyn Fn() -> u64 + Send + Sync>) {
        let cell = Arc::new(AtomicU64::new(0));
        let cell_fn = {
            let cell = Arc::clone(&cell);
            Arc::new(move || cell.load(std::sync::atomic::Ordering::SeqCst))
                as Arc<dyn Fn() -> u64 + Send + Sync>
        };
        (cell, cell_fn)
    }

    #[test]
    fn nominal_when_under_threshold_and_never_tripped() {
        let (_, clock) = fake_clock();
        let breaker = MemoryCircuitBreaker::new_with_clock(500, Duration::from_millis(1000), clock);
        breaker.record_sample(100);

        assert_eq!(breaker.check(), MemoryPressure::Nominal);
    }

    #[test]
    fn trips_when_sample_exceeds_threshold() {
        let (_, clock) = fake_clock();
        let breaker = MemoryCircuitBreaker::new_with_clock(500, Duration::from_millis(1000), clock);
        breaker.record_sample(750);

        assert_eq!(breaker.check(), MemoryPressure::Tripped { current_mb: 750, threshold_mb: 500 });
    }

    #[test]
    fn holds_tripped_through_cooldown_even_when_pressure_drops() {
        let (clock_cell, clock) = fake_clock();
        let breaker = MemoryCircuitBreaker::new_with_clock(500, Duration::from_millis(1000), clock);

        // Trip at t=0.
        breaker.record_sample(800);
        assert!(matches!(breaker.check(), MemoryPressure::Tripped { .. }));

        // Pressure drops at t=500ms — still within cooldown ⇒ stays tripped.
        breaker.record_sample(100);
        clock_cell.store(500, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(breaker.check(), MemoryPressure::Tripped { .. }));

        // After cooldown (t=1000ms) and pressure under threshold ⇒ clears.
        clock_cell.store(1000, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(breaker.check(), MemoryPressure::Nominal);
    }

    #[test]
    fn stays_tripped_when_pressure_remains_above_threshold_past_cooldown() {
        let (clock_cell, clock) = fake_clock();
        let breaker = MemoryCircuitBreaker::new_with_clock(500, Duration::from_millis(1000), clock);

        breaker.record_sample(900);
        assert!(matches!(breaker.check(), MemoryPressure::Tripped { .. }));

        // Well past the cooldown, but RSS is still over the threshold.
        clock_cell.store(10_000, std::sync::atomic::Ordering::SeqCst);
        breaker.record_sample(900);
        assert!(matches!(breaker.check(), MemoryPressure::Tripped { .. }));
    }

    #[test]
    fn re_trips_after_clearing_when_pressure_rises_again() {
        let (clock_cell, clock) = fake_clock();
        let breaker = MemoryCircuitBreaker::new_with_clock(500, Duration::from_millis(1000), clock);

        breaker.record_sample(800);
        assert!(matches!(breaker.check(), MemoryPressure::Tripped { .. }));
        breaker.record_sample(100);
        clock_cell.store(1000, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(breaker.check(), MemoryPressure::Nominal);

        // A new spike trips again, with a fresh cooldown window.
        breaker.record_sample(700);
        clock_cell.store(1001, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(breaker.check(), MemoryPressure::Tripped { .. }));
        breaker.record_sample(100);
        clock_cell.store(1002, std::sync::atomic::Ordering::SeqCst);
        // Immediately after re-trip the cooldown has not elapsed.
        assert!(matches!(breaker.check(), MemoryPressure::Tripped { .. }));
    }
}
