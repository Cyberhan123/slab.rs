//! Backend worker adapter for `ggml.whisper`.
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
//! Prefers typed [`slab_whisper::ContextParams`] payloads inside `slab-core`,
//! with legacy `GgmlWhisperLoadConfig` JSON/typed deserialization kept as a compatibility fallback.

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::domain::services::codec::build_ggml_whisper_full_params_from_legacy;
use crate::infra::backends::ggml::whisper::adapter::GGMLWhisperEngine;
use slab_runtime_core::Payload;
use slab_runtime_core::backend::{
    BackendReply, BackendRequest, DeploymentSnapshot, PeerWorkerCommand, RuntimeControlSignal,
    SyncMessage, WorkerCommand,
};
use slab_runtime_macros::backend_handler;
use slab_types::{AudioTranscriptionOpOptions, GgmlWhisperLoadConfig};
use slab_whisper::{ContextParams as WhisperContextParams, FullParams as WhisperFullParams};

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
    /// Broadcast sender shared among all workers so that any worker can
    /// propagate state-change commands (e.g. `Unload`) to all peers.
    bc_tx: broadcast::Sender<WorkerCommand>,
    /// Stable index used to populate `sender_id` when broadcasting.
    worker_id: usize,
    last_model_config: Option<Payload>,
}

fn parse_context_payload(raw: &Payload) -> Result<WhisperContextParams, String> {
    if let Ok(params) = raw.to_typed::<WhisperContextParams>() {
        return Ok(params);
    }

    let legacy: GgmlWhisperLoadConfig =
        raw.to_typed().map_err(|e| format!("invalid model.load config: {e}"))?;
    Ok(WhisperContextParams { model_path: Some(legacy.model_path), ..Default::default() })
}

fn parse_inference_options(raw: &Payload) -> Result<WhisperFullParams, String> {
    if let Ok(params) = raw.to_typed::<WhisperFullParams>() {
        return Ok(params);
    }

    let opts: AudioTranscriptionOpOptions =
        raw.to_typed().map_err(|e| format!("invalid whisper inference options: {e}"))?;
    build_ggml_whisper_full_params_from_legacy(opts.language, opts.prompt, opts.vad, opts.decode)
        .map_err(|error| error.to_string())
}

#[backend_handler]
impl WhisperWorker {
    pub fn new(
        engine: Option<GGMLWhisperEngine>,
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

    #[on_event(Inference)]
    async fn on_inference(&mut self, req: BackendRequest) {
        let invocation = match req.invocation() {
            Ok(invocation) => invocation,
            Err(error) => {
                let _ = req.reply_tx.send(BackendReply::Error(error));
                return;
            }
        };
        let BackendRequest { input, reply_tx, .. } = req;
        let params = match parse_inference_options(&invocation.options) {
            Ok(options) => options,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e));
                return;
            }
        };
        self.handle_inference(input, params, reply_tx).await;
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

        let params = match parse_context_payload(&input) {
            Ok(c) => c,
            Err(e) => {
                let _ =
                    reply_tx.send(BackendReply::Error(format!("invalid model.load config: {e}")));
                return;
            }
        };

        // Model loading is CPU/I-O bound; use block_in_place on this thread.
        let result = tokio::task::block_in_place(|| engine.new_context(params.clone()));

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

    // ── inference ─────────────────────────────────────────────────────────────

    async fn handle_inference(
        &mut self,
        input: Payload,
        params: WhisperFullParams,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
    ) {
        let engine = match self.engine.as_ref() {
            Some(e) => e,
            None => {
                let _ = reply_tx.send(BackendReply::Error(
                    "whisper backend not ready: model not loaded. Call model.load first".into(),
                ));
                return;
            }
        };

        let samples = match input.to_f32_arc() {
            Ok(b) => b,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "invalid input for whisper inference: expected f32 PCM audio samples, got: {e}"
                )));
                return;
            }
        };

        if samples.is_empty() {
            let _ = reply_tx.send(BackendReply::Error(
                "invalid input for whisper inference: audio samples are empty".into(),
            ));
            return;
        }

        // Whisper inference is CPU/GPU-bound; use block_in_place so the engine
        // context stays on this thread without needing an additional spawn_blocking.
        let vad_enabled = params.vad.unwrap_or(false);
        let decode_configured = params.offset_ms.is_some()
            || params.duration_ms.is_some()
            || params.no_context.is_some()
            || params.no_timestamps.is_some()
            || params.token_timestamps.is_some()
            || params.split_on_word.is_some()
            || params.suppress_nst.is_some()
            || params.thold_pt.is_some()
            || params.max_len.is_some()
            || params.max_tokens.is_some()
            || params.temperature.is_some()
            || params.temperature_inc.is_some()
            || params.entropy_thold.is_some()
            || params.logprob_thold.is_some()
            || params.no_speech_thold.is_some()
            || params.tdrz_enable.is_some()
            || params.language.is_some()
            || params.initial_prompt.is_some();
        let result = tokio::task::block_in_place(|| {
            tracing::debug!(
                sample_count = samples.len(),
                duration_sec = samples.len() as f64 / 16000.0,
                vad_enabled,
                decode_configured,
                "starting whisper inference"
            );
            engine.inference(&samples, &params)
        });

        match result {
            Err(e) => {
                tracing::error!(error = %e, "whisper inference failed");
                let _ =
                    reply_tx.send(BackendReply::Error(format!("whisper inference failed: {e}")));
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
                let _ =
                    reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(out.as_bytes()))));
            }
        }
    }

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(&mut self, cmd: PeerWorkerCommand) {
        let Some(snapshot) = cmd.deployment() else {
            return;
        };
        let Some(model_payload) = snapshot.model.as_ref() else {
            tracing::warn!("whisper worker: deployment snapshot missing model payload");
            return;
        };
        let params = match parse_context_payload(model_payload) {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(error = %error, "whisper worker: invalid model deployment snapshot");
                return;
            }
        };
        let model_path =
            params.model_path.clone().map(|path| path.display().to_string()).unwrap_or_default();
        if let Some(engine) = self.engine.as_mut()
            && !engine.is_model_loaded()
        {
            let result = tokio::task::block_in_place(|| engine.new_context(params.clone()));
            if let Err(e) = result {
                tracing::warn!(
                    model_path = %model_path,
                    error = %e,
                    "whisper worker: broadcast LoadModel failed"
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
                tracing::debug!(op_id, "whisper runtime global unload");
                if let Some(engine) = self.engine.as_mut() {
                    engine.unload();
                }
                self.last_model_config = None;
            }
            RuntimeControlSignal::GlobalLoad { op_id, payload } => {
                let _ = payload;
                tracing::debug!(op_id, "whisper runtime global load pre-cleanup");
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
    fn deployment_snapshot_reads_typed_whisper_model_config() {
        let snapshot = DeploymentSnapshot::with_model(
            7,
            Payload::typed(WhisperContextParams {
                model_path: Some(PathBuf::from("model.bin")),
                ..Default::default()
            }),
        );

        let config = snapshot
            .typed_model_config::<WhisperContextParams>()
            .expect("typed deployment snapshot should decode");

        assert_eq!(config.model_path, Some(PathBuf::from("model.bin")));
    }

    #[test]
    fn deployment_snapshot_typed_model_config_falls_back_to_json() {
        let snapshot = DeploymentSnapshot::with_model(
            8,
            Payload::json(serde_json::json!({ "model_path": "legacy-model.bin" })),
        );

        let config = snapshot
            .typed_model_config::<GgmlWhisperLoadConfig>()
            .expect("json deployment snapshot should still decode through typed helper");

        assert_eq!(config.model_path, PathBuf::from("legacy-model.bin"));
    }
}
