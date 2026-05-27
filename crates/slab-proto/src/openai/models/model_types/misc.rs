use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum Summary {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "concise")]
    Concise,
    #[serde(rename = "detailed")]
    Detailed,
}

