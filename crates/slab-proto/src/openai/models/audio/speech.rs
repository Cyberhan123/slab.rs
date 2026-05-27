use crate::models;
use serde::{Deserialize, Serialize};

use super::format::AudioResponseFormat;
use super::format::StreamFormat;
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateSpeechRequest {
    #[serde(rename = "model")]
    pub model: Box<models::CreateSpeechRequestModel>,
    /// The text to generate audio for. The maximum length is 4096 characters.
    #[serde(rename = "input")]
    pub input: String,
    /// The voice to use when generating the audio. Supported built-in voices are `alloy`, `ash`, `ballad`, `coral`, `echo`, `fable`, `onyx`, `nova`, `sage`, `shimmer`, `verse`, `marin`, and `cedar`. You may also provide a custom voice object with an `id`, for example `{ \"id\": \"voice_1234\" }`. Previews of the voices are available in the [Text to speech guide](/docs/guides/text-to-speech#voice-options).
    #[serde(rename = "voice")]
    pub voice: Box<models::VoiceIdsOrCustomVoice>,
    /// Control the voice of your generated audio with additional instructions. Does not work with `tts-1` or `tts-1-hd`.
    #[serde(rename = "instructions", skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// The format to audio in. Supported formats are `mp3`, `opus`, `aac`, `flac`, `wav`, and `pcm`.
    #[serde(rename = "response_format", skip_serializing_if = "Option::is_none")]
    pub response_format: Option<AudioResponseFormat>,
    /// The speed of the generated audio. Select a value from `0.25` to `4.0`. `1.0` is the default.
    #[serde(rename = "speed", skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    /// The format to stream the audio in. Supported formats are `sse` and `audio`. `sse` is not supported for `tts-1` or `tts-1-hd`.
    #[serde(rename = "stream_format", skip_serializing_if = "Option::is_none")]
    pub stream_format: Option<StreamFormat>,
}

impl CreateSpeechRequest {
    pub fn new(
        model: models::CreateSpeechRequestModel,
        input: String,
        voice: models::VoiceIdsOrCustomVoice,
    ) -> CreateSpeechRequest {
        CreateSpeechRequest {
            model: Box::new(model),
            input,
            voice: Box::new(voice),
            instructions: None,
            response_format: None,
            speed: None,
            stream_format: None,
        }
    }
}
// The format to audio in. Supported formats are `mp3`, `opus`, `aac`, `flac`, `wav`, and `pcm`.

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateSpeechRequestModel {}

impl CreateSpeechRequestModel {
    /// One of the available [TTS models](/docs/models#tts): `tts-1`, `tts-1-hd`, `gpt-4o-mini-tts`, or `gpt-4o-mini-tts-2025-12-15`.
    pub fn new() -> CreateSpeechRequestModel {
        CreateSpeechRequestModel {}
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CreateSpeechResponseStreamEvent {}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeechAudioDeltaEvent {
    /// The type of the event. Always `speech.audio.delta`.
    #[serde(rename = "type")]
    pub r#type: SpeechAudioDeltaEventType,
    /// A chunk of Base64-encoded audio data.
    #[serde(rename = "audio")]
    pub audio: String,
}

impl SpeechAudioDeltaEvent {
    /// Emitted for each chunk of audio data generated during speech synthesis.
    pub fn new(r#type: SpeechAudioDeltaEventType, audio: String) -> SpeechAudioDeltaEvent {
        SpeechAudioDeltaEvent { r#type, audio }
    }
}
/// The type of the event. Always `speech.audio.delta`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum SpeechAudioDeltaEventType {
    #[serde(rename = "speech.audio.delta")]
    #[default]
    SpeechAudioDelta,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeechAudioDoneEvent {
    /// The type of the event. Always `speech.audio.done`.
    #[serde(rename = "type")]
    pub r#type: SpeechAudioDoneEventType,
    #[serde(rename = "usage")]
    pub usage: Box<models::SpeechAudioDoneEventUsage>,
}

impl SpeechAudioDoneEvent {
    /// Emitted when the speech synthesis is complete and all audio has been streamed.
    pub fn new(
        r#type: SpeechAudioDoneEventType,
        usage: models::SpeechAudioDoneEventUsage,
    ) -> SpeechAudioDoneEvent {
        SpeechAudioDoneEvent { r#type, usage: Box::new(usage) }
    }
}
/// The type of the event. Always `speech.audio.done`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum SpeechAudioDoneEventType {
    #[serde(rename = "speech.audio.done")]
    #[default]
    SpeechAudioDone,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeechAudioDoneEventUsage {
    /// Number of input tokens in the prompt.
    #[serde(rename = "input_tokens")]
    pub input_tokens: i32,
    /// Number of output tokens generated.
    #[serde(rename = "output_tokens")]
    pub output_tokens: i32,
    /// Total number of tokens used (input + output).
    #[serde(rename = "total_tokens")]
    pub total_tokens: i32,
}

impl SpeechAudioDoneEventUsage {
    /// Token usage statistics for the request.
    pub fn new(
        input_tokens: i32,
        output_tokens: i32,
        total_tokens: i32,
    ) -> SpeechAudioDoneEventUsage {
        SpeechAudioDoneEventUsage { input_tokens, output_tokens, total_tokens }
    }
}
