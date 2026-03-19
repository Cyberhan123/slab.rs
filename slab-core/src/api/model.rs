use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::inference::JsonOptions;

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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriverHints {
    #[serde(default)]
    pub prefer_drivers: Vec<String>,
    #[serde(default)]
    pub avoid_drivers: Vec<String>,
    #[serde(default)]
    pub require_streaming: bool,
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
    pub driver_hints: DriverHints,
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
            driver_hints: DriverHints::default(),
            load_options: JsonOptions::default(),
            metadata: BTreeMap::default(),
        }
    }

    pub fn named(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn with_driver_hints(mut self, driver_hints: DriverHints) -> Self {
        self.driver_hints = driver_hints;
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
