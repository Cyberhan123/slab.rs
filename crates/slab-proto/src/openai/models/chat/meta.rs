use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ChatMetaFormat {
    #[serde(rename = "wav")]
    #[default]
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


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum Modalities {
    #[serde(rename = "text")]
    #[default]
    Text,
    #[serde(rename = "audio")]
    Audio,
}


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ChatMetaPromptCacheRetention {
    #[serde(rename = "in_memory")]
    #[default]
    InMemory,
    #[serde(rename = "24h")]
    Variant24h,
}


