use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use slab_types::{RuntimeBackendId, RuntimeBackendLoadSpec};
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
    replay_plan: Option<ModelReplayPlan>,
    runtime_restart_attempts: usize,
}

#[derive(Debug)]
pub struct ModelAutoUnloadManager {
    pmid: Arc<crate::domain::services::PmidService>,
    grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
    runtime_status: Arc<crate::runtime_supervisor::RuntimeSupervisorStatus>,
    states: tokio::sync::Mutex<HashMap<RuntimeBackendId, BackendRefState>>,
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
        Self { pmid, grpc, runtime_status, states: tokio::sync::Mutex::new(HashMap::new()) }
    }

    pub async fn acquire(self: &Arc<Self>, backend_id: RuntimeBackendId) -> ModelUsageGuard {
        let backend = backend_id;
        let mut states = self.states.lock().await;
        let state = states.entry(backend).or_default();
        state.active_refs = state.active_refs.saturating_add(1);
        state.idle_seq = state.idle_seq.saturating_add(1);
        debug!(
            backend = %backend,
            active_refs = state.active_refs,
            idle_seq = state.idle_seq,
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

        {
            let mut states = self.states.lock().await;
            let state = states.entry(backend).or_default();
            state.idle_seq = state.idle_seq.saturating_add(1);
            state.auto_unloaded = false;
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
                self.mark_auto_unloaded(backend_id).await;
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

    async fn mark_auto_unloaded(&self, backend_id: RuntimeBackendId) {
        let backend = backend_id;
        let mut states = self.states.lock().await;
        let state = states.entry(backend).or_default();
        state.idle_seq = state.idle_seq.saturating_add(1);
        state.auto_unloaded = true;
        debug!(backend = %backend, "model unload state updated (auto)");
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
            return Err(format!("backend channel unavailable for auto-reload: {backend}"));
        };

        let req = build_model_load_request(&plan.load_spec);

        match rpc::client::load_model(channel, req).await {
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
                state.runtime_restart_attempts = runtime_snapshot.restart_attempts;
                Ok(())
            }
            Err(error) => {
                let mut states = self.states.lock().await;
                let state = states.entry(backend).or_default();
                state.auto_unloaded = true;
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
