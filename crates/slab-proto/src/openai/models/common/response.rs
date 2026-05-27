use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct JsonSchema {
    /// The name of the response format. Must be a-z, A-Z, 0-9, or contain underscores and dashes, with a maximum length of 64.
    #[serde(rename = "name")]
    pub name: String,
    /// A description of what the response format is for, used by the model to determine how to respond in the format.
    #[serde(rename = "description", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The schema for the response format, described as a JSON Schema object. Learn how to build JSON schemas [here](https://json-schema.org/).
    #[serde(rename = "schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<std::collections::HashMap<String, serde_json::Value>>,
    /// Whether to enable strict schema adherence when generating the output. If set to true, the model will always follow the exact schema defined in the `schema` field. Only a subset of JSON Schema is supported when `strict` is `true`. To learn more, read the [Structured Outputs guide](/docs/guides/structured-outputs).
    #[serde(
        rename = "strict",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub strict: Option<Option<bool>>,
}

impl JsonSchema {
    /// Structured Outputs configuration options, including a JSON Schema.
    pub fn new(name: String) -> JsonSchema {
        JsonSchema { name, description: None, schema: None, strict: None }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TextResponseFormatConfiguration {
    ResponseFormatText(Box<models::ResponseFormatText>),
    TextResponseFormatJsonSchema(Box<models::TextResponseFormatJsonSchema>),
    ResponseFormatJsonObject(Box<models::ResponseFormatJsonObject>),
}

impl Default for TextResponseFormatConfiguration {
    fn default() -> Self {
        Self::ResponseFormatText(Default::default())
    }
}
/// The type of response format being defined. Always `text`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum TextResponseFormatConfigurationType {
    #[serde(rename = "text")]
    #[default]
    Text,
    #[serde(rename = "json_schema")]
    JsonSchema,
    #[serde(rename = "json_object")]
    JsonObject,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextResponseFormatJsonSchema {
    /// The type of response format being defined. Always `json_schema`.
    #[serde(rename = "type")]
    pub r#type: TextResponseFormatJsonSchemaType,
    /// The name of the response format. Must be a-z, A-Z, 0-9, or contain underscores and dashes, with a maximum length of 64.
    #[serde(rename = "name")]
    pub name: String,
    /// The schema for the response format, described as a JSON Schema object. Learn how to build JSON schemas [here](https://json-schema.org/).
    #[serde(rename = "schema")]
    pub schema: std::collections::HashMap<String, serde_json::Value>,
    /// A description of what the response format is for, used by the model to determine how to respond in the format.
    #[serde(rename = "description", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether to enable strict schema adherence when generating the output. If set to true, the model will always follow the exact schema defined in the `schema` field. Only a subset of JSON Schema is supported when `strict` is `true`. To learn more, read the [Structured Outputs guide](/docs/guides/structured-outputs).
    #[serde(
        rename = "strict",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub strict: Option<Option<bool>>,
}

impl TextResponseFormatJsonSchema {
    /// JSON Schema response format. Used to generate structured JSON responses. Learn more about [Structured Outputs](/docs/guides/structured-outputs).
    pub fn new(
        r#type: TextResponseFormatJsonSchemaType,
        name: String,
        schema: std::collections::HashMap<String, serde_json::Value>,
    ) -> TextResponseFormatJsonSchema {
        TextResponseFormatJsonSchema { r#type, name, schema, description: None, strict: None }
    }
}
/// The type of response format being defined. Always `json_schema`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum TextResponseFormatJsonSchemaType {
    #[serde(rename = "json_schema")]
    #[default]
    JsonSchema,
}

