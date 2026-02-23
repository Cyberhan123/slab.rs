//! Backend worker adapter for `ggml.diffusion`.
//!
//! Provides [`spawn_backend`] which starts a Tokio task that translates
//! [`BackendRequest`] messages into `GGMLDiffusionEngine` API calls.
//!
//! Supported ops
//! - `"model.load"` – load the stable-diffusion dynamic library and a model.
//!   Input bytes must be a UTF-8 JSON object:
//!   ```json
//!   { "lib_path": "/path/to/libstable-diffusion.so",
//!     "model_path": "/path/to/model.gguf" }
//!   ```
//! - `"generate_image"` – text-to-image; input is a UTF-8 JSON object
//!   matching the generation parameters:
//!   ```json
//!   { "prompt": "a lovely cat",
//!     "width": 512, "height": 512,
//!     "sample_steps": 20 }
//!   ```
//!   Returns the raw pixel data of the first generated image as bytes
//!   (RGB, `width * height * 3` bytes).
//!
//! Any op called before `"model.load"` returns
//! `BackendReply::Error("model not loaded")`.

use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::mpsc;

use crate::engine::ggml::diffusion::adapter::GGMLDiffusionEngine;
use crate::runtime::backend::protocol::{BackendReply, BackendRequest};
use crate::runtime::types::Payload;

// ── Load configuration ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LoadConfig {
    lib_path: String,
    model_path: String,
}

// ── Generate-image parameters ─────────────────────────────────────────────────

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
}

fn default_width() -> u32 { 512 }
fn default_height() -> u32 { 512 }
fn default_steps() -> i32 { 20 }

// ── Worker ────────────────────────────────────────────────────────────────────

struct DiffusionWorker {
    /// Non-None after a successful `model.load`.
    engine: Option<Arc<GGMLDiffusionEngine>>,
}

impl DiffusionWorker {
    fn new() -> Self {
        Self { engine: None }
    }

    async fn handle(&mut self, req: BackendRequest) {
        let BackendRequest {
            op,
            input,
            reply_tx,
            ..
        } = req;

        match op.name.as_str() {
            "model.load" => self.handle_load(input, reply_tx).await,
            "model.unload" => self.handle_unload(reply_tx).await,
            "inference_image" => self.handle_inference_image(input, reply_tx).await,
            other => {
                let _ = reply_tx.send(BackendReply::Error(format!("unknown op: {other}")));
            }
        }
    }

    async fn handle_load(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let config: LoadConfig = match input.to_json() {
            Ok(c) => c,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!("invalid model.load config: {e}")));
                return;
            }
        };

        let engine = match GGMLDiffusionEngine::init(&config.lib_path) {
            Ok(e) => e,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!("init engine: {e}")));
                return;
            }
        };

        use slab_diffusion::SdContextParams;
        let ctx_params = SdContextParams::with_model(&config.model_path);
        if let Err(e) = engine.new_context(&ctx_params) {
            let _ = reply_tx.send(BackendReply::Error(format!("load model: {e}")));
            return;
        }

        self.engine = Some(engine);
        let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from([] as [u8; 0]))));
    }

    async fn handle_unload(&mut self, reply_tx: tokio::sync::oneshot::Sender<BackendReply>) {
        self.engine = None;
        let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(std::sync::Arc::from(&b""[..]))));
    }

    async fn handle_inference_image(
        &self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
            None => {
                let _ = reply_tx.send(BackendReply::Error("model not loaded".into()));
                return;
            }
        };

        let gen_params: GenImageParams = match input.to_json() {
            Ok(p) => p,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!("invalid generate_image params: {e}")));
                return;
            }
        };

        // Image generation is CPU/GPU-bound; run in spawn_blocking.
        let result = tokio::task::spawn_blocking(move || {
            use slab_diffusion::SdImgGenParams;
            let params = SdImgGenParams {
                prompt: gen_params.prompt,
                negative_prompt: gen_params.negative_prompt,
                width: gen_params.width,
                height: gen_params.height,
                sample_steps: gen_params.sample_steps,
                ..SdImgGenParams::default()
            };
            engine.generate_image(&params)
        })
        .await;

        match result {
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!("spawn_blocking panic: {e}")));
            }
            Ok(Err(e)) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
            Ok(Ok(images)) => {
                let data = images.into_iter().next().map(|img| img.data).unwrap_or_default();
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(
                    data.as_slice(),
                ))));
            }
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Spawn the diffusion backend worker and return its ingress sender.
///
/// The worker task handles [`BackendRequest`] messages sequentially.
/// It starts with no model loaded; send `op="model.load"` first.
///
/// # Panics
/// Panics if called outside a Tokio runtime.
pub fn spawn_backend(capacity: usize) -> mpsc::Sender<BackendRequest> {
    let (tx, mut rx) = mpsc::channel::<BackendRequest>(capacity);
    tokio::spawn(async move {
        let mut worker = DiffusionWorker::new();
        while let Some(req) = rx.recv().await {
            worker.handle(req).await;
        }
    });
    tx
}
