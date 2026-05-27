use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ToolBackground {
    #[serde(rename = "transparent")]
    #[default]
    Transparent,
    #[serde(rename = "opaque")]
    Opaque,
    #[serde(rename = "auto")]
    Auto,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ToolModeration {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "low")]
    Low,
}

// Background type for the generated image. One of `transparent`, `opaque`, or `auto`. Default: `auto`.

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ToolOutputFormat {
    #[serde(rename = "png")]
    #[default]
    Png,
    #[serde(rename = "webp")]
    Webp,
    #[serde(rename = "jpeg")]
    Jpeg,
}

// Moderation level for the generated image. Default: `auto`.

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ToolQuality {
    #[serde(rename = "low")]
    #[default]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "auto")]
    Auto,
}

// The output format of the generated image. One of `png`, `webp`, or `jpeg`. Default: `png`.

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ToolStatus {
    #[serde(rename = "in_progress")]
    #[default]
    InProgress,
    #[serde(rename = "searching")]
    Searching,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}
