//! Scheduler tunables. Plain data so the crate stays free of `slab-config`;
//! hosts build this from their own settings.

use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct SchedulerParams {
    /// Periodic probe cadence for the background refresh loop.
    pub refresh_interval: Duration,
    /// Age beyond which the cached snapshot is considered stale for display
    /// paths (a refresh is triggered on read).
    pub max_cache_age: Duration,
    /// VRAM headroom reserved above weights + KV when sizing an `auto`
    /// context (compute buffers, CUDA context, driver release lag).
    pub vram_buffer_bytes: u64,
    /// Conservative fallback context when no VRAM signal or degenerate dims.
    pub auto_context_fallback: u32,
    /// `auto` contexts are floored to a multiple of this many tokens.
    pub auto_context_quantum: u32,
}

impl Default for SchedulerParams {
    fn default() -> Self {
        Self {
            refresh_interval: Duration::from_secs(5),
            max_cache_age: Duration::from_secs(10),
            vram_buffer_bytes: 2 * 1024 * 1024 * 1024,
            auto_context_fallback: 8192,
            auto_context_quantum: 512,
        }
    }
}
