use serde::{Deserialize, Serialize};

pub mod audio_delta_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.audio.delta")]
        #[default]
        ResponseAudioDelta,
    }
    
}
pub use audio_delta_type::Type as AudioDeltaType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseAudioDeltaEvent {
    /// The type of the event. Always `response.audio.delta`.
    #[serde(rename = "type")]
    pub r#type: AudioDeltaType,
    /// A sequence number for this chunk of the stream response.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// A chunk of Base64 encoded response audio bytes.
    #[serde(rename = "delta")]
    pub delta: String,
}

impl ResponseAudioDeltaEvent {
    /// Emitted when there is a partial audio response.
    pub fn new(
        r#type: AudioDeltaType,
        sequence_number: i32,
        delta: String,
    ) -> ResponseAudioDeltaEvent {
        ResponseAudioDeltaEvent { r#type, sequence_number, delta }
    }
}

pub mod audio_done_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.audio.done")]
        #[default]
        ResponseAudioDone,
    }
    
}
pub use audio_done_type::Type as AudioDoneType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseAudioDoneEvent {
    /// The type of the event. Always `response.audio.done`.
    #[serde(rename = "type")]
    pub r#type: AudioDoneType,
    /// The sequence number of the delta.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseAudioDoneEvent {
    /// Emitted when the audio response is complete.
    pub fn new(r#type: AudioDoneType, sequence_number: i32) -> ResponseAudioDoneEvent {
        ResponseAudioDoneEvent { r#type, sequence_number }
    }
}

pub mod audio_transcript_delta_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.audio.transcript.delta")]
        #[default]
        ResponseAudioTranscriptDelta,
    }
    
}
pub use audio_transcript_delta_type::Type as AudioTranscriptDeltaType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseAudioTranscriptDeltaEvent {
    /// The type of the event. Always `response.audio.transcript.delta`.
    #[serde(rename = "type")]
    pub r#type: AudioTranscriptDeltaType,
    /// The partial transcript of the audio response.
    #[serde(rename = "delta")]
    pub delta: String,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseAudioTranscriptDeltaEvent {
    /// Emitted when there is a partial transcript of audio.
    pub fn new(
        r#type: AudioTranscriptDeltaType,
        delta: String,
        sequence_number: i32,
    ) -> ResponseAudioTranscriptDeltaEvent {
        ResponseAudioTranscriptDeltaEvent { r#type, delta, sequence_number }
    }
}

pub mod audio_transcript_done_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.audio.transcript.done")]
        #[default]
        ResponseAudioTranscriptDone,
    }
    
}
pub use audio_transcript_done_type::Type as AudioTranscriptDoneType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseAudioTranscriptDoneEvent {
    /// The type of the event. Always `response.audio.transcript.done`.
    #[serde(rename = "type")]
    pub r#type: AudioTranscriptDoneType,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseAudioTranscriptDoneEvent {
    /// Emitted when the full audio transcript is completed.
    pub fn new(
        r#type: AudioTranscriptDoneType,
        sequence_number: i32,
    ) -> ResponseAudioTranscriptDoneEvent {
        ResponseAudioTranscriptDoneEvent { r#type, sequence_number }
    }
}
