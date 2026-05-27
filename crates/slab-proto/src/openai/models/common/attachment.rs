use crate::openai::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    /// Attachment discriminator.
    #[serde(rename = "type")]
    pub r#type: models::AttachmentType,
    /// Identifier for the attachment.
    #[serde(rename = "id")]
    pub id: String,
    /// Original display name for the attachment.
    #[serde(rename = "name")]
    pub name: String,
    /// MIME type of the attachment.
    #[serde(rename = "mime_type")]
    pub mime_type: String,
    /// Preview URL for rendering the attachment inline.
    #[serde(rename = "preview_url", deserialize_with = "Option::deserialize")]
    pub preview_url: Option<String>,
}

impl Attachment {
    /// Attachment metadata included on thread items.
    pub fn new(
        r#type: models::AttachmentType,
        id: String,
        name: String,
        mime_type: String,
        preview_url: Option<String>,
    ) -> Attachment {
        Attachment { r#type, id, name, mime_type, preview_url }
    }
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum AttachmentType {
    #[serde(rename = "image")]
    #[default]
    Image,
    #[serde(rename = "file")]
    File,
}

impl std::fmt::Display for AttachmentType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Image => write!(f, "image"),
            Self::File => write!(f, "file"),
        }
    }
}
