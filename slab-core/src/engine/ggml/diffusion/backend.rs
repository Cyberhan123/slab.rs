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

use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::broadcast;

use crate::engine::ggml::config::{LibLoadConfig, ModelLoadConfig};
use crate::engine::ggml::diffusion::adapter::GGMLDiffusionEngine;
use crate::runtime::backend::backend_handler;
use crate::runtime::backend::protocol::{
    BackendReply, BackendRequest, PeerWorkerCommand, RuntimeControlSignal, WorkerCommand,
};
use crate::runtime::types::Payload;

// ── Configurations ────────────────────────────────────────────────────────────

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
/// context.  When `num_workers > 1` multiple workers are spawned; each
/// worker owns an independent engine forked from the same library handle and
/// manages its own model context independently.
///
/// Workers listen on both the shared `mpsc` ingress queue (competitive 鈥?/// only one worker processes each request) and a `broadcast` channel
/// (fan-out – every worker receives management commands such as `Unload`).
pub(crate) struct DiffusionWorker {
    /// - `None` 鈫?library not loaded.
    /// - `Some(e)` where `e.ctx` is None 鈫?lib loaded, no model.
    /// - `Some(e)` where `e.ctx` is Some 鈫?lib + model loaded.
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
                // Broadcast so peer workers also load the same model.
                let _ = self
                    .bc_tx
                    .send(WorkerCommand::Peer(PeerWorkerCommand::LoadModel {
                        model_path: config.model_path,
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
