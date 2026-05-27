use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageDetail {
    #[serde(rename = "low")]
    #[default]
    Low,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "original")]
    Original,
}

impl std::fmt::Display for ImageDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::High => write!(f, "high"),
            Self::Auto => write!(f, "auto"),
            Self::Original => write!(f, "original"),
        }
    }
}
