use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use slab_config::RuntimeModelAutoUnloadConfig;
use slab_types::{RuntimeBackendId, RuntimeBackendLoadSpec};
use tracing::{debug, info, warn};

use crate::domain::ports::{RuntimeBackendStatus, RuntimeInferenceGateway};
use crate::error::AppCoreError;
use slab_gpu_memory_scheduler::{
    EvictionCandidate, GpuMemoryScheduler, LoadContext, LoadOutcome, MemoryPressureInput,
    PressureThresholds, choose_pressure_eviction_candidate, is_under_memory_pressure,
};

#[derive(Debug, Clone)]
pub struct ModelReplayPlan {
    pub backend_id: RuntimeBackendId,
    pub model_id: Option<String>,
    pub load_spec: RuntimeBackendLoadSpec,
    /// GGUF `tokenizer.chat_template` read back from the runtime at load time.
    /// Surfaced through the per-model runtime snapshot so the chat-prompt
    /// resolver can fall back to it when no pack template is configured.
    pub chat_template: Option<String>,
}

#[derive(Debug, Default, Clone)]
struct BackendRefState {
    active_refs: u64,
    idle_seq: u64,
    auto_unloaded: bool,
    resident: bool,
    last_access_seq: u64,
    replay_plan: Option<ModelReplayPlan>,
    runtime_restart_attempts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRuntimeStateSnapshot {
    pub backend_id: RuntimeBackendId,
    pub loaded: bool,
    pub active_refs: u64,
    pub chat_template: Option<String>,
}

#[derive(Debug)]
pub struct ModelAutoUnloadManager {
    pmid: Arc<crate::domain::services::PmidService>,
    runtime: Arc<dyn RuntimeInferenceGateway>,
    runtime_status: Arc<crate::runtime_supervisor::RuntimeSupervisorStatus>,
    gpu_scheduler: Arc<GpuMemoryScheduler>,
    load_pressure_lock: Arc<tokio::sync::Mutex<()>>,
    access_seq: AtomicU64,
    states: tokio::sync::Mutex<HashMap<RuntimeBackendId, BackendRefState>>,
}

#[derive(Debug, Clone, Copy)]
struct MemoryPressureConfig {
    enabled: bool,
    min_free_system_memory_bytes: u64,
    min_free_gpu_memory_bytes: u64,
    max_pressure_evictions_per_load: u32,
}

impl MemoryPressureConfig {
    fn thresholds(self) -> PressureThresholds {
        PressureThresholds {
            min_free_system_memory_bytes: self.min_free_system_memory_bytes,
            min_free_gpu_memory_bytes: self.min_free_gpu_memory_bytes,
        }
    }
}

impl From<RuntimeModelAutoUnloadConfig> for MemoryPressureConfig {
    fn from(config: RuntimeModelAutoUnloadConfig) -> Self {
        Self {
            enabled: config.enabled,
            min_free_system_memory_bytes: config.min_free_system_memory_bytes,
            min_free_gpu_memory_bytes: config.min_free_gpu_memory_bytes,
            max_pressure_evictions_per_load: config.max_pressure_evictions_per_load,
        }
    }
}

#[derive(Debug)]
pub struct ModelUsageGuard {
    manager: Arc<ModelAutoUnloadManager>,
    backend_id: RuntimeBackendId,
    released: bool,
}

impl Drop for ModelUsageGuard {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        self.manager.release_ref(self.backend_id);
    }
}

impl ModelAutoUnloadManager {
    pub fn new(
        pmid: Arc<crate::domain::services::PmidService>,
        runtime: Arc<dyn RuntimeInferenceGateway>,
        runtime_status: Arc<crate::runtime_supervisor::RuntimeSupervisorStatus>,
        gpu_scheduler: Arc<GpuMemoryScheduler>,
    ) -> Self {
        Self {
            pmid,
            runtime,
            runtime_status,
            gpu_scheduler,
            load_pressure_lock: Arc::new(tokio::sync::Mutex::new(())),
            access_seq: AtomicU64::new(0),
            states: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    fn next_access_seq(&self) -> u64 {
        self.access_seq.fetch_add(1, Ordering::Relaxed).saturating_add(1)
    }

    pub async fn acquire(self: &Arc<Self>, backend_id: RuntimeBackendId) -> ModelUsageGuard {
        let backend = backend_id;
        let access_seq = self.next_access_seq();
        let mut states = self.states.lock().await;
        let state = states.entry(backend).or_default();
        state.active_refs = state.active_refs.saturating_add(1);
        state.idle_seq = state.idle_seq.saturating_add(1);
        state.last_access_seq = access_seq;
        debug!(
            backend = %backend,
            active_refs = state.active_refs,
            idle_seq = state.idle_seq,
            access_seq,
            "model usage acquired"
        );
        drop(states);

        ModelUsageGuard { manager: Arc::clone(self), backend_id: backend, released: false }
    }

    pub async fn acquire_for_inference(
        self: &Arc<Self>,
        backend_id: RuntimeBackendId,
    ) -> Result<ModelUsageGuard, String> {
        let backend = backend_id;
        let guard = self.acquire(backend).await;

        if let Err(error) = self.try_reload_if_needed(backend).await {
            drop(guard);
            return Err(error);
        }

        self.gpu_scheduler
            .hooks()
            .dispatch_before_inference(&slab_gpu_memory_scheduler::InferenceContext { backend })
            .await;

        Ok(guard)
    }

    pub async fn notify_model_loaded(self: &Arc<Self>, plan: ModelReplayPlan) {
        let backend = plan.backend_id;
        let mut should_schedule = None;
        let access_seq = self.next_access_seq();

        {
            let mut states = self.states.lock().await;
            let state = states.entry(backend).or_default();
            state.idle_seq = state.idle_seq.saturating_add(1);
            state.auto_unloaded = false;
            state.resident = true;
            state.last_access_seq = access_seq;
            state.replay_plan = Some(plan);
            state.runtime_restart_attempts = self.runtime_status.snapshot(backend).restart_attempts;
            if state.active_refs == 0 {
                should_schedule = Some(state.idle_seq);
            }
        }

        if let Some(seq) = should_schedule {
            self.spawn_idle_timer(backend, seq);
        }
    }

    pub async fn notify_model_unloaded(self: &Arc<Self>, backend_id: RuntimeBackendId) {
        let backend = backend_id;
        {
            let mut states = self.states.lock().await;
            let state = states.entry(backend).or_default();
            state.idle_seq = state.idle_seq.saturating_add(1);
            state.auto_unloaded = false;
            state.resident = false;
            state.last_access_seq = self.next_access_seq();
            state.replay_plan = None;
        }
        debug!(backend = %backend, "model unload state updated (manual)");
        self.gpu_scheduler
            .hooks()
            .dispatch_after_unload(&slab_gpu_memory_scheduler::UnloadContext {
                backend,
                reason: slab_gpu_memory_scheduler::UnloadReason::Manual,
            })
            .await;
    }

    pub async fn ensure_idle_for_manual_unload(
        &self,
        backend_id: RuntimeBackendId,
    ) -> Result<(), AppCoreError> {
        let states = self.states.lock().await;
        let Some(state) = states.get(&backend_id) else {
            return Ok(());
        };

        if state.active_refs > 0 {
            return Err(AppCoreError::Conflict(format!(
                "model backend '{backend_id}' is busy with {} active inference request(s)",
                state.active_refs
            )));
        }

        Ok(())
    }

    pub async fn snapshot_for_model(
        &self,
        backend_id: RuntimeBackendId,
        model_id: &str,
    ) -> Option<ModelRuntimeStateSnapshot> {
        let model_id = model_id.trim();
        if model_id.is_empty() {
            return None;
        }

        let states = self.states.lock().await;
        let state = states.get(&backend_id)?;
        let matches_model = state
            .replay_plan
            .as_ref()
            .and_then(|plan| plan.model_id.as_deref())
            .is_some_and(|candidate| candidate == model_id);
        matches_model.then_some(ModelRuntimeStateSnapshot {
            backend_id,
            loaded: state.resident,
            active_refs: state.active_refs,
            chat_template: state.replay_plan.as_ref().and_then(|plan| plan.chat_template.clone()),
        })
    }

    pub async fn sync_runtime_restart_states(&self) {
        let mut changed = Vec::new();
        {
            let mut states = self.states.lock().await;
            for (backend, state) in states.iter_mut() {
                let runtime_snapshot = self.runtime_status.snapshot(*backend);
                if runtime_snapshot.restart_attempts <= state.runtime_restart_attempts {
                    continue;
                }

                let previous_restart_attempts = state.runtime_restart_attempts;
                state.runtime_restart_attempts = runtime_snapshot.restart_attempts;
                if state.replay_plan.is_some() {
                    state.auto_unloaded = true;
                    state.resident = false;
                    changed.push((
                        *backend,
                        previous_restart_attempts,
                        runtime_snapshot.restart_attempts,
                        runtime_snapshot.status.as_str(),
                    ));
                }
            }
        }

        for (backend, previous_restart_attempts, current_restart_attempts, runtime_status) in
            changed
        {
            info!(
                backend = %backend,
                previous_restart_attempts,
                current_restart_attempts,
                runtime_status,
                "runtime restart detected; resident model state marked for replay"
            );
            self.gpu_scheduler
                .hooks()
                .dispatch_after_unload(&slab_gpu_memory_scheduler::UnloadContext {
                    backend,
                    reason: slab_gpu_memory_scheduler::UnloadReason::RuntimeRestart,
                })
                .await;
        }
    }

    pub async fn invalidate_model_replay(&self, model_id: &str, reason: &'static str) {
        let model_id = model_id.trim();
        if model_id.is_empty() {
            return;
        }

        let mut invalidated_backends = Vec::new();
        {
            let mut states = self.states.lock().await;
            for (backend_id, state) in states.iter_mut() {
                let matches_model = state
                    .replay_plan
                    .as_ref()
                    .and_then(|plan| plan.model_id.as_deref())
                    .is_some_and(|candidate| candidate == model_id);

                if !matches_model {
                    continue;
                }

                state.replay_plan = None;
                state.auto_unloaded = false;
                invalidated_backends.push(*backend_id);
            }
        }

        if invalidated_backends.is_empty() {
            return;
        }

        let backends = invalidated_backends
            .into_iter()
            .map(|backend_id| backend_id.to_string())
            .collect::<Vec<_>>();
        info!(model_id, ?backends, reason, "invalidated compiled model replay plan");
    }

    fn release_ref(self: &Arc<Self>, backend_id: RuntimeBackendId) {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            manager.release_ref_async(backend_id).await;
        });
    }

    async fn release_ref_async(self: Arc<Self>, backend_id: RuntimeBackendId) {
        let mut should_schedule = None;
        {
            let mut states = self.states.lock().await;
            let state = states.entry(backend_id).or_default();
            if state.active_refs == 0 {
                warn!(
                    backend = %backend_id,
                    "model usage ref-count underflow prevented"
                );
                return;
            }
            state.active_refs -= 1;
            debug!(
                backend = %backend_id,
                active_refs = state.active_refs,
                idle_seq = state.idle_seq,
                "model usage released"
            );
            if state.active_refs == 0 {
                state.idle_seq = state.idle_seq.saturating_add(1);
                should_schedule = Some(state.idle_seq);
            }
        }

        self.gpu_scheduler
            .hooks()
            .dispatch_after_inference(&slab_gpu_memory_scheduler::InferenceContext {
                backend: backend_id,
            })
            .await;

        if let Some(seq) = should_schedule {
            self.spawn_idle_timer(backend_id, seq);
        }
    }

    fn spawn_idle_timer(self: &Arc<Self>, backend_id: RuntimeBackendId, idle_seq: u64) {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            manager.run_idle_timer(backend_id, idle_seq).await;
        });
    }

    async fn run_idle_timer(self: Arc<Self>, backend_id: RuntimeBackendId, idle_seq: u64) {
        let Some(idle_duration) = self.resolve_idle_timeout().await else {
            return;
        };

        tokio::time::sleep(idle_duration).await;

        let can_unload = {
            let states = self.states.lock().await;
            states
                .get(&backend_id)
                .is_some_and(|state| state.active_refs == 0 && state.idle_seq == idle_seq)
        };

        if !can_unload {
            return;
        }

        if !self.auto_unload_enabled().await {
            return;
        }

        if !self.runtime.backend_available(backend_id) {
            warn!(
                backend = %backend_id,
                "skipping auto-unload because backend channel is unavailable"
            );
            return;
        }

        self.gpu_scheduler
            .hooks()
            .dispatch_before_unload(&slab_gpu_memory_scheduler::UnloadContext {
                backend: backend_id,
                reason: slab_gpu_memory_scheduler::UnloadReason::IdleTimeout,
            })
            .await;

        match self.runtime.unload_model(backend_id).await {
            Ok(_) => {
                info!(
                    backend = %backend_id,
                    idle_seq,
                    idle_seconds = idle_duration.as_secs(),
                    "auto-unloaded model after idle timeout"
                );
                self.mark_replayable_unloaded(
                    backend_id,
                    slab_gpu_memory_scheduler::UnloadReason::IdleTimeout,
                )
                .await;
            }
            Err(error) => {
                warn!(
                    backend = %backend_id,
                    idle_seq,
                    error = %error,
                    "auto-unload request failed"
                );
            }
        }
    }

    /// Single after_unload choke point for the non-manual unload paths (idle
    /// timeout, pressure eviction, runtime restart).
    ///
    /// before/after symmetry contract across the four `UnloadReason`s:
    /// - IdleTimeout / MemoryPressure: the owning path dispatches
    ///   `before_unload` before the unload RPC, then lands here for the after.
    /// - Manual: `ModelService::unload_model` dispatches the before; the
    ///   after fires from `notify_model_unloaded`.
    /// - RuntimeRestart: the process died under us, so no actionable before
    ///   ever existed — this choke point dispatches a *post-hoc* before here
    ///   (hooks observe, they cannot prevent) so every after_unload is still
    ///   preceded by a before_unload.
    async fn mark_replayable_unloaded(
        &self,
        backend_id: RuntimeBackendId,
        reason: slab_gpu_memory_scheduler::UnloadReason,
    ) {
        let backend = backend_id;
        {
            let mut states = self.states.lock().await;
            let state = states.entry(backend).or_default();
            state.idle_seq = state.idle_seq.saturating_add(1);
            state.auto_unloaded = true;
            state.resident = false;
        }
        debug!(backend = %backend, ?reason, "model unload state updated (replayable)");
        if matches!(reason, slab_gpu_memory_scheduler::UnloadReason::RuntimeRestart) {
            self.gpu_scheduler
                .hooks()
                .dispatch_before_unload(&slab_gpu_memory_scheduler::UnloadContext {
                    backend,
                    reason,
                })
                .await;
        }
        self.gpu_scheduler
            .hooks()
            .dispatch_after_unload(&slab_gpu_memory_scheduler::UnloadContext { backend, reason })
            .await;
    }

    pub async fn load_model_with_pressure_control(
        &self,
        load_spec: &RuntimeBackendLoadSpec,
    ) -> Result<RuntimeBackendStatus, AppCoreError> {
        let _guard = Arc::clone(&self.load_pressure_lock).lock_owned().await;
        let target_backend = load_spec.backend();
        if !self.runtime.backend_available(target_backend) {
            return Err(AppCoreError::BackendNotReady(format!(
                "backend channel unavailable for model load: {target_backend}"
            )));
        }
        let config = self.pressure_config().await;
        let mut evictions_remaining = config.max_pressure_evictions_per_load;

        // Admission pre-check: evict idle residents when this load's
        // projected footprint cannot fit — the ledger-guided proactive
        // replacement for relying solely on the reactive OOM retry below.
        if let Err(error) = self
            .evict_until_projected_fit(target_backend, load_spec, config, &mut evictions_remaining)
            .await
        {
            warn!(
                backend = %target_backend,
                error = %error,
                "admission pre-check eviction failed before model load"
            );
        }

        if let Err(error) = self
            .evict_until_pressure_relieved(target_backend, config, &mut evictions_remaining)
            .await
        {
            warn!(
                backend = %target_backend,
                error = %error,
                "failed to relieve memory pressure before model load"
            );
        }

        loop {
            match self.runtime.load_model(load_spec).await {
                Ok(response) => return Ok(response),
                Err(error)
                    if config.enabled
                        && evictions_remaining > 0
                        && is_memory_pressure_error(&error) =>
                {
                    let Some(candidate) =
                        self.select_pressure_eviction_candidate(target_backend).await
                    else {
                        return Err(error);
                    };

                    if let Err(eviction_error) = self.unload_for_pressure(candidate).await {
                        warn!(
                            backend = %target_backend,
                            candidate = %candidate,
                            error = %eviction_error,
                            "pressure eviction failed during model load retry"
                        );
                        return Err(error);
                    }

                    evictions_remaining = evictions_remaining.saturating_sub(1);
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Full-lifecycle model load shared by the normal load path and the
    /// replay path: dispatch `before_load` (the dispatch-time VRAM baseline
    /// rides in the spec — callers sample once before building it, never
    /// here), load under pressure control, record a fresh replay plan via
    /// `notify_model_loaded` (chat template read back from the runtime's
    /// response, never the stale snapshot), then dispatch `after_load` with
    /// the engine-resolved `n_ctx` — the value the compaction budget reads.
    /// Routing every load through this sequence keeps the ledger
    /// authoritative for all resident models, replay reloads included.
    pub async fn load_with_lifecycle(
        self: &Arc<Self>,
        load_spec: &RuntimeBackendLoadSpec,
        model_id: Option<String>,
    ) -> Result<RuntimeBackendStatus, AppCoreError> {
        let backend = load_spec.backend();
        let load_ctx = LoadContext {
            backend,
            model_id: model_id.clone(),
            model_path: load_spec_model_path(load_spec).display().to_string(),
            num_workers: load_spec_num_workers(load_spec).unwrap_or(1),
            requested_context: load_spec_context_length(load_spec),
            mmproj_path: match load_spec {
                RuntimeBackendLoadSpec::GgmlLlama(config) => {
                    config.mmproj_path.as_ref().map(|path| path.display().to_string())
                }
                _ => None,
            },
            free_vram_bytes: match load_spec {
                RuntimeBackendLoadSpec::GgmlLlama(config) => config.free_vram_bytes,
                _ => None,
            },
        };
        self.gpu_scheduler.hooks().dispatch_before_load(&load_ctx).await;

        let status = self.load_model_with_pressure_control(load_spec).await?;

        self.notify_model_loaded(ModelReplayPlan {
            backend_id: backend,
            model_id,
            load_spec: load_spec.clone(),
            chat_template: status.chat_template.clone(),
        })
        .await;

        self.gpu_scheduler
            .hooks()
            .dispatch_after_load(
                &load_ctx,
                &LoadOutcome {
                    resolved_context_length: status.context_length,
                    training_context_length: status.training_context_length,
                },
            )
            .await;

        Ok(status)
    }

    async fn pressure_config(&self) -> MemoryPressureConfig {
        self.pmid.config().runtime.model_auto_unload.into()
    }

    /// Admission pre-check: project the load's VRAM cost (weights +
    /// projector + headroom buffer — see [`projected_load_bytes`]) against a
    /// fresh free-bytes sample (the probe measures reality, so resident
    /// models are already reflected in `free`). When the projection
    /// overflows, evict idle residents before dispatching — bounded by the
    /// shared per-load eviction budget; if it still does not fit afterwards,
    /// the load is attempted anyway (the reactive OOM retry stays as the
    /// backstop). Fail-open when telemetry reports no free bytes or the
    /// weights file cannot be stat'ed.
    async fn evict_until_projected_fit(
        &self,
        target_backend: RuntimeBackendId,
        load_spec: &RuntimeBackendLoadSpec,
        config: MemoryPressureConfig,
        evictions_remaining: &mut u32,
    ) -> Result<u32, String> {
        if !config.enabled {
            return Ok(0);
        }

        let (weights_bytes, mmproj_bytes) = load_spec_file_sizes(load_spec).await;
        if weights_bytes.is_none() {
            // Unstat-able path (or a non-file model source): no projection
            // is possible, fail open.
            return Ok(0);
        }
        let projected_bytes = projected_load_bytes(
            weights_bytes,
            mmproj_bytes,
            self.gpu_scheduler.params().vram_buffer_bytes,
        );

        let mut evicted = 0u32;
        while *evictions_remaining > 0 {
            let Some(free_bytes) = self.gpu_scheduler.free_bytes_for_load().await else {
                break;
            };
            if projected_bytes <= free_bytes {
                break;
            }

            let Some(candidate) = self.select_pressure_eviction_candidate(target_backend).await
            else {
                break;
            };

            debug!(
                backend = %target_backend,
                candidate = %candidate,
                projected_bytes,
                free_bytes,
                "projected load exceeds free VRAM; evicting idle resident (admission)"
            );
            self.unload_for_pressure(candidate).await?;
            *evictions_remaining = evictions_remaining.saturating_sub(1);
            evicted = evicted.saturating_add(1);
        }

        Ok(evicted)
    }

    async fn evict_until_pressure_relieved(
        &self,
        target_backend: RuntimeBackendId,
        config: MemoryPressureConfig,
        evictions_remaining: &mut u32,
    ) -> Result<u32, String> {
        if !config.enabled {
            return Ok(0);
        }

        let mut evicted = 0u32;
        while *evictions_remaining > 0 {
            let snapshot = self.sample_memory_pressure().await;
            if !is_under_memory_pressure(snapshot, config.thresholds()) {
                break;
            }

            let Some(candidate) = self.select_pressure_eviction_candidate(target_backend).await
            else {
                break;
            };

            self.unload_for_pressure(candidate).await?;
            *evictions_remaining = evictions_remaining.saturating_sub(1);
            evicted = evicted.saturating_add(1);
        }

        Ok(evicted)
    }

    async fn sample_memory_pressure(&self) -> MemoryPressureInput {
        let system = tokio::task::spawn_blocking(|| {
            let mut sys = sysinfo::System::new();
            sys.refresh_memory();
            let total_bytes = sys.total_memory();
            let available_bytes = sys.available_memory();
            let used_bytes = total_bytes.saturating_sub(available_bytes);
            (total_bytes > 0)
                .then_some(slab_gpu_memory_scheduler::MemoryGauge { used_bytes, total_bytes })
        })
        .await
        .ok()
        .flatten();

        // Force a probe round — pressure checks between evictions must see
        // the relief, not a stale cache.
        self.gpu_scheduler.refresh_now().await;
        let gpu =
            self.gpu_scheduler.primary_gauge().filter(|gauge| gauge.total_bytes > 0).map(|gauge| {
                slab_gpu_memory_scheduler::MemoryGauge {
                    used_bytes: gauge.used_bytes,
                    total_bytes: gauge.total_bytes,
                }
            });

        MemoryPressureInput { system, gpu }
    }

    async fn select_pressure_eviction_candidate(
        &self,
        target_backend: RuntimeBackendId,
    ) -> Option<RuntimeBackendId> {
        let states = self.states.lock().await;
        let candidates: Vec<EvictionCandidate> = states
            .iter()
            .map(|(backend, state)| EvictionCandidate {
                backend: *backend,
                resident: state.resident,
                active_refs: state.active_refs,
                last_access_seq: state.last_access_seq,
            })
            .collect();
        choose_pressure_eviction_candidate(&candidates, target_backend)
    }

    async fn unload_for_pressure(&self, backend_id: RuntimeBackendId) -> Result<(), String> {
        if !self.runtime.backend_available(backend_id) {
            return Err(format!("backend channel unavailable for pressure eviction: {backend_id}"));
        }

        // Symmetric with the idle/manual paths: observers see the eviction
        // coming before the RPC, then the after from the choke point below.
        self.gpu_scheduler
            .hooks()
            .dispatch_before_unload(&slab_gpu_memory_scheduler::UnloadContext {
                backend: backend_id,
                reason: slab_gpu_memory_scheduler::UnloadReason::MemoryPressure,
            })
            .await;

        self.runtime.unload_model(backend_id).await.map_err(|error| error.to_string())?;
        self.mark_replayable_unloaded(
            backend_id,
            slab_gpu_memory_scheduler::UnloadReason::MemoryPressure,
        )
        .await;
        info!(backend = %backend_id, "evicted idle model under memory pressure");
        Ok(())
    }

    async fn try_reload_if_needed(
        self: &Arc<Self>,
        backend_id: RuntimeBackendId,
    ) -> Result<(), String> {
        let backend = backend_id;
        let runtime_snapshot = self.runtime_status.snapshot(backend);
        let plan = {
            let mut states = self.states.lock().await;
            let state = states.entry(backend).or_default();
            if runtime_snapshot.restart_attempts > state.runtime_restart_attempts {
                let previous_restart_attempts = state.runtime_restart_attempts;
                state.runtime_restart_attempts = runtime_snapshot.restart_attempts;
                if state.replay_plan.is_some() {
                    state.auto_unloaded = true;
                    state.resident = false;
                    info!(
                        backend = %backend,
                        previous_restart_attempts,
                        current_restart_attempts = runtime_snapshot.restart_attempts,
                        runtime_status = runtime_snapshot.status.as_str(),
                        "runtime restart detected; replay plan will be re-applied before inference"
                    );
                }
            }
            if !state.auto_unloaded {
                return Ok(());
            }

            let Some(plan) = state.replay_plan.clone() else {
                state.auto_unloaded = false;
                warn!(
                    backend = %backend,
                    "cannot auto-reload because compiled replay plan is unavailable"
                );
                return Ok(());
            };

            plan
        };

        if !self.runtime.backend_available(backend) {
            let mut states = self.states.lock().await;
            let state = states.entry(backend).or_default();
            state.auto_unloaded = true;
            state.resident = false;
            return Err(format!("backend channel unavailable for auto-reload: {backend}"));
        }

        // The replay plan's `free_vram_bytes` was snapshotted at the original
        // load; after a runtime restart the VRAM picture has changed, so
        // re-sample before dispatching (auto context sizing must not run on
        // stale input).
        let mut load_spec = plan.load_spec.clone();
        if let RuntimeBackendLoadSpec::GgmlLlama(config) = &mut load_spec {
            config.free_vram_bytes = self.gpu_scheduler.free_bytes_for_load().await;
        }

        // The shared lifecycle helper fires before_load/after_load — so the
        // ledger records the reloaded model with its resolved `n_ctx` — and
        // refreshes the replay plan through `notify_model_loaded`, including
        // a chat template read back fresh from the runtime. Its restart-
        // attempts bookkeeping writes the current supervisor snapshot; a
        // restart landing *during* the reload is therefore not re-detected
        // by the next sweep (same semantics as the normal load path).
        match self.load_with_lifecycle(&load_spec, plan.model_id.clone()).await {
            Ok(status) => {
                info!(
                    backend = %backend,
                    model_id = ?plan.model_id,
                    model_path = %load_spec_model_path(&plan.load_spec).display(),
                    num_workers = load_spec_num_workers(&plan.load_spec).unwrap_or(0),
                    context_length = status.context_length.unwrap_or(0),
                    restart_attempts = runtime_snapshot.restart_attempts,
                    "re-applied model replay plan before inference"
                );
                Ok(())
            }
            Err(error) => {
                let mut states = self.states.lock().await;
                let state = states.entry(backend).or_default();
                state.auto_unloaded = true;
                state.resident = false;
                Err(format!("auto-reload failed for {backend}: {error}"))
            }
        }
    }

    async fn resolve_idle_timeout(&self) -> Option<Duration> {
        if !self.auto_unload_enabled().await {
            return None;
        }

        let minutes = u64::from(self.pmid.config().runtime.model_auto_unload.idle_minutes);

        Some(Duration::from_secs(minutes.saturating_mul(60)))
    }

    async fn auto_unload_enabled(&self) -> bool {
        self.pmid.config().runtime.model_auto_unload.enabled
    }
}

fn is_memory_pressure_error(error: &AppCoreError) -> bool {
    matches!(error, AppCoreError::RuntimeMemoryPressure(_))
        || matches!(
            error,
            AppCoreError::RuntimeFailure { data, .. }
                if data.runtime_code() == Some("runtime_memory_pressure")
        )
}

/// Projected VRAM cost of a load: weights + projector + the scheduler's
/// headroom buffer (saturating). Coarse by design — GGUF dims are unknown
/// host-side, so per-token KV math cannot run pre-load; an `auto` context
/// self-fits in the engine, and a fixed one is the engine's own risk to
/// reject at load.
fn projected_load_bytes(
    weights_bytes: Option<u64>,
    mmproj_bytes: Option<u64>,
    vram_buffer_bytes: u64,
) -> u64 {
    weights_bytes
        .unwrap_or(0)
        .saturating_add(mmproj_bytes.unwrap_or(0))
        .saturating_add(vram_buffer_bytes)
}

/// Stat the weights (and projector, when configured) file sizes off the
/// async path. Best effort — unstat-able files stay `None`.
async fn load_spec_file_sizes(load_spec: &RuntimeBackendLoadSpec) -> (Option<u64>, Option<u64>) {
    let model_path = load_spec_model_path(load_spec).to_path_buf();
    let mmproj_path = match load_spec {
        RuntimeBackendLoadSpec::GgmlLlama(config) => config.mmproj_path.clone(),
        _ => None,
    };
    tokio::task::spawn_blocking(move || {
        let weights = std::fs::metadata(&model_path).ok().map(|metadata| metadata.len());
        let mmproj =
            mmproj_path.as_ref().and_then(|path| std::fs::metadata(path).ok()).map(|m| m.len());
        (weights, mmproj)
    })
    .await
    .unwrap_or((None, None))
}

fn load_spec_model_path(spec: &RuntimeBackendLoadSpec) -> &Path {
    match spec {
        RuntimeBackendLoadSpec::GgmlLlama(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::GgmlWhisper(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::GgmlParakeet(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::GgmlDiffusion(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::CandleLlama(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::CandleWhisper(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::CandleDiffusion(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::Onnx(config) => config.model_path.as_path(),
    }
}

fn load_spec_num_workers(spec: &RuntimeBackendLoadSpec) -> Option<usize> {
    match spec {
        RuntimeBackendLoadSpec::GgmlLlama(config) => Some(config.num_workers),
        _ => None,
    }
}

fn load_spec_context_length(spec: &RuntimeBackendLoadSpec) -> Option<u32> {
    match spec {
        RuntimeBackendLoadSpec::GgmlLlama(config) => {
            config.context_length.and_then(|spec| spec.as_fixed_u32())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::test_support::TestAppCore;
    use async_trait::async_trait;
    use slab_types::load_config::GgmlLlamaLoadConfig;

    #[test]
    fn memory_pressure_error_detection_requires_memory_signals() {
        let memory_error =
            AppCoreError::RuntimeMemoryPressure("GPU out of memory while allocating tensor".into());
        assert!(is_memory_pressure_error(&memory_error));
        let structured_memory_error = AppCoreError::RuntimeFailure {
            message: "GPU out of memory while allocating tensor".into(),
            data: Box::new(crate::error::AppCoreErrorData::runtime_failure(
                "runtime_memory_pressure",
                serde_json::json!({"message": "oom"}),
            )),
        };
        assert!(is_memory_pressure_error(&structured_memory_error));

        let queue_error = AppCoreError::Internal(
            "load_model RPC failed: status: ResourceExhausted, message: queue full: ggml.llama"
                .to_owned(),
        );
        assert!(!is_memory_pressure_error(&queue_error));
    }

    /// Records every lifecycle dispatch in arrival order.
    #[derive(Debug, Clone, PartialEq)]
    enum HookEvent {
        BeforeLoad,
        AfterLoad(Option<u32>),
        BeforeUnload(slab_gpu_memory_scheduler::UnloadReason),
        AfterUnload(slab_gpu_memory_scheduler::UnloadReason),
        BeforeInference,
        AfterInference,
    }

    #[derive(Default)]
    struct RecordingHook {
        events: std::sync::Mutex<Vec<HookEvent>>,
    }

    impl RecordingHook {
        fn record(&self, event: HookEvent) {
            self.events.lock().unwrap_or_else(|error| error.into_inner()).push(event);
        }

        fn events(&self) -> Vec<HookEvent> {
            self.events.lock().unwrap_or_else(|error| error.into_inner()).clone()
        }
    }

    #[async_trait]
    impl slab_gpu_memory_scheduler::ModelLifecycleHook for RecordingHook {
        fn name(&self) -> &str {
            "recording"
        }

        async fn before_load(&self, _ctx: &LoadContext) -> Result<(), GpuMemoryHookError> {
            self.record(HookEvent::BeforeLoad);
            Ok(())
        }

        async fn after_load(
            &self,
            _ctx: &LoadContext,
            outcome: &LoadOutcome,
        ) -> Result<(), GpuMemoryHookError> {
            self.record(HookEvent::AfterLoad(outcome.resolved_context_length));
            Ok(())
        }

        async fn before_unload(
            &self,
            ctx: &slab_gpu_memory_scheduler::UnloadContext,
        ) -> Result<(), GpuMemoryHookError> {
            self.record(HookEvent::BeforeUnload(ctx.reason));
            Ok(())
        }

        async fn after_unload(
            &self,
            ctx: &slab_gpu_memory_scheduler::UnloadContext,
        ) -> Result<(), GpuMemoryHookError> {
            self.record(HookEvent::AfterUnload(ctx.reason));
            Ok(())
        }

        async fn before_inference(
            &self,
            _ctx: &slab_gpu_memory_scheduler::InferenceContext,
        ) -> Result<(), GpuMemoryHookError> {
            self.record(HookEvent::BeforeInference);
            Ok(())
        }

        async fn after_inference(
            &self,
            _ctx: &slab_gpu_memory_scheduler::InferenceContext,
        ) -> Result<(), GpuMemoryHookError> {
            self.record(HookEvent::AfterInference);
            Ok(())
        }
    }

    type GpuMemoryHookError = slab_gpu_memory_scheduler::GpuMemoryError;

    fn llama_load_spec(model_path: PathBuf) -> RuntimeBackendLoadSpec {
        RuntimeBackendLoadSpec::GgmlLlama(GgmlLlamaLoadConfig {
            model_path,
            num_workers: 1,
            context_length: None,
            free_vram_bytes: None,
            flash_attn: true,
            chat_template: None,
            gbnf: None,
            mmproj_path: None,
            vram_buffer_bytes: None,
            auto_context_quantum: None,
            auto_context_fallback: None,
        })
    }

    /// Test app with an allowed llama backend, a scripted load status
    /// (engine-resolved n_ctx + fresh chat template), and a recording hook.
    async fn lifecycle_app() -> (TestAppCore, Arc<RecordingHook>, RuntimeBackendLoadSpec) {
        let app = TestAppCore::new().await;
        app.runtime.allow_backend(RuntimeBackendId::GgmlLlama);
        let model_path = app.write_model_file("lifecycle.gguf");
        app.runtime.set_scripted_load_status(RuntimeBackendStatus {
            backend: RuntimeBackendId::GgmlLlama,
            status: "ready".to_owned(),
            context_length: Some(4096),
            training_context_length: None,
            chat_template: Some("tpl-fresh".to_owned()),
        });
        let hook = Arc::new(RecordingHook::default());
        app.gpu_scheduler.hooks().register(hook.clone());
        (app, hook, llama_load_spec(model_path))
    }

    #[tokio::test]
    async fn replay_reload_fires_full_lifecycle_and_refreshes_plan() {
        let (app, hook, mut spec) = lifecycle_app().await;
        // Sizing tunables snapshotted in the plan must survive the replay.
        if let RuntimeBackendLoadSpec::GgmlLlama(config) = &mut spec {
            config.vram_buffer_bytes = Some(2 * 1024 * 1024 * 1024);
            config.auto_context_quantum = Some(512);
            config.auto_context_fallback = Some(8192);
        }
        app.auto_unload
            .notify_model_loaded(ModelReplayPlan {
                backend_id: RuntimeBackendId::GgmlLlama,
                model_id: Some("m".to_owned()),
                load_spec: spec.clone(),
                chat_template: Some("tpl-stale".to_owned()),
            })
            .await;
        app.auto_unload
            .mark_replayable_unloaded(
                RuntimeBackendId::GgmlLlama,
                slab_gpu_memory_scheduler::UnloadReason::MemoryPressure,
            )
            .await;

        let _guard = app
            .auto_unload
            .acquire_for_inference(RuntimeBackendId::GgmlLlama)
            .await
            .expect("replay reload before inference");

        assert_eq!(
            hook.events(),
            vec![
                HookEvent::AfterUnload(slab_gpu_memory_scheduler::UnloadReason::MemoryPressure),
                HookEvent::BeforeLoad,
                HookEvent::AfterLoad(Some(4096)),
                HookEvent::BeforeInference,
            ]
        );
        assert_eq!(app.runtime.loads().len(), 1, "only the replay reload hit the runtime");
        match app.runtime.loads().first() {
            Some(RuntimeBackendLoadSpec::GgmlLlama(config)) => {
                assert_eq!(config.vram_buffer_bytes, Some(2 * 1024 * 1024 * 1024));
                assert_eq!(config.auto_context_quantum, Some(512));
                assert_eq!(config.auto_context_fallback, Some(8192));
            }
            other => panic!("expected llama load spec, got {other:?}"),
        }

        // The refreshed replay plan carries the chat template read back from
        // the reload response, not the stale original snapshot.
        let snapshot = app
            .auto_unload
            .snapshot_for_model(RuntimeBackendId::GgmlLlama, "m")
            .await
            .expect("resident snapshot");
        assert!(snapshot.loaded);
        assert_eq!(snapshot.chat_template.as_deref(), Some("tpl-fresh"));

        // The ledger hook records its entry on a detached task — poll for it.
        let mut recorded = None;
        for _ in 0..100 {
            let entry = app
                .gpu_scheduler
                .ledger()
                .snapshot()
                .await
                .iter()
                .flat_map(|device| device.resident.iter())
                .find(|entry| entry.model_id.as_deref() == Some("m"))
                .cloned();
            if let Some(entry) = entry {
                recorded = Some(entry);
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let entry = recorded.expect("ledger entry recorded after replay reload");
        assert_eq!(entry.backend, RuntimeBackendId::GgmlLlama);
        assert_eq!(entry.resolved_context_length, Some(4096));

        // The scheduler-side budget resolver serves the reloaded n_ctx — the
        // value the compaction threshold reads instead of the 12k fallback.
        assert_eq!(app.gpu_scheduler.effective_context_budget("m").await, Some(4096));
    }

    #[tokio::test]
    async fn normal_load_path_is_unchanged_after_helper_extraction() {
        let (app, hook, spec) = lifecycle_app().await;

        let status = app
            .auto_unload
            .load_with_lifecycle(&spec, Some("m".to_owned()))
            .await
            .expect("lifecycle load");
        assert_eq!(status.context_length, Some(4096));
        assert_eq!(hook.events(), vec![HookEvent::BeforeLoad, HookEvent::AfterLoad(Some(4096))]);
        assert_eq!(app.runtime.loads().len(), 1);
        let snapshot = app
            .auto_unload
            .snapshot_for_model(RuntimeBackendId::GgmlLlama, "m")
            .await
            .expect("resident snapshot");
        assert!(snapshot.loaded);
    }

    #[tokio::test]
    async fn pressure_unload_fires_before_and_after_unload() {
        let (app, hook, spec) = lifecycle_app().await;
        app.auto_unload
            .notify_model_loaded(ModelReplayPlan {
                backend_id: RuntimeBackendId::GgmlLlama,
                model_id: Some("m".to_owned()),
                load_spec: spec,
                chat_template: None,
            })
            .await;

        app.auto_unload
            .unload_for_pressure(RuntimeBackendId::GgmlLlama)
            .await
            .expect("pressure unload");

        assert_eq!(
            hook.events(),
            vec![
                HookEvent::BeforeUnload(slab_gpu_memory_scheduler::UnloadReason::MemoryPressure),
                HookEvent::AfterUnload(slab_gpu_memory_scheduler::UnloadReason::MemoryPressure),
            ]
        );
        assert_eq!(app.runtime.unloads(), vec![RuntimeBackendId::GgmlLlama]);
    }

    /// The `Manual` before_unload fires in `ModelService::unload_model`
    /// (cross-module); this pins the notify-side half of the pair.
    #[tokio::test]
    async fn manual_notify_unloaded_fires_after_only() {
        let (app, hook, _spec) = lifecycle_app().await;

        app.auto_unload.notify_model_unloaded(RuntimeBackendId::GgmlLlama).await;

        assert_eq!(
            hook.events(),
            vec![HookEvent::AfterUnload(slab_gpu_memory_scheduler::UnloadReason::Manual)]
        );
    }

    #[tokio::test]
    async fn runtime_restart_choke_point_fires_post_hoc_before_then_after() {
        let (app, hook, _spec) = lifecycle_app().await;

        app.auto_unload
            .mark_replayable_unloaded(
                RuntimeBackendId::GgmlLlama,
                slab_gpu_memory_scheduler::UnloadReason::RuntimeRestart,
            )
            .await;

        assert_eq!(
            hook.events(),
            vec![
                HookEvent::BeforeUnload(slab_gpu_memory_scheduler::UnloadReason::RuntimeRestart),
                HookEvent::AfterUnload(slab_gpu_memory_scheduler::UnloadReason::RuntimeRestart),
            ]
        );
    }

    /// Parameterized over the choke-point-reachable reasons: after_unload
    /// always fires with the matching reason. (IdleTimeout's real
    /// before_unload lives in the multi-minute idle timer, which is not
    /// drivable in-crate; Manual's before lives in `ModelService`.)
    #[tokio::test]
    async fn after_unload_always_fires_with_matching_reason() {
        for reason in [
            slab_gpu_memory_scheduler::UnloadReason::IdleTimeout,
            slab_gpu_memory_scheduler::UnloadReason::MemoryPressure,
            slab_gpu_memory_scheduler::UnloadReason::RuntimeRestart,
        ] {
            let (app, hook, _spec) = lifecycle_app().await;
            app.auto_unload.mark_replayable_unloaded(RuntimeBackendId::GgmlLlama, reason).await;
            assert_eq!(
                hook.events().last(),
                Some(&HookEvent::AfterUnload(reason)),
                "reason {reason:?}"
            );
        }
    }

    #[test]
    fn projected_load_bytes_saturates_and_reserves_buffer() {
        assert_eq!(projected_load_bytes(Some(100), Some(20), 50), 170);
        assert_eq!(projected_load_bytes(None, None, 50), 50);
        assert_eq!(
            projected_load_bytes(Some(u64::MAX), Some(u64::MAX), u64::MAX),
            u64::MAX,
            "saturates instead of overflowing"
        );
    }

    /// Admission pre-check: when the projected load (weights + buffer) cannot
    /// fit in the free VRAM the probe reports, an idle resident on another
    /// worker backend is evicted before dispatch — and the load still
    /// proceeds once no more candidates remain.
    #[tokio::test]
    async fn admission_pre_check_evicts_idle_resident_that_blocks_fit() {
        use crate::test_support::FixedGpuProbe;
        use slab_config::{UpdateSettingCommand, UpdateSettingOperation};
        use slab_types::load_config::GgmlWhisperLoadConfig;

        const GIB: u64 = 1024 * 1024 * 1024;
        // 12 GiB total, 10 GiB used → 2 GiB free: comfortably above the
        // pressure floor (so only the admission path acts) but far below a
        // 3 GiB weights + 2 GiB buffer projection.
        let app = TestAppCore::new_with_gpu_probe(Arc::new(FixedGpuProbe {
            total_memory_bytes: 12 * GIB,
            used_memory_bytes: 10 * GIB,
        }))
        .await;
        app.runtime.allow_backend(RuntimeBackendId::GgmlLlama);
        app.runtime.allow_backend(RuntimeBackendId::GgmlWhisper);

        // Enable pressure control with a long idle timeout and a low GPU
        // floor: the idle timer and the pressure-relief sweep must stay out
        // of the way so only admission decides.
        for (pmid, value) in [
            ("models.auto_unload.enabled", serde_json::json!(true)),
            ("models.auto_unload.idle_minutes", serde_json::json!(30)),
            ("models.auto_unload.min_free_gpu_memory_bytes", serde_json::json!(256 * 1024 * 1024)),
        ] {
            app.pmid
                .update_setting(
                    pmid,
                    UpdateSettingCommand {
                        op: UpdateSettingOperation::Set,
                        value: Some(value.into()),
                    },
                )
                .await
                .expect("update auto-unload setting");
        }

        // Resident idle model on another worker backend.
        app.auto_unload
            .notify_model_loaded(ModelReplayPlan {
                backend_id: RuntimeBackendId::GgmlWhisper,
                model_id: Some("whisper-model".to_owned()),
                load_spec: RuntimeBackendLoadSpec::GgmlWhisper(GgmlWhisperLoadConfig {
                    model_path: app.write_model_file("resident-whisper.bin"),
                    flash_attn: true,
                }),
                chat_template: None,
            })
            .await;

        // 3 GiB weights (sparse) for the incoming llama load: 3 GiB + 2 GiB
        // buffer = 5 GiB projected against 2 GiB free.
        let big_weights = app.model_cache_dir.join("big-weights.gguf");
        std::fs::File::create(&big_weights)
            .and_then(|file| file.set_len(3 * GIB))
            .expect("create sparse weights file");
        let incoming = llama_load_spec(big_weights);

        app.auto_unload
            .load_model_with_pressure_control(&incoming)
            .await
            .expect("load still dispatched after admission evictions");

        assert_eq!(app.runtime.unloads(), vec![RuntimeBackendId::GgmlWhisper]);
        assert_eq!(app.runtime.loads().len(), 1, "the incoming load was dispatched");
    }
}
