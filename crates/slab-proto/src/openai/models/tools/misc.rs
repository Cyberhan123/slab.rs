use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Background {
    #[serde(rename = "transparent")]
    Transparent,
    #[serde(rename = "opaque")]
    Opaque,
    #[serde(rename = "auto")]
    Auto,
}

impl Default for Background {
    fn default() -> Background {
        Self::Transparent
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Moderation {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "low")]
    Low,
}

impl Default for Moderation {
    fn default() -> Moderation {
        Self::Auto
    }
}
// Background type for the generated image. One of `transparent`, `opaque`, or `auto`. Default: `auto`.

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum OutputFormat {
    #[serde(rename = "png")]
    Png,
    #[serde(rename = "webp")]
    Webp,
    #[serde(rename = "jpeg")]
    Jpeg,
}

impl Default for OutputFormat {
    fn default() -> OutputFormat {
        Self::Png
    }
}
// Moderation level for the generated image. Default: `auto`.

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Quality {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "auto")]
    Auto,
}

impl Default for Quality {
    fn default() -> Quality {
        Self::Low
    }
}
// The output format of the generated image. One of `png`, `webp`, or `jpeg`. Default: `png`.

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum Status {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "searching")]
    Searching,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

impl Default for Status {
    fn default() -> Status {
        Self::InProgress
    }
}
