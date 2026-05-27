use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputAudio {
    /// The type of the input item. Always `input_audio`.
    #[serde(rename = "type")]
    pub r#type: AudioInputType,
    #[serde(rename = "input_audio")]
    pub input_audio: Box<models::InputAudioInputAudio>,
}

impl InputAudio {
    /// An audio input to the model.
    pub fn new(r#type: AudioInputType, input_audio: models::InputAudioInputAudio) -> InputAudio {
        InputAudio { r#type, input_audio: Box::new(input_audio) }
    }
}
/// The type of the input item. Always `input_audio`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum AudioInputType {
    #[serde(rename = "input_audio")]
    #[default]
    InputAudio,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputAudioInputAudio {
    /// Base64-encoded audio data.
    #[serde(rename = "data")]
    pub data: String,
    /// The format of the audio data. Currently supported formats are `mp3` and `wav`.
    #[serde(rename = "format")]
    pub format: AudioInputFormat,
}

impl InputAudioInputAudio {
    pub fn new(data: String, format: AudioInputFormat) -> InputAudioInputAudio {
        InputAudioInputAudio { data, format }
    }
}
// The format of the audio data. Currently supported formats are `mp3` and `wav`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum AudioInputFormat {
    #[serde(rename = "mp3")]
    #[default]
    Mp3,
    #[serde(rename = "wav")]
    Wav,
}
