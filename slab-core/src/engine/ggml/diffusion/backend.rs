//! Backend worker adapter for `ggml.diffusion`.
//!
//! Defines [`DiffusionWorker`] logic for runtime-managed worker loops.
//!
//! # Supported ops
//!
//! | Op string           | Event variant    | Description                                       |
//! |---------------------|------------------|---------------------------------------------------|
//! | `"lib.load"`        | `LoadLibrary`    | Load (skip if already loaded) the dylib.          |
//! | `"lib.reload"`      | `ReloadLibrary`  | Replace the library, discarding current model.    |
//! | `"model.load"`      | `LoadModel`      | Load a model from the pre-loaded library.         |
//! | `"model.unload"`    | `UnloadModel`    | Drop the model handle; call model.load to restore. |
//! | `"inference.image"` | `InferenceImage` | Image generation; input is JSON generation params. |
//!
//! ### `lib.load` / `lib.reload` input JSON
//! ```json
//! { "lib_path": "/path/to/libstable-diffusion.so" }
//! ```
//!
//! ### `model.load` input JSON (diffusion extended)
//! ```json
//! {
//!   "model_path": "/path/to/model.gguf",
//!   "vae_path": "",
//!   "taesd_path": "",
//!   "lora_model_dir": "",
//!   "flash_attn": false,
//!   "keep_vae_on_cpu": false,
//!   "keep_clip_on_cpu": false,
//!   "offload_params_to_cpu": false
//! }
//! ```
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
use tokio::sync::broadcast;

use crate::engine::ggml::config::{DiffusionModelLoadConfig, LibLoadConfig};
use crate::engine::ggml::diffusion::adapter::GGMLDiffusionEngine;
use crate::scheduler::backend::backend_handler;
use crate::scheduler::backend::protocol::{
    BackendReply, BackendRequest, PeerWorkerCommand, RuntimeControlSignal, WorkerCommand,
};
use crate::scheduler::types::Payload;

// ── Configurations ────────────────────────────────────────────────────────────

/// Parse a sample method string into the native enum value.
fn parse_sample_method(s: &str) -> slab_diffusion::SampleMethod {
    use slab_diffusion::{
        SAMPLE_EULER as SD_EULER, SAMPLE_EULER_A as SD_EULER_A, SAMPLE_LCM as SD_LCM,
        SAMPLE_METHOD_COUNT,
    };
    use slab_diffusion_sys::{
        sample_method_t_DPM2_SAMPLE_METHOD as SD_DPM2,
        sample_method_t_DPMPP2M_SAMPLE_METHOD as SD_DPM_PP_2M,
        sample_method_t_DPMPP2Mv2_SAMPLE_METHOD as SD_DPM_PP_2M_V2,
        sample_method_t_DPMPP2S_A_SAMPLE_METHOD as SD_DPM_PP_2S_A,
        sample_method_t_HEUN_SAMPLE_METHOD as SD_HEUN,
        sample_method_t_IPNDM_SAMPLE_METHOD as SD_IPNDM,
        sample_method_t_IPNDM_V_SAMPLE_METHOD as SD_IPNDM_V,
    };
    match s {
        "euler" => SD_EULER,
        "euler_a" => SD_EULER_A,
        "lcm" => SD_LCM,
        "heun" => SD_HEUN,
        "dpm2" => SD_DPM2,
        "dpm++2s_a" => SD_DPM_PP_2S_A,
        "dpm++2m" => SD_DPM_PP_2M,
        "dpm++2mv2" => SD_DPM_PP_2M_V2,
        "ipndm" => SD_IPNDM,
        "ipndm_v" => SD_IPNDM_V,
        _ => SAMPLE_METHOD_COUNT, // auto
    }
}

/// Parse a scheduler string into the native enum value.
fn parse_scheduler(s: &str) -> slab_diffusion::Scheduler {
    use slab_diffusion::{
        SCHEDULER_COUNT, SCHEDULER_DISCRETE as SD_DISCRETE, SCHEDULER_KARRAS as SD_KARRAS,
    };
    use slab_diffusion_sys::{
        scheduler_t_AYS_SCHEDULER as SD_AYS, scheduler_t_EXPONENTIAL_SCHEDULER as SD_EXPONENTIAL,
        scheduler_t_GITS_SCHEDULER as SD_GITS,
    };
    match s {
        "discrete" => SD_DISCRETE,
        "karras" => SD_KARRAS,
        "exponential" => SD_EXPONENTIAL,
        "ays" => SD_AYS,
        "gits" => SD_GITS,
        _ => SCHEDULER_COUNT, // auto
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
    #[serde(default)]
    clip_skip: i32,
    #[serde(default = "default_strength")]
    strength: f32,
    #[serde(default)]
    eta: f32,
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
fn default_batch_count() -> i32 {
    1
}
fn default_channels() -> u32 {
    3
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
    /// - `None` → library not loaded.
    /// - `Some(e)` where `e.ctx` is None → lib loaded, no model.
    /// - `Some(e)` where `e.ctx` is Some → lib + model loaded.
    engine: Option<GGMLDiffusionEngine>,
    /// Broadcast sender shared among all workers so that any worker can
    /// propagate state-change commands (e.g. `Unload`) to all peers.
    bc_tx: broadcast::Sender<WorkerCommand>,
    /// Stable index used to populate `sender_id` when broadcasting.
    worker_id: usize,
}

#[backend_handler]
impl DiffusionWorker {
    pub(crate) fn new(
        engine: Option<GGMLDiffusionEngine>,
        bc_tx: broadcast::Sender<WorkerCommand>,
        worker_id: usize,
    ) -> Self {
        Self {
            engine,
            bc_tx,
            worker_id,
        }
    }

    #[on_event(LoadLibrary)]
    async fn on_load_library(&mut self, req: BackendRequest) {
        let BackendRequest {
            input,
            broadcast_seq,
            reply_tx,
            ..
        } = req;
        let seq_id = broadcast_seq.unwrap_or(0);
        self.handle_load_library(input, reply_tx, seq_id).await;
    }

    #[on_event(ReloadLibrary)]
    async fn on_reload_library(&mut self, req: BackendRequest) {
        let BackendRequest {
            input,
            broadcast_seq,
            reply_tx,
            ..
        } = req;
        let seq_id = broadcast_seq.unwrap_or(0);
        self.handle_reload_library(input, reply_tx, seq_id).await;
    }

    #[on_event(LoadModel)]
    async fn on_load_model(&mut self, req: BackendRequest) {
        let BackendRequest {
            input,
            broadcast_seq,
            reply_tx,
            ..
        } = req;
        let seq_id = broadcast_seq.unwrap_or(0);
        self.handle_load_model(input, reply_tx, seq_id).await;
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self, req: BackendRequest) {
        let BackendRequest {
            broadcast_seq,
            reply_tx,
            ..
        } = req;
        let seq_id = broadcast_seq.unwrap_or(0);
        self.handle_unload_model(reply_tx, seq_id).await;
    }

    #[on_event(InferenceImage)]
    async fn on_inference_image(&mut self, req: BackendRequest) {
        let BackendRequest {
            input, reply_tx, ..
        } = req;
        self.handle_inference_image(input, reply_tx).await;
    }

    // ── lib.load ──────────────────────────────────────────────────────────────

    async fn handle_load_library(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        if self.engine.is_some() {
            let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(
                Arc::from([] as [u8; 0]),
            )));
            return;
        }

        let config: LibLoadConfig = match input.to_json() {
            Ok(c) => c,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!("invalid lib.load config: {e}")));
                return;
            }
        };

        match GGMLDiffusionEngine::from_path(&config.lib_path) {
            Ok(engine) => {
                self.engine = Some(engine);
                // Broadcast so peer workers also load the same library.
                let _ = self
                    .bc_tx
                    .send(WorkerCommand::Peer(PeerWorkerCommand::LoadLibrary {
                        lib_path: config.lib_path,
                        sender_id: self.worker_id,
                        seq_id,
                    }));
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(
                    Arc::from([] as [u8; 0]),
                )));
            }
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
        }
    }

    // ── lib.reload ────────────────────────────────────────────────────────────

    async fn handle_reload_library(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        let config: LibLoadConfig = match input.to_json() {
            Ok(c) => c,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "invalid lib.reload config: {e}"
                )));
                return;
            }
        };

        // Drop current engine (lib + model context).
        self.engine = None;

        match GGMLDiffusionEngine::from_path(&config.lib_path) {
            Ok(engine) => {
                self.engine = Some(engine);
                // Broadcast so peer workers drop their old engine and reload too.
                let _ = self
                    .bc_tx
                    .send(WorkerCommand::Peer(PeerWorkerCommand::ReloadLibrary {
                        lib_path: config.lib_path,
                        sender_id: self.worker_id,
                        seq_id,
                    }));
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(
                    Arc::from([] as [u8; 0]),
                )));
            }
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
        }
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
                let _ = reply_tx.send(BackendReply::Error(
                    "library not loaded; call lib.load first".into(),
                ));
                return;
            }
        };

        let config: DiffusionModelLoadConfig = match input.to_json() {
            Ok(c) => c,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "invalid model.load config: {e}"
                )));
                return;
            }
        };

        // Model loading is CPU/I-O bound; use block_in_place on this thread.
        let result = tokio::task::block_in_place(|| {
            use slab_diffusion::SdContextParams;
            let mut ctx_params = SdContextParams::with_model(&config.model_path);
            ctx_params.diffusion_model_path = config.diffusion_model_path;
            ctx_params.vae_path = config.vae_path;
            ctx_params.taesd_path = config.taesd_path;
            ctx_params.clip_l_path = config.clip_l_path;
            ctx_params.clip_g_path = config.clip_g_path;
            ctx_params.t5xxl_path = config.t5xxl_path;
            ctx_params.clip_vision_path = config.clip_vision_path;
            ctx_params.control_net_path = config.control_net_path;
            ctx_params.flash_attn = config.flash_attn;
            ctx_params.keep_vae_on_cpu = config.keep_vae_on_cpu;
            ctx_params.keep_clip_on_cpu = config.keep_clip_on_cpu;
            ctx_params.offload_params_to_cpu = config.offload_params_to_cpu;
            ctx_params.enable_mmap = config.enable_mmap;
            if config.n_threads != 0 {
                ctx_params.n_threads = config.n_threads;
            }
            engine.new_context(&ctx_params)
        });

        let model_path = config.model_path;
        match result {
            Ok(()) => {
                // Broadcast so peer workers also load the same model.
                let _ = self
                    .bc_tx
                    .send(WorkerCommand::Peer(PeerWorkerCommand::LoadModel {
                        model_path,
                        sender_id: self.worker_id,
                        seq_id,
                    }));
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(
                    Arc::from([] as [u8; 0]),
                )));
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
                // Broadcast so every peer worker also drops its context.
                // Ignore errors: no receivers simply means no other workers.
                let _ = self
                    .bc_tx
                    .send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
                        sender_id: self.worker_id,
                        seq_id,
                    }));
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(
                    Arc::from([] as [u8; 0]),
                )));
            }
            None => {
                let _ = reply_tx.send(BackendReply::Error(
                    "library not loaded; call lib.load first".into(),
                ));
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
                let _ = reply_tx.send(BackendReply::Error(
                    "library not loaded; call lib.load first".into(),
                ));
                return;
            }
        };

        let gen_params: GenImageParams = match input.to_json() {
            Ok(p) => p,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "invalid inference.image params: {e}"
                )));
                return;
            }
        };

        // Decode optional init image (base64-encoded raw RGB pixels).
        let init_image = if let Some(ref b64) = gen_params.init_image_b64 {
            match base64::engine::general_purpose::STANDARD.decode(b64) {
                Ok(data) => Some(slab_diffusion::SdImage {
                    width: gen_params.init_image_width,
                    height: gen_params.init_image_height,
                    channel: gen_params.init_image_channels,
                    data,
                }),
                Err(e) => {
                    let _ = reply_tx.send(BackendReply::Error(format!(
                        "failed to decode init_image_b64: {e}"
                    )));
                    return;
                }
            }
        } else {
            None
        };

        // Image generation is CPU/GPU-bound; use block_in_place so the engine
        // context stays on this thread without needing an additional spawn_blocking.
        let result = tokio::task::block_in_place(|| {
            use slab_diffusion::SdImgGenParams;
            let params = SdImgGenParams {
                prompt: gen_params.prompt,
                negative_prompt: gen_params.negative_prompt,
                width: gen_params.width,
                height: gen_params.height,
                sample_steps: gen_params.sample_steps,
                cfg_scale: gen_params.cfg_scale,
                guidance: gen_params.guidance,
                seed: gen_params.seed,
                sample_method: parse_sample_method(&gen_params.sample_method),
                scheduler: parse_scheduler(&gen_params.scheduler),
                clip_skip: gen_params.clip_skip,
                strength: gen_params.strength,
                eta: gen_params.eta,
                batch_count: gen_params.batch_count.max(1),
                init_image,
            };
            engine.generate_image(&params)
        });

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
                            "b64": b64,
                            "width": w,
                            "height": h,
                            "channels": channels,
                        }))
                    })
                    .collect();
                let json_bytes = serde_json::to_vec(&encoded).unwrap_or_default();
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(
                    json_bytes.as_slice(),
                ))));
            }
        }
    }

    #[on_peer_control(LoadLibrary)]
    async fn on_peer_load_library(&mut self, cmd: PeerWorkerCommand) {
        let PeerWorkerCommand::LoadLibrary { lib_path, .. } = cmd else {
            return;
        };
        if self.engine.is_none() {
            if let Ok(engine) = GGMLDiffusionEngine::from_path(&lib_path) {
                self.engine = Some(engine);
            }
        }
    }

    #[on_peer_control(ReloadLibrary)]
    async fn on_peer_reload_library(&mut self, cmd: PeerWorkerCommand) {
        let PeerWorkerCommand::ReloadLibrary { lib_path, .. } = cmd else {
            return;
        };
        self.engine = None;
        if let Ok(engine) = GGMLDiffusionEngine::from_path(&lib_path) {
            self.engine = Some(engine);
        }
    }

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(&mut self, cmd: PeerWorkerCommand) {
        let PeerWorkerCommand::LoadModel { model_path, .. } = cmd else {
            return;
        };
        if let Some(engine) = self.engine.as_mut() {
            if !engine.is_model_loaded() {
                let result = tokio::task::block_in_place(|| {
                    use slab_diffusion::SdContextParams;
                    let ctx_params = SdContextParams::with_model(&model_path);
                    engine.new_context(&ctx_params)
                });
                if let Err(e) = result {
                    tracing::warn!(
                        model_path,
                        error = %e,
                        "diffusion worker: broadcast LoadModel failed"
                    );
                }
            }
        }
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) {
        if let Some(e) = self.engine.as_mut() {
            e.unload();
        }
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
            }
            RuntimeControlSignal::GlobalLoad { op_id, payload } => {
                let _ = payload;
                tracing::debug!(op_id, "diffusion runtime global load pre-cleanup");
                if let Some(engine) = self.engine.as_mut() {
                    engine.unload();
                }
            }
        }
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) {
        if let Some(e) = self.engine.as_mut() {
            e.unload();
        }
    }
}
