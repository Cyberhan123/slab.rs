//! Backend worker for `ggml.whisper`.
//!
//! Defines [`WhisperWorker`] logic for runtime-managed worker loops.
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
//! Expects typed runtime-owned `GgmlWhisperLoadConfig` payloads.

use super::contract::{
    AudioTranscriptionOptions, AudioTranscriptionResponse, GgmlWhisperLoadConfig,
};
use super::error::GGMLWhisperWorkerError;
use super::engine::GGMLWhisperEngine;
use slab_runtime_core::Payload;
use slab_runtime_core::backend::{
    BroadcastSeq, ControlOpId, Input, Options, PeerControlBus, Typed,
};
use slab_runtime_macros::backend_handler;

// ── Worker ────────────────────────────────────────────────────────────────────

/// A single whisper backend worker.
///
/// Each worker **owns** its engine (library handle + model context).  There is
/// no shared mutable state between workers, so no `Mutex` is needed on the
/// context.  When `num_workers > 1` multiple workers are spawned; each worker
/// owns an independent engine forked from the same library handle and manages
/// its own model context independently.
///
/// Workers listen on both the shared `mpsc` ingress queue (competitive –
/// only one worker processes each request) and a `broadcast` channel
/// (fan-out – every worker receives management commands such as `Unload`).
pub struct WhisperWorker {
    /// - `None` → engine not initialized.
    /// - `Some(e)` where `e.ctx` is None → engine loaded, no model.
    /// - `Some(e)` where `e.ctx` is Some → engine + model loaded.
    engine: Option<GGMLWhisperEngine>,
    /// Peer synchronization emitter shared among workers.
    peer_bus: PeerControlBus,
    last_model_config: Option<Payload>,
}

#[backend_handler(peer_bus = peer_bus)]
impl WhisperWorker {
    pub fn new(engine: Option<GGMLWhisperEngine>, peer_bus: PeerControlBus) -> Self {
        Self { engine, peer_bus, last_model_config: None }
    }

    #[on_event(LoadModel)]
    async fn on_load_model(
        &mut self,
        params: Input<GgmlWhisperLoadConfig>,
        seq: BroadcastSeq,
    ) -> Result<(), GGMLWhisperWorkerError> {
        self.handle_load_model(params.0, seq.0).await
    }

    #[on_event(UnloadModel)]
    async fn on_unload_model(&mut self, seq: BroadcastSeq) -> Result<(), GGMLWhisperWorkerError> {
        self.handle_unload_model(seq.0).await
    }

    #[on_event(Inference)]
    async fn on_inference(
        &mut self,
        input: Payload,
        options: Options<AudioTranscriptionOptions>,
    ) -> Result<Typed<AudioTranscriptionResponse>, GGMLWhisperWorkerError> {
        self.handle_inference(input, options.0).await
    }

    // ── model.load ────────────────────────────────────────────────────────────

    async fn handle_load_model(
        &mut self,
        params: GgmlWhisperLoadConfig,
        seq_id: u64,
    ) -> Result<(), GGMLWhisperWorkerError> {
        let engine = match self.engine.as_mut() {
            Some(e) => e,
            None => {
                return Err(GGMLWhisperWorkerError::load("engine not initialized"));
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
            Err(error) => Err(GGMLWhisperWorkerError::load(error.to_string())),
        }
    }

    // ── model.unload ──────────────────────────────────────────────────────────

    async fn handle_unload_model(
        &mut self,
        seq_id: u64,
    ) -> Result<(), GGMLWhisperWorkerError> {
        match self.engine.as_mut() {
            Some(e) => {
                e.unload();
                self.last_model_config = None;
                // Broadcast so every peer worker also drops its context.
                self.emit_peer_unload_generation(seq_id);
                Ok(())
            }
            None => Err(GGMLWhisperWorkerError::unload("engine not initialized")),
        }
    }

    // ── inference ─────────────────────────────────────────────────────────────

    async fn handle_inference(
        &mut self,
        input: Payload,
        params: AudioTranscriptionOptions,
    ) -> Result<Typed<AudioTranscriptionResponse>, GGMLWhisperWorkerError> {
        let engine = match self.engine.as_ref() {
            Some(e) => e,
            None => {
                return Err(GGMLWhisperWorkerError::inference(
                    "whisper backend not ready: model not loaded. Call model.load first",
                ));
            }
        };

        let samples = match input.to_f32_arc() {
            Ok(b) => b,
            Err(e) => {
                return Err(GGMLWhisperWorkerError::contract(format!(
                    "invalid input for whisper inference: expected f32 PCM audio samples, got: {e}"
                )));
            }
        };

        if samples.is_empty() {
            return Err(GGMLWhisperWorkerError::contract(
                "invalid input for whisper inference: audio samples are empty",
            ));
        }

        // Whisper inference is CPU/GPU-bound; use block_in_place so the engine
        // context stays on this thread without needing an additional spawn_blocking.
        let vad_enabled = params.vad.as_ref().is_some_and(|vad| vad.enabled);
        let decode_configured = params.decode.as_ref().is_some_and(|decode| {
            decode.offset_ms.is_some()
                || decode.duration_ms.is_some()
                || decode.no_context.is_some()
                || decode.no_timestamps.is_some()
                || decode.token_timestamps.is_some()
                || decode.split_on_word.is_some()
                || decode.suppress_nst.is_some()
                || decode.word_thold.is_some()
                || decode.max_len.is_some()
                || decode.max_tokens.is_some()
                || decode.temperature.is_some()
                || decode.temperature_inc.is_some()
                || decode.entropy_thold.is_some()
                || decode.logprob_thold.is_some()
                || decode.no_speech_thold.is_some()
                || decode.tdrz_enable.is_some()
        }) || params.language.is_some()
            || params.prompt.is_some();
        let result = tokio::task::block_in_place(|| {
            tracing::debug!(
                sample_count = samples.len(),
                duration_sec = samples.len() as f64 / 16000.0,
                vad_enabled,
                decode_configured,
                "starting whisper inference"
            );
            engine.inference_with_options(&samples, &params)
        });

        match result {
            Err(e) => {
                tracing::error!(error = %e, "whisper inference failed");
                Err(GGMLWhisperWorkerError::inference(format!(
                    "whisper inference failed: {e}"
                )))
            }
            Ok(entries) => {
                tracing::debug!(segment_count = entries.len(), "whisper inference succeeded");
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
        params: Input<GgmlWhisperLoadConfig>,
    ) -> Result<(), GGMLWhisperWorkerError> {
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
                    "whisper worker: broadcast LoadModel failed"
                );
            }
        }
        self.last_model_config = Some(Payload::typed(params));
        Ok(())
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) -> Result<(), GGMLWhisperWorkerError> {
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
    ) -> Result<(), GGMLWhisperWorkerError> {
        tracing::debug!(op_id = op_id.0, "whisper runtime control pre-cleanup");
        if let Some(engine) = self.engine.as_mut() {
            engine.unload();
        }
        self.last_model_config = None;
        Ok(())
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) -> Result<(), GGMLWhisperWorkerError> {
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
    fn deployment_snapshot_reads_typed_whisper_model_config() {
        let snapshot = DeploymentSnapshot::with_model(
            7,
            Payload::typed(GgmlWhisperLoadConfig {
                model_path: PathBuf::from("model.bin"),
                flash_attn: Some(true),
            }),
        );

        let config = snapshot
            .typed_model_config::<GgmlWhisperLoadConfig>()
            .expect("typed deployment snapshot should decode");

        assert_eq!(config.model_path, PathBuf::from("model.bin"));
        assert_eq!(config.flash_attn, Some(true));
    }
}
