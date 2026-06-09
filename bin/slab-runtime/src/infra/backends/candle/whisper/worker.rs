//! Backend worker for `candle.whisper`.

use slab_candle::CandleRuntimeEngine;
use slab_candle::whisper::{
    CandleWhisperEngine, CandleWhisperLoadConfig as SlabCandleWhisperLoadConfig,
    TranscriptionRequest, WhisperTask, WhisperWeightSource,
};
use slab_runtime_core::Payload;
use slab_runtime_core::backend::spawn_workers;
use slab_runtime_core::backend::{
    BroadcastSeq, ControlOpId, Input, Options, PeerControlBus, Typed, WorkerCommand,
};
use slab_runtime_macros::backend_handler;
use tokio::sync::broadcast;

use super::error::CandleWhisperWorkerError;
use crate::domain::models::{
    AudioTranscriptionOptions, AudioTranscriptionResponse, CandleWhisperLoadConfig,
};

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
        options: Options<AudioTranscriptionOptions>,
    ) -> Result<Typed<AudioTranscriptionResponse>, CandleWhisperWorkerError> {
        self.handle_inference(input, options.0).await
    }

    #[on_peer_control(LoadModel)]
    async fn on_peer_load_model(
        &mut self,
        config: Input<CandleWhisperLoadConfig>,
    ) -> Result<(), CandleWhisperWorkerError> {
        let load_config = map_load_config(config.0);
        if let Some(engine) = self.engine.as_mut()
            && !engine.is_model_loaded()
        {
            let model_path = load_config.model_path.clone();
            let result = tokio::task::block_in_place(|| engine.load_model(load_config));
            if let Err(error) = result {
                tracing::warn!(
                    model_path = %model_path.display(),
                    error = %error,
                    "candle.whisper worker: broadcast LoadModel failed"
                );
            }
        }
        Ok(())
    }

    #[on_peer_control(Unload)]
    async fn on_peer_unload(&mut self) -> Result<(), CandleWhisperWorkerError> {
        if let Some(engine) = self.engine.as_mut() {
            engine.unload_model();
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
        if let Some(engine) = self.engine.as_mut() {
            engine.unload_model();
        }
        Ok(())
    }

    #[on_control_lagged]
    async fn on_control_lagged(&mut self) -> Result<(), CandleWhisperWorkerError> {
        if let Some(engine) = self.engine.as_mut() {
            engine.unload_model();
        }
        Ok(())
    }

    async fn handle_load_model(
        &mut self,
        config: CandleWhisperLoadConfig,
        seq_id: u64,
    ) -> Result<(), CandleWhisperWorkerError> {
        let model_payload = Payload::typed(config.clone());
        let load_config = map_load_config(config);
        let engine = self.engine.get_or_insert_with(CandleWhisperEngine::new);

        let result = tokio::task::block_in_place(|| engine.load_model(load_config));

        match result {
            Ok(()) => {
                self.emit_peer_load_model_deployment_payload(seq_id, model_payload);
                Ok(())
            }
            Err(error) => Err(CandleWhisperWorkerError::load(error.to_string())),
        }
    }

    async fn handle_unload_model(&mut self, seq_id: u64) -> Result<(), CandleWhisperWorkerError> {
        match self.engine.as_mut() {
            Some(engine) => {
                engine.unload_model();
                self.emit_peer_unload_generation(seq_id);
                Ok(())
            }
            None => Err(CandleWhisperWorkerError::unload("model not loaded")),
        }
    }

    async fn handle_inference(
        &mut self,
        input: Payload,
        options: AudioTranscriptionOptions,
    ) -> Result<Typed<AudioTranscriptionResponse>, CandleWhisperWorkerError> {
        let engine = self.engine.as_mut().ok_or_else(|| {
            CandleWhisperWorkerError::inference(
                "candle.whisper backend not ready: model not loaded. Call model.load first",
            )
        })?;

        let samples = match input.to_f32_arc() {
            Ok(samples) => samples,
            Err(error) => {
                return Err(CandleWhisperWorkerError::contract(format!(
                    "invalid input: expected f32 PCM audio samples, got: {error}"
                )));
            }
        };

        if samples.is_empty() {
            return Err(CandleWhisperWorkerError::contract(
                "invalid input: audio samples are empty",
            ));
        }

        let request = map_transcription_request(samples.as_ref(), options);
        let result = tokio::task::block_in_place(|| engine.infer(request));

        match result {
            Ok(response) => Ok(Typed(AudioTranscriptionResponse { text: response.text })),
            Err(error) => Err(CandleWhisperWorkerError::inference(format!(
                "candle.whisper inference failed: {error}"
            ))),
        }
    }
}

pub fn spawn_backend(
    shared_ingress_rx: slab_runtime_core::backend::SharedIngressRx,
    control_tx: broadcast::Sender<WorkerCommand>,
    count: usize,
) {
    spawn_workers(shared_ingress_rx, control_tx, count.max(1), |peer_bus| {
        CandleWhisperWorker::new(Some(CandleWhisperEngine::new()), peer_bus)
    });
}

fn map_load_config(config: CandleWhisperLoadConfig) -> SlabCandleWhisperLoadConfig {
    SlabCandleWhisperLoadConfig {
        model_path: config.model_path,
        tokenizer_path: config.tokenizer_path,
        device: config.device,
        config_path: None,
        mel_filters_path: None,
        weight_source: WhisperWeightSource::Safetensors,
    }
}

fn map_transcription_request(
    samples: &[f32],
    options: AudioTranscriptionOptions,
) -> TranscriptionRequest {
    let decode = options.decode;
    let timestamps = decode.as_ref().and_then(|value| value.token_timestamps).unwrap_or(false)
        && !decode.as_ref().and_then(|value| value.no_timestamps).unwrap_or(false);

    TranscriptionRequest {
        samples: samples.to_vec(),
        language: options.language,
        detect_language: options.detect_language.unwrap_or(false),
        task: WhisperTask::Transcribe,
        timestamps,
        prompt: options.prompt,
        max_tokens: decode
            .as_ref()
            .and_then(|value| value.max_tokens)
            .and_then(|value| usize::try_from(value).ok()),
        temperature: decode.as_ref().and_then(|value| value.temperature).map(f64::from),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::CandleWhisperWorker;
    use crate::domain::models::CandleWhisperLoadConfig;
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
                device: None,
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
    }
}
