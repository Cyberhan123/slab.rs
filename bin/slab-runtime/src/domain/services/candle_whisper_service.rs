use slab_runtime_core::{CoreError, Payload};
use slab_types::{CandleWhisperLoadConfig, Capability, ModelFamily};

use slab_proto::convert::dto;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    audio_decode_stage, decode_utf8_payload, model_spec, required_path,
    whisper_transcription_from_raw,
};

#[derive(Clone, Debug)]
pub(crate) struct CandleWhisperService {
    runtime: DriverRuntime,
}

impl CandleWhisperService {
    pub(crate) fn new(
        execution: ExecutionHub,
        request: dto::CandleWhisperLoadRequest,
    ) -> Result<Self, CoreError> {
        let model_path = required_path("candle_whisper.model_path", request.model_path)?;
        let load_payload = Payload::typed(CandleWhisperLoadConfig {
            model_path: model_path.clone(),
            tokenizer_path: request.tokenizer_path,
        });

        Ok(Self {
            runtime: DriverRuntime::new(
                execution,
                model_spec(ModelFamily::Whisper, Capability::AudioTranscription, model_path),
                "candle.whisper",
                load_payload,
            ),
        })
    }

    pub(crate) async fn load(&self) -> Result<(), CoreError> {
        self.runtime.load().await
    }

    pub(crate) async fn unload(&self) -> Result<(), CoreError> {
        self.runtime.unload().await
    }

    pub(crate) async fn transcribe(
        &self,
        request: dto::CandleWhisperTranscribeRequest,
    ) -> Result<dto::CandleWhisperTranscribeResponse, CoreError> {
        let audio_path = required_path("candle_whisper.path", request.path)?;
        let payload = self
            .runtime
            .submit(
                Capability::AudioTranscription,
                false,
                Payload::None,
                vec![audio_decode_stage(audio_path)],
                Payload::None,
            )
            .await?
            .result()
            .await?;
        let raw = decode_utf8_payload(payload, "candle_whisper")?;

        Ok(dto::CandleWhisperTranscribeResponse {
            transcription: whisper_transcription_from_raw(raw, None),
        })
    }
}
