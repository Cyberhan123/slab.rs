use crate::openai::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateTranscriptionRequestChunkingStrategy {
    /// Must be set to `server_vad` to enable manual chunking using server side VAD.
    #[serde(rename = "type")]
    pub r#type: CreateTranscriptionRequestChunkingStrategyType,
    /// Amount of audio to include before the VAD detected speech (in  milliseconds).
    #[serde(rename = "prefix_padding_ms", skip_serializing_if = "Option::is_none")]
    pub prefix_padding_ms: Option<i32>,
    /// Duration of silence to detect speech stop (in milliseconds). With shorter values the model will respond more quickly,  but may jump in on short pauses from the user.
    #[serde(rename = "silence_duration_ms", skip_serializing_if = "Option::is_none")]
    pub silence_duration_ms: Option<i32>,
    /// Sensitivity threshold (0.0 to 1.0) for voice activity detection. A  higher threshold will require louder audio to activate the model, and  thus might perform better in noisy environments.
    #[serde(rename = "threshold", skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
}

impl CreateTranscriptionRequestChunkingStrategy {
    /// Controls how the audio is cut into chunks. When set to `\"auto\"`, the server first normalizes loudness and then uses voice activity detection (VAD) to choose boundaries. `server_vad` object can be provided to tweak VAD detection parameters manually. If unset, the audio is transcribed as a single block. Required when using `gpt-4o-transcribe-diarize` for inputs longer than 30 seconds.
    pub fn new(
        r#type: CreateTranscriptionRequestChunkingStrategyType,
    ) -> CreateTranscriptionRequestChunkingStrategy {
        CreateTranscriptionRequestChunkingStrategy {
            r#type,
            prefix_padding_ms: None,
            silence_duration_ms: None,
            threshold: None,
        }
    }
}
/// Must be set to `server_vad` to enable manual chunking using server side VAD.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum CreateTranscriptionRequestChunkingStrategyType {
    #[serde(rename = "server_vad")]
    #[default]
    ServerVad,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateTranscriptionResponseDiarizedJson {
    /// The type of task that was run. Always `transcribe`.
    #[serde(rename = "task")]
    pub task: CreateTranscriptionResponseDiarizedJsonTask,
    /// Duration of the input audio in seconds.
    #[serde(rename = "duration")]
    pub duration: f64,
    /// The concatenated transcript text for the entire audio input.
    #[serde(rename = "text")]
    pub text: String,
    /// Segments of the transcript annotated with timestamps and speaker labels.
    #[serde(rename = "segments")]
    pub segments: Vec<models::TranscriptionDiarizedSegment>,
    #[serde(rename = "usage", skip_serializing_if = "Option::is_none")]
    pub usage: Option<Box<serde_json::Value>>,
}

impl CreateTranscriptionResponseDiarizedJson {
    /// Represents a diarized transcription response returned by the model, including the combined transcript and speaker-segment annotations.
    pub fn new(
        task: CreateTranscriptionResponseDiarizedJsonTask,
        duration: f64,
        text: String,
        segments: Vec<models::TranscriptionDiarizedSegment>,
    ) -> CreateTranscriptionResponseDiarizedJson {
        CreateTranscriptionResponseDiarizedJson { task, duration, text, segments, usage: None }
    }
}
/// The type of task that was run. Always `transcribe`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum CreateTranscriptionResponseDiarizedJsonTask {
    #[serde(rename = "transcribe")]
    #[default]
    Transcribe,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateTranscriptionRequestModel {}

impl CreateTranscriptionRequestModel {
    /// ID of the model to use. The options are `gpt-4o-transcribe`, `gpt-4o-mini-transcribe`, `gpt-4o-mini-transcribe-2025-12-15`, `whisper-1` (which is powered by our open source Whisper V2 model), and `gpt-4o-transcribe-diarize`.
    pub fn new() -> CreateTranscriptionRequestModel {
        CreateTranscriptionRequestModel {}
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateTranscription200Response {
    CreateTranscriptionResponseJson(Box<serde_json::Value>),
    CreateTranscriptionResponseDiarizedJson(Box<models::CreateTranscriptionResponseDiarizedJson>),
    CreateTranscriptionResponseVerboseJson(Box<serde_json::Value>),
}

impl Default for CreateTranscription200Response {
    fn default() -> Self {
        Self::CreateTranscriptionResponseJson(Default::default())
    }
}
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioTranscription {
    #[serde(rename = "model", skip_serializing_if = "Option::is_none")]
    pub model: Option<Box<models::AudioTranscriptionModel>>,
    /// The language of the input audio. Supplying the input language in [ISO-639-1](https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes) (e.g. `en`) format will improve accuracy and latency.
    #[serde(rename = "language", skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// An optional text to guide the model's style or continue a previous audio segment. For `whisper-1`, the [prompt is a list of keywords](/docs/guides/speech-to-text#prompting). For `gpt-4o-transcribe` models (excluding `gpt-4o-transcribe-diarize`), the prompt is a free text string, for example \"expect words related to technology\". Prompt is not supported with `gpt-realtime-whisper` in GA Realtime sessions.
    #[serde(rename = "prompt", skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Controls how long the model waits before emitting transcription text. Higher values can improve transcription accuracy at the cost of latency. Only supported with `gpt-realtime-whisper` in GA Realtime sessions.
    #[serde(rename = "delay", skip_serializing_if = "Option::is_none")]
    pub delay: Option<Delay>,
}

impl AudioTranscription {
    pub fn new() -> AudioTranscription {
        AudioTranscription { model: None, language: None, prompt: None, delay: None }
    }
}
/// Controls how long the model waits before emitting transcription text. Higher values can improve transcription accuracy at the cost of latency. Only supported with `gpt-realtime-whisper` in GA Realtime sessions.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum Delay {
    #[serde(rename = "minimal")]
    #[default]
    Minimal,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "xhigh")]
    Xhigh,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TranscriptionChunkingStrategy {
    /// Automatically set chunking parameters based on the audio. Must be set to `\"auto\"`.
    String(String),
    VadConfig(Box<models::VadConfig>),
}

impl Default for TranscriptionChunkingStrategy {
    fn default() -> Self {
        Self::String(Default::default())
    }
}
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptionDiarizedSegment {
    /// The type of the segment. Always `transcript.text.segment`.
    #[serde(rename = "type")]
    pub r#type: TranscriptionDiarizedSegmentType,
    /// Unique identifier for the segment.
    #[serde(rename = "id")]
    pub id: String,
    /// Start timestamp of the segment in seconds.
    #[serde(rename = "start")]
    pub start: f64,
    /// End timestamp of the segment in seconds.
    #[serde(rename = "end")]
    pub end: f64,
    /// Transcript text for this segment.
    #[serde(rename = "text")]
    pub text: String,
    /// Speaker label for this segment. When known speakers are provided, the label matches `known_speaker_names[]`. Otherwise speakers are labeled sequentially using capital letters (`A`, `B`, ...).
    #[serde(rename = "speaker")]
    pub speaker: String,
}

impl TranscriptionDiarizedSegment {
    /// A segment of diarized transcript text with speaker metadata.
    pub fn new(
        r#type: TranscriptionDiarizedSegmentType,
        id: String,
        start: f64,
        end: f64,
        text: String,
        speaker: String,
    ) -> TranscriptionDiarizedSegment {
        TranscriptionDiarizedSegment { r#type, id, start, end, text, speaker }
    }
}
/// The type of the segment. Always `transcript.text.segment`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum TranscriptionDiarizedSegmentType {
    #[serde(rename = "transcript.text.segment")]
    #[default]
    TranscriptTextSegment,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum TranscriptionInclude {
    #[serde(rename = "logprobs")]
    #[default]
    Logprobs,
}

impl std::fmt::Display for TranscriptionInclude {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Logprobs => write!(f, "logprobs"),
        }
    }
}

/// AudioTranscriptionModel : The model to use for transcription. Current options are `whisper-1`, `gpt-4o-mini-transcribe`, `gpt-4o-mini-transcribe-2025-12-15`, `gpt-4o-transcribe`, `gpt-4o-transcribe-diarize`, and `gpt-realtime-whisper`. Use `gpt-4o-transcribe-diarize` when you need diarization with speaker labels.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioTranscriptionModel {}

impl AudioTranscriptionModel {
    /// The model to use for transcription. Current options are `whisper-1`, `gpt-4o-mini-transcribe`, `gpt-4o-mini-transcribe-2025-12-15`, `gpt-4o-transcribe`, `gpt-4o-transcribe-diarize`, and `gpt-realtime-whisper`. Use `gpt-4o-transcribe-diarize` when you need diarization with speaker labels.
    pub fn new() -> AudioTranscriptionModel {
        AudioTranscriptionModel {}
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioTranscriptionResponse {
    #[serde(rename = "model", skip_serializing_if = "Option::is_none")]
    pub model: Option<Box<models::AudioTranscriptionResponseModel>>,
    /// The language of the input audio.
    #[serde(rename = "language", skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// The prompt configured for input audio transcription, when present.
    #[serde(rename = "prompt", skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

impl AudioTranscriptionResponse {
    pub fn new() -> AudioTranscriptionResponse {
        AudioTranscriptionResponse { model: None, language: None, prompt: None }
    }
}

/// AudioTranscriptionResponseModel : The model used for transcription. Current options are `whisper-1`, `gpt-4o-mini-transcribe`, `gpt-4o-mini-transcribe-2025-12-15`, `gpt-4o-transcribe`, `gpt-4o-transcribe-diarize`, and `gpt-realtime-whisper`.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioTranscriptionResponseModel {}

impl AudioTranscriptionResponseModel {
    /// The model used for transcription. Current options are `whisper-1`, `gpt-4o-mini-transcribe`, `gpt-4o-mini-transcribe-2025-12-15`, `gpt-4o-transcribe`, `gpt-4o-transcribe-diarize`, and `gpt-realtime-whisper`.
    pub fn new() -> AudioTranscriptionResponseModel {
        AudioTranscriptionResponseModel {}
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptionSegment {
    /// Unique identifier of the segment.
    #[serde(rename = "id")]
    pub id: i32,
    /// Seek offset of the segment.
    #[serde(rename = "seek")]
    pub seek: i32,
    /// Start time of the segment in seconds.
    #[serde(rename = "start")]
    pub start: f64,
    /// End time of the segment in seconds.
    #[serde(rename = "end")]
    pub end: f64,
    /// Text content of the segment.
    #[serde(rename = "text")]
    pub text: String,
    /// Array of token IDs for the text content.
    #[serde(rename = "tokens")]
    pub tokens: Vec<i32>,
    /// Temperature parameter used for generating the segment.
    #[serde(rename = "temperature")]
    pub temperature: f32,
    /// Average logprob of the segment. If the value is lower than -1, consider the logprobs failed.
    #[serde(rename = "avg_logprob")]
    pub avg_logprob: f32,
    /// Compression ratio of the segment. If the value is greater than 2.4, consider the compression failed.
    #[serde(rename = "compression_ratio")]
    pub compression_ratio: f32,
    /// Probability of no speech in the segment. If the value is higher than 1.0 and the `avg_logprob` is below -1, consider this segment silent.
    #[serde(rename = "no_speech_prob")]
    pub no_speech_prob: f32,
}

impl TranscriptionSegment {
    pub fn new(
        id: i32,
        seek: i32,
        start: f64,
        end: f64,
        text: String,
        tokens: Vec<i32>,
        temperature: f32,
        avg_logprob: f32,
        compression_ratio: f32,
        no_speech_prob: f32,
    ) -> TranscriptionSegment {
        TranscriptionSegment {
            id,
            seek,
            start,
            end,
            text,
            tokens,
            temperature,
            avg_logprob,
            compression_ratio,
            no_speech_prob,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptionWord {
    /// The text content of the word.
    #[serde(rename = "word")]
    pub word: String,
    /// Start time of the word in seconds.
    #[serde(rename = "start")]
    pub start: f64,
    /// End time of the word in seconds.
    #[serde(rename = "end")]
    pub end: f64,
}

impl TranscriptionWord {
    pub fn new(word: String, start: f64, end: f64) -> TranscriptionWord {
        TranscriptionWord { word, start, end }
    }
}
