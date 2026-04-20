//! Backend worker for `candle.whisper`.
//!
//! Mirrors the `ggml.whisper` backend contract.
//!
//! # Supported ops
//!
//! | Op string        | Event variant | Description                                    |
//! |------------------|---------------|------------------------------------------------|
//! | `"model.load"`   | `LoadModel`   | Load Whisper model weights from disk.          |
//! | `"model.unload"` | `UnloadModel` | Drop model weights from memory.                |
//! | `"inference"`    | `Inference`   | Transcribe f32 PCM audio; returns raw text.    |
//!
//! ### `model.load` input payload
//! Uses a typed runtime-owned `CandleWhisperLoadConfig` payload inside `slab-runtime`.

use tokio::sync::broadcast;

use super::contract::{
    AudioTranscriptionOptions, AudioTranscriptionResponse, CandleWhisperLoadConfig,
};
use super::engine::CandleWhisperEngine;
use super::error::CandleWhisperWorkerError;
use slab_runtime_core::Payload;
use slab_runtime_core::backend::spawn_workers;
use slab_runtime_core::backend::{
    BroadcastSeq, ControlOpId, Input, Options, PeerControlBus, Typed, WorkerCommand,
};
use slab_runtime_macros::backend_handler;

// ── Worker ────────────────────────────────────────────────────────────────────

pub(crate) struct CandleWhisperWorker {
    engine: Option<CandleWhisperEngine>,
    peer_bus: PeerControlBus,
}

#[backend_handler(peer_bus = peer_bus)]
impl CandleWhisperWorker {
    pub(crate) fn new(engine: Option<CandleWhisperEngine>, peer_bus: PeerControlBus) -> Self {
        Self { engine, peer_bus }
    }

    #[on_event(LoadModel)]
    async fn on_load_model(
        &mut self,
        config: Input<CandleWhisperLoadConfig>,
        seq: BroadcastSeq,
    ) -> Result<(), CandleWhisperWorkerError> {
        self.handle_load_model(config.0, seq.0).await
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self, seq: BroadcastSeq) -> Result<(), CandleWhisperWorkerError> {
        self.handle_unload_model(seq.0).await
    }

    #[on_event(Inference)]
    async fn on_inference(
        &mut self,
        input: Payload,
        _options: Options<AudioTranscriptionOptions>,
    ) -> Result<Typed<AudioTranscriptionResponse>, CandleWhisperWorkerError> {
        self.handle_inference(input).await
    }

    // ── Runtime / peer control ────────────────────────────────────────────────

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(
        &mut self,
        config: Input<CandleWhisperLoadConfig>,
    ) -> Result<(), CandleWhisperWorkerError> {
        let config = config.0;
        let model_path = config.model_path;
        let tokenizer_path = config.tokenizer_path;
        if let Some(engine) = self.engine.as_ref()
            && !engine.is_model_loaded()
        {
            let engine = engine.clone();
            let result = tokio::task::block_in_place(|| {
                engine.load_model(&model_path, tokenizer_path.as_deref())
            });
            if let Err(e) = result {
                tracing::warn!(
                    model_path = %model_path.display(),
                    error = %e,
                    "candle.whisper worker: broadcast LoadModel failed"
                );
            }
        }
        Ok(())
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) -> Result<(), CandleWhisperWorkerError> {
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
    ) -> Result<(), CandleWhisperWorkerError> {
        tracing::debug!(op_id = op_id.0, "candle.whisper runtime control pre-cleanup");
        if let Some(e) = self.engine.as_ref() {
            e.unload();
        }
        Ok(())
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) -> Result<(), CandleWhisperWorkerError> {
        if let Some(e) = self.engine.as_ref() {
            e.unload();
        }
        Ok(())
    }

    // ── Handler helpers ───────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        config: CandleWhisperLoadConfig,
        seq_id: u64,
    ) -> Result<(), CandleWhisperWorkerError> {
        let model_payload = Payload::typed(config.clone());
        let engine = self.engine.get_or_insert_with(CandleWhisperEngine::new);
        let tokenizer_path = config.tokenizer_path;
        let model_path = config.model_path;
        let engine_clone = engine.clone();

        let result = tokio::task::block_in_place(move || {
            engine_clone.load_model(&model_path, tokenizer_path.as_deref())
        });

        match result {
            Ok(()) => {
                self.emit_peer_load_model_deployment_payload(seq_id, model_payload);
                Ok(())
            }
            Err(error) => Err(CandleWhisperWorkerError::load(error.to_string())),
        }
    }

    async fn handle_unload_model(&mut self, seq_id: u64) -> Result<(), CandleWhisperWorkerError> {
        match self.engine.as_ref() {
            Some(engine) => {
                engine.unload();
                self.emit_peer_unload_generation(seq_id);
                Ok(())
            }
            None => Err(CandleWhisperWorkerError::unload("model not loaded")),
        }
    }

    async fn handle_inference(
        &mut self,
        input: Payload,
    ) -> Result<Typed<AudioTranscriptionResponse>, CandleWhisperWorkerError> {
        let engine = match self.engine.as_ref() {
            Some(e) => e.clone(),
            None => {
                return Err(CandleWhisperWorkerError::inference(
                    "candle.whisper backend not ready: model not loaded. Call model.load first",
                ));
            }
        };

        let samples = match input.to_f32_arc() {
            Ok(s) => s,
            Err(e) => {
                return Err(CandleWhisperWorkerError::contract(format!(
                    "invalid input: expected f32 PCM audio samples, got: {e}"
                )));
            }
        };

        if samples.is_empty() {
            return Err(CandleWhisperWorkerError::contract(
                "invalid input: audio samples are empty",
            ));
        }

        let result = tokio::task::block_in_place(|| engine.inference(&samples));

        match result {
            Ok(text) => Ok(Typed(AudioTranscriptionResponse { text })),
            Err(error) => Err(CandleWhisperWorkerError::inference(format!(
                "candle.whisper inference failed: {error}"
            ))),
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Spawn `count` Candle Whisper backend workers.
pub fn spawn_backend(
    shared_ingress_rx: slab_runtime_core::backend::SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    count: usize,
) {
    spawn_workers(shared_ingress_rx, control_tx, count.max(1), |peer_bus| {
        CandleWhisperWorker::new(Some(CandleWhisperEngine::new()), peer_bus)
    });
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::super::contract::CandleWhisperLoadConfig;
    use super::CandleWhisperWorker;
    use slab_runtime_core::Payload;
    use slab_runtime_core::backend::{
        ControlOpId, DeploymentSnapshot, PeerControlBus, WorkerCommand,
    };
    use tokio::sync::broadcast;

    fn make_worker() -> CandleWhisperWorker {
        let (bc_tx, _bc_rx) = broadcast::channel::<WorkerCommand>(8);
        CandleWhisperWorker::new(None, PeerControlBus::new(bc_tx, 0))
    }

    #[test]
    fn deployment_snapshot_reads_typed_candle_whisper_model_config() {
        let snapshot = DeploymentSnapshot::with_model(
            5,
            Payload::typed(CandleWhisperLoadConfig {
                model_path: PathBuf::from("model.safetensors"),
                tokenizer_path: Some(PathBuf::from("tokenizer.json")),
            }),
        );

        let config = snapshot
            .typed_model_config::<CandleWhisperLoadConfig>()
            .expect("typed deployment snapshot should decode");

        assert_eq!(config.model_path, PathBuf::from("model.safetensors"));
        assert_eq!(config.tokenizer_path, Some(PathBuf::from("tokenizer.json")));
    }

    #[tokio::test]
    async fn global_unload_is_safe_without_engine() {
        let mut worker = make_worker();
        worker.apply_runtime_control(ControlOpId(1)).await.expect("control cleanup should succeed");
        // No panic – test passes.
    }
}
