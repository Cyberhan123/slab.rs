use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const fn default_channels() -> u8 {
    3
}

/// Raw pixel input used by image-to-image and video-to-video flows.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RawImageInput {
    #[serde(default)]
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_channels")]
    pub channels: u8,
}

/// A generated encoded image artifact, typically a PNG or JPEG byte buffer.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct GeneratedImage {
    #[serde(default)]
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_channels")]
    pub channels: u8,
}

/// A generated raw frame payload used by video assembly pipelines.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct GeneratedFrame {
    #[serde(default)]
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_channels")]
    pub channels: u8,
}
