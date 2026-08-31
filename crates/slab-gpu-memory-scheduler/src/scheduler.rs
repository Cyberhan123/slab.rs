//! The GPU memory scheduler: probe + last-good cache + periodic refresh.
//!
//! This is the single read point for GPU telemetry in the host process. The
//! `/v1/system/gpu` display path serves from cache (refreshing when stale);
//! the model-load path forces a fresh round via [`GpuMemoryScheduler::free_bytes_for_load`].

use std::cmp::Reverse;
use std::sync::{Arc, RwLock, Weak};
use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;
use tracing::{debug, warn};

use crate::GpuMemoryError;
use crate::hooks::{HookRegistry, LoadContext, LoadOutcome, ModelLifecycleHook, UnloadContext};
use crate::ledger::{LedgerEntry, ModelLedger};
use crate::params::SchedulerParams;
use crate::probe::GpuProbe;
use crate::snapshot::{DeviceMemoryGauge, GpuDeviceSnapshot, GpuStatusSnapshot};

/// Consecutive probe failures tolerated before the cached snapshot degrades
/// from last-good to unavailable.
const MAX_CONSECUTIVE_FAILURES: u32 = 3;

struct CacheState {
    snapshot: GpuStatusSnapshot,
    last_success: Option<Instant>,
    consecutive_failures: u32,
}

pub struct GpuMemoryScheduler {
    probe: Arc<dyn GpuProbe>,
    params: SchedulerParams,
    cache: RwLock<CacheState>,
    /// When a consumer last needed fresh telemetry (load sizing, display
    /// refresh). Drives the periodic loop's idle backoff — see
    /// [`next_refresh_delay`].
    last_demand: RwLock<Option<Instant>>,
    ledger: ModelLedger,
    hooks: HookRegistry,
    /// Serializes probe rounds so concurrent readers trigger at most one
    /// probe (they coalesce on the lock while a round is in flight).
    refresh_lock: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for GpuMemoryScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuMemoryScheduler")
            .field("backend", &self.probe.backend_name())
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

impl GpuMemoryScheduler {
    pub fn new(probe: Arc<dyn GpuProbe>, params: SchedulerParams) -> Arc<Self> {
        let initial = GpuStatusSnapshot {
            available: false,
            backend: probe.backend_name().to_owned(),
            updated_at: String::new(),
            devices: Vec::new(),
            error: Some("GPU telemetry not yet sampled".to_owned()),
        };
        let scheduler = Arc::new(Self {
            probe,
            params,
            cache: RwLock::new(CacheState {
                snapshot: initial,
                last_success: None,
                consecutive_failures: 0,
            }),
            last_demand: RwLock::new(None),
            ledger: ModelLedger::default(),
            hooks: HookRegistry::default(),
            refresh_lock: tokio::sync::Mutex::new(()),
        });
        // The built-in ledger hook keeps model accounting current wherever
        // hosts fire lifecycle events.
        scheduler.hooks.register(Arc::new(LedgerHook {
            ledger: scheduler.ledger.clone(),
            scheduler: Arc::downgrade(&scheduler),
        }));
        scheduler
    }

    /// Background refresh loop: detached task, warn-and-continue on
    /// failures, lives for the process lifetime and keeps the cache warm.
    /// The cadence backs off sixfold while no consumer has needed fresh
    /// telemetry (see `next_refresh_delay`) — on an idle laptop the probe
    /// runs every 30s instead of every 5s; any load-sizing or display
    /// refresh snaps it back to the fast cadence.
    pub fn spawn_periodic_refresh(self: &Arc<Self>) {
        let scheduler = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                let last_demand =
                    *scheduler.last_demand.read().expect("gpu scheduler demand lock poisoned");
                let delay = next_refresh_delay(
                    last_demand,
                    Instant::now(),
                    scheduler.params.refresh_interval,
                );
                tokio::time::sleep(delay).await;
                scheduler.refresh_now().await;
            }
        });
    }

    /// Record that a consumer needed fresh telemetry; resets the periodic
    /// loop's backoff to the fast cadence. Called from the consumer entry
    /// points — never from `refresh_now` itself (the loop calls that too and
    /// would keep itself permanently "in demand").
    fn mark_demand(&self) {
        *self.last_demand.write().expect("gpu scheduler demand lock poisoned") =
            Some(Instant::now());
    }

    /// Force one probe round now and update the cache. Callers that need
    /// dispatch-time freshness (model loads) use this; display paths prefer
    /// [`Self::gpu_status`].
    pub async fn refresh_now(&self) -> GpuStatusSnapshot {
        let _guard = self.refresh_lock.lock().await;
        let result = self.probe.probe().await;
        let gauges = result.as_ref().ok().map(|devices| {
            devices
                .iter()
                .map(|device| DeviceMemoryGauge {
                    uuid: device.uuid.clone(),
                    used_bytes: device.used_memory_bytes,
                    total_bytes: device.total_memory_bytes,
                })
                .collect::<Vec<_>>()
        });
        // Scope the sync lock guard so it never lives across the ledger await
        // below (an explicit `drop` does not shorten async-fn captures).
        let snapshot = {
            let mut cache = self.cache.write().expect("gpu scheduler cache lock poisoned");
            match result {
                Ok(devices) => {
                    cache.last_success = Some(Instant::now());
                    cache.consecutive_failures = 0;
                    cache.snapshot = snapshot_from_devices(self.probe.backend_name(), devices);
                    debug!(
                        device_count = cache.snapshot.devices.len(),
                        "gpu telemetry snapshot ready"
                    );
                    cache.snapshot.clone()
                }
                Err(error) => {
                    cache.consecutive_failures = cache.consecutive_failures.saturating_add(1);
                    // Keep serving the last-good snapshot during a brief failure
                    // streak; degrade to the legacy unavailable shape once the
                    // streak is sustained.
                    let degraded = cache.last_success.is_none()
                        || cache.consecutive_failures >= MAX_CONSECUTIVE_FAILURES;
                    if degraded {
                        cache.snapshot.available = false;
                        cache.snapshot.devices.clear();
                        cache.snapshot.error = Some(error.snapshot_error_message());
                        cache.snapshot.updated_at = Utc::now().to_rfc3339();
                    }
                    match &error {
                        // A disabled telemetry backend is an expected build
                        // configuration, not a fault: keep it out of the warn
                        // stream (it would fire every refresh round forever).
                        GpuMemoryError::TelemetryDisabled => debug!(
                            consecutive_failures = cache.consecutive_failures,
                            "gpu telemetry disabled in this build; snapshot left degraded"
                        ),
                        error => warn!(
                            %error,
                            consecutive_failures = cache.consecutive_failures,
                            "failed to refresh gpu telemetry"
                        ),
                    }
                    cache.snapshot.clone()
                }
            }
        };
        if let Some(gauges) = gauges {
            self.ledger.sync_gauges(&gauges).await;
        }
        snapshot
    }

    /// Display-path accessor: the cached snapshot when fresh, else one refresh.
    /// A cache miss counts as demand for the periodic loop's backoff (an
    /// actively polled `/v1/system/gpu` keeps the fast cadence).
    pub async fn gpu_status(&self) -> GpuStatusSnapshot {
        if self.cache_fresh() {
            return self.cached();
        }
        self.mark_demand();
        self.refresh_now().await
    }

    /// Last snapshot without awaiting. May be the initial not-yet-sampled
    /// state before the first refresh completes.
    pub fn cached(&self) -> GpuStatusSnapshot {
        self.cache.read().expect("gpu scheduler cache lock poisoned").snapshot.clone()
    }

    /// Deterministic primary device: the largest-total-VRAM device in the
    /// cached snapshot (enumeration order breaks ties). Replaces the legacy
    /// `devices.first()` pick so multi-GPU hosts target the real workhorse.
    pub fn primary_gauge(&self) -> Option<DeviceMemoryGauge> {
        let cache = self.cache.read().expect("gpu scheduler cache lock poisoned");
        cache.snapshot.devices.iter().min_by_key(|device| Reverse(device.total_memory_bytes)).map(
            |device| DeviceMemoryGauge {
                uuid: device.uuid.clone(),
                used_bytes: device.used_memory_bytes,
                total_bytes: device.total_memory_bytes,
            },
        )
    }

    /// Fresh VRAM budget for model-load sizing: forces a probe round so the
    /// snapshot the runtime receives is taken at dispatch time (replaces the
    /// legacy `primary_gpu_free_bytes`). Counts as demand for the periodic
    /// loop's backoff.
    pub async fn free_bytes_for_load(&self) -> Option<u64> {
        self.mark_demand();
        self.refresh_now().await;
        self.primary_gauge().filter(|gauge| gauge.total_bytes > 0).map(|gauge| gauge.free_bytes())
    }

    pub fn params(&self) -> SchedulerParams {
        self.params
    }

    /// Model-memory ledger (attribution/diagnostics; fed by the lifecycle
    /// hooks and the refresh loop).
    pub fn ledger(&self) -> &ModelLedger {
        &self.ledger
    }

    /// Lifecycle hook registry. Hosts dispatch from model boundaries; new
    /// observers `register` here.
    pub fn hooks(&self) -> &HookRegistry {
        &self.hooks
    }

    /// Effective context budget for a model: the engine-resolved `n_ctx`
    /// (what `auto` sized to — workers and projector accounted) when the
    /// model is resident. `None` when the ledger has no entry (model
    /// unloaded, cloud model, or load not yet reported).
    pub async fn effective_context_budget(&self, model_id: &str) -> Option<u32> {
        self.ledger.resolved_context_for_model(model_id).await
    }

    /// Cached memory-pressure signal for the primary GPU: `1.0 − free/total`
    /// (0-1 fill ratio). Reads only the last snapshot — no probe — so hot
    /// paths (e.g. the compaction gate) can call it every turn; the periodic
    /// refresh keeps it current within `refresh_interval`. `None` when
    /// telemetry is unavailable or the gauge reports no total.
    pub fn gpu_memory_pressure(&self) -> Option<f64> {
        let gauge = self.primary_gauge()?;
        if gauge.total_bytes == 0 {
            return None;
        }
        Some(1.0 - (gauge.free_bytes() as f64 / gauge.total_bytes as f64))
    }

    fn cache_fresh(&self) -> bool {
        let cache = self.cache.read().expect("gpu scheduler cache lock poisoned");
        cache.last_success.is_some_and(|at| at.elapsed() < self.params.max_cache_age)
    }
}

fn snapshot_from_devices(
    backend: &'static str,
    devices: Vec<GpuDeviceSnapshot>,
) -> GpuStatusSnapshot {
    let available = !devices.is_empty();
    GpuStatusSnapshot {
        available,
        backend: backend.to_owned(),
        updated_at: Utc::now().to_rfc3339(),
        error: (!available).then(|| format!("No GPU device detected by {backend}")),
        devices,
    }
}

/// Backoff policy for the periodic refresh loop: probe at the configured
/// cadence while a consumer recently needed fresh data; stretch the interval
/// sixfold when nothing has (laptop power saving — WMI/NVML polling never
/// stops otherwise). `None` (no demand ever) backs off too: display reads
/// self-heal by forcing their own refresh past `max_cache_age`, and the
/// load/eviction paths force rounds regardless of the loop.
fn next_refresh_delay(
    last_demand: Option<Instant>,
    now: Instant,
    refresh_interval: std::time::Duration,
) -> std::time::Duration {
    const BACKOFF_FACTOR: u32 = 6;
    let idle_interval = refresh_interval * BACKOFF_FACTOR;
    match last_demand {
        Some(at) if now.duration_since(at) < idle_interval => refresh_interval,
        _ => idle_interval,
    }
}

/// Built-in hook that mirrors lifecycle events into the ledger. Holds a
/// `Weak` scheduler so the registry (owned by the scheduler) stays
/// cycle-free; entry writes happen on a detached task after the load settles
/// so host call sites never wait on file stats or the post-load probe.
struct LedgerHook {
    ledger: ModelLedger,
    scheduler: Weak<GpuMemoryScheduler>,
}

#[async_trait]
impl ModelLifecycleHook for LedgerHook {
    fn name(&self) -> &str {
        "ledger"
    }

    async fn after_load(
        &self,
        ctx: &LoadContext,
        outcome: &LoadOutcome,
    ) -> Result<(), GpuMemoryError> {
        let ledger = self.ledger.clone();
        let scheduler = self.scheduler.clone();
        let ctx = ctx.clone();
        let outcome = outcome.clone();
        tokio::spawn(async move {
            let (weights_bytes, mmproj_bytes) = file_sizes(&ctx).await;

            // Measured delta: re-probe now that the load settled and compare
            // against the dispatch-time baseline.
            let measured_delta_bytes = scheduler.upgrade().map(|scheduler| {
                let before = ctx.free_vram_bytes;
                async move {
                    scheduler.refresh_now().await;
                    scheduler
                        .primary_gauge()
                        .filter(|gauge| gauge.total_bytes > 0)
                        .map(|gauge| gauge.free_bytes())
                        .zip(before)
                        .map(|(after_free, before)| before.saturating_sub(after_free))
                }
            });
            let measured_delta_bytes = match measured_delta_bytes {
                Some(future) => future.await,
                None => None,
            };

            let uuid = scheduler
                .upgrade()
                .and_then(|scheduler| scheduler.primary_gauge().and_then(|gauge| gauge.uuid));
            ledger
                .note_model_loaded(
                    uuid.as_deref(),
                    LedgerEntry {
                        backend: ctx.backend,
                        model_id: ctx.model_id.clone(),
                        model_path: ctx.model_path.clone(),
                        num_workers: ctx.num_workers,
                        resolved_context_length: outcome.resolved_context_length,
                        mmproj_resident: ctx.mmproj_path.is_some(),
                        weights_bytes,
                        mmproj_bytes,
                        measured_delta_bytes,
                        recorded_at: Utc::now(),
                    },
                )
                .await;
            debug!(
                backend = %ctx.backend,
                model_path = %ctx.model_path,
                resolved_context = ?outcome.resolved_context_length,
                measured_delta_bytes = ?measured_delta_bytes,
                "model ledger entry recorded"
            );
        });
        Ok(())
    }

    async fn after_unload(&self, ctx: &UnloadContext) -> Result<(), GpuMemoryError> {
        self.ledger.note_model_unloaded(ctx.backend).await;
        debug!(backend = %ctx.backend, reason = ?ctx.reason, "model ledger entry cleared");
        Ok(())
    }
}

/// Stat the model (and projector, when configured) file sizes off the async
/// path. Best effort — unstat-able files stay `None`.
async fn file_sizes(ctx: &LoadContext) -> (Option<u64>, Option<u64>) {
    let model_path = ctx.model_path.clone();
    let mmproj_path = ctx.mmproj_path.clone();
    tokio::task::spawn_blocking(move || {
        let weights = std::fs::metadata(&model_path).ok().map(|metadata| metadata.len());
        let mmproj =
            mmproj_path.as_ref().and_then(|path| std::fs::metadata(path).ok()).map(|m| m.len());
        (weights, mmproj)
    })
    .await
    .unwrap_or((None, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::GpuMemoryError;
    use async_trait::async_trait;
    use std::collections::VecDeque;
    use std::time::Duration;

    /// Scripted probe: each `probe()` pops the next queued round; an empty
    /// queue answers "no devices" so an unexpected probe is observable.
    #[derive(Default)]
    struct StaticGpuProbe {
        rounds: tokio::sync::Mutex<VecDeque<Result<Vec<GpuDeviceSnapshot>, GpuMemoryError>>>,
    }

    impl StaticGpuProbe {
        async fn queue(&self, round: Result<Vec<GpuDeviceSnapshot>, GpuMemoryError>) {
            self.rounds.lock().await.push_back(round);
        }

        async fn pending(&self) -> usize {
            self.rounds.lock().await.len()
        }
    }

    #[async_trait]
    impl GpuProbe for StaticGpuProbe {
        fn backend_name(&self) -> &'static str {
            "static"
        }

        async fn probe(&self) -> Result<Vec<GpuDeviceSnapshot>, GpuMemoryError> {
            self.rounds.lock().await.pop_front().unwrap_or(Ok(Vec::new()))
        }
    }

    fn device(total: u64, used: u64, uuid: &str) -> GpuDeviceSnapshot {
        GpuDeviceSnapshot {
            id: 0,
            uuid: Some(uuid.to_owned()),
            name: String::new(),
            device_type: "GPU".to_owned(),
            utilization_percent: 0.0,
            temperature_celsius: 0,
            used_memory_bytes: used,
            total_memory_bytes: total,
            memory_usage_percent: 0.0,
            power_draw_watts: 0.0,
        }
    }

    fn fast_params() -> SchedulerParams {
        SchedulerParams { max_cache_age: Duration::from_millis(80), ..SchedulerParams::default() }
    }

    #[tokio::test]
    async fn gpu_status_serves_cache_within_max_age() {
        let probe = Arc::new(StaticGpuProbe::default());
        let scheduler = GpuMemoryScheduler::new(probe.clone(), fast_params());

        probe.queue(Ok(vec![device(8, 4, "gpu-0")])).await;
        let first = scheduler.gpu_status().await;
        assert!(first.available);
        assert_eq!(first.devices.len(), 1);

        // Within the cache age no new probe fires: the queue is empty, so a
        // probe here would have returned "no devices" and flipped available.
        let second = scheduler.gpu_status().await;
        assert!(second.available);
        assert_eq!(second.devices.len(), 1);
        assert_eq!(probe.pending().await, 0);

        // After the cache ages out, gpu_status refreshes again.
        tokio::time::sleep(Duration::from_millis(120)).await;
        probe.queue(Ok(vec![device(8, 5, "gpu-0")])).await;
        let third = scheduler.gpu_status().await;
        assert_eq!(third.devices[0].used_memory_bytes, 5);
    }

    #[tokio::test]
    async fn failed_probes_keep_last_good_then_degrade() {
        let probe = Arc::new(StaticGpuProbe::default());
        let scheduler = GpuMemoryScheduler::new(probe.clone(), fast_params());

        probe.queue(Err(GpuMemoryError::Probe { message: "driver hiccup".to_owned() })).await;
        let first = scheduler.refresh_now().await;
        assert!(!first.available, "no last-good yet: failure degrades immediately");

        probe.queue(Ok(vec![device(8, 4, "gpu-0")])).await;
        assert!(scheduler.refresh_now().await.available);

        // Brief failure streak keeps the last-good snapshot.
        for _ in 0..(MAX_CONSECUTIVE_FAILURES - 1) {
            probe.queue(Err(GpuMemoryError::Probe { message: "driver hiccup".to_owned() })).await;
            let snapshot = scheduler.refresh_now().await;
            assert!(snapshot.available, "grace window keeps last-good");
            assert_eq!(snapshot.devices.len(), 1);
        }

        // The sustained streak (third consecutive failure) degrades.
        probe.queue(Err(GpuMemoryError::Probe { message: "driver hiccup".to_owned() })).await;
        let degraded = scheduler.refresh_now().await;
        assert!(!degraded.available);
        assert!(degraded.error.as_deref().unwrap().contains("GPU telemetry unavailable"));
    }

    #[tokio::test]
    async fn primary_gauge_prefers_largest_total_with_stable_ties() {
        let probe = Arc::new(StaticGpuProbe::default());
        let scheduler = GpuMemoryScheduler::new(probe.clone(), SchedulerParams::default());

        probe
            .queue(Ok(vec![
                device(8, 4, "small"),
                device(24, 10, "large"),
                device(24, 2, "large-2"),
            ]))
            .await;
        scheduler.refresh_now().await;

        let gauge = scheduler.primary_gauge().expect("primary gauge");
        assert_eq!(gauge.uuid.as_deref(), Some("large"), "first of equal maxima wins");
        assert_eq!(gauge.free_bytes(), 14);
    }

    #[tokio::test]
    async fn free_bytes_for_load_forces_a_fresh_round() {
        let probe = Arc::new(StaticGpuProbe::default());
        let scheduler = GpuMemoryScheduler::new(probe.clone(), SchedulerParams::default());

        probe.queue(Ok(vec![device(16, 6, "gpu-0")])).await;
        let free = scheduler.free_bytes_for_load().await;
        assert_eq!(free, Some(10));
        assert_eq!(probe.pending().await, 0, "the queued round was consumed");
    }

    #[tokio::test]
    async fn empty_device_list_reports_unavailable() {
        let probe = Arc::new(StaticGpuProbe::default());
        let scheduler = GpuMemoryScheduler::new(probe.clone(), SchedulerParams::default());

        let snapshot = scheduler.refresh_now().await;
        assert!(!snapshot.available);
        assert_eq!(snapshot.error.as_deref(), Some("No GPU device detected by static"));
    }

    #[tokio::test]
    async fn noop_probe_reports_disabled_snapshot() {
        use crate::probe::NoopGpuProbe;

        let scheduler = GpuMemoryScheduler::new(Arc::new(NoopGpuProbe), SchedulerParams::default());

        // TelemetryDisabled is an expected path in this build; the snapshot
        // still degrades to the legacy unavailable shape.
        let snapshot = scheduler.refresh_now().await;
        assert!(!snapshot.available);
        assert!(snapshot.error.as_deref().unwrap().contains("disabled"));
        assert!(!snapshot.updated_at.is_empty());
    }

    #[test]
    fn next_refresh_delay_backs_off_without_recent_demand() {
        let interval = Duration::from_secs(5);
        let now = Instant::now();

        // Fresh demand keeps the fast cadence.
        assert_eq!(next_refresh_delay(Some(now), now, interval), interval);
        // Demand inside the idle window (5s cadence, 30s window) stays fast.
        let recent = now - Duration::from_secs(20);
        assert_eq!(next_refresh_delay(Some(recent), now, interval), interval);
        // Stale demand backs off sixfold.
        let stale = now - Duration::from_secs(31);
        assert_eq!(next_refresh_delay(Some(stale), now, interval), interval * 6);
        // Never any demand backs off immediately.
        assert_eq!(next_refresh_delay(None, now, interval), interval * 6);
    }

    #[tokio::test]
    async fn fresh_data_consumers_mark_demand_for_backoff() {
        let probe = Arc::new(StaticGpuProbe::default());
        let scheduler = GpuMemoryScheduler::new(probe.clone(), fast_params());

        assert!(
            scheduler.last_demand.read().expect("demand lock").is_none(),
            "no demand before any consumer"
        );

        probe.queue(Ok(vec![device(16, 6, "gpu-0")])).await;
        scheduler.free_bytes_for_load().await;
        assert!(
            scheduler.last_demand.read().expect("demand lock").is_some(),
            "load sizing marks demand"
        );

        // A cache hit (fresh) serves without marking — only the forced
        // refresh path counts as a consumer.
        let before = *scheduler.last_demand.read().expect("demand lock");
        scheduler.gpu_status().await;
        assert_eq!(*scheduler.last_demand.read().expect("demand lock"), before);
    }

    #[tokio::test]
    async fn gpu_memory_pressure_reads_cached_primary_gauge() {
        let probe = Arc::new(StaticGpuProbe::default());
        let scheduler = GpuMemoryScheduler::new(probe.clone(), SchedulerParams::default());

        assert_eq!(scheduler.gpu_memory_pressure(), None, "no snapshot yet");

        probe.queue(Ok(vec![device(8, 6, "gpu-0")])).await;
        scheduler.refresh_now().await;
        assert_eq!(scheduler.gpu_memory_pressure(), Some(0.75), "1 - free/total");
    }
}
