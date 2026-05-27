use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingRequest {
    #[serde(rename = "input")]
    pub input: serde_json::Value,
    #[serde(rename = "model")]
    pub model: CreateEmbeddingRequestModel,
    #[serde(rename = "encoding_format", skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<EmbeddingEncodingFormat>,
    #[serde(rename = "dimensions", skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<i32>,
    #[serde(rename = "user", skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateEmbeddingRequestModel {
    ModelEnum(CreateEmbeddingRequestModelEnum),
    StringValue(String),
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum CreateEmbeddingRequestModelEnum {
    #[serde(rename = "text-embedding-ada-002")]
    TextEmbeddingAda002,
    #[serde(rename = "text-embedding-3-small")]
    #[default]
    TextEmbedding3Small,
    #[serde(rename = "text-embedding-3-large")]
    TextEmbedding3Large,
}

impl Default for CreateEmbeddingRequestModel {
    fn default() -> Self {
        Self::StringValue(String::new())
    }
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum EmbeddingEncodingFormat {
    #[serde(rename = "float")]
    #[default]
    Float,
    #[serde(rename = "base64")]
    Base64,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingResponse {
    #[serde(rename = "object")]
    pub object: EmbeddingResponseObject,
    #[serde(rename = "model")]
    pub model: String,
    #[serde(rename = "data")]
    pub data: Vec<Embedding>,
    #[serde(rename = "usage")]
    pub usage: CreateEmbeddingResponseUsage,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum EmbeddingResponseObject {
    #[serde(rename = "list")]
    #[default]
    List,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Embedding {
    #[serde(rename = "index")]
    pub index: i32,
    #[serde(rename = "object")]
    pub object: EmbeddingObject,
    #[serde(rename = "embedding")]
    pub embedding: Vec<f32>,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum EmbeddingObject {
    #[serde(rename = "embedding")]
    #[default]
    Embedding,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateEmbeddingResponseUsage {
    #[serde(rename = "prompt_tokens")]
    pub prompt_tokens: i32,
    #[serde(rename = "total_tokens")]
    pub total_tokens: i32,
}
