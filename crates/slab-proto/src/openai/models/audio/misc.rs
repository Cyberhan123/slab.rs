use crate::models;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputAudio {
    /// The type of the output audio. Always `output_audio`.
    #[serde(rename = "type")]
    pub r#type: NoiseReductionType,
    /// Base64-encoded audio data from the model.
    #[serde(rename = "data")]
    pub data: String,
    /// The transcript of the audio data from the model.
    #[serde(rename = "transcript")]
    pub transcript: String,
}

impl OutputAudio {
    /// An audio output from the model.
    pub fn new(r#type: NoiseReductionType, data: String, transcript: String) -> OutputAudio {
        OutputAudio { r#type, data, transcript }
    }
}
/// The type of the output audio. Always `output_audio`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum Type {
    #[serde(rename = "output_audio")]
    OutputAudio,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum NoiseReductionType {
    #[serde(rename = "near_field")]
    NearField,
    #[serde(rename = "far_field")]
    FarField,
}

impl std::fmt::Display for NoiseReductionType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::NearField => write!(f, "near_field"),
            Self::FarField => write!(f, "far_field"),
        }
    }
}

impl Default for NoiseReductionType {
    fn default() -> NoiseReductionType {
        Self::NearField
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Eagerness {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "auto")]
    Auto,
}

impl Default for Eagerness {
    fn default() -> Eagerness {
        Self::Low
    }
}

#[repr(i64)]
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize_repr, Deserialize_repr,
)]
pub enum Rate {
    Variant24000 = 24000,
}

impl std::fmt::Display for Rate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Variant24000 => "24000",
            }
        )
    }
}

impl Default for Rate {
    fn default() -> Rate {
        Self::Variant24000
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateTranslation200Response {
    CreateTranslationResponseJson(Box<models::CreateTranslationResponseJson>),
    CreateTranslationResponseVerboseJson(Box<models::CreateTranslationResponseVerboseJson>),
}

impl Default for CreateTranslation200Response {
    fn default() -> Self {
        Self::CreateTranslationResponseJson(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateTranslationRequestModel {}

impl CreateTranslationRequestModel {
    /// ID of the model to use. Only `whisper-1` (which is powered by our open source Whisper V2 model) is currently available.
    pub fn new() -> CreateTranslationRequestModel {
        CreateTranslationRequestModel {}
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateTranslationResponseJson {
    #[serde(rename = "text")]
    pub text: String,
}

impl CreateTranslationResponseJson {
    pub fn new(text: String) -> CreateTranslationResponseJson {
        CreateTranslationResponseJson { text }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateTranslationResponseVerboseJson {
    /// The language of the output translation (always `english`).
    #[serde(rename = "language")]
    pub language: String,
    /// The duration of the input audio.
    #[serde(rename = "duration")]
    pub duration: f64,
    /// The translated text.
    #[serde(rename = "text")]
    pub text: String,
    /// Segments of the translated text and their corresponding details.
    #[serde(rename = "segments", skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<models::TranscriptionSegment>>,
}

impl CreateTranslationResponseVerboseJson {
    pub fn new(
        language: String,
        duration: f64,
        text: String,
    ) -> CreateTranslationResponseVerboseJson {
        CreateTranslationResponseVerboseJson { language, duration, text, segments: None }
    }
}
