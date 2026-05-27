use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptTextDeltaEvent {
    /// The type of the event. Always `transcript.text.delta`.
    #[serde(rename = "type")]
    pub r#type: TranscriptTextDeltaEventType,
    /// The text delta that was additionally transcribed.
    #[serde(rename = "delta")]
    pub delta: String,
    /// The log probabilities of the delta. Only included if you [create a transcription](/docs/api-reference/audio/create-transcription) with the `include[]` parameter set to `logprobs`.
    #[serde(rename = "logprobs", skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<Vec<models::TranscriptTextDeltaEventLogprobsInner>>,
    /// Identifier of the diarized segment that this delta belongs to. Only present when using `gpt-4o-transcribe-diarize`.
    #[serde(rename = "segment_id", skip_serializing_if = "Option::is_none")]
    pub segment_id: Option<String>,
}

impl TranscriptTextDeltaEvent {
    /// Emitted when there is an additional text delta. This is also the first event emitted when the transcription starts. Only emitted when you [create a transcription](/docs/api-reference/audio/create-transcription) with the `Stream` parameter set to `true`.
    pub fn new(r#type: TranscriptTextDeltaEventType, delta: String) -> TranscriptTextDeltaEvent {
        TranscriptTextDeltaEvent { r#type, delta, logprobs: None, segment_id: None }
    }
}
/// The type of the event. Always `transcript.text.delta`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum TranscriptTextDeltaEventType {
    #[serde(rename = "transcript.text.delta")]
    TranscriptTextDelta,
}

impl Default for TranscriptTextDeltaEventType {
    fn default() -> TranscriptTextDeltaEventType {
        Self::TranscriptTextDelta
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptTextDeltaEventLogprobsInner {
    /// The token that was used to generate the log probability.
    #[serde(rename = "token", skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// The log probability of the token.
    #[serde(rename = "logprob", skip_serializing_if = "Option::is_none")]
    pub logprob: Option<f64>,
    /// The bytes that were used to generate the log probability.
    #[serde(rename = "bytes", skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<i32>>,
}

impl TranscriptTextDeltaEventLogprobsInner {
    pub fn new() -> TranscriptTextDeltaEventLogprobsInner {
        TranscriptTextDeltaEventLogprobsInner { token: None, logprob: None, bytes: None }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptTextDoneEvent {
    /// The type of the event. Always `transcript.text.done`.
    #[serde(rename = "type")]
    pub r#type: TranscriptTextDoneEventType,
    /// The text that was transcribed.
    #[serde(rename = "text")]
    pub text: String,
    /// The log probabilities of the individual tokens in the transcription. Only included if you [create a transcription](/docs/api-reference/audio/create-transcription) with the `include[]` parameter set to `logprobs`.
    #[serde(rename = "logprobs", skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<Vec<models::TranscriptTextDeltaEventLogprobsInner>>,
    #[serde(rename = "usage", skip_serializing_if = "Option::is_none")]
    pub usage: Option<Box<models::TranscriptTextUsageTokens>>,
}

impl TranscriptTextDoneEvent {
    /// Emitted when the transcription is complete. Contains the complete transcription text. Only emitted when you [create a transcription](/docs/api-reference/audio/create-transcription) with the `Stream` parameter set to `true`.
    pub fn new(r#type: TranscriptTextDoneEventType, text: String) -> TranscriptTextDoneEvent {
        TranscriptTextDoneEvent { r#type, text, logprobs: None, usage: None }
    }
}
/// The type of the event. Always `transcript.text.done`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum TranscriptTextDoneEventType {
    #[serde(rename = "transcript.text.done")]
    TranscriptTextDone,
}

impl Default for TranscriptTextDoneEventType {
    fn default() -> TranscriptTextDoneEventType {
        Self::TranscriptTextDone
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptTextSegmentEvent {
    /// The type of the event. Always `transcript.text.segment`.
    #[serde(rename = "type")]
    pub r#type: TranscriptTextSegmentEventType,
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
    /// Speaker label for this segment.
    #[serde(rename = "speaker")]
    pub speaker: String,
}

impl TranscriptTextSegmentEvent {
    /// Emitted when a diarized transcription returns a completed segment with speaker information. Only emitted when you [create a transcription](/docs/api-reference/audio/create-transcription) with `stream` set to `true` and `response_format` set to `diarized_json`.
    pub fn new(
        r#type: TranscriptTextSegmentEventType,
        id: String,
        start: f64,
        end: f64,
        text: String,
        speaker: String,
    ) -> TranscriptTextSegmentEvent {
        TranscriptTextSegmentEvent { r#type, id, start, end, text, speaker }
    }
}
/// The type of the event. Always `transcript.text.segment`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum TranscriptTextSegmentEventType {
    #[serde(rename = "transcript.text.segment")]
    TranscriptTextSegment,
}

impl Default for TranscriptTextSegmentEventType {
    fn default() -> TranscriptTextSegmentEventType {
        Self::TranscriptTextSegment
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptTextUsageDuration {
    /// The type of the usage object. Always `duration` for this variant.
    #[serde(rename = "type")]
    pub r#type: TranscriptTextUsageDurationType,
    /// Duration of the input audio in seconds.
    #[serde(rename = "seconds")]
    pub seconds: f64,
}

impl TranscriptTextUsageDuration {
    /// Usage statistics for models billed by audio input duration.
    pub fn new(
        r#type: TranscriptTextUsageDurationType,
        seconds: f64,
    ) -> TranscriptTextUsageDuration {
        TranscriptTextUsageDuration { r#type, seconds }
    }
}
/// The type of the usage object. Always `duration` for this variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum TranscriptTextUsageDurationType {
    #[serde(rename = "duration")]
    Duration,
}

impl Default for TranscriptTextUsageDurationType {
    fn default() -> TranscriptTextUsageDurationType {
        Self::Duration
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptTextUsageTokens {
    /// The type of the usage object. Always `tokens` for this variant.
    #[serde(rename = "type")]
    pub r#type: TranscriptTextUsageTokensType,
    /// Number of input tokens billed for this request.
    #[serde(rename = "input_tokens")]
    pub input_tokens: i32,
    /// Number of output tokens generated.
    #[serde(rename = "output_tokens")]
    pub output_tokens: i32,
    /// Total number of tokens used (input + output).
    #[serde(rename = "total_tokens")]
    pub total_tokens: i32,
    #[serde(rename = "input_token_details", skip_serializing_if = "Option::is_none")]
    pub input_token_details: Option<Box<models::TranscriptTextUsageTokensInputTokenDetails>>,
}

impl TranscriptTextUsageTokens {
    /// Usage statistics for models billed by token usage.
    pub fn new(
        r#type: TranscriptTextUsageTokensType,
        input_tokens: i32,
        output_tokens: i32,
        total_tokens: i32,
    ) -> TranscriptTextUsageTokens {
        TranscriptTextUsageTokens {
            r#type,
            input_tokens,
            output_tokens,
            total_tokens,
            input_token_details: None,
        }
    }
}
/// The type of the usage object. Always `tokens` for this variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum TranscriptTextUsageTokensType {
    #[serde(rename = "tokens")]
    Tokens,
}

impl Default for TranscriptTextUsageTokensType {
    fn default() -> TranscriptTextUsageTokensType {
        Self::Tokens
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscriptTextUsageTokensInputTokenDetails {
    /// Number of text tokens billed for this request.
    #[serde(rename = "text_tokens", skip_serializing_if = "Option::is_none")]
    pub text_tokens: Option<i32>,
    /// Number of audio tokens billed for this request.
    #[serde(rename = "audio_tokens", skip_serializing_if = "Option::is_none")]
    pub audio_tokens: Option<i32>,
}

impl TranscriptTextUsageTokensInputTokenDetails {
    /// Details about the input tokens billed for this request.
    pub fn new() -> TranscriptTextUsageTokensInputTokenDetails {
        TranscriptTextUsageTokensInputTokenDetails { text_tokens: None, audio_tokens: None }
    }
}
