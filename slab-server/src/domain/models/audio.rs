use crate::api::v1::audio::schema::{
    CompletionRequest, TranscribeDecodeRequest, TranscribeVadRequest,
};

#[derive(Debug, Clone)]
pub struct AudioTranscriptionCommand {
    pub path: String,
    pub vad: Option<TranscribeVadOptions>,
    pub decode: Option<TranscribeDecodeOptions>,
}

#[derive(Debug, Clone)]
pub struct TranscribeVadOptions {
    pub enabled: bool,
    pub model_path: Option<String>,
    pub threshold: Option<f32>,
    pub min_speech_duration_ms: Option<i32>,
    pub min_silence_duration_ms: Option<i32>,
    pub max_speech_duration_s: Option<f32>,
    pub speech_pad_ms: Option<i32>,
    pub samples_overlap: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct TranscribeDecodeOptions {
    pub offset_ms: Option<i32>,
    pub duration_ms: Option<i32>,
    pub no_context: Option<bool>,
    pub no_timestamps: Option<bool>,
    pub token_timestamps: Option<bool>,
    pub split_on_word: Option<bool>,
    pub suppress_nst: Option<bool>,
    pub word_thold: Option<f32>,
    pub max_len: Option<i32>,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
    pub temperature_inc: Option<f32>,
    pub entropy_thold: Option<f32>,
    pub logprob_thold: Option<f32>,
    pub no_speech_thold: Option<f32>,
    pub tdrz_enable: Option<bool>,
}

impl From<TranscribeVadRequest> for TranscribeVadOptions {
    fn from(request: TranscribeVadRequest) -> Self {
        Self {
            enabled: request.enabled,
            model_path: request.model_path,
            threshold: request.threshold,
            min_speech_duration_ms: request.min_speech_duration_ms,
            min_silence_duration_ms: request.min_silence_duration_ms,
            max_speech_duration_s: request.max_speech_duration_s,
            speech_pad_ms: request.speech_pad_ms,
            samples_overlap: request.samples_overlap,
        }
    }
}

impl From<TranscribeDecodeRequest> for TranscribeDecodeOptions {
    fn from(request: TranscribeDecodeRequest) -> Self {
        Self {
            offset_ms: request.offset_ms,
            duration_ms: request.duration_ms,
            no_context: request.no_context,
            no_timestamps: request.no_timestamps,
            token_timestamps: request.token_timestamps,
            split_on_word: request.split_on_word,
            suppress_nst: request.suppress_nst,
            word_thold: request.word_thold,
            max_len: request.max_len,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            temperature_inc: request.temperature_inc,
            entropy_thold: request.entropy_thold,
            logprob_thold: request.logprob_thold,
            no_speech_thold: request.no_speech_thold,
            tdrz_enable: request.tdrz_enable,
        }
    }
}

impl From<CompletionRequest> for AudioTranscriptionCommand {
    fn from(request: CompletionRequest) -> Self {
        Self {
            path: request.path,
            vad: request.vad.map(Into::into),
            decode: request.decode.map(Into::into),
        }
    }
}
