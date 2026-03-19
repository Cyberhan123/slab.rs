//! Backend worker adapter for `ggml.whisper`.
//!
//! Defines [`WhisperWorker`] logic for runtime-managed worker loops.
//!
//! # Supported ops
//!
//! | Op string          | Event variant    | Description                                        |
//! |--------------------|------------------|----------------------------------------------------|
//! | `"lib.load"`       | `LoadLibrary`    | Load (skip if already loaded) the whisper dylib.   |
//! | `"lib.reload"`     | `ReloadLibrary`  | Replace the library, discarding current model.     |
//! | `"model.load"`     | `LoadModel`      | Load a model from the pre-loaded library.          |
//! | `"model.unload"`   | `UnloadModel`    | Drop the model handle; call model.load to restore. |
//! | `"inference"`      | `Inference`      | Transcribe audio; input is packed `f32` PCM.       |
//!
//! ### `lib.load` / `lib.reload` input JSON
//! ```json
//! { "lib_path": "/path/to/libwhisper.so" }
//! ```
//!
//! ### `model.load` input JSON
//! ```json
//! { "model_path": "/path/to/model.bin" }
//! ```

use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::broadcast;

use crate::engine::ggml::config::{LibLoadConfig, ModelLoadConfig};
use crate::engine::ggml::whisper::adapter::{
    GGMLWhisperEngine, WhisperDecodeConfig, WhisperVadConfig,
};
use crate::scheduler::backend::backend_handler;
use crate::scheduler::backend::protocol::{
    BackendReply, BackendRequest, DeploymentSnapshot, PeerWorkerCommand, RuntimeControlSignal,
    SyncMessage, WorkerCommand,
};
use crate::scheduler::types::Payload;

// ── Worker ────────────────────────────────────────────────────────────────────

/// A single whisper backend worker.
///
/// Each worker **owns** its engine (library handle + model context).  There is
/// no shared mutable state between workers, so no `Mutex` is needed on the
/// context.  When `num_workers > 1` multiple workers are spawned; each worker
/// owns an independent engine forked from the same library handle and manages
/// its own model context independently.
///
/// Workers listen on both the shared `mpsc` ingress queue (competitive 鈥?/// only one worker processes each request) and a `broadcast` channel
/// (fan-out 鈥?every worker receives management commands such as `Unload`).
pub(crate) struct WhisperWorker {
    /// - `None` 鈫?library not loaded.
    /// - `Some(e)` where `e.ctx` is None 鈫?lib loaded, no model.
    /// - `Some(e)` where `e.ctx` is Some 鈫?lib + model loaded.
    engine: Option<GGMLWhisperEngine>,
    /// Broadcast sender shared among all workers so that any worker can
    /// propagate state-change commands (e.g. `Unload`) to all peers.
    bc_tx: broadcast::Sender<WorkerCommand>,
    /// Stable index used to populate `sender_id` when broadcasting.
    worker_id: usize,
    last_lib_config: Option<Payload>,
    last_model_config: Option<Payload>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct WhisperInferenceOptions {
    vad: Option<WhisperVadInferenceOptions>,
    decode: Option<WhisperDecodeInferenceOptions>,
}

#[derive(Debug, Clone, Deserialize)]
struct WhisperVadInferenceOptions {
    model_path: String,
    threshold: Option<f32>,
    min_speech_duration_ms: Option<i32>,
    min_silence_duration_ms: Option<i32>,
    max_speech_duration_s: Option<f32>,
    speech_pad_ms: Option<i32>,
    samples_overlap: Option<f32>,
}

impl WhisperVadInferenceOptions {
    fn into_engine_config(self) -> WhisperVadConfig {
        WhisperVadConfig {
            model_path: self.model_path,
            threshold: self.threshold,
            min_speech_duration_ms: self.min_speech_duration_ms,
            min_silence_duration_ms: self.min_silence_duration_ms,
            max_speech_duration_s: self.max_speech_duration_s,
            speech_pad_ms: self.speech_pad_ms,
            samples_overlap: self.samples_overlap,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct WhisperDecodeInferenceOptions {
    offset_ms: Option<i32>,
    duration_ms: Option<i32>,
    no_context: Option<bool>,
    no_timestamps: Option<bool>,
    token_timestamps: Option<bool>,
    split_on_word: Option<bool>,
    suppress_nst: Option<bool>,
    word_thold: Option<f32>,
    max_len: Option<i32>,
    max_tokens: Option<i32>,
    temperature: Option<f32>,
    temperature_inc: Option<f32>,
    entropy_thold: Option<f32>,
    logprob_thold: Option<f32>,
    no_speech_thold: Option<f32>,
    tdrz_enable: Option<bool>,
}

impl WhisperDecodeInferenceOptions {
    fn into_engine_config(self) -> WhisperDecodeConfig {
        WhisperDecodeConfig {
            offset_ms: self.offset_ms,
            duration_ms: self.duration_ms,
            no_context: self.no_context,
            no_timestamps: self.no_timestamps,
            token_timestamps: self.token_timestamps,
            split_on_word: self.split_on_word,
            suppress_nst: self.suppress_nst,
            word_thold: self.word_thold,
            max_len: self.max_len,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            temperature_inc: self.temperature_inc,
            entropy_thold: self.entropy_thold,
            logprob_thold: self.logprob_thold,
            no_speech_thold: self.no_speech_thold,
            tdrz_enable: self.tdrz_enable,
        }
    }
}

fn parse_inference_options(raw: &Payload) -> Result<WhisperInferenceOptions, String> {
    let value = raw.to_serde_value();
    if value.is_null() {
        return Ok(WhisperInferenceOptions::default());
    }
    let opts: WhisperInferenceOptions = serde_json::from_value(value)
        .map_err(|e| format!("invalid whisper inference options: {e}"))?;

    if let Some(vad) = opts.vad.as_ref() {
        if vad.model_path.trim().is_empty() {
            return Err("invalid whisper inference options: vad.model_path is empty".into());
        }
    }

    if let Some(decode) = opts.decode.as_ref() {
        for (name, value) in [
            ("decode.offset_ms", decode.offset_ms),
            ("decode.duration_ms", decode.duration_ms),
            ("decode.max_len", decode.max_len),
            ("decode.max_tokens", decode.max_tokens),
        ] {
            if value.is_some_and(|v| v < 0) {
                return Err(format!(
                    "invalid whisper inference options: {name} must be >= 0"
                ));
            }
        }

        if let Some(word_thold) = decode.word_thold {
            if !(0.0..=1.0).contains(&word_thold) {
                return Err(
                    "invalid whisper inference options: decode.word_thold must be between 0.0 and 1.0"
                        .into(),
                );
            }
        }

        for (name, value) in [
            ("decode.temperature", decode.temperature),
            ("decode.temperature_inc", decode.temperature_inc),
        ] {
            if value.is_some_and(|v| v < 0.0) {
                return Err(format!(
                    "invalid whisper inference options: {name} must be >= 0.0"
                ));
            }
        }
    }

    Ok(opts)
}

#[backend_handler]
impl WhisperWorker {
    pub(crate) fn new(
        engine: Option<GGMLWhisperEngine>,
        bc_tx: broadcast::Sender<WorkerCommand>,
        worker_id: usize,
    ) -> Self {
        Self {
            engine,
            bc_tx,
            worker_id,
            last_lib_config: None,
            last_model_config: None,
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

    #[on_event(Inference)]
    async fn on_inference(&mut self, req: BackendRequest) {
        let invocation = match req.invocation() {
            Ok(invocation) => invocation,
            Err(error) => {
                let _ = req.reply_tx.send(BackendReply::Error(error));
                return;
            }
        };
        let BackendRequest {
            input,
            reply_tx,
            ..
        } = req;
        let options = match parse_inference_options(&invocation.options) {
            Ok(options) => options,
            Err(e) => {
                let _ = reply_tx.send(BackendReply::Error(e));
                return;
            }
        };
        self.handle_inference(
            input,
            options.vad.map(|v| v.into_engine_config()),
            options.decode.map(|d| d.into_engine_config()),
            reply_tx,
        )
        .await;
    }

    // ── lib.load ──────────────────────────────────────────────────────────────

    async fn handle_load_library(
        &mut self,
        input: Payload,
        reply_tx: tokio::sync::oneshot::Sender<BackendReply>,
        seq_id: u64,
    ) {
        let deployment = DeploymentSnapshot::with_library(seq_id, input.clone());
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

        match GGMLWhisperEngine::from_path(&config.lib_path) {
            Ok(engine) => {
                self.engine = Some(engine);
                self.last_lib_config = Some(input);
                self.last_model_config = None;
                // Broadcast so peer workers also load the same library.
                let _ = self
                    .bc_tx
                    .send(WorkerCommand::Peer(PeerWorkerCommand::LoadLibrary {
                        sync: SyncMessage::Deployment(deployment),
                        sender_id: self.worker_id,
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
        let deployment = DeploymentSnapshot::with_library(seq_id, input.clone());
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

        match GGMLWhisperEngine::from_path(&config.lib_path) {
            Ok(engine) => {
                self.engine = Some(engine);
                self.last_lib_config = Some(input);
                self.last_model_config = None;
                // Broadcast so peer workers drop their old engine and reload too.
                let _ = self
                    .bc_tx
                    .send(WorkerCommand::Peer(PeerWorkerCommand::ReloadLibrary {
                        sync: SyncMessage::Deployment(deployment),
                        sender_id: self.worker_id,
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
            use slab_whisper::WhisperContextParameters;
            let params = WhisperContextParameters::default();
            engine.new_context(&config.model_path, params)
        });

        match result {
            Ok(()) => {
                self.last_model_config = Some(input.clone());
                let deployment = if let Some(library) = self.last_lib_config.clone() {
                    DeploymentSnapshot::with_library_and_model(seq_id, library, input)
                } else {
                    DeploymentSnapshot::with_model(seq_id, input)
                };
                // Broadcast so peer workers also load the same model.
                let _ = self
                    .bc_tx
                    .send(WorkerCommand::Peer(PeerWorkerCommand::LoadModel {
                        sync: SyncMessage::Deployment(deployment),
                        sender_id: self.worker_id,
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
                self.last_model_config = None;
                // Broadcast so every peer worker also drops its context.
                // Ignore errors: no receivers simply means no other workers.
                let _ = self
                    .bc_tx
                    .send(WorkerCommand::Peer(PeerWorkerCommand::Unload {
                        sync: SyncMessage::Generation { generation: seq_id },
                        sender_id: self.worker_id,
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
                    "whisper backend not ready: library or model not loaded. Call lib.load and model.load first".into(),
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
                let _ = reply_tx.send(BackendReply::Error(format!(
                    "whisper inference failed: {e}"
                )));
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
                let _ = reply_tx.send(BackendReply::Value(Payload::Bytes(Arc::from(
                    out.as_bytes(),
                ))));
            }
        }
    }

    #[on_peer_control(LoadLibrary)]
    async fn on_peer_load_library(&mut self, cmd: PeerWorkerCommand) {
        let Some(snapshot) = cmd.deployment() else {
            return;
        };
        let config: LibLoadConfig = match snapshot.library_config() {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(error = %error, "whisper worker: invalid library deployment snapshot");
                return;
            }
        };
        let lib_path = config.lib_path;
        if self.engine.is_none() {
            if let Ok(engine) = GGMLWhisperEngine::from_path(&lib_path) {
                self.engine = Some(engine);
            }
        }
        self.last_lib_config = snapshot.library.clone();
    }

    #[on_peer_control(ReloadLibrary)]
    async fn on_peer_reload_library(&mut self, cmd: PeerWorkerCommand) {
        let Some(snapshot) = cmd.deployment() else {
            return;
        };
        let config: LibLoadConfig = match snapshot.library_config() {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(error = %error, "whisper worker: invalid library deployment snapshot");
                return;
            }
        };
        let lib_path = config.lib_path;
        self.engine = None;
        if let Ok(engine) = GGMLWhisperEngine::from_path(&lib_path) {
            self.engine = Some(engine);
        }
        self.last_lib_config = snapshot.library.clone();
        self.last_model_config = None;
    }

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(&mut self, cmd: PeerWorkerCommand) {
        let Some(snapshot) = cmd.deployment() else {
            return;
        };
        if self.engine.is_none() {
            if let Some(lib_payload) = snapshot.library.as_ref() {
                if let Ok(config) = lib_payload.to_json::<LibLoadConfig>() {
                    if let Ok(engine) = GGMLWhisperEngine::from_path(&config.lib_path) {
                        self.engine = Some(engine);
                    }
                }
            }
        }
        let config: ModelLoadConfig = match snapshot.model_config() {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(error = %error, "whisper worker: invalid model deployment snapshot");
                return;
            }
        };
        let model_path = config.model_path;
        if let Some(engine) = self.engine.as_mut() {
            if !engine.is_model_loaded() {
                let result = tokio::task::block_in_place(|| {
                    use slab_whisper::WhisperContextParameters;
                    let params = WhisperContextParameters::default();
                    engine.new_context(&model_path, params)
                });
                if let Err(e) = result {
                    tracing::warn!(
                        model_path,
                        error = %e,
                        "whisper worker: broadcast LoadModel failed"
                    );
                }
            }
        }
        if snapshot.library.is_some() {
            self.last_lib_config = snapshot.library.clone();
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
                self.last_lib_config = None;
                self.last_model_config = None;
            }
            RuntimeControlSignal::GlobalLoad { op_id, payload } => {
                let _ = payload;
                tracing::debug!(op_id, "whisper runtime global load pre-cleanup");
                if let Some(engine) = self.engine.as_mut() {
                    engine.unload();
                }
                self.last_lib_config = None;
                self.last_model_config = None;
            }
        }
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) {
        if let Some(e) = self.engine.as_mut() {
            e.unload();
        }
        self.last_lib_config = None;
        self.last_model_config = None;
    }
}
