use slab_runtime_core::{CoreError, Payload};
use slab_types::{Capability, ModelFamily};
use slab_whisper::{
    ContextParams as WhisperContextParams, FullParams as WhisperFullParams,
    SamplingStrategy as WhisperSamplingStrategy, WhisperVadParams as CanonicalWhisperVadParams,
};

use slab_proto::convert::dto;

use super::ExecutionHub;
use super::driver_runtime::DriverRuntime;
use super::helpers::{
    audio_decode_stage, decode_utf8_payload, invalid_model, model_spec, required_path,
    whisper_transcription_from_raw,
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
        let load_payload = Payload::typed(WhisperContextParams {
            model_path: Some(model_path.clone()),
            flash_attn: request.flash_attn,
            ..Default::default()
        });

        Ok(Self {
            runtime: DriverRuntime::new(
                execution,
                model_spec(ModelFamily::Whisper, Capability::AudioTranscription, model_path),
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
        let payload = self
            .runtime
            .submit(
                Capability::AudioTranscription,
                false,
                Payload::None,
                vec![audio_decode_stage(audio_path)],
                Payload::typed(build_full_params(request)?),
            )
            .await?
            .result()
            .await?;

        let raw = decode_utf8_payload(payload, "ggml_whisper")?;
        Ok(dto::GgmlWhisperTranscribeResponse {
            transcription: whisper_transcription_from_raw(raw, language),
        })
    }
}

fn build_full_params(
    request: dto::GgmlWhisperTranscribeRequest,
) -> Result<WhisperFullParams, CoreError> {
    let language = request.language.filter(|value| !value.trim().is_empty());
    let mut params = WhisperFullParams {
        strategy: WhisperSamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 },
        language: language.clone(),
        initial_prompt: request.prompt.filter(|value| !value.trim().is_empty()),
        detect_language: language.is_none().then_some(request.detect_language).flatten(),
        ..Default::default()
    };

    if let Some(vad) = request.vad
        && vad.enabled.unwrap_or(false)
    {
        let model_path = vad
            .model_path
            .ok_or_else(|| invalid_model("ggml_whisper.vad.model_path", "missing required path"))?;
        if model_path.as_os_str().is_empty() {
            return Err(invalid_model("ggml_whisper.vad.model_path", "path must not be empty"));
        }

        params.vad = Some(true);
        params.vad_model_path = Some(model_path);

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

            params.vad_params = Some(CanonicalWhisperVadParams {
                threshold: vad_params.threshold,
                min_speech_duration_ms: vad_params.min_speech_duration_ms,
                min_silence_duration_ms: vad_params.min_silence_duration_ms,
                max_speech_duration_s: vad_params.max_speech_duration_s,
                speech_pad_ms: vad_params.speech_pad_ms,
                samples_overlap: vad_params.samples_overlap,
            });
        }
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

        params.offset_ms = decode.offset_ms;
        params.duration_ms = decode.duration_ms;
        params.no_context = decode.no_context;
        params.no_timestamps = decode.no_timestamps;
        params.token_timestamps = decode.token_timestamps;
        params.split_on_word = decode.split_on_word;
        params.suppress_nst = decode.suppress_nst;
        params.thold_pt = decode.word_thold;
        params.max_len = decode.max_len;
        params.max_tokens = decode.max_tokens;
        params.temperature = decode.temperature;
        params.temperature_inc = decode.temperature_inc;
        params.entropy_thold = decode.entropy_thold;
        params.logprob_thold = decode.logprob_thold;
        params.no_speech_thold = decode.no_speech_thold;
        params.tdrz_enable = decode.tdrz_enable;
    }

    Ok(params)
}
