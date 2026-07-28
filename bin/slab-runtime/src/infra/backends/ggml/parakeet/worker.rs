//! Backend worker for `ggml.parakeet`.
//!
//! Defines [`ParakeetWorker`] logic for runtime-managed worker loops.
//!
//! # Supported ops
//!
//! | Op string          | Event variant    | Description                                        |
//! |--------------------|------------------|----------------------------------------------------|
//! | `"model.load"`     | `LoadModel`      | Load a model from the engine.                      |
//! | `"model.unload"`   | `UnloadModel`    | Drop the model handle; call model.load to restore. |
//! | `"inference"`      | `Inference`      | Transcribe audio; input is packed `f32` PCM.       |
//!
//! ### `model.load` input payload
//! Expects typed runtime-owned `GgmlParakeetLoadConfig` payloads.

use super::engine::GGMLParakeetEngine;
use super::error::GGMLParakeetWorkerError;
use crate::domain::models::{
    AudioTranscriptionOptions, AudioTranscriptionResponse, GgmlParakeetLoadConfig,
};
use slab_runtime_core::Payload;
use slab_runtime_core::backend::{
    BroadcastSeq, ControlOpId, Input, Options, PeerControlBus, Typed,
};
use slab_runtime_macros::backend_handler;

// ── Worker ────────────────────────────────────────────────────────────────────

/// A single parakeet backend worker.
///
/// Each worker **owns** its engine (library handle + model context). There is
/// no shared mutable state between workers, so no `Mutex` is needed on the
/// context. When `num_workers > 1` multiple workers are spawned; each worker
/// owns an independent engine forked from the same library handle and manages
/// its own model context independently.
pub struct ParakeetWorker {
    /// - `None` → engine not initialized.
    /// - `Some(e)` where `e.ctx` is None → engine loaded, no model.
    /// - `Some(e)` where `e.ctx` is Some → engine + model loaded.
    engine: Option<GGMLParakeetEngine>,
    /// Peer synchronization emitter shared among workers.
    peer_bus: PeerControlBus,
    last_model_config: Option<Payload>,
}

#[backend_handler(peer_bus = peer_bus)]
impl ParakeetWorker {
    pub fn new(engine: Option<GGMLParakeetEngine>, peer_bus: PeerControlBus) -> Self {
        Self { engine, peer_bus, last_model_config: None }
    }

    #[on_event(LoadModel)]
    async fn on_load_model(
        &mut self,
        params: Input<GgmlParakeetLoadConfig>,
        seq: BroadcastSeq,
    ) -> Result<(), GGMLParakeetWorkerError> {
        self.handle_load_model(params.0, seq.0).await
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self, seq: BroadcastSeq) -> Result<(), GGMLParakeetWorkerError> {
        self.handle_unload_model(seq.0).await
    }

    #[on_event(Inference)]
    async fn on_inference(
        &mut self,
        input: Payload,
        options: Options<AudioTranscriptionOptions>,
    ) -> Result<Typed<AudioTranscriptionResponse>, GGMLParakeetWorkerError> {
        self.handle_inference(input, options.0).await
    }

    // ── model.load ────────────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        params: GgmlParakeetLoadConfig,
        seq_id: u64,
    ) -> Result<(), GGMLParakeetWorkerError> {
        let engine = match self.engine.as_mut() {
            Some(e) => e,
            None => {
                return Err(GGMLParakeetWorkerError::load("engine not initialized"));
            }
        };
        let model_payload = Payload::typed(params.clone());

        // Model loading is CPU/I-O bound; use block_in_place on this thread.
        let result = tokio::task::block_in_place(|| engine.new_context_from_config(params.clone()));

        match result {
            Ok(()) => {
                self.last_model_config = Some(model_payload.clone());
                // Broadcast so peer workers also load the same model.
                self.emit_peer_load_model_deployment_payload(seq_id, model_payload);
                Ok(())
            }
            Err(error) => Err(GGMLParakeetWorkerError::load(error.to_string())),
        }
    }

    // ── model.unload ──────────────────────────────────────────────────────────

    async fn handle_unload_model(&mut self, seq_id: u64) -> Result<(), GGMLParakeetWorkerError> {
        match self.engine.as_mut() {
            Some(e) => {
                e.unload();
                self.last_model_config = None;
                // Broadcast so every peer worker also drops its context.
                self.emit_peer_unload_generation(seq_id);
                Ok(())
            }
            None => Err(GGMLParakeetWorkerError::unload("engine not initialized")),
        }
    }

    // ── inference ─────────────────────────────────────────────────────────────

    async fn handle_inference(
        &mut self,
        input: Payload,
        params: AudioTranscriptionOptions,
    ) -> Result<Typed<AudioTranscriptionResponse>, GGMLParakeetWorkerError> {
        let engine = match self.engine.as_ref() {
            Some(e) => e,
            None => {
                return Err(GGMLParakeetWorkerError::inference(
                    "parakeet backend not ready: model not loaded. Call model.load first",
                ));
            }
        };

        let samples = match input.to_f32_arc() {
            Ok(b) => b,
            Err(e) => {
                return Err(GGMLParakeetWorkerError::contract(format!(
                    "invalid input for parakeet inference: expected f32 PCM audio samples, got: {e}"
                )));
            }
        };

        if samples.is_empty() {
            return Err(GGMLParakeetWorkerError::contract(
                "invalid input for parakeet inference: audio samples are empty",
            ));
        }

        // Parakeet inference is CPU/GPU-bound; use block_in_place so the engine
        // context stays on this thread without needing an additional spawn_blocking.
        let decode_configured = params.decode.as_ref().is_some_and(|decode| {
            decode.offset_ms.is_some()
                || decode.duration_ms.is_some()
                || decode.no_context.is_some()
        });
        let result = tokio::task::block_in_place(|| {
            tracing::debug!(
                sample_count = samples.len(),
                duration_sec = samples.len() as f64 / 16000.0,
                decode_configured,
                "starting parakeet inference"
            );
            engine.inference_with_options(&samples, &params)
        });

        match result {
            Err(e) => {
                tracing::error!(error = %e, "parakeet inference failed");
                Err(GGMLParakeetWorkerError::inference(format!("parakeet inference failed: {e}")))
            }
            Ok(entries) => {
                tracing::debug!(segment_count = entries.len(), "parakeet inference succeeded");
                let mut out = String::new();
                for entry in entries {
                    if let Some(line) = entry.line {
                        let ts = entry.timespan;
                        out.push_str(&format!(
                            "{} --> {}: {}\n",
                            ts.start.msecs(),
                            ts.end.msecs(),
                            line
                        ));
                    }
                }
                Ok(Typed(AudioTranscriptionResponse { text: out }))
            }
        }
    }

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(
        &mut self,
        params: Input<GgmlParakeetLoadConfig>,
    ) -> Result<(), GGMLParakeetWorkerError> {
        let params = params.0;
        let model_path = params.model_path.display().to_string();
        if let Some(engine) = self.engine.as_mut()
            && !engine.is_model_loaded()
        {
            let result =
                tokio::task::block_in_place(|| engine.new_context_from_config(params.clone()));
            if let Err(e) = result {
                tracing::warn!(
                    model_path = %model_path,
                    error = %e,
                    "parakeet worker: broadcast LoadModel failed"
                );
            }
        }
        self.last_model_config = Some(Payload::typed(params));
        Ok(())
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) -> Result<(), GGMLParakeetWorkerError> {
        if let Some(e) = self.engine.as_mut() {
            e.unload();
        }
        self.last_model_config = None;
        Ok(())
    }

    #[on_runtime_control(GlobalUnload)]
    #[on_runtime_control(GlobalLoad)]
    async fn apply_runtime_control(
        &mut self,
        op_id: ControlOpId,
    ) -> Result<(), GGMLParakeetWorkerError> {
        tracing::debug!(op_id = op_id.0, "parakeet runtime control pre-cleanup");
        if let Some(engine) = self.engine.as_mut() {
            engine.unload();
        }
        self.last_model_config = None;
        Ok(())
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) -> Result<(), GGMLParakeetWorkerError> {
        if let Some(e) = self.engine.as_mut() {
            e.unload();
        }
        self.last_model_config = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use slab_runtime_core::backend::DeploymentSnapshot;

    #[test]
    fn deployment_snapshot_reads_typed_parakeet_model_config() {
        let snapshot = DeploymentSnapshot::with_model(
            7,
            Payload::typed(GgmlParakeetLoadConfig { model_path: PathBuf::from("model.bin") }),
        );

        let config = snapshot
            .typed_model_config::<GgmlParakeetLoadConfig>()
            .expect("typed deployment snapshot should decode");

        assert_eq!(config.model_path, PathBuf::from("model.bin"));
    }
}
