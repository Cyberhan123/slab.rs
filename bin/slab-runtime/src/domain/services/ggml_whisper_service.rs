use slab_runtime_core::backend::RequestRoute;

use crate::application::dtos as dto;
use crate::domain::models::{
    AudioTranscriptionDecodeOptions, AudioTranscriptionOptions, AudioTranscriptionResponse,
    AudioTranscriptionVadOptions, AudioTranscriptionVadParams, GgmlWhisperLoadConfig,
};
use crate::domain::runtime::CoreError;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    audio_decode_stage, invalid_model, required_path, whisper_transcription_from_raw,
};

#[derive(Clone, Debug)]
pub(crate) struct GgmlWhisperService {
    runtime: DriverRuntime,
}

impl GgmlWhisperService {
    pub(crate) fn new(
        execution: ExecutionHub,
        request: dto::GgmlWhisperLoadRequest,
    ) -> Result<Self, CoreError> {
        let model_path = required_path("ggml_whisper.model_path", request.model_path)?;
        let load_payload = GgmlWhisperLoadConfig {
            model_path: model_path.clone(),
            flash_attn: request.flash_attn,
        };

        Ok(Self {
            runtime: DriverRuntime::new_typed(
                execution,
                "ggml.whisper",
                "ggml.whisper",
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
        request: dto::GgmlWhisperTranscribeRequest,
    ) -> Result<dto::GgmlWhisperTranscribeResponse, CoreError> {
        let audio_path = required_path("ggml_whisper.path", request.path.clone())?;
        let language = request.language.clone();
        let response: AudioTranscriptionResponse = self
            .runtime
            .invoke_preprocessed_typed(
                RequestRoute::Inference,
                vec![audio_decode_stage(audio_path)],
                build_transcription_options(request)?,
            )
            .await?;

        Ok(dto::GgmlWhisperTranscribeResponse {
            transcription: whisper_transcription_from_raw(response.text, language),
        })
    }
}

fn build_transcription_options(
    request: dto::GgmlWhisperTranscribeRequest,
) -> Result<AudioTranscriptionOptions, CoreError> {
    let language = request.language.filter(|value| !value.trim().is_empty());
    let prompt = request.prompt.filter(|value| !value.trim().is_empty());
    let detect_language = language.is_none().then_some(request.detect_language).flatten();
    let mut vad_options = None;
    let mut decode_options = None;

    if let Some(vad) = request.vad
        && vad.enabled.unwrap_or(false)
    {
        let model_path = vad
            .model_path
            .ok_or_else(|| invalid_model("ggml_whisper.vad.model_path", "missing required path"))?;
        if model_path.as_os_str().is_empty() {
            return Err(invalid_model("ggml_whisper.vad.model_path", "path must not be empty"));
        }

        let mut runtime_vad = AudioTranscriptionVadOptions {
            enabled: true,
            model_path: Some(model_path),
            params: None,
        };

        if let Some(vad_params) = vad.params {
            if let Some(threshold) = vad_params.threshold
                && !(0.0..=1.0).contains(&threshold)
            {
                return Err(invalid_model(
                    "ggml_whisper.vad.threshold",
                    "must be between 0.0 and 1.0",
                ));
            }
            for (field, value) in [
                ("ggml_whisper.vad.min_speech_duration_ms", vad_params.min_speech_duration_ms),
                ("ggml_whisper.vad.min_silence_duration_ms", vad_params.min_silence_duration_ms),
                ("ggml_whisper.vad.speech_pad_ms", vad_params.speech_pad_ms),
            ] {
                if value.is_some_and(|value| value < 0) {
                    return Err(invalid_model(field, "must be >= 0"));
                }
            }
            if let Some(max_speech_duration_s) = vad_params.max_speech_duration_s
                && max_speech_duration_s <= 0.0
            {
                return Err(invalid_model(
                    "ggml_whisper.vad.max_speech_duration_s",
                    "must be > 0.0",
                ));
            }
            if let Some(samples_overlap) = vad_params.samples_overlap
                && samples_overlap < 0.0
            {
                return Err(invalid_model("ggml_whisper.vad.samples_overlap", "must be >= 0.0"));
            }

            runtime_vad.params = Some(AudioTranscriptionVadParams {
                threshold: vad_params.threshold,
                min_speech_duration_ms: vad_params.min_speech_duration_ms,
                min_silence_duration_ms: vad_params.min_silence_duration_ms,
                max_speech_duration_s: vad_params.max_speech_duration_s,
                speech_pad_ms: vad_params.speech_pad_ms,
                samples_overlap: vad_params.samples_overlap,
            });
        }

        vad_options = Some(runtime_vad);
    }

    if let Some(decode) = request.decode {
        for (field, value) in [
            ("ggml_whisper.decode.offset_ms", decode.offset_ms),
            ("ggml_whisper.decode.duration_ms", decode.duration_ms),
            ("ggml_whisper.decode.max_len", decode.max_len),
            ("ggml_whisper.decode.max_tokens", decode.max_tokens),
        ] {
            if value.is_some_and(|value| value < 0) {
                return Err(invalid_model(field, "must be >= 0"));
            }
        }
        if let Some(word_thold) = decode.word_thold
            && !(0.0..=1.0).contains(&word_thold)
        {
            return Err(invalid_model(
                "ggml_whisper.decode.word_thold",
                "must be between 0.0 and 1.0",
            ));
        }
        for (field, value) in [
            ("ggml_whisper.decode.temperature", decode.temperature),
            ("ggml_whisper.decode.temperature_inc", decode.temperature_inc),
        ] {
            if value.is_some_and(|value| value < 0.0) {
                return Err(invalid_model(field, "must be >= 0.0"));
            }
        }

        decode_options = Some(AudioTranscriptionDecodeOptions {
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
        });
    }

    Ok(AudioTranscriptionOptions {
        language,
        prompt,
        detect_language,
        vad: vad_options,
        decode: decode_options,
    })
}
