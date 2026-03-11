use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tracing::{debug, info, warn};

use crate::entities::{AnyStore, ConfigStore};
use crate::grpc;

pub const MODEL_AUTO_UNLOAD_ENABLED_CONFIG_KEY: &str = "model_auto_unload_enabled";
pub const MODEL_AUTO_UNLOAD_IDLE_MINUTES_CONFIG_KEY: &str = "model_auto_unload_idle_minutes";
const DEFAULT_IDLE_MINUTES: u64 = 10;

#[derive(Debug, Default, Clone, Copy)]
struct BackendRefState {
    active_refs: u64,
    idle_seq: u64,
}

#[derive(Debug)]
pub struct ModelAutoUnloadManager {
    store: Arc<AnyStore>,
    grpc: Arc<crate::grpc::gateway::GrpcGateway>,
    states: tokio::sync::Mutex<HashMap<String, BackendRefState>>,
}

#[derive(Debug)]
pub struct ModelUsageGuard {
    manager: Arc<ModelAutoUnloadManager>,
    backend_id: String,
    released: bool,
}

impl Drop for ModelUsageGuard {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        self.manager.release_ref(self.backend_id.clone());
    }
}

impl ModelAutoUnloadManager {
    pub fn new(store: Arc<AnyStore>, grpc: Arc<crate::grpc::gateway::GrpcGateway>) -> Self {
        Self {
            store,
            grpc,
            states: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    pub async fn acquire(self: &Arc<Self>, backend_id: &str) -> ModelUsageGuard {
        let backend = canonical_backend_id(backend_id).to_owned();
        let mut states = self.states.lock().await;
        let state = states.entry(backend.clone()).or_default();
        state.active_refs = state.active_refs.saturating_add(1);
        state.idle_seq = state.idle_seq.saturating_add(1);
        debug!(
            backend = %backend,
            active_refs = state.active_refs,
            idle_seq = state.idle_seq,
            "model usage acquired"
        );
        drop(states);

        ModelUsageGuard {
            manager: Arc::clone(self),
            backend_id: backend,
            released: false,
        }
    }

    pub async fn notify_model_loaded(self: &Arc<Self>, backend_id: &str) {
        let backend = canonical_backend_id(backend_id).to_owned();
        let mut should_schedule = None;

        {
            let mut states = self.states.lock().await;
            let state = states.entry(backend.clone()).or_default();
            state.idle_seq = state.idle_seq.saturating_add(1);
            if state.active_refs == 0 {
                should_schedule = Some(state.idle_seq);
            }
        }

        if let Some(seq) = should_schedule {
            self.spawn_idle_timer(backend, seq);
        }
    }

    pub async fn notify_model_unloaded(self: &Arc<Self>, backend_id: &str) {
        let backend = canonical_backend_id(backend_id).to_owned();
        let mut states = self.states.lock().await;
        let state = states.entry(backend).or_default();
        state.idle_seq = state.idle_seq.saturating_add(1);
    }

    fn release_ref(self: &Arc<Self>, backend_id: String) {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            manager.release_ref_async(backend_id).await;
        });
    }

    async fn release_ref_async(self: Arc<Self>, backend_id: String) {
        let mut should_schedule = None;
        {
            let mut states = self.states.lock().await;
            let state = states.entry(backend_id.clone()).or_default();
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

    fn spawn_idle_timer(self: &Arc<Self>, backend_id: String, idle_seq: u64) {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            manager.run_idle_timer(backend_id, idle_seq).await;
        });
    }

    async fn run_idle_timer(self: Arc<Self>, backend_id: String, idle_seq: u64) {
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

        let Some(channel) = self.grpc.backend_channel(&backend_id) else {
            warn!(
                backend = %backend_id,
                "skipping auto-unload because backend channel is unavailable"
            );
            return;
        };

        match grpc::client::unload_model(
            channel,
            &backend_id,
            grpc::pb::ModelUnloadRequest::default(),
        )
        .await
        {
            Ok(_) => {
                info!(
                    backend = %backend_id,
                    idle_seq,
                    idle_seconds = idle_duration.as_secs(),
                    "auto-unloaded model after idle timeout"
                );
                self.notify_model_unloaded(&backend_id).await;
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

    async fn resolve_idle_timeout(&self) -> Option<Duration> {
        if !self.auto_unload_enabled().await {
            return None;
        }

        let raw_minutes = match self
            .store
            .get_config_value(MODEL_AUTO_UNLOAD_IDLE_MINUTES_CONFIG_KEY)
            .await
        {
            Ok(v) => v,
            Err(error) => {
                warn!(
                    error = %error,
                    "failed to read model auto-unload idle minutes config"
                );
                None
            }
        };

        let minutes = raw_minutes
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| match value.parse::<u64>() {
                Ok(parsed) if parsed > 0 => Some(parsed),
                _ => {
                    warn!(
                        key = MODEL_AUTO_UNLOAD_IDLE_MINUTES_CONFIG_KEY,
                        value = value,
                        default_minutes = DEFAULT_IDLE_MINUTES,
                        "invalid model auto-unload idle minutes config; using default"
                    );
                    None
                }
            })
            .unwrap_or(DEFAULT_IDLE_MINUTES);

        Some(Duration::from_secs(minutes.saturating_mul(60)))
    }

    async fn auto_unload_enabled(&self) -> bool {
        let raw_enabled = match self
            .store
            .get_config_value(MODEL_AUTO_UNLOAD_ENABLED_CONFIG_KEY)
            .await
        {
            Ok(v) => v,
            Err(error) => {
                warn!(
                    error = %error,
                    "failed to read model auto-unload enabled config"
                );
                None
            }
        };

        raw_enabled
            .as_deref()
            .map(str::trim)
            .is_some_and(parse_bool)
    }
}

fn canonical_backend_id(backend_id: &str) -> &str {
    match backend_id.trim().to_ascii_lowercase().as_str() {
        "llama" | "ggml.llama" => "ggml.llama",
        "whisper" | "ggml.whisper" => "ggml.whisper",
        "diffusion" | "ggml.diffusion" => "ggml.diffusion",
        _ => backend_id,
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}
