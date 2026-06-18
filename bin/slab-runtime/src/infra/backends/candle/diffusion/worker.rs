//! Backend worker for `candle.diffusion`.

use slab_candle::CandleRuntimeEngine;
use slab_candle::diffusion::{
    CandleDiffusionEngine, CandleDiffusionLoadConfig as SlabCandleDiffusionLoadConfig,
    DiffusionPipelineKind, FluxModelKind, FluxWeightSource, GeneratedImage as SlabGeneratedImage,
    ImageGenerationRequest as SlabImageGenerationRequest, StableDiffusionVersion,
};
use slab_runtime_core::Payload;
use slab_runtime_core::backend::spawn_workers;
use slab_runtime_core::backend::{
    BroadcastSeq, ControlOpId, Input, PeerControlBus, Typed, WorkerCommand,
};
use slab_runtime_macros::backend_handler;
use tokio::sync::broadcast;

use super::error::CandleDiffusionWorkerError;
use crate::domain::models::{
    CandleDiffusionLoadConfig, GeneratedImage, ImageGenerationRequest, ImageGenerationResponse,
};

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

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(
        &mut self,
        config: Input<CandleDiffusionLoadConfig>,
    ) -> Result<(), CandleDiffusionWorkerError> {
        let load_config = map_load_config(config.0)?;
        if let Some(engine) = self.engine.as_mut()
            && !engine.is_model_loaded()
        {
            let model_path = load_config.model_path.clone();
            let result = tokio::task::block_in_place(|| engine.load_model(load_config));
            if let Err(error) = result {
                tracing::warn!(
                    model_path = %model_path.display(),
                    error = %error,
                    "candle.diffusion worker: broadcast LoadModel failed"
                );
            }
        }
        Ok(())
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) -> Result<(), CandleDiffusionWorkerError> {
        if let Some(engine) = self.engine.as_mut() {
            engine.unload_model();
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
        if let Some(engine) = self.engine.as_mut() {
            engine.unload_model();
        }
        Ok(())
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) -> Result<(), CandleDiffusionWorkerError> {
        if let Some(engine) = self.engine.as_mut() {
            engine.unload_model();
        }
        Ok(())
    }

    async fn handle_load_model(
        &mut self,
        config: CandleDiffusionLoadConfig,
        seq_id: u64,
    ) -> Result<(), CandleDiffusionWorkerError> {
        let model_payload = Payload::typed(config.clone());
        let load_config = map_load_config(config)?;
        let engine = self.engine.get_or_insert_with(CandleDiffusionEngine::new);
        let result = tokio::task::block_in_place(|| engine.load_model(load_config));

        match result {
            Ok(()) => {
                self.emit_peer_load_model_deployment_payload(seq_id, model_payload);
                Ok(())
            }
            Err(error) => Err(CandleDiffusionWorkerError::load(error.to_string())),
        }
    }

    async fn handle_unload_model(&mut self, seq_id: u64) -> Result<(), CandleDiffusionWorkerError> {
        match self.engine.as_mut() {
            Some(engine) => {
                engine.unload_model();
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
        let engine = self.engine.as_mut().ok_or_else(|| {
            CandleDiffusionWorkerError::inference(
                "candle.diffusion backend not ready: model not loaded",
            )
        })?;

        let (request, count) = build_image_request(&raw)?;
        let result = tokio::task::block_in_place(|| {
            let mut images = Vec::with_capacity(count);
            for index in 0..count {
                let mut item = request.clone();
                if raw.seed.is_some() {
                    item.seed = item.seed.saturating_add(index as u64);
                }
                let image = engine
                    .infer(item)
                    .map_err(|error| CandleDiffusionWorkerError::inference(error.to_string()))?;
                images.push(map_generated_image(image));
            }
            Ok::<Vec<GeneratedImage>, CandleDiffusionWorkerError>(images)
        });

        result.map(|images| Typed(ImageGenerationResponse { images }))
    }
}

pub fn spawn_backend(
    shared_ingress_rx: slab_runtime_core::backend::SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    count: usize,
) {
    spawn_workers(shared_ingress_rx, control_tx, count.max(1), |peer_bus| {
        CandleDiffusionWorker::new(Some(CandleDiffusionEngine::new()), peer_bus)
    });
}

fn map_load_config(
    config: CandleDiffusionLoadConfig,
) -> Result<SlabCandleDiffusionLoadConfig, CandleDiffusionWorkerError> {
    Ok(SlabCandleDiffusionLoadConfig {
        model_path: config.model_path,
        vae_path: config.vae_path,
        device: config.device,
        text_encoder_path: None,
        text_encoder2_path: None,
        tokenizer_path: None,
        tokenizer2_path: None,
        pipeline: DiffusionPipelineKind::StableDiffusion,
        sd_version: parse_sd_version(&config.sd_version)?,
        flux_model: FluxModelKind::default(),
        flux_weight_source: FluxWeightSource::default(),
        flux_t5_encoder_path: None,
        flux_t5_config_path: None,
        flux_t5_tokenizer_path: None,
        flux_clip_encoder_path: None,
        flux_clip_tokenizer_path: None,
        flux_autoencoder_path: None,
    })
}

fn parse_sd_version(value: &str) -> Result<StableDiffusionVersion, CandleDiffusionWorkerError> {
    match value {
        "v1-5" => Ok(StableDiffusionVersion::V1_5),
        "v1-5-inpaint" => Ok(StableDiffusionVersion::V1_5Inpaint),
        "v2-1" => Ok(StableDiffusionVersion::V2_1),
        "sdxl" => Ok(StableDiffusionVersion::Sdxl),
        "sdxl-inpaint" => Ok(StableDiffusionVersion::SdxlInpaint),
        "sdxl-turbo" => Ok(StableDiffusionVersion::SdxlTurbo),
        other => Err(CandleDiffusionWorkerError::contract(format!(
            "unsupported candle diffusion sd_version: {other}"
        ))),
    }
}

fn build_image_request(
    raw: &ImageGenerationRequest,
) -> Result<(SlabImageGenerationRequest, usize), CandleDiffusionWorkerError> {
    let prompt = raw.prompt.trim();
    if prompt.is_empty() {
        return Err(CandleDiffusionWorkerError::contract("prompt must not be empty"));
    }

    let width = raw.width.unwrap_or(512);
    let height = raw.height.unwrap_or(512);
    if width < 1 {
        return Err(CandleDiffusionWorkerError::contract("width must be >= 1"));
    }
    if height < 1 {
        return Err(CandleDiffusionWorkerError::contract("height must be >= 1"));
    }

    let steps = match raw.sample_steps {
        Some(steps) if steps < 1 => {
            return Err(CandleDiffusionWorkerError::contract("sample_steps must be >= 1"));
        }
        Some(steps) => usize::try_from(steps).map_err(|_| {
            CandleDiffusionWorkerError::contract("sample_steps exceeds usize range")
        })?,
        None => 20,
    };
    let count = raw.batch_count;
    if count < 1 {
        return Err(CandleDiffusionWorkerError::contract("batch_count must be >= 1"));
    }

    Ok((
        SlabImageGenerationRequest {
            prompt: prompt.to_owned(),
            negative_prompt: raw.negative_prompt.clone().unwrap_or_default(),
            width,
            height,
            steps,
            cfg_scale: raw.guidance_scale.or(raw.distilled_guidance).map(f64::from).unwrap_or(7.5),
            seed: raw.seed.unwrap_or(42),
            init_image: raw.init_image.as_ref().map(map_runtime_image),
            mask_image: None,
            strength: raw.strength,
            scheduler: raw.scheduler.clone(),
            sample_method: raw.sample_method.clone(),
        },
        usize::try_from(count)
            .map_err(|_| CandleDiffusionWorkerError::contract("batch_count exceeds usize range"))?,
    ))
}

fn map_runtime_image(image: &GeneratedImage) -> SlabGeneratedImage {
    SlabGeneratedImage {
        width: image.width,
        height: image.height,
        channels: image.channels,
        data: image.data.clone(),
    }
}

fn map_generated_image(image: SlabGeneratedImage) -> GeneratedImage {
    GeneratedImage {
        width: image.width,
        height: image.height,
        channels: image.channels,
        data: image.data,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::CandleDiffusionWorker;
    use crate::domain::models::CandleDiffusionLoadConfig;
    use crate::infra::backends::candle::test_support::peer_control_bus;
    use slab_runtime_core::Payload;
    use slab_runtime_core::backend::{ControlOpId, DeploymentSnapshot};

    fn make_worker() -> CandleDiffusionWorker {
        CandleDiffusionWorker::new(None, peer_control_bus())
    }

    #[tokio::test]
    async fn global_unload_is_safe_without_engine() {
        let mut worker = make_worker();
        worker.apply_runtime_control(ControlOpId(1)).await.expect("control cleanup should succeed");
    }

    #[test]
    fn deployment_snapshot_reads_typed_candle_diffusion_model_config() {
        let snapshot = DeploymentSnapshot::with_model(
            6,
            Payload::typed(CandleDiffusionLoadConfig {
                model_path: PathBuf::from("model.safetensors"),
                vae_path: Some(PathBuf::from("vae.safetensors")),
                device: None,
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
