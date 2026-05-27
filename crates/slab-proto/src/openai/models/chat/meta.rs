use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Format {
    #[serde(rename = "wav")]
    Wav,
    #[serde(rename = "aac")]
    Aac,
    #[serde(rename = "mp3")]
    Mp3,
    #[serde(rename = "flac")]
    Flac,
    #[serde(rename = "opus")]
    Opus,
    #[serde(rename = "pcm16")]
    Pcm16,
}

impl Default for Format {
    fn default() -> Format {
        Self::Wav
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Modalities {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "audio")]
    Audio,
}

impl Default for Modalities {
    fn default() -> Modalities {
        Self::Text
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum PromptCacheRetention {
    #[serde(rename = "in_memory")]
    InMemory,
    #[serde(rename = "24h")]
    Variant24h,
}

impl Default for PromptCacheRetention {
    fn default() -> PromptCacheRetention {
        Self::InMemory
    }
}
// Output types that you would like the model to generate. Most models are capable of generating text, which is the default:  `[\"text\"]`  The `gpt-4o-audio-preview` model can also be used to [generate audio](/docs/guides/audio). To request that this model generate both text and audio responses, you can use:  `[\"text\", \"audio\"]`
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum Type {
    #[serde(rename = "function")]
    Function,
    #[serde(rename = "custom")]
    Custom,
}

impl Default for Type {
    fn default() -> Type {
        Self::Function
    }
}
