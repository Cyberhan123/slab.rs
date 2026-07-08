//! User-supplied input items for a harness turn.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A single piece of user input within a [`crate::harness::operation::UserSubmissionOp`]
/// turn.
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UserInput {
    /// Free-form text. `text_elements` marks byte ranges within `text` that
    /// carry rich markers (e.g. image placeholders) without mutating the text.
    Text {
        text: String,
        #[serde(default, rename = "textElements")]
        text_elements: Vec<TextElement>,
    },

    /// Pre-encoded data: URI image.
    Image {
        #[serde(rename = "imageUrl")]
        image_url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
    },

    /// Local image path. Converted to an [`UserInput::Image`] (base64 data URL)
    /// during request serialization.
    LocalImage {
        path: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
    },

    /// Skill selected by the user (name + path to `SKILL.md`).
    Skill { name: String, path: PathBuf },

    /// Explicit structured mention. `path` identifies the target, e.g.
    /// `app://<connector-id>` or `plugin://<plugin-name>@<marketplace-name>`.
    Mention { name: String, path: String },
}

impl UserInput {
    /// Convenience: is this a text input?
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text { .. })
    }

    /// The literal text payload if this is [`UserInput::Text`].
    pub fn as_text(&self) -> Option<&str> {
        if let Self::Text { text, .. } = self { Some(text) } else { None }
    }
}

/// A byte range within a UTF-8 text buffer.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
pub struct ByteRange {
    /// Start byte offset (inclusive).
    pub start: usize,
    /// End byte offset (exclusive).
    pub end: usize,
}

impl From<std::ops::Range<usize>> for ByteRange {
    fn from(range: std::ops::Range<usize>) -> Self {
        Self { start: range.start, end: range.end }
    }
}

/// A span within [`UserInput::Text`] marking a rich element.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
pub struct TextElement {
    /// Byte range in the parent text buffer.
    pub byte_range: ByteRange,
    /// Optional human-readable placeholder for the element.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

impl TextElement {
    pub fn new(byte_range: ByteRange, placeholder: Option<String>) -> Self {
        Self { byte_range, placeholder }
    }
}

/// Resolution hint for an image input, mirroring the OpenAI image detail enum.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    Low,
    High,
    Auto,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_round_trips() {
        let input = UserInput::Text { text: "hi".to_owned(), text_elements: vec![] };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "hi");
        let back: UserInput = serde_json::from_value(json).unwrap();
        assert_eq!(input, back);
    }

    #[test]
    fn image_input_omits_absent_detail() {
        let input = UserInput::Image { image_url: "data:".to_owned(), detail: None };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["type"], "image");
        assert!(json.get("detail").is_none());
    }

    #[test]
    fn mention_input_round_trips() {
        let input =
            UserInput::Mention { name: "n".to_owned(), path: "plugin://foo@bar".to_owned() };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["type"], "mention");
        assert_eq!(json["path"], "plugin://foo@bar");
    }
}
