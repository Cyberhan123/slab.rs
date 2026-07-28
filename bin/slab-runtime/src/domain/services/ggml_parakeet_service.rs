use slab_runtime_core::backend::RequestRoute;

use crate::application::dtos as dto;
use crate::domain::models::{
    AudioTranscriptionDecodeOptions, AudioTranscriptionOptions, AudioTranscriptionResponse,
    GgmlParakeetLoadConfig,
};
use crate::domain::runtime::CoreError;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{audio_decode_stage, required_path, whisper_transcription_from_raw};

#[derive(Clone, Debug)]
pub(crate) struct GgmlParakeetService {
    runtime: DriverRuntime,
}

impl GgmlParakeetService {
    pub(crate) fn new(
        execution: ExecutionHub,
        request: dto::GgmlParakeetLoadRequest,
    ) -> Result<Self, CoreError> {
        let model_path = required_path("ggml_parakeet.model_path", request.model_path)?;
        let load_payload = GgmlParakeetLoadConfig { model_path };

        Ok(Self {
            runtime: DriverRuntime::new_typed(
                execution,
                "ggml.parakeet",
                "ggml.parakeet",
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
        request: dto::GgmlParakeetTranscribeRequest,
    ) -> Result<dto::GgmlParakeetTranscribeResponse, CoreError> {
        let audio_path = required_path("ggml_parakeet.path", request.path.clone())?;
        let response: AudioTranscriptionResponse = self
            .runtime
            .invoke_preprocessed_typed(
                RequestRoute::Inference,
                vec![audio_decode_stage(audio_path)],
                build_transcription_options(request)?,
            )
            .await?;

        // Parakeet has no language detection, so the transcription language is None.
        Ok(dto::GgmlParakeetTranscribeResponse {
            transcription: whisper_transcription_from_raw(response.text, None),
        })
    }
}

/// Parakeet exposes only a subset of whisper's decode knobs (offset_ms /
/// duration_ms / no_context); language, prompt and VAD are not supported by the
/// parakeet C API, so they are left at their defaults.
fn build_transcription_options(
    request: dto::GgmlParakeetTranscribeRequest,
) -> Result<AudioTranscriptionOptions, CoreError> {
    let decode_options = request.decode.map(|decode| AudioTranscriptionDecodeOptions {
        offset_ms: decode.offset_ms,
        duration_ms: decode.duration_ms,
        no_context: decode.no_context,
        ..Default::default()
    });

    Ok(AudioTranscriptionOptions { decode: decode_options, ..Default::default() })
}
