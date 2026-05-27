use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseFormatJsonObject {
    /// The type of response format being defined. Always `json_object`.
    #[serde(rename = "type")]
    pub r#type: ResponseFormatJsonObjectType,
}

impl ResponseFormatJsonObject {
    /// JSON object response format. An older method of generating JSON responses. Using `json_schema` is recommended for models that support it. Note that the model will not generate JSON without a system or user message instructing it to do so.
    pub fn new(r#type: ResponseFormatJsonObjectType) -> ResponseFormatJsonObject {
        ResponseFormatJsonObject { r#type }
    }
}
/// The type of response format being defined. Always `json_object`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum ResponseFormatJsonObjectType {
    #[serde(rename = "json_object")]
    JsonObject,
}

impl Default for ResponseFormatJsonObjectType {
    fn default() -> ResponseFormatJsonObjectType {
        Self::JsonObject
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseFormatJsonSchema {
    /// The type of response format being defined. Always `json_schema`.
    #[serde(rename = "type")]
    pub r#type: ResponseFormatJsonSchemaType,
    #[serde(rename = "json_schema")]
    pub json_schema: Box<models::JsonSchema>,
}

impl ResponseFormatJsonSchema {
    /// JSON Schema response format. Used to generate structured JSON responses. Learn more about [Structured Outputs](/docs/guides/structured-outputs).
    pub fn new(
        r#type: ResponseFormatJsonSchemaType,
        json_schema: models::JsonSchema,
    ) -> ResponseFormatJsonSchema {
        ResponseFormatJsonSchema { r#type, json_schema: Box::new(json_schema) }
    }
}
/// The type of response format being defined. Always `json_schema`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum ResponseFormatJsonSchemaType {
    #[serde(rename = "json_schema")]
    JsonSchema,
}

impl Default for ResponseFormatJsonSchemaType {
    fn default() -> ResponseFormatJsonSchemaType {
        Self::JsonSchema
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseFormatText {
    /// The type of response format being defined. Always `text`.
    #[serde(rename = "type")]
    pub r#type: ResponseFormatTextType,
}

impl ResponseFormatText {
    /// Default response format. Used to generate text responses.
    pub fn new(r#type: ResponseFormatTextType) -> ResponseFormatText {
        ResponseFormatText { r#type }
    }
}
/// The type of response format being defined. Always `text`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum ResponseFormatTextType {
    #[serde(rename = "text")]
    Text,
}

impl Default for ResponseFormatTextType {
    fn default() -> ResponseFormatTextType {
        Self::Text
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseFormatTextGrammar {
    /// The type of response format being defined. Always `grammar`.
    #[serde(rename = "type")]
    pub r#type: ResponseFormatTextGrammarType,
    /// The custom grammar for the model to follow.
    #[serde(rename = "grammar")]
    pub grammar: String,
}

impl ResponseFormatTextGrammar {
    /// A custom grammar for the model to follow when generating text. Learn more in the [custom grammars guide](/docs/guides/custom-grammars).
    pub fn new(
        r#type: ResponseFormatTextGrammarType,
        grammar: String,
    ) -> ResponseFormatTextGrammar {
        ResponseFormatTextGrammar { r#type, grammar }
    }
}
/// The type of response format being defined. Always `grammar`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum ResponseFormatTextGrammarType {
    #[serde(rename = "grammar")]
    Grammar,
}

impl Default for ResponseFormatTextGrammarType {
    fn default() -> ResponseFormatTextGrammarType {
        Self::Grammar
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseFormatTextPython {
    /// The type of response format being defined. Always `python`.
    #[serde(rename = "type")]
    pub r#type: ResponseFormatTextPythonType,
}

impl ResponseFormatTextPython {
    /// Configure the model to generate valid Python code. See the [custom grammars guide](/docs/guides/custom-grammars) for more details.
    pub fn new(r#type: ResponseFormatTextPythonType) -> ResponseFormatTextPython {
        ResponseFormatTextPython { r#type }
    }
}
/// The type of response format being defined. Always `python`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum ResponseFormatTextPythonType {
    #[serde(rename = "python")]
    Python,
}

impl Default for ResponseFormatTextPythonType {
    fn default() -> ResponseFormatTextPythonType {
        Self::Python
    }
}
