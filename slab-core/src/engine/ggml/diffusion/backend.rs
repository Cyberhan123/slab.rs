//! Backend worker adapter for `ggml.diffusion`.
//!
//! Provides [`spawn_backend_with_engine`] which starts one or more Tokio tasks
//! translating [`BackendRequest`] messages into stable-diffusion inference calls.
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

/// A single diffusion backend worker.
///
/// Each worker **owns** its engine (library handle + model context).  There is
/// no shared mutable state between workers, so no `Mutex` is needed on the
/// context.  When `backend_capacity > 1` multiple workers are spawned; each
/// worker owns an independent engine forked from the same library handle and
/// manages its own model context independently.
struct DiffusionWorker {
    /// - `None` → library not loaded.
    /// - `Some(e)` where `e.ctx` is None → lib loaded, no model.
    /// - `Some(e)` where `e.ctx` is Some → lib + model loaded.
    engine: Option<GGMLDiffusionEngine>,
}

impl DiffusionWorker {
    fn new(engine: Option<GGMLDiffusionEngine>) -> Self {
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
        let engine = match self.engine.as_mut() {
            Some(e) => e,
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
        match self.engine.as_mut() {
            Some(e) => {
                e.unload();
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
///
/// A single worker is used even when `backend_capacity > 1` because stateful
/// ops (`lib.load`, `lib.reload`, `model.load`, `model.unload`) must be applied
/// to one shared engine/context.  Distributing these ops across multiple workers
/// would leave some workers with stale library handles or unloaded models,
/// causing nondeterministic inference failures.  The `ResourceManager` semaphore
/// still guards admission so the worker is never overwhelmed.
pub(crate) fn spawn_backend_with_engine(
    channel_capacity: usize,
    _num_workers: usize,
    engine: Option<GGMLDiffusionEngine>,
) -> mpsc::Sender<BackendRequest> {
    let (tx, mut rx) = mpsc::channel::<BackendRequest>(channel_capacity);
    tokio::spawn(async move {
        let mut worker = DiffusionWorker::new(engine);
        while let Some(req) = rx.recv().await {
            worker.handle(req).await;
        }
    });
    tx
}
