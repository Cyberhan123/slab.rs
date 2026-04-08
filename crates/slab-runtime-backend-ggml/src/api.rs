use slab_runtime_core::CoreError;
use slab_types::{WhisperDecodeOptions, WhisperVadOptions};
use slab_whisper::{
    FullParams as WhisperFullParams, SamplingStrategy as WhisperSamplingStrategy,
    WhisperVadParams as CanonicalWhisperVadParams,
};

pub(crate) fn build_ggml_whisper_full_params_from_legacy(
    language: Option<String>,
    prompt: Option<String>,
    vad: Option<WhisperVadOptions>,
    decode: Option<WhisperDecodeOptions>,
) -> Result<WhisperFullParams, CoreError> {
    let mut params = WhisperFullParams {
        strategy: WhisperSamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 },
        language: language.filter(|value| !value.trim().is_empty()),
        initial_prompt: prompt.filter(|value| !value.trim().is_empty()),
        ..Default::default()
    };

    if let Some(vad) = vad
        && vad.enabled
    {
        let model_path = vad.model_path.ok_or_else(|| {
            decode_error(
                "invalid whisper inference options: vad.model_path is missing",
            )
        })?;
        if model_path.as_os_str().is_empty() {
            return Err(decode_error(
                "invalid whisper inference options: vad.model_path is empty",
            ));
        }

        params.vad = Some(true);
        params.vad_model_path = Some(model_path);

        if let Some(vad_params) = vad.params {
            if let Some(threshold) = vad_params.threshold
                && !(0.0..=1.0).contains(&threshold)
            {
                return Err(decode_error(
                    "invalid whisper inference options: vad.threshold must be between 0.0 and 1.0",
                ));
            }

            for (name, value) in [
                ("vad.min_speech_duration_ms", vad_params.min_speech_duration_ms),
                ("vad.min_silence_duration_ms", vad_params.min_silence_duration_ms),
                ("vad.speech_pad_ms", vad_params.speech_pad_ms),
            ] {
                if value.is_some_and(|value| value < 0) {
                    return Err(decode_error(format!(
                        "invalid whisper inference options: {name} must be >= 0"
                    )));
                }
            }

            if let Some(max_speech_duration_s) = vad_params.max_speech_duration_s
                && max_speech_duration_s <= 0.0
            {
                return Err(decode_error(
                    "invalid whisper inference options: vad.max_speech_duration_s must be > 0.0",
                ));
            }

            if let Some(samples_overlap) = vad_params.samples_overlap
                && samples_overlap < 0.0
            {
                return Err(decode_error(
                    "invalid whisper inference options: vad.samples_overlap must be >= 0.0",
                ));
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

    if let Some(decode) = decode {
        for (name, value) in [
            ("decode.offset_ms", decode.offset_ms),
            ("decode.duration_ms", decode.duration_ms),
            ("decode.max_len", decode.max_len),
            ("decode.max_tokens", decode.max_tokens),
        ] {
            if value.is_some_and(|value| value < 0) {
                return Err(decode_error(format!(
                    "invalid whisper inference options: {name} must be >= 0"
                )));
            }
        }

        if let Some(word_thold) = decode.word_thold
            && !(0.0..=1.0).contains(&word_thold)
        {
            return Err(decode_error(
                "invalid whisper inference options: decode.word_thold must be between 0.0 and 1.0",
            ));
        }

        for (name, value) in [
            ("decode.temperature", decode.temperature),
            ("decode.temperature_inc", decode.temperature_inc),
        ] {
            if value.is_some_and(|value| value < 0.0) {
                return Err(decode_error(format!(
                    "invalid whisper inference options: {name} must be >= 0.0"
                )));
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

fn decode_error(message: impl Into<String>) -> CoreError {
    CoreError::ResultDecodeFailed {
        task_kind: "audio_transcription".to_owned(),
        message: message.into(),
    }
}
