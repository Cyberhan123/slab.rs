//! Backend worker adapter for `ggml.diffusion`.
//!
//! Provides [`spawn_backend`] and [`spawn_backend_with_path`] which start a
//! Tokio task translating [`BackendRequest`] messages into stable-diffusion
//! inference calls.
//!
//! # Supported ops
//!
//! | Op string           | Event variant    | Description                                       |
//! |---------------------|------------------|---------------------------------------------------|
//! | `"lib.load"`        | `LoadLibrary`    | Load (skip if already loaded) the dylib.          |
//! | `"lib.reload"`      | `ReloadLibrary`  | Replace the library, discarding current model.    |
//! | `"model.load"`      | `LoadModel`      | Load a model from the pre-loaded library.         |
//! | `"model.unload"`    | `UnloadModel`    | Drop the model handle; call model.load to restore. |
//! | `"inference.image"` | `InferenceImage` | Text-to-image; input is JSON generation params.   |
//!
//! ### `lib.load` / `lib.reload` input JSON
//! ```json
//! { "lib_path": "/path/to/libstable-diffusion.so" }
//! ```
//!
//! ### `model.load` input JSON
//! ```json
//! { "model_path": "/path/to/model.gguf" }
//! ```

use std::str::FromStr;
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::mpsc;

use crate::api::Event;
use crate::engine::ggml::diffusion::adapter::GGMLDiffusionEngine;
use crate::runtime::backend::protocol::{BackendReply, BackendRequest};
use crate::runtime::types::Payload;

// ── Configurations ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LibLoadConfig {
    lib_path: String,
}

#[derive(Deserialize)]
struct ModelLoadConfig {
    model_path: String,
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

// ── Worker ────────────────────────────────────────────────────────────────────

struct DiffusionWorker {
    /// Wraps both the library handle and the optional model context.
    /// - `None` → library not loaded.
    /// - `Some(e)` where `e.ctx` is None → lib loaded, no model.
    /// - `Some(e)` where `e.ctx` is Some → lib + model loaded.
    engine: Option<Arc<GGMLDiffusionEngine>>,
}

impl DiffusionWorker {
    fn new(engine: Option<Arc<GGMLDiffusionEngine>>) -> Self {
        Self { engine }
    }

    async fn handle(&mut self, req: BackendRequest) {
        let BackendRequest {
            op,
            input,
            reply_tx,
            ..
        } = req;

        match Event::from_str(&op.name) {
            Ok(Event::LoadLibrary) => self.handle_load_library(input, reply_tx).await,
            Ok(Event::ReloadLibrary) => self.handle_reload_library(input, reply_tx).await,
            Ok(Event::LoadModel) => self.handle_load_model(input, reply_tx).await,
            Ok(Event::UnloadModel) => self.handle_unload_model(reply_tx).await,
            Ok(Event::InferenceImage) => self.handle_inference_image(input, reply_tx).await,
            Ok(_) | Err(_) => {
                let _ = reply_tx.send(BackendReply::Error(format!("unknown op: {}", op.name)));
            }
        }
    }

    // ── lib.load ──────────────────────────────────────────────────────────────

    async fn handle_load_library(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
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
    ) {
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
            None => {
                let _ = reply_tx.send(BackendReply::Error(
                    "library not loaded; call lib.load first".into(),
                ));
                return;
            }
        };

        let config: ModelLoadConfig = match input.to_json() {
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
            let ctx_params = SdContextParams::with_model(&config.model_path);
            engine.new_context(&ctx_params)
        });

        match result {
            Ok(()) => {
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

    async fn handle_unload_model(&mut self, reply_tx: tokio::sync::oneshot::Sender<BackendReply>) {
        let engine = match self.engine.as_ref() {
            Some(e) => Arc::clone(e),
            None => {
                let _ = reply_tx.send(BackendReply::Error(
                    "library not loaded; call lib.load first".into(),
                ));
                return;
            }
        };

        match engine.unload() {
            Ok(()) => {
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(
                    Arc::from([] as [u8; 0]),
                )));
            }
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
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
            Some(e) => Arc::clone(e),
            None => {
                let _ = reply_tx.send(BackendReply::Error("model not loaded".into()));
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

        // Image generation is CPU/GPU-bound; use block_in_place so the engine
        // (and its internal Mutex<ctx>) stays on this thread.
        let result = tokio::task::block_in_place(|| {
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
        });

        match result {
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e.to_string()));
            }
            Ok(images) => {
                let data = images
                    .into_iter()
                    .next()
                    .map(|img| img.data)
                    .unwrap_or_default();
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(
                    data.as_slice(),
                ))));
            }
        }
    }
}

// ── Public entry points ───────────────────────────────────────────────────────
/// Spawn a diffusion backend worker with a pre-loaded engine handle.
///
/// Used by `api::init` to separate library loading (phase 1) from worker
/// spawning (phase 2) so that no tasks are started if any library fails.
pub(crate) fn spawn_backend_with_engine(
    capacity: usize,
    engine: Option<Arc<GGMLDiffusionEngine>>,
) -> mpsc::Sender<BackendRequest> {
    let (tx, mut rx) = mpsc::channel::<BackendRequest>(capacity);
    tokio::spawn(async move {
        let mut worker = DiffusionWorker::new(engine);
        while let Some(req) = rx.recv().await {
            worker.handle(req).await;
        }
    });
    tx
}
