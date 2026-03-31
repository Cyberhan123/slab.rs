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
//! | `"inference.image"` | `InferenceImage` | Image generation; input is JSON generation params. |
//! ### `model.load` input payload
//! Uses a typed [`slab_types::GgmlDiffusionLoadConfig`] payload inside `slab-core`.
//!
//! ### `inference.image` input JSON
//! ```json
//! {
//!   "prompt": "a lovely cat",
//!   "negative_prompt": "",
//!   "width": 512,
//!   "height": 512,
//!   "cfg_scale": 7.0,
//!   "guidance": 3.5,
//!   "sample_steps": 20,
//!   "seed": 42,
//!   "sample_method": "auto",
//!   "scheduler": "auto",
//!   "clip_skip": 0,
//!   "strength": 0.75,
//!   "eta": 0.0,
//!   "batch_count": 1,
//!   "init_image_b64": null
//! }
//! ```

use std::sync::Arc;

use base64::Engine as _;
use serde::Deserialize;
use slab_diffusion::{GuidanceParams, Image as DiffusionImage, SampleMethod, Scheduler, SlgParams};
use slab_types::GgmlDiffusionLoadConfig;
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

#[derive(Deserialize)]
struct GenImageParams {
    prompt: String,
    #[serde(default)]
    negative_prompt: String,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_steps")]
    sample_steps: i32,
    #[serde(default = "default_cfg_scale")]
    cfg_scale: f32,
    #[serde(default = "default_guidance")]
    guidance: f32,
    #[serde(default = "default_seed")]
    seed: i64,
    #[serde(default = "default_sample_method")]
    sample_method: String,
    #[serde(default = "default_scheduler")]
    scheduler: String,
    #[serde(default, rename = "clip_skip")]
    _clip_skip: i32,
    #[serde(default = "default_strength")]
    strength: f32,
    #[serde(default)]
    eta: f32,
    #[serde(default = "default_flow_shift")]
    flow_shift: f32,
    #[serde(default = "default_batch_count")]
    batch_count: i32,
    /// Base64-encoded raw RGB pixel data for img2img / video generation.
    /// When present, `init_image_width`, `init_image_height`, and
    /// `init_image_channels` must also be provided.
    #[serde(default)]
    init_image_b64: Option<String>,
    #[serde(default)]
    init_image_width: u32,
    #[serde(default)]
    init_image_height: u32,
    #[serde(default = "default_channels")]
    init_image_channels: u32,
}

fn default_width() -> u32 {
    512
}
fn default_height() -> u32 {
    512
}
fn default_steps() -> i32 {
    20
}
fn default_cfg_scale() -> f32 {
    7.0
}
fn default_guidance() -> f32 {
    3.5
}
fn default_seed() -> i64 {
    42
}
fn default_sample_method() -> String {
    "auto".to_string()
}
fn default_scheduler() -> String {
    "auto".to_string()
}
fn default_strength() -> f32 {
    0.75
}
fn default_flow_shift() -> f32 {
    f32::INFINITY
}
fn default_batch_count() -> i32 {
    1
}
fn default_channels() -> u32 {
    3
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
    gen_params: GenImageParams,
) -> Result<slab_diffusion::ImgParams, String> {
    let init_image = if let Some(b64) = gen_params.init_image_b64 {
        let data = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| format!("failed to decode init_image_b64: {e}"))?;
        Some(DiffusionImage {
            width: gen_params.init_image_width,
            height: gen_params.init_image_height,
            channel: gen_params.init_image_channels,
            data,
        })
    } else {
        None
    };

    let width = i32::try_from(gen_params.width)
        .map_err(|_| format!("width {} exceeds i32 range", gen_params.width))?;
    let height = i32::try_from(gen_params.height)
        .map_err(|_| format!("height {} exceeds i32 range", gen_params.height))?;

    let mut sample_params = engine.new_sample_params();
    sample_params.set_sample_steps(gen_params.sample_steps);
    sample_params.set_sample_method(parse_sample_method(&gen_params.sample_method));
    sample_params.set_scheduler(parse_scheduler(&gen_params.scheduler));
    sample_params.set_eta(gen_params.eta);
    sample_params.set_flow_shift(gen_params.flow_shift);
    sample_params.set_guidance(GuidanceParams {
        txt_cfg: gen_params.cfg_scale,
        img_cfg: gen_params.cfg_scale,
        distilled_guidance: gen_params.guidance,
        slg: SlgParams { layers: Vec::new(), layer_start: 0.0, layer_end: 0.0, scale: 0.0 },
    });

    let mut params = engine.new_image_params();
    params.set_prompt(&gen_params.prompt);
    params.set_negative_prompt(&gen_params.negative_prompt);
    params.set_width(width);
    params.set_height(height);
    params.set_sample_params(sample_params);
    params.set_strength(gen_params.strength);
    params.set_seed(gen_params.seed);
    params.set_batch_count(gen_params.batch_count);
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
                let _ =
                    reply_tx.send(BackendReply::Error(format!("invalid model.load config: {e}")));
                return;
            }
        };

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
                let _ =
                    reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
            }
            Err(e) => {
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

        let gen_params: GenImageParams = match input.to_json() {
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
                // Encode each output image as a PNG file, then base64-encode that PNG
                // so callers receive a standard image that can be decoded by any PNG
                // library.  `SdImage.data` contains raw pixel bytes (width × height ×
                // channel), NOT an image file — we must encode it ourselves.
                let encoded: Vec<serde_json::Value> = images
                    .into_iter()
                    .filter(|img| !img.data.is_empty())
                    .filter_map(|img| {
                        let (w, h, channels) = (img.width, img.height, img.channel as u8);
                        // Move `img.data` into the ImageBuffer — no clone needed since
                        // `img` is consumed by `into_iter()`.
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
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
                        Some(serde_json::json!({
                            "image": b64,
                            "width": w,
                            "height": h,
                            "channels": channels,
                        }))
                    })
                    .collect();
                let payload = serde_json::json!({ "images": encoded });
                let _ = reply_tx.send(BackendReply::Value(Payload::Json(payload)));
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
        if let Some(engine) = self.engine.as_mut() {
            if !engine.is_model_loaded() {
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
