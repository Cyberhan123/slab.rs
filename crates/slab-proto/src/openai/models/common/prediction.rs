use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct PredictionContent {
    /// The type of the predicted content you want to provide. This type is currently always `content`.
    #[serde(rename = "type")]
    pub r#type: Type,
    #[serde(rename = "content")]
    pub content: Box<models::PredictionContentContent>,
}

impl PredictionContent {
    /// Static predicted output content, such as the content of a text file that is being regenerated.
    pub fn new(r#type: Type, content: models::PredictionContentContent) -> PredictionContent {
        PredictionContent { r#type, content: Box::new(content) }
    }
}
/// The type of the predicted content you want to provide. This type is currently always `content`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum Type {
    #[serde(rename = "content")]
    #[default]
    Content,
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PredictionContentContent {
    /// The content used for a Predicted Output. This is often the text of a file you are regenerating with minor changes.
    TextContent(String),
    /// An array of content parts with a defined type. Supported options differ based on the [model](/docs/models) being used to generate the response. Can contain text inputs.
    ArrayContentParts(Vec<models::ChatCompletionRequestMessageContentPartText>),
}

impl Default for PredictionContentContent {
    fn default() -> Self {
        Self::TextContent(Default::default())
    }
}
