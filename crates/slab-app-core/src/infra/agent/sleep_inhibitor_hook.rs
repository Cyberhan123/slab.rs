//! Keeps the machine awake for the lifetime of each agent turn when
//! `agent.sleep_inhibitor` is enabled.
//!
//! The platform inhibition is owned by a single hook and driven off the
//! [`AgentHook`] lifecycle: [`HookEvent::OnAgentStart`] (one per run) and
//! [`HookEvent::OnAgentEnd`] (fired on every terminal path — completed, errored,
//! interrupted, aborted). Because multiple agent threads run concurrently, the
//! underlying platform assertion is reference-counted rather than toggled
//! directly, so one thread ending cannot release the assertion while another is
//! still running. The setting is read live from [`PmidService`] at each event,
//! so toggling it in the UI takes effect on the next turn boundary with no
//! restart.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use slab_agent::{AgentHook, HookEvent, HookOutcome};
use slab_config::PmidService;
use slab_utils::sleep_inhibitor::SleepInhibitor;

/// Reference count of in-flight agent runs. The caller should acquire the
/// platform assertion on the `0 -> 1` transition and release it on the
/// `1 -> 0` transition. Extracted as a unit so the bookkeeping is testable
/// independently of the platform-specific inhibitor.
struct ActiveCounter(u32);

impl ActiveCounter {
    const fn new() -> Self {
        Self(0)
    }

    /// Record a run starting. Returns `true` iff the count transitioned
    /// `0 -> 1`, meaning the caller should acquire the assertion.
    fn start(&mut self) -> bool {
        let was_zero = self.0 == 0;
        self.0 = self.0.saturating_add(1);
        was_zero
    }

    /// Record a run ending. Returns `true` iff the count transitioned `1 -> 0`,
    /// meaning the caller should release the assertion. Never underflows.
    fn end(&mut self) -> bool {
        if self.0 == 0 {
            return false;
        }
        self.0 -= 1;
        self.0 == 0
    }

    #[cfg(test)]
    fn active(&self) -> u32 {
        self.0
    }
}

struct Inner {
    active: ActiveCounter,
    inhibitor: SleepInhibitor,
}

/// Inhibits idle sleep for the duration of each agent turn while the
/// `agent.sleep_inhibitor` setting is enabled.
pub(crate) struct SleepInhibitorHook {
    pmid: Arc<PmidService>,
    state: Mutex<Inner>,
}

impl SleepInhibitorHook {
    pub(crate) fn new(pmid: Arc<PmidService>) -> Self {
        // The inhibitor is constructed with `enabled = true` so its internal
        // acquire/release always act; the per-event decision of *whether* to
        // inhibit is driven by the live setting, so toggling it applies on the
        // next turn boundary rather than requiring a restart.
        Self {
            pmid,
            state: Mutex::new(Inner {
                active: ActiveCounter::new(),
                inhibitor: SleepInhibitor::new(true),
            }),
        }
    }

    fn with_inner<R>(&self, f: impl FnOnce(&mut Inner) -> R) -> R {
        // Recover from a poisoned lock (a prior holder panicked) rather than
        // propagating the panic, matching the agent hook registry's policy.
        let mut inner = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut inner)
    }
}

#[async_trait]
impl AgentHook for SleepInhibitorHook {
    async fn on_event(&self, event: &HookEvent) -> HookOutcome {
        match event {
            HookEvent::OnAgentStart { .. } => {
                let enabled = self.pmid.config().agent.sleep_inhibitor;
                self.with_inner(|inner| {
                    // Always count the run so release logic stays balanced across
                    // overlapping turns; only acquire when inhibition is enabled
                    // and this is the first active run.
                    let first = inner.active.start();
                    if enabled && first {
                        inner.inhibitor.set_turn_running(true);
                    }
                });
            }
            HookEvent::OnAgentEnd { .. } => {
                self.with_inner(|inner| {
                    // Release unconditionally on the last run; this is a no-op when
                    // nothing was acquired (e.g. inhibition was disabled).
                    if inner.active.end() {
                        inner.inhibitor.set_turn_running(false);
                    }
                });
            }
            _ => {}
        }
        HookOutcome::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::ActiveCounter;

    #[test]
    fn counter_acquire_release_transitions() {
        let mut counter = ActiveCounter::new();
        assert!(counter.start(), "0 -> 1 should signal acquire");
        assert!(!counter.start(), "1 -> 2 should not signal acquire");
        assert_eq!(counter.active(), 2);
        assert!(!counter.end(), "2 -> 1 should not signal release");
        assert!(counter.end(), "1 -> 0 should signal release");
        assert_eq!(counter.active(), 0);
    }

    #[test]
    fn counter_end_never_underflows() {
        let mut counter = ActiveCounter::new();
        assert!(!counter.end());
        assert!(!counter.end());
        assert_eq!(counter.active(), 0);
    }
}
