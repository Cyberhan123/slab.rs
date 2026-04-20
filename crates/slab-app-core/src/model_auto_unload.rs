use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use slab_types::settings::RuntimeModelAutoUnloadConfig;
use slab_types::{RuntimeBackendId, RuntimeBackendLoadSpec};
use tonic::transport::Channel;
use tracing::{debug, info, warn};

use crate::infra::rpc;

#[derive(Debug, Clone)]
pub struct ModelReplayPlan {
    pub backend_id: RuntimeBackendId,
    pub model_id: Option<String>,
    pub load_spec: RuntimeBackendLoadSpec,
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

#[derive(Debug)]
pub struct ModelAutoUnloadManager {
    pmid: Arc<crate::domain::services::PmidService>,
    grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
    runtime_status: Arc<crate::runtime_supervisor::RuntimeSupervisorStatus>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MemoryGauge {
    used_bytes: u64,
    total_bytes: u64,
}

impl MemoryGauge {
    fn free_bytes(self) -> u64 {
        self.total_bytes.saturating_sub(self.used_bytes)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct MemoryPressureSnapshot {
    system: Option<MemoryGauge>,
    gpu: Option<MemoryGauge>,
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
        grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
        runtime_status: Arc<crate::runtime_supervisor::RuntimeSupervisorStatus>,
    ) -> Self {
        Self {
            pmid,
            grpc,
            runtime_status,
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
        let mut states = self.states.lock().await;
        let state = states.entry(backend).or_default();
        state.idle_seq = state.idle_seq.saturating_add(1);
        state.auto_unloaded = false;
        state.resident = false;
        state.last_access_seq = self.next_access_seq();
        state.replay_plan = None;
        debug!(backend = %backend, "model unload state updated (manual)");
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

        let Some(channel) = self.grpc.backend_channel(backend_id) else {
            warn!(
                backend = %backend_id,
                "skipping auto-unload because backend channel is unavailable"
            );
            return;
        };

        match rpc::client::unload_model(channel, backend_id, rpc::pb::ModelUnloadRequest::default())
            .await
        {
            Ok(_) => {
                info!(
                    backend = %backend_id,
                    idle_seq,
                    idle_seconds = idle_duration.as_secs(),
                    "auto-unloaded model after idle timeout"
                );
                self.mark_replayable_unloaded(backend_id, "idle timeout").await;
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

    async fn mark_replayable_unloaded(&self, backend_id: RuntimeBackendId, reason: &'static str) {
        let backend = backend_id;
        let mut states = self.states.lock().await;
        let state = states.entry(backend).or_default();
        state.idle_seq = state.idle_seq.saturating_add(1);
        state.auto_unloaded = true;
        state.resident = false;
        debug!(backend = %backend, reason, "model unload state updated (replayable)");
    }

    pub async fn load_model_with_pressure_control(
        &self,
        channel: Channel,
        load_spec: &RuntimeBackendLoadSpec,
    ) -> anyhow::Result<rpc::pb::ModelStatusResponse> {
        let _guard = Arc::clone(&self.load_pressure_lock).lock_owned().await;
        let target_backend = load_spec.backend();
        let config = self.pressure_config().await;
        let mut evictions_remaining = config.max_pressure_evictions_per_load;

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

        let request = build_model_load_request(load_spec);

        loop {
            match rpc::client::load_model(channel.clone(), request.clone()).await {
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

    async fn pressure_config(&self) -> MemoryPressureConfig {
        self.pmid.config().runtime.model_auto_unload.into()
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
            if !is_under_memory_pressure(snapshot, config) {
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

    async fn sample_memory_pressure(&self) -> MemoryPressureSnapshot {
        let system = tokio::task::spawn_blocking(|| {
            let mut sys = sysinfo::System::new();
            sys.refresh_memory();
            let total_bytes = sys.total_memory();
            let available_bytes = sys.available_memory();
            let used_bytes = total_bytes.saturating_sub(available_bytes);
            (total_bytes > 0).then_some(MemoryGauge { used_bytes, total_bytes })
        })
        .await
        .ok()
        .flatten();

        let gpu = self.primary_gpu_memory_gauge().await.filter(|gauge| gauge.total_bytes > 0);

        MemoryPressureSnapshot { system, gpu }
    }

    async fn primary_gpu_memory_gauge(&self) -> Option<MemoryGauge> {
        let status = crate::domain::services::SystemService::new().gpu_status().await;
        status.devices.first().map(|device| MemoryGauge {
            used_bytes: device.used_memory_bytes,
            total_bytes: device.total_memory_bytes,
        })
    }

    async fn select_pressure_eviction_candidate(
        &self,
        target_backend: RuntimeBackendId,
    ) -> Option<RuntimeBackendId> {
        let states = self.states.lock().await;
        choose_pressure_eviction_candidate(&states, target_backend)
    }

    async fn unload_for_pressure(&self, backend_id: RuntimeBackendId) -> Result<(), String> {
        let Some(channel) = self.grpc.backend_channel(backend_id) else {
            return Err(format!("backend channel unavailable for pressure eviction: {backend_id}"));
        };

        rpc::client::unload_model(channel, backend_id, rpc::pb::ModelUnloadRequest::default())
            .await
            .map_err(|error| error.to_string())?;
        self.mark_replayable_unloaded(backend_id, "memory pressure").await;
        info!(backend = %backend_id, "evicted idle model under memory pressure");
        Ok(())
    }

    async fn try_reload_if_needed(&self, backend_id: RuntimeBackendId) -> Result<(), String> {
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

        let Some(channel) = self.grpc.backend_channel(backend) else {
            let mut states = self.states.lock().await;
            let state = states.entry(backend).or_default();
            state.auto_unloaded = true;
            state.resident = false;
            return Err(format!("backend channel unavailable for auto-reload: {backend}"));
        };

        match self.load_model_with_pressure_control(channel, &plan.load_spec).await {
            Ok(_) => {
                info!(
                    backend = %backend,
                    model_id = ?plan.model_id,
                    model_path = %load_spec_model_path(&plan.load_spec).display(),
                    num_workers = load_spec_num_workers(&plan.load_spec).unwrap_or(0),
                    context_length = load_spec_context_length(&plan.load_spec).unwrap_or(0),
                    restart_attempts = runtime_snapshot.restart_attempts,
                    "re-applied model replay plan before inference"
                );
                let mut states = self.states.lock().await;
                let state = states.entry(backend).or_default();
                state.replay_plan = Some(plan);
                state.auto_unloaded = false;
                state.resident = true;
                state.last_access_seq = self.next_access_seq();
                state.runtime_restart_attempts = runtime_snapshot.restart_attempts;
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

fn is_under_memory_pressure(
    snapshot: MemoryPressureSnapshot,
    config: MemoryPressureConfig,
) -> bool {
    let system_pressure = snapshot
        .system
        .is_some_and(|system| system.free_bytes() < config.min_free_system_memory_bytes);
    let gpu_pressure =
        snapshot.gpu.is_some_and(|gpu| gpu.free_bytes() < config.min_free_gpu_memory_bytes);
    system_pressure || gpu_pressure
}

fn is_memory_pressure_error(error: &anyhow::Error) -> bool {
    let Some(status) = error.chain().find_map(|cause| cause.downcast_ref::<tonic::Status>()) else {
        return false;
    };
    let message = status.message().trim().to_ascii_lowercase();
    let mentions_memory = [
        "out of memory",
        "not enough memory",
        "insufficient memory",
        "memory allocation",
        "memory",
        "oom",
        "vram",
        "cudaerrormemoryallocation",
    ]
    .iter()
    .any(|needle| message.contains(needle));

    mentions_memory
        && matches!(
            status.code(),
            tonic::Code::ResourceExhausted | tonic::Code::Internal | tonic::Code::Unknown
        )
}

fn choose_pressure_eviction_candidate(
    states: &HashMap<RuntimeBackendId, BackendRefState>,
    target_backend: RuntimeBackendId,
) -> Option<RuntimeBackendId> {
    states
        .iter()
        .filter(|(backend_id, state)| {
            **backend_id != target_backend
                && backend_id.is_runtime_worker_backend()
                && state.resident
                && state.active_refs == 0
        })
        .min_by(|(left_backend, left_state), (right_backend, right_state)| {
            left_state
                .last_access_seq
                .cmp(&right_state.last_access_seq)
                .then_with(|| left_backend.canonical_id().cmp(right_backend.canonical_id()))
        })
        .map(|(backend_id, _)| *backend_id)
}

pub(crate) fn build_model_load_request(
    spec: &RuntimeBackendLoadSpec,
) -> rpc::codec::ModelLoadRpcRequest {
    rpc::codec::encode_model_load_request(spec)
}

fn load_spec_model_path(spec: &RuntimeBackendLoadSpec) -> &Path {
    match spec {
        RuntimeBackendLoadSpec::GgmlLlama(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::GgmlWhisper(config) => config.model_path.as_path(),
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
        RuntimeBackendLoadSpec::GgmlLlama(config) => config.context_length,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_pressure_checks_respect_free_memory_thresholds() {
        let config = MemoryPressureConfig {
            enabled: true,
            min_free_system_memory_bytes: 1_024,
            min_free_gpu_memory_bytes: 512,
            max_pressure_evictions_per_load: 3,
        };
        let snapshot = MemoryPressureSnapshot {
            system: Some(MemoryGauge { used_bytes: 9_500, total_bytes: 10_000 }),
            gpu: Some(MemoryGauge { used_bytes: 7_000, total_bytes: 8_000 }),
        };

        assert!(is_under_memory_pressure(snapshot, config));

        let relaxed = MemoryPressureConfig {
            min_free_system_memory_bytes: 400,
            min_free_gpu_memory_bytes: 400,
            ..config
        };
        assert!(!is_under_memory_pressure(snapshot, relaxed));
    }

    #[test]
    fn memory_pressure_error_detection_requires_memory_signals() {
        let memory_error = anyhow::Error::new(tonic::Status::resource_exhausted(
            "GPU out of memory while allocating tensor",
        ));
        assert!(is_memory_pressure_error(&memory_error));

        let queue_error =
            anyhow::Error::new(tonic::Status::resource_exhausted("queue full: ggml.llama"));
        assert!(!is_memory_pressure_error(&queue_error));
    }

    #[test]
    fn pressure_eviction_candidate_uses_oldest_idle_resident_backend() {
        let mut states = HashMap::new();
        states.insert(
            RuntimeBackendId::GgmlLlama,
            BackendRefState { resident: true, last_access_seq: 10, ..BackendRefState::default() },
        );
        states.insert(
            RuntimeBackendId::GgmlWhisper,
            BackendRefState { resident: true, last_access_seq: 4, ..BackendRefState::default() },
        );
        states.insert(
            RuntimeBackendId::GgmlDiffusion,
            BackendRefState {
                resident: true,
                active_refs: 1,
                last_access_seq: 1,
                ..BackendRefState::default()
            },
        );

        let candidate = choose_pressure_eviction_candidate(&states, RuntimeBackendId::GgmlLlama);
        assert_eq!(candidate, Some(RuntimeBackendId::GgmlWhisper));
    }
}
