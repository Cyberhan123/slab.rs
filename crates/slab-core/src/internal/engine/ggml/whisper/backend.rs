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
//! Uses a typed [`slab_types::GgmlWhisperLoadConfig`] payload inside `slab-core`,
//! with JSON deserialization kept as a compatibility fallback.

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::internal::engine::ggml::whisper::adapter::{
    GGMLWhisperEngine, WhisperDecodeConfig, WhisperVadConfig,
};
use crate::internal::scheduler::backend::protocol::{
    BackendReply, BackendRequest, DeploymentSnapshot, PeerWorkerCommand, RuntimeControlSignal,
    SyncMessage, WorkerCommand,
};
use crate::internal::scheduler::types::Payload;
use slab_core_macros::backend_handler;
use slab_types::{
    AudioTranscriptionOpOptions, GgmlWhisperLoadConfig, WhisperDecodeOptions, WhisperVadOptions,
};

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
pub(crate) struct WhisperWorker {
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

fn into_vad_config(vad: WhisperVadOptions) -> Result<Option<WhisperVadConfig>, String> {
    if !vad.enabled {
        return Ok(None);
    }

    let model_path = vad
        .model_path
        .ok_or_else(|| "invalid whisper inference options: vad.model_path is missing".to_owned())?;
    if model_path.as_os_str().is_empty() {
        return Err("invalid whisper inference options: vad.model_path is empty".into());
    }

    let params = vad.params.unwrap_or_default();
    if let Some(threshold) = params.threshold
        && !(0.0..=1.0).contains(&threshold)
    {
        return Err(
            "invalid whisper inference options: vad.threshold must be between 0.0 and 1.0".into()
        );
    }

    for (name, value) in [
        ("vad.min_speech_duration_ms", params.min_speech_duration_ms),
        ("vad.min_silence_duration_ms", params.min_silence_duration_ms),
        ("vad.speech_pad_ms", params.speech_pad_ms),
    ] {
        if value.is_some_and(|value| value < 0) {
            return Err(format!("invalid whisper inference options: {name} must be >= 0"));
        }
    }

    if let Some(max_speech_duration_s) = params.max_speech_duration_s
        && max_speech_duration_s <= 0.0
    {
        return Err(
            "invalid whisper inference options: vad.max_speech_duration_s must be > 0.0".into()
        );
    }

    if let Some(samples_overlap) = params.samples_overlap
        && samples_overlap < 0.0
    {
        return Err("invalid whisper inference options: vad.samples_overlap must be >= 0.0".into());
    }

    Ok(Some(WhisperVadConfig {
        model_path: model_path.to_string_lossy().into_owned(),
        threshold: params.threshold,
        min_speech_duration_ms: params.min_speech_duration_ms,
        min_silence_duration_ms: params.min_silence_duration_ms,
        max_speech_duration_s: params.max_speech_duration_s,
        speech_pad_ms: params.speech_pad_ms,
        samples_overlap: params.samples_overlap,
    }))
}

fn into_decode_config(decode: WhisperDecodeOptions) -> Result<Option<WhisperDecodeConfig>, String> {
    for (name, value) in [
        ("decode.offset_ms", decode.offset_ms),
        ("decode.duration_ms", decode.duration_ms),
        ("decode.max_len", decode.max_len),
        ("decode.max_tokens", decode.max_tokens),
    ] {
        if value.is_some_and(|value| value < 0) {
            return Err(format!("invalid whisper inference options: {name} must be >= 0"));
        }
    }

    if let Some(word_thold) = decode.word_thold
        && !(0.0..=1.0).contains(&word_thold)
    {
        return Err(
            "invalid whisper inference options: decode.word_thold must be between 0.0 and 1.0"
                .into(),
        );
    }

    for (name, value) in [
        ("decode.temperature", decode.temperature),
        ("decode.temperature_inc", decode.temperature_inc),
    ] {
        if value.is_some_and(|value| value < 0.0) {
            return Err(format!("invalid whisper inference options: {name} must be >= 0.0"));
        }
    }

    Ok(Some(WhisperDecodeConfig {
        offset_ms: decode.offset_ms,
        duration_ms: decode.duration_ms,
        no_context: decode.no_context,
        no_timestamps: decode.no_timestamps,
        token_timestamps: decode.token_timestamps,
        split_on_word: decode.split_on_word,
        suppress_nst: decode.suppress_nst,
        word_thold: decode.word_thold,
        max_len: decode.max_len,
        max_tokens: decode.max_tokens,
        temperature: decode.temperature,
        temperature_inc: decode.temperature_inc,
        entropy_thold: decode.entropy_thold,
        logprob_thold: decode.logprob_thold,
        no_speech_thold: decode.no_speech_thold,
        tdrz_enable: decode.tdrz_enable,
    }))
}

fn parse_inference_options(
    raw: &Payload,
) -> Result<(Option<WhisperVadConfig>, Option<WhisperDecodeConfig>), String> {
    let opts: AudioTranscriptionOpOptions =
        raw.to_typed().map_err(|e| format!("invalid whisper inference options: {e}"))?;

    let vad = match opts.vad {
        Some(vad) => into_vad_config(vad)?,
        None => None,
    };
    let decode = match opts.decode {
        Some(decode) => into_decode_config(decode)?,
        None => None,
    };

    Ok((vad, decode))
}

#[backend_handler]
impl WhisperWorker {
    pub(crate) fn new(
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
        let (vad, decode) = match parse_inference_options(&invocation.options) {
            Ok(options) => options,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e));
                return;
            }
        };
        self.handle_inference(input, vad, decode, reply_tx).await;
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

        let config: GgmlWhisperLoadConfig = match input.to_typed() {
            Ok(c) => c,
            Err(e) => {
                let _ =
                    reply_tx.send(BackendReply::Error(format!("invalid model.load config: {e}")));
                return;
            }
        };

        // Model loading is CPU/I-O bound; use block_in_place on this thread.
        let result = tokio::task::block_in_place(|| {
            use slab_whisper::WhisperContextParameters;
            let params = WhisperContextParameters::default();
            engine.new_context(&config.model_path, params)
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

    // ── inference ─────────────────────────────────────────────────────────────

    async fn handle_inference(
        &mut self,
        input: Payload,
        vad: Option<WhisperVadConfig>,
        decode: Option<WhisperDecodeConfig>,
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
        let vad_enabled = vad.is_some();
        let decode_configured = decode.is_some();
        let result = tokio::task::block_in_place(|| {
            tracing::debug!(
                sample_count = samples.len(),
                duration_sec = samples.len() as f64 / 16000.0,
                vad_enabled,
                decode_configured,
                "starting whisper inference"
            );
            engine.inference(&samples, vad.as_ref(), decode.as_ref())
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
        let config: GgmlWhisperLoadConfig = match snapshot.typed_model_config() {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(error = %error, "whisper worker: invalid model deployment snapshot");
                return;
            }
        };
        let model_path = config.model_path;
        if let Some(engine) = self.engine.as_mut()
            && !engine.is_model_loaded()
        {
            let result = tokio::task::block_in_place(|| {
                use slab_whisper::WhisperContextParameters;
                let params = WhisperContextParameters::default();
                engine.new_context(&model_path, params)
            });
            if let Err(e) = result {
                tracing::warn!(
                    model_path = %model_path.display(),
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
            Payload::typed(GgmlWhisperLoadConfig { model_path: PathBuf::from("model.bin") }),
        );

        let config = snapshot
            .typed_model_config::<GgmlWhisperLoadConfig>()
            .expect("typed deployment snapshot should decode");

        assert_eq!(config.model_path, PathBuf::from("model.bin"));
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
