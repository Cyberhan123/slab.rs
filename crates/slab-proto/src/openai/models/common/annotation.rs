use crate::openai::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Annotation {
    #[serde(rename = "file_citation")]
    FileCitationBody(Box<models::FileCitationBody>),
    #[serde(rename = "url_citation")]
    UrlCitationBody(Box<models::UrlCitationBody>),
    #[serde(rename = "container_file_citation")]
    ContainerFileCitationBody(Box<models::ContainerFileCitationBody>),
    #[serde(rename = "file_path")]
    FilePath(Box<models::FilePath>),
}

impl Default for Annotation {
    fn default() -> Self {
        Self::FileCitationBody(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrlAnnotation {
    /// URL referenced by the annotation.
    #[serde(rename = "source")]
    pub source: Box<models::UrlAnnotationSource>,
}

impl UrlAnnotation {
    /// Annotation that references a URL.
    pub fn new(source: models::UrlAnnotationSource) -> UrlAnnotation {
        UrlAnnotation { source: Box::new(source) }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrlAnnotationSource {
    /// UrlAnnotationType discriminator that is always `url`.
    #[serde(rename = "type")]
    pub r#type: UrlAnnotationSourceType,
    /// URL referenced by the annotation.
    #[serde(rename = "url")]
    pub url: String,
}

impl UrlAnnotationSource {
    /// URL backing an annotation entry.
    pub fn new(r#type: UrlAnnotationSourceType, url: String) -> UrlAnnotationSource {
        UrlAnnotationSource { r#type, url }
    }
}
/// Type discriminator that is always `url`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum UrlAnnotationSourceType {
    #[serde(rename = "url")]
    #[default]
    Url,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrlCitationBody {
    /// The URL of the web resource.
    #[serde(rename = "url")]
    pub url: String,
    /// The index of the first character of the URL citation in the message.
    #[serde(rename = "start_index")]
    pub start_index: i32,
    /// The index of the last character of the URL citation in the message.
    #[serde(rename = "end_index")]
    pub end_index: i32,
    /// The title of the web resource.
    #[serde(rename = "title")]
    pub title: String,
}

impl UrlCitationBody {
    /// A citation for a web resource used to generate a model response.
    pub fn new(url: String, start_index: i32, end_index: i32, title: String) -> UrlCitationBody {
        UrlCitationBody { url, start_index, end_index, title }
    }
}
