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
use tokio::sync::broadcast;

use super::contract::{
    CandleDiffusionLoadConfig, GeneratedImage, ImageGenerationRequest, ImageGenerationResponse,
};
use super::engine::{CandleDiffusionEngine, GenImageParams};
use super::error::CandleDiffusionWorkerError;
use slab_runtime_core::Payload;
use slab_runtime_core::backend::spawn_workers;
use slab_runtime_core::backend::{
    BroadcastSeq, ControlOpId, Input, PeerControlBus, Typed, WorkerCommand,
};
use slab_runtime_macros::backend_handler;

fn build_gen_image_params(
    raw: &ImageGenerationRequest,
) -> Result<(GenImageParams, usize), CandleDiffusionWorkerError> {
    let prompt = raw.prompt.trim();
    if prompt.is_empty() {
        return Err(CandleDiffusionWorkerError::contract("prompt must not be empty"));
    }

    let mut params = GenImageParams {
        prompt: prompt.to_owned(),
        negative_prompt: raw.negative_prompt.clone().unwrap_or_default(),
        ..GenImageParams::default()
    };

    if let Some(width) = raw.width {
        if width < 1 {
            return Err(CandleDiffusionWorkerError::contract("width must be >= 1"));
        }
        params.width = width;
    }
    if let Some(height) = raw.height {
        if height < 1 {
            return Err(CandleDiffusionWorkerError::contract("height must be >= 1"));
        }
        params.height = height;
    }
    if let Some(seed) = raw.seed {
        params.seed = seed;
    }

    if let Some(steps) = raw.sample_steps {
        if steps < 1 {
            return Err(CandleDiffusionWorkerError::contract("sample_steps must be >= 1"));
        }
        params.steps = usize::try_from(steps).map_err(|_| {
            CandleDiffusionWorkerError::contract("sample_steps exceeds usize range")
        })?;
    }
    if let Some(guidance_scale) = raw.guidance_scale.or(raw.distilled_guidance) {
        params.cfg_scale = f64::from(guidance_scale);
    }

    let count = raw.batch_count;
    if count < 1 {
        return Err(CandleDiffusionWorkerError::contract("batch_count must be >= 1"));
    }

    Ok((
        params,
        usize::try_from(count)
            .map_err(|_| CandleDiffusionWorkerError::contract("batch_count exceeds usize range"))?,
    ))
}

fn decode_png_to_generated_image(
    png_bytes: Vec<u8>,
) -> Result<GeneratedImage, CandleDiffusionWorkerError> {
    let image = image::load_from_memory(&png_bytes).map_err(|error| {
        CandleDiffusionWorkerError::inference(format!(
            "failed to decode candle diffusion PNG output: {error}"
        ))
    })?;
    let (width, height) = image.dimensions();
    let (data, channels) = if image.color().channel_count() == 4 {
        (image.to_rgba8().into_raw(), 4u32)
    } else {
        (image.to_rgb8().into_raw(), 3u32)
    };

    Ok(GeneratedImage { width, height, channels, data })
}

// ── Input parameters ──────────────────────────────────────────────────────────

// ── Worker ────────────────────────────────────────────────────────────────────

pub(crate) struct CandleDiffusionWorker {
    engine: Option<CandleDiffusionEngine>,
    peer_bus: PeerControlBus,
}

#[backend_handler(peer_bus = peer_bus)]
impl CandleDiffusionWorker {
    pub(crate) fn new(engine: Option<CandleDiffusionEngine>, peer_bus: PeerControlBus) -> Self {
        Self { engine, peer_bus }
    }

    #[on_event(LoadModel)]
    async fn on_load_model(
        &mut self,
        config: Input<CandleDiffusionLoadConfig>,
        seq: BroadcastSeq,
    ) -> Result<(), CandleDiffusionWorkerError> {
        self.handle_load_model(config.0, seq.0).await
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(
        &mut self,
        seq: BroadcastSeq,
    ) -> Result<(), CandleDiffusionWorkerError> {
        self.handle_unload_model(seq.0).await
    }

    #[on_event(InferenceImage)]
    async fn on_inference_image(
        &mut self,
        raw: Input<ImageGenerationRequest>,
    ) -> Result<Typed<ImageGenerationResponse>, CandleDiffusionWorkerError> {
        self.handle_inference_image(raw.0).await
    }

    // ── Runtime / peer control ────────────────────────────────────────────────

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(
        &mut self,
        config: Input<CandleDiffusionLoadConfig>,
    ) -> Result<(), CandleDiffusionWorkerError> {
        let config = config.0;
        let model_path = config.model_path;
        let vae_path = config.vae_path;
        let sd_version = config.sd_version;
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
        Ok(())
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) -> Result<(), CandleDiffusionWorkerError> {
        if let Some(e) = self.engine.as_ref() {
            e.unload();
        }
        Ok(())
    }

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(
        &mut self,
        op_id: ControlOpId,
    ) -> Result<(), CandleDiffusionWorkerError> {
        tracing::debug!(op_id = op_id.0, "candle.diffusion runtime control pre-cleanup");
        if let Some(e) = self.engine.as_ref() {
            e.unload();
        }
        Ok(())
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) -> Result<(), CandleDiffusionWorkerError> {
        if let Some(e) = self.engine.as_ref() {
            e.unload();
        }
        Ok(())
    }

    // ── Handler helpers ───────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        config: CandleDiffusionLoadConfig,
        seq_id: u64,
    ) -> Result<(), CandleDiffusionWorkerError> {
        let model_payload = Payload::typed(config.clone());
        let engine = self.engine.get_or_insert_with(CandleDiffusionEngine::new).clone();
        let model_path = config.model_path.clone();
        let vae_path = config.vae_path.clone();
        let sd_version = config.sd_version.clone();

        let result = tokio::task::block_in_place(move || {
            engine.load_model(&model_path, vae_path.as_deref(), &sd_version)
        });

        match result {
            Ok(()) => {
                self.emit_peer_load_model_deployment_payload(seq_id, model_payload);
                Ok(())
            }
            Err(error) => Err(CandleDiffusionWorkerError::load(error.to_string())),
        }
    }

    async fn handle_unload_model(&mut self, seq_id: u64) -> Result<(), CandleDiffusionWorkerError> {
        match self.engine.as_ref() {
            Some(engine) => {
                engine.unload();
                self.emit_peer_unload_generation(seq_id);
                Ok(())
            }
            None => Err(CandleDiffusionWorkerError::unload("model not loaded")),
        }
    }

    async fn handle_inference_image(
        &mut self,
        raw: ImageGenerationRequest,
    ) -> Result<Typed<ImageGenerationResponse>, CandleDiffusionWorkerError> {
        let engine = match self.engine.as_ref() {
            Some(e) => e.clone(),
            None => {
                return Err(CandleDiffusionWorkerError::inference(
                    "candle.diffusion backend not ready: model not loaded",
                ));
            }
        };

        let (params, count) = build_gen_image_params(&raw)?;

        let result = tokio::task::block_in_place(move || {
            let mut images = Vec::with_capacity(count);
            for index in 0..count {
                let mut item = params.clone();
                if raw.seed.is_some() {
                    item.seed = item.seed.saturating_add(index as u64);
                }
                let png_bytes = engine
                    .inference(&item)
                    .map_err(|error| CandleDiffusionWorkerError::inference(error.to_string()))?;
                images.push(decode_png_to_generated_image(png_bytes)?);
            }
            Ok::<Vec<GeneratedImage>, CandleDiffusionWorkerError>(images)
        });

        result.map(|images| Typed(ImageGenerationResponse { images }))
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Spawn `count` Candle Diffusion backend workers.
pub fn spawn_backend(
    shared_ingress_rx: slab_runtime_core::backend::SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    count: usize,
) {
    spawn_workers(shared_ingress_rx, control_tx, count.max(1), |peer_bus| {
        CandleDiffusionWorker::new(Some(CandleDiffusionEngine::new()), peer_bus)
    });
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::super::contract::CandleDiffusionLoadConfig;
    use super::CandleDiffusionWorker;
    use slab_runtime_core::Payload;
    use slab_runtime_core::backend::{
        ControlOpId, DeploymentSnapshot, PeerControlBus, WorkerCommand,
    };
    use tokio::sync::broadcast;

    fn make_worker() -> CandleDiffusionWorker {
        let (bc_tx, _) = broadcast::channel::<WorkerCommand>(8);
        CandleDiffusionWorker::new(None, PeerControlBus::new(bc_tx, 0))
    }

    #[tokio::test]
    async fn global_unload_is_safe_without_engine() {
        let mut worker = make_worker();
        worker.apply_runtime_control(ControlOpId(1)).await.expect("control cleanup should succeed");
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
