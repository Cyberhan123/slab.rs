//! Backend worker adapter for `ggml.diffusion`.
//!
//! Defines [`DiffusionWorker`] logic for runtime-managed worker loops.
//!
//! # Supported ops
//!
//! | Op string           | Event variant    | Description                                       |
//! |---------------------|------------------|---------------------------------------------------|
//! | `"model.load"`      | `LoadModel`      | Load a model from the engine.                     |
//! | `"model.unload"`    | `UnloadModel`    | Drop the model handle; call model.load to restore. |
//! | `"inference.image"` | `InferenceImage` | Image generation from typed diffusion params.     |
//! ### `model.load` input payload
//! Uses a typed [`slab_types::GgmlDiffusionLoadConfig`] payload inside `slab-core`.

use std::sync::Arc;
use std::time::Instant;

use slab_diffusion::{GuidanceParams, Image as DiffusionImage, SampleMethod, Scheduler, SlgParams};
use slab_types::{
    DiffusionImageRequest, DiffusionImageResponse, GeneratedImage, GgmlDiffusionLoadConfig,
};
use tokio::sync::broadcast;

use crate::internal::engine::ggml::diffusion::adapter::GGMLDiffusionEngine;
use crate::internal::scheduler::backend::protocol::{
    BackendReply, BackendRequest, DeploymentSnapshot, PeerWorkerCommand, RuntimeControlSignal,
    SyncMessage, WorkerCommand,
};
use crate::internal::scheduler::types::Payload;
use slab_core_macros::backend_handler;

// ── Configurations ────────────────────────────────────────────────────────────

/// Parse a sample method string into the native enum value.
fn parse_sample_method(s: &str) -> SampleMethod {
    match s {
        "euler" => SampleMethod::Euler,
        "euler_a" => SampleMethod::EULER_A,
        "lcm" => SampleMethod::LCM,
        "heun" => SampleMethod::HEUN,
        "dpm2" => SampleMethod::DPM2,
        "dpm++2s_a" => SampleMethod::DPMPP2S_A,
        "dpm++2m" => SampleMethod::DPMPP2M,
        "dpm++2mv2" => SampleMethod::DPMPP2Mv2,
        "ipndm" => SampleMethod::IPNDM,
        "ipndm_v" => SampleMethod::IPNDM_V,
        _ => SampleMethod::Unknown,
    }
}

/// Parse a scheduler string into the native enum value.
fn parse_scheduler(s: &str) -> Scheduler {
    match s {
        "discrete" => Scheduler::DISCRETE,
        "karras" => Scheduler::KARRAS,
        "exponential" => Scheduler::EXPONENTIAL,
        "ays" => Scheduler::AYS,
        "gits" => Scheduler::GITS,
        _ => Scheduler::UNKNOWN,
    }
}

fn default_flow_shift() -> f32 {
    f32::INFINITY
}

fn build_context_params(
    engine: &GGMLDiffusionEngine,
    config: &GgmlDiffusionLoadConfig,
) -> slab_diffusion::ContextParams {
    let mut params = engine.new_context_params();
    params.set_model_path(&config.model_path.to_string_lossy());

    if let Some(path) = config.diffusion_model_path.as_ref() {
        params.set_diffusion_model_path(&path.to_string_lossy());
    }
    if let Some(path) = config.vae_path.as_ref() {
        params.set_vae_path(&path.to_string_lossy());
    }
    if let Some(path) = config.taesd_path.as_ref() {
        params.set_taesd_path(&path.to_string_lossy());
    }
    if let Some(path) = config.clip_l_path.as_ref() {
        params.set_clip_l_path(&path.to_string_lossy());
    }
    if let Some(path) = config.clip_g_path.as_ref() {
        params.set_clip_g_path(&path.to_string_lossy());
    }
    if let Some(path) = config.t5xxl_path.as_ref() {
        params.set_t5xxl_path(&path.to_string_lossy());
    }
    if let Some(path) = config.clip_vision_path.as_ref() {
        params.set_clip_vision_path(&path.to_string_lossy());
    }
    if let Some(path) = config.control_net_path.as_ref() {
        params.set_control_net_path(&path.to_string_lossy());
    }
    if let Some(device) = config.vae_device.as_deref() {
        params.set_vae_device(device);
    }
    if let Some(device) = config.clip_device.as_deref() {
        params.set_clip_device(device);
    }
    if let Some(n_threads) = config.n_threads {
        params.set_n_threads(n_threads);
    }

    params.set_flash_attn(config.flash_attn);
    params.set_offload_params_to_cpu(config.offload_params_to_cpu);
    params.set_enable_mmap(config.enable_mmap);
    params
}

fn build_image_params(
    engine: &GGMLDiffusionEngine,
    gen_params: DiffusionImageRequest,
) -> Result<slab_diffusion::ImgParams, String> {
    let init_image = gen_params.init_image.as_ref().map(|image| DiffusionImage {
        width: image.width,
        height: image.height,
        channel: u32::from(image.channels.max(1)),
        data: image.data.clone(),
    });

    let width = i32::try_from(gen_params.width)
        .map_err(|_| format!("width {} exceeds i32 range", gen_params.width))?;
    let height = i32::try_from(gen_params.height)
        .map_err(|_| format!("height {} exceeds i32 range", gen_params.height))?;
    let sample_steps = gen_params.steps.unwrap_or(20).max(1);
    let cfg_scale = gen_params.cfg_scale.unwrap_or(7.0);
    let guidance = gen_params.guidance.unwrap_or(3.5);
    let seed = gen_params.seed.unwrap_or(42);
    let sample_method = gen_params.sample_method.as_deref().unwrap_or("auto");
    let scheduler = gen_params.scheduler.as_deref().unwrap_or("auto");
    let eta = gen_params.eta.unwrap_or(0.0);
    let strength = gen_params.strength.unwrap_or(0.75);
    let batch_count = i32::try_from(gen_params.count.max(1))
        .map_err(|_| format!("count {} exceeds i32 range", gen_params.count))?;

    let mut sample_params = engine.new_sample_params();
    sample_params.set_sample_steps(sample_steps);
    sample_params.set_sample_method(parse_sample_method(sample_method));
    sample_params.set_scheduler(parse_scheduler(scheduler));
    sample_params.set_eta(eta);
    sample_params.set_flow_shift(default_flow_shift());
    sample_params.set_guidance(GuidanceParams {
        txt_cfg: cfg_scale,
        img_cfg: cfg_scale,
        distilled_guidance: guidance,
        slg: SlgParams { layers: Vec::new(), layer_start: 0.0, layer_end: 0.0, scale: 0.0 },
    });

    let mut params = engine.new_image_params();
    params.set_prompt(&gen_params.prompt);
    params.set_negative_prompt(gen_params.negative_prompt.as_deref().unwrap_or_default());
    params.set_width(width);
    params.set_height(height);
    params.set_sample_params(sample_params);
    params.set_strength(strength);
    params.set_seed(seed);
    params.set_batch_count(batch_count);
    if let Some(init_image) = init_image {
        params.set_init_image(init_image);
    }
    Ok(params)
}

// ── Worker ────────────────────────────────────────────────────────────────────

/// A single diffusion backend worker.
///
/// Each worker **owns** its engine (library handle + model context).  There is
/// no shared mutable state between workers, so no `Mutex` is needed on the
/// context.  When `num_workers > 1` multiple workers are spawned; each
/// worker owns an independent engine forked from the same library handle and
/// manages its own model context independently.
///
/// Workers listen on both the shared `mpsc` ingress queue (competitive –
/// only one worker processes each request) and a `broadcast` channel
/// (fan-out – every worker receives management commands such as `Unload`).
pub(crate) struct DiffusionWorker {
    /// - `None` → engine not initialized.
    /// - `Some(e)` where `e.ctx` is None → engine loaded, no model.
    /// - `Some(e)` where `e.ctx` is Some → engine + model loaded.
    engine: Option<GGMLDiffusionEngine>,
    /// Broadcast sender shared among all workers so that any worker can
    /// propagate state-change commands (e.g. `Unload`) to all peers.
    bc_tx: broadcast::Sender<WorkerCommand>,
    /// Stable index used to populate `sender_id` when broadcasting.
    worker_id: usize,
    last_model_config: Option<Payload>,
}

#[backend_handler]
impl DiffusionWorker {
    pub(crate) fn new(
        engine: Option<GGMLDiffusionEngine>,
        bc_tx: broadcast::Sender<WorkerCommand>,
        worker_id: usize,
    ) -> Self {
        Self { engine, bc_tx, worker_id, last_model_config: None }
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

    // ── model.load ────────────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        let engine = match self.engine.as_mut() {
            Some(e) => e,
            None => {
                let _ = reply_tx.send(BackendReply::Error("engine not initialized".into()));
                return;
            }
        };

        let config: GgmlDiffusionLoadConfig = match input.to_typed() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    worker_id = self.worker_id,
                    seq_id,
                    error = %e,
                    "diffusion model.load rejected: invalid typed config"
                );
                let _ =
                    reply_tx.send(BackendReply::Error(format!("invalid model.load config: {e}")));
                return;
            }
        };

        tracing::info!(
            worker_id = self.worker_id,
            seq_id,
            model_path = %config.model_path.display(),
            diffusion_model_path = ?config.diffusion_model_path.as_ref().map(|p| p.display().to_string()),
            vae_path = ?config.vae_path.as_ref().map(|p| p.display().to_string()),
            flash_attn = config.flash_attn,
            offload_params_to_cpu = config.offload_params_to_cpu,
            n_threads = ?config.n_threads,
            "diffusion model.load started"
        );

        let started_at = Instant::now();

        // Model loading is CPU/I-O bound; use block_in_place on this thread.
        let result = tokio::task::block_in_place(|| {
            let ctx_params = build_context_params(engine, &config);
            engine.new_context(ctx_params)
        });

        match result {
            Ok(()) => {
                self.last_model_config = Some(input.clone());
                let deployment = DeploymentSnapshot::with_model(seq_id, input);
                // Broadcast so peer workers also load the same model.
                let _ = self.bc_tx.send(WorkerCommand::Peer(PeerWorkerCommand::LoadModel {
                    sync: SyncMessage::Deployment(deployment),
                    sender_id: self.worker_id,
                }));
                tracing::info!(
                    worker_id = self.worker_id,
                    seq_id,
                    elapsed_ms = started_at.elapsed().as_millis(),
                    "diffusion model.load completed"
                );
                let _ =
                    reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
            }
            Err(e) => {
                tracing::error!(
                    worker_id = self.worker_id,
                    seq_id,
                    elapsed_ms = started_at.elapsed().as_millis(),
                    error = %e,
                    "diffusion model.load failed"
                );
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
        }
    }

    // ── model.unload ──────────────────────────────────────────────────────────

    async fn handle_unload_model(
        &mut self,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        match self.engine.as_mut() {
            Some(e) => {
                e.unload();
                self.last_model_config = None;
                // Broadcast so every peer worker also drops its context.
                // Ignore errors: no receivers simply means no other workers.
                let _ = self.bc_tx.send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
                    sync: SyncMessage::Generation { generation: seq_id },
                    sender_id: self.worker_id,
                }));
                let _ =
                    reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
            }
            None => {
                let _ = reply_tx.send(BackendReply::Error("engine not initialized".into()));
            }
        }
    }

    // ── inference.image ───────────────────────────────────────────────────────

    async fn handle_inference_image(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let engine = match self.engine.as_ref() {
            Some(e) => e,
            None => {
                let _ = reply_tx.send(BackendReply::Error("engine not initialized".into()));
                return;
            }
        };

        let gen_params: DiffusionImageRequest = match input.to_typed() {
            Ok(p) => p,
            Err(e) => {
                let _ = reply_tx
                    .send(BackendReply::Error(format!("invalid inference.image params: {e}")));
                return;
            }
        };

        let image_params = match build_image_params(engine, gen_params) {
            Ok(params) => params,
            Err(error) => {
                let _ = reply_tx.send(BackendReply::Error(error));
                return;
            }
        };

        // Image generation is CPU/GPU-bound; use block_in_place so the engine
        // context stays on this thread without needing an additional spawn_blocking.
        let result = tokio::task::block_in_place(|| engine.generate_image(image_params));

        match result {
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
            Ok(images) => {
                let encoded: Vec<GeneratedImage> = images
                    .into_iter()
                    .filter(|img| !img.data.is_empty())
                    .filter_map(|img| {
                        let (w, h, channels) = (img.width, img.height, img.channel as u8);
                        let dyn_img: Option<image::DynamicImage> = if channels == 3 {
                            image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(w, h, img.data)
                                .map(image::DynamicImage::ImageRgb8)
                        } else {
                            image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(w, h, img.data)
                                .map(image::DynamicImage::ImageRgba8)
                        };
                        let dyn_img = dyn_img?;
                        let mut png_bytes: Vec<u8> = Vec::new();
                        dyn_img
                            .write_to(
                                &mut std::io::Cursor::new(&mut png_bytes),
                                image::ImageFormat::Png,
                            )
                            .ok()?;
                        Some(GeneratedImage { bytes: png_bytes, width: w, height: h, channels })
                    })
                    .collect();
                let payload =
                    DiffusionImageResponse { images: encoded, metadata: Default::default() };
                let _ = reply_tx.send(BackendReply::Value(Payload::typed(payload)));
            }
        }
    }

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(&mut self, cmd: PeerWorkerCommand) {
        let Some(snapshot) = cmd.deployment() else {
            return;
        };
        let config: GgmlDiffusionLoadConfig = match snapshot.typed_model_config() {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(error = %error, "diffusion worker: invalid model deployment snapshot");
                return;
            }
        };
        let model_path = config.model_path.clone();
        if let Some(engine) = self.engine.as_mut()
            && !engine.is_model_loaded()
        {
            let result = tokio::task::block_in_place(|| {
                let ctx_params = build_context_params(engine, &config);
                engine.new_context(ctx_params)
            });
            if let Err(e) = result {
                tracing::warn!(
                    model_path = %model_path.display(),
                    error = %e,
                    "diffusion worker: broadcast LoadModel failed"
                );
            }
        }
        self.last_model_config = snapshot.model.clone();
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) {
        if let Some(e) = self.engine.as_mut() {
            e.unload();
        }
        self.last_model_config = None;
    }

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(&mut self, signal: RuntimeControlSignal) {
        match signal {
            RuntimeControlSignal::GlobalUnload { op_id } => {
                tracing::debug!(op_id, "diffusion runtime global unload");
                if let Some(engine) = self.engine.as_mut() {
                    engine.unload();
                }
                self.last_model_config = None;
            }
            RuntimeControlSignal::GlobalLoad { op_id, payload } => {
                let _ = payload;
                tracing::debug!(op_id, "diffusion runtime global load pre-cleanup");
                if let Some(engine) = self.engine.as_mut() {
                    engine.unload();
                }
                self.last_model_config = None;
            }
        }
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) {
        if let Some(e) = self.engine.as_mut() {
            e.unload();
        }
        self.last_model_config = None;
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn deployment_snapshot_reads_typed_ggml_diffusion_model_config() {
        let snapshot = DeploymentSnapshot::with_model(
            11,
            Payload::typed(GgmlDiffusionLoadConfig {
                model_path: PathBuf::from("model.gguf"),
                diffusion_model_path: Some(PathBuf::from("diffusion.gguf")),
                vae_path: Some(PathBuf::from("vae.gguf")),
                taesd_path: None,
                clip_l_path: None,
                clip_g_path: None,
                t5xxl_path: None,
                clip_vision_path: None,
                control_net_path: None,
                flash_attn: true,
                vae_device: Some("cpu".to_owned()),
                clip_device: None,
                offload_params_to_cpu: false,
                enable_mmap: true,
                n_threads: Some(8),
            }),
        );

        let config = snapshot
            .typed_model_config::<GgmlDiffusionLoadConfig>()
            .expect("typed deployment snapshot should decode");

        assert_eq!(config.model_path, PathBuf::from("model.gguf"));
        assert_eq!(config.diffusion_model_path, Some(PathBuf::from("diffusion.gguf")));
        assert_eq!(config.vae_path, Some(PathBuf::from("vae.gguf")));
        assert_eq!(config.vae_device.as_deref(), Some("cpu"));
        assert_eq!(config.n_threads, Some(8));
    }
}
