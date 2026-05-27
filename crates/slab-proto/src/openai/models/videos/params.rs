use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoModel {}

impl VideoModel {
    pub fn new() -> VideoModel {
        VideoModel {}
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoReferenceInputParam {
    /// The identifier of the completed video.
    #[serde(rename = "id")]
    pub id: String,
}

impl VideoReferenceInputParam {
    /// Reference to the completed video.
    pub fn new(id: String) -> VideoReferenceInputParam {
        VideoReferenceInputParam { id }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum VideoSeconds {
    #[serde(rename = "4")]
    #[default]
    Variant4,
    #[serde(rename = "8")]
    Variant8,
    #[serde(rename = "12")]
    Variant12,
}

impl std::fmt::Display for VideoSeconds {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Variant4 => write!(f, "4"),
            Self::Variant8 => write!(f, "8"),
            Self::Variant12 => write!(f, "12"),
        }
    }
}


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum VideoSize {
    #[serde(rename = "720x1280")]
    #[default]
    Variant720x1280,
    #[serde(rename = "1280x720")]
    Variant1280x720,
    #[serde(rename = "1024x1792")]
    Variant1024x1792,
    #[serde(rename = "1792x1024")]
    Variant1792x1024,
}

impl std::fmt::Display for VideoSize {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Variant720x1280 => write!(f, "720x1280"),
            Self::Variant1280x720 => write!(f, "1280x720"),
            Self::Variant1024x1792 => write!(f, "1024x1792"),
            Self::Variant1792x1024 => write!(f, "1792x1024"),
        }
    }
}


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum VideoStatus {
    #[serde(rename = "queued")]
    #[default]
    Queued,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

impl std::fmt::Display for VideoStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

