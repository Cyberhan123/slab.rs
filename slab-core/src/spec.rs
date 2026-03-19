use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub type JsonOptions = BTreeMap<String, serde_json::Value>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ModelFamily {
    Llama,
    Whisper,
    Diffusion,
    Onnx,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    TextGeneration,
    AudioTranscription,
    ImageGeneration,
    ImageEmbedding,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    TextGeneration,
    AudioTranscription,
    ImageGeneration,
    ImageEmbedding,
}

impl TaskKind {
    pub fn capability(self) -> Capability {
        match self {
            Self::TextGeneration => Capability::TextGeneration,
            Self::AudioTranscription => Capability::AudioTranscription,
            Self::ImageGeneration => Capability::ImageGeneration,
            Self::ImageEmbedding => Capability::ImageEmbedding,
        }
    }
}

impl From<Capability> for TaskKind {
    fn from(value: Capability) -> Self {
        match value {
            Capability::TextGeneration => Self::TextGeneration,
            Capability::AudioTranscription => Self::AudioTranscription,
            Capability::ImageGeneration => Self::ImageGeneration,
            Capability::ImageEmbedding => Self::ImageEmbedding,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ResourcePreference {
    Balanced,
    Cpu,
    Gpu,
    LowMemory,
    HighThroughput,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DispatchHints {
    #[serde(default)]
    pub prefer_drivers: Vec<String>,
    #[serde(default)]
    pub avoid_drivers: Vec<String>,
    #[serde(default)]
    pub require_streaming: bool,
    #[serde(default)]
    pub resource_preference: Option<ResourcePreference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModelSource {
    LocalPath {
        path: PathBuf,
    },
    LocalArtifacts {
        #[serde(default)]
        files: BTreeMap<String, PathBuf>,
    },
    HuggingFace {
        repo_id: String,
        #[serde(default)]
        revision: Option<String>,
        #[serde(default)]
        files: BTreeMap<String, PathBuf>,
    },
}

impl ModelSource {
    pub fn primary_path(&self) -> Option<&Path> {
        match self {
            Self::LocalPath { path } => Some(path.as_path()),
            Self::LocalArtifacts { files } | Self::HuggingFace { files, .. } => files
                .get("model")
                .or_else(|| files.values().next())
                .map(PathBuf::as_path),
        }
    }

    pub fn artifact(&self, name: &str) -> Option<&Path> {
        match self {
            Self::LocalPath { path } => (name == "model").then_some(path.as_path()),
            Self::LocalArtifacts { files } | Self::HuggingFace { files, .. } => {
                files.get(name).map(PathBuf::as_path)
            }
        }
    }

    pub fn files(&self) -> BTreeMap<String, PathBuf> {
        match self {
            Self::LocalPath { path } => {
                let mut files = BTreeMap::new();
                files.insert("model".to_owned(), path.clone());
                files
            }
            Self::LocalArtifacts { files } | Self::HuggingFace { files, .. } => files.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSpec {
    #[serde(default)]
    pub id: Option<String>,
    pub family: ModelFamily,
    pub capability: Capability,
    pub source: ModelSource,
    #[serde(default)]
    pub dispatch: DispatchHints,
    #[serde(default)]
    pub load_options: JsonOptions,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl ModelSpec {
    pub fn new(family: ModelFamily, capability: Capability, source: ModelSource) -> Self {
        Self {
            id: None,
            family,
            capability,
            source,
            dispatch: DispatchHints::default(),
            load_options: JsonOptions::default(),
            metadata: BTreeMap::default(),
        }
    }

    pub fn task_kind(&self) -> TaskKind {
        self.capability.into()
    }

    pub fn named(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn with_dispatch(mut self, dispatch: DispatchHints) -> Self {
        self.dispatch = dispatch;
        self
    }

    pub fn with_load_option(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.load_options.insert(key.into(), value.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TextGenerationRequest {
    pub prompt: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub session_key: Option<String>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub options: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TextGenerationResponse {
    pub text: String,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub tokens_used: Option<u32>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TextGenerationChunk {
    pub delta: String,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub metadata: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AudioTranscriptionRequest {
    pub audio_path: PathBuf,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub options: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AudioTranscriptionResponse {
    pub text: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub guidance: f32,
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub options: JsonOptions,
}

impl Default for ImageGenerationRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: None,
            width: 512,
            height: 512,
            steps: 20,
            guidance: 7.5,
            seed: None,
            options: JsonOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ImageGenerationResponse {
    #[serde(default)]
    pub images: Vec<Vec<u8>>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ImageEmbeddingRequest {
    #[serde(default)]
    pub image: Vec<u8>,
    #[serde(default)]
    pub options: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ImageEmbeddingResponse {
    #[serde(default)]
    pub embedding: Vec<f32>,
    #[serde(default)]
    pub metadata: JsonOptions,
}
