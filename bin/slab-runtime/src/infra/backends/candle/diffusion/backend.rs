//! Backend worker for `candle.diffusion`.
//!
//! Mirrors the `ggml.diffusion` backend contract.
//!
//! # Supported ops
//!
//! | Op string           | Event variant    | Description                                   |
//! |---------------------|------------------|-----------------------------------------------|
//! | `"model.load"`      | `LoadModel`      | Load Stable Diffusion model weights.          |
//! | `"model.unload"`    | `UnloadModel`    | Drop model weights from memory.               |
//! | `"inference.image"` | `InferenceImage` | Generate an image from typed diffusion params. |
//!
//! ### `model.load` input payload
//! Uses a typed runtime-owned `CandleDiffusionLoadConfig` payload inside `slab-runtime`.
//!
use image::GenericImageView;
use slab_diffusion::{Image as DiffusionImage, ImgParams as DiffusionImgParams};
use tokio::sync::broadcast;

use crate::domain::models::CandleDiffusionLoadConfig;
use crate::infra::backends::candle::diffusion::adapter::{CandleDiffusionEngine, GenImageParams};
use slab_runtime_core::Payload;
use slab_runtime_core::backend::spawn_workers;
use slab_runtime_core::backend::{
    BackendReply, BackendRequest, DeploymentSnapshot, PeerWorkerCommand, RuntimeControlSignal,
    SyncMessage, WorkerCommand,
};
use slab_runtime_macros::backend_handler;

fn build_gen_image_params(raw: &DiffusionImgParams) -> Result<(GenImageParams, usize), String> {
    let prompt = raw.prompt.as_deref().unwrap_or_default().trim();
    if prompt.is_empty() {
        return Err("prompt must not be empty".to_owned());
    }

    let mut params = GenImageParams {
        prompt: prompt.to_owned(),
        negative_prompt: raw.negative_prompt.clone().unwrap_or_default(),
        ..GenImageParams::default()
    };

    if let Some(width) = raw.width {
        if width < 1 {
            return Err("width must be >= 1".to_owned());
        }
        params.width = width;
    }
    if let Some(height) = raw.height {
        if height < 1 {
            return Err("height must be >= 1".to_owned());
        }
        params.height = height;
    }
    if let Some(seed) = raw.seed {
        params.seed = u64::try_from(seed).map_err(|_| "seed must be >= 0".to_owned())?;
    }

    if let Some(sample) = raw.sample_params.as_ref() {
        if let Some(steps) = sample.sample_steps {
            if steps < 1 {
                return Err("sample_steps must be >= 1".to_owned());
            }
            params.steps = usize::try_from(steps)
                .map_err(|_| "sample_steps exceeds usize range".to_owned())?;
        }
        if let Some(guidance) = sample.guidance.as_ref() {
            params.cfg_scale = f64::from(guidance.txt_cfg.max(guidance.distilled_guidance));
        }
    }

    let count = raw.batch_count.unwrap_or(1);
    if count < 1 {
        return Err("batch_count must be >= 1".to_owned());
    }

    Ok((params, usize::try_from(count).map_err(|_| "batch_count exceeds usize range".to_owned())?))
}

fn decode_png_to_diffusion_image(png_bytes: Vec<u8>) -> Result<DiffusionImage, String> {
    let image = image::load_from_memory(&png_bytes)
        .map_err(|error| format!("failed to decode candle diffusion PNG output: {error}"))?;
    let (width, height) = image.dimensions();
    let (data, channel) = if image.color().channel_count() == 4 {
        (image.to_rgba8().into_raw(), 4u32)
    } else {
        (image.to_rgb8().into_raw(), 3u32)
    };

    Ok(DiffusionImage { width, height, channel, data })
}

// ── Input parameters ──────────────────────────────────────────────────────────

// ── Worker ────────────────────────────────────────────────────────────────────

pub(crate) struct CandleDiffusionWorker {
    engine: Option<CandleDiffusionEngine>,
    bc_tx: broadcast::Sender<WorkerCommand>,
    worker_id: usize,
}

#[backend_handler]
impl CandleDiffusionWorker {
    pub(crate) fn new(
        engine: Option<CandleDiffusionEngine>,
        bc_tx: broadcast::Sender<WorkerCommand>,
        worker_id: usize,
    ) -> Self {
        Self { engine, bc_tx, worker_id }
    }

    #[on_event(LoadModel)]
    async fn on_load_model(&mut self, req: BackendRequest) {
        let BackendRequest { input, broadcast_seq, reply_tx, .. } = req;
        let seq_id = broadcast_seq.unwrap_or(0);
        self.handle_load_model(input, reply_tx, seq_id).await;
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self, req: BackendRequest) {
        let BackendRequest { broadcast_seq, reply_tx, .. } = req;
        let seq_id = broadcast_seq.unwrap_or(0);
        self.handle_unload_model(reply_tx, seq_id).await;
    }

    #[on_event(InferenceImage)]
    async fn on_inference_image(&mut self, req: BackendRequest) {
        let BackendRequest { input, reply_tx, .. } = req;
        self.handle_inference_image(input, reply_tx).await;
    }

    // ── Runtime / peer control ────────────────────────────────────────────────

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(&mut self, cmd: PeerWorkerCommand) {
        let Some(snapshot) = cmd.deployment() else {
            return;
        };
        let config: CandleDiffusionLoadConfig = match snapshot.typed_model_config() {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(error = %error, "candle.diffusion worker: invalid deployment snapshot");
                return;
            }
        };
        let model_path = config.model_path;
        let vae_path = config.vae_path;
        let sd_version = config.sd_version;
        let PeerWorkerCommand::LoadModel { .. } = cmd else {
            return;
        };
        if let Some(engine) = self.engine.as_ref()
            && !engine.is_model_loaded()
        {
            let engine_clone = engine.clone();
            let result = tokio::task::block_in_place(|| {
                engine_clone.load_model(&model_path, vae_path.as_deref(), &sd_version)
            });
            if let Err(e) = result {
                tracing::warn!(
                    model_path = %model_path.display(),
                    error = %e,
                    "candle.diffusion worker: broadcast LoadModel failed"
                );
            }
        }
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) {
        if let Some(e) = self.engine.as_ref() {
            e.unload();
        }
    }

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(&mut self, signal: RuntimeControlSignal) {
        match signal {
            RuntimeControlSignal::GlobalUnload { op_id } => {
                tracing::debug!(op_id, "candle.diffusion runtime global unload");
                if let Some(e) = self.engine.as_ref() {
                    e.unload();
                }
            }
            RuntimeControlSignal::GlobalLoad { op_id, payload } => {
                let _ = payload;
                tracing::debug!(op_id, "candle.diffusion runtime global load pre-cleanup");
                if let Some(e) = self.engine.as_ref() {
                    e.unload();
                }
            }
        }
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) {
        if let Some(e) = self.engine.as_ref() {
            e.unload();
        }
    }

    // ── Handler helpers ───────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        let deployment = DeploymentSnapshot::with_model(seq_id, input.clone());
        let config: CandleDiffusionLoadConfig = match input.to_typed() {
            Ok(c) => c,
            Err(e) => {
                let _ =
                    reply_tx.send(BackendReply::Error(format!("invalid model.load config: {e}")));
                return;
            }
        };

        let engine = self.engine.get_or_insert_with(CandleDiffusionEngine::new).clone();
        let model_path = config.model_path.clone();
        let vae_path = config.vae_path.clone();
        let sd_version = config.sd_version.clone();

        let result = tokio::task::block_in_place(move || {
            engine.load_model(&model_path, vae_path.as_deref(), &sd_version)
        });

        match result {
            Ok(()) => {
                let _ = self.bc_tx.send(WorkerCommand::Peer(PeerWorkerCommand::LoadModel {
                    sync: SyncMessage::Deployment(deployment),
                    sender_id: self.worker_id,
                }));
                let _ = reply_tx.send(BackendReply::Ack);
            }
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
        }
    }

    async fn handle_unload_model(
        &mut self,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        match self.engine.as_ref() {
            Some(engine) => {
                engine.unload();
                let _ = self.bc_tx.send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
                    sync: SyncMessage::Generation { generation: seq_id },
                    sender_id: self.worker_id,
                }));
                let _ = reply_tx.send(BackendReply::Ack);
            }
            None => {
                let _ = reply_tx.send(BackendReply::Error("model not loaded".into()));
            }
        }
    }

    async fn handle_inference_image(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let engine = match self.engine.as_ref() {
            Some(e) => e.clone(),
            None => {
                let _ = reply_tx.send(BackendReply::Error(
                    "candle.diffusion backend not ready: model not loaded".into(),
                ));
                return;
            }
        };

        let raw: DiffusionImgParams = match input.to_typed() {
            Ok(v) => v,
            Err(e) => {
                let _ = reply_tx
                    .send(BackendReply::Error(format!("invalid inference.image params: {e}")));
                return;
            }
        };

        let (params, count) = match build_gen_image_params(&raw) {
            Ok(params) => params,
            Err(error) => {
                let _ = reply_tx.send(BackendReply::Error(error));
                return;
            }
        };

        let result = tokio::task::block_in_place(move || {
            let mut images = Vec::with_capacity(count);
            for index in 0..count {
                let mut item = params.clone();
                if raw.seed.is_some() {
                    item.seed = item.seed.saturating_add(index as u64);
                }
                let png_bytes = engine.inference(&item).map_err(|error| error.to_string())?;
                images.push(decode_png_to_diffusion_image(png_bytes)?);
            }
            Ok::<Vec<DiffusionImage>, String>(images)
        });

        match result {
            Ok(images) => {
                let _ = reply_tx.send(BackendReply::Value(Payload::typed(images)));
            }
            Err(e) => {
                let _ = reply_tx
                    .send(BackendReply::Error(format!("candle.diffusion inference failed: {e}")));
            }
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Spawn `count` Candle Diffusion backend workers.
pub fn spawn_backend(
    shared_ingress_rx: slab_runtime_core::backend::SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    count: usize,
) {
    spawn_workers(shared_ingress_rx, control_tx, count.max(1), |worker_id, bc_tx| {
        CandleDiffusionWorker::new(Some(CandleDiffusionEngine::new()), bc_tx, worker_id)
    });
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::CandleDiffusionWorker;
    use crate::domain::models::CandleDiffusionLoadConfig;
    use slab_runtime_core::Payload;
    use slab_runtime_core::backend::{DeploymentSnapshot, RuntimeControlSignal, WorkerCommand};
    use tokio::sync::broadcast;

    fn make_worker() -> CandleDiffusionWorker {
        let (bc_tx, _) = broadcast::channel::<WorkerCommand>(8);
        CandleDiffusionWorker::new(None, bc_tx, 0)
    }

    #[tokio::test]
    async fn global_unload_is_safe_without_engine() {
        let mut worker = make_worker();
        worker.apply_runtime_control(RuntimeControlSignal::GlobalUnload { op_id: 1 }).await;
        // No panic – test passes.
    }

    #[test]
    fn deployment_snapshot_reads_typed_candle_diffusion_model_config() {
        let snapshot = DeploymentSnapshot::with_model(
            6,
            Payload::typed(CandleDiffusionLoadConfig {
                model_path: PathBuf::from("model.safetensors"),
                vae_path: Some(PathBuf::from("vae.safetensors")),
                sd_version: "v1-5".to_owned(),
            }),
        );

        let config = snapshot
            .typed_model_config::<CandleDiffusionLoadConfig>()
            .expect("typed deployment snapshot should decode");

        assert_eq!(config.model_path, PathBuf::from("model.safetensors"));
        assert_eq!(config.vae_path, Some(PathBuf::from("vae.safetensors")));
        assert_eq!(config.sd_version, "v1-5");
    }
}
