/*
 * OpenAI API - Merged type definitions
 */

use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateVideoEditJsonBody {
    /// Reference to the completed video to edit.
    #[serde(rename = "video")]
    pub video: Box<models::VideoReferenceInputParam>,
    /// Text prompt that describes how to edit the source video.
    #[serde(rename = "prompt")]
    pub prompt: String,
}

impl CreateVideoEditJsonBody {
    /// JSON parameters for editing an existing generated video.
    pub fn new(video: models::VideoReferenceInputParam, prompt: String) -> CreateVideoEditJsonBody {
        CreateVideoEditJsonBody { video: Box::new(video), prompt }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateVideoEditMultipartBodyVideo {
    /// Reference to the completed video to edit.
    String(std::path::PathBuf),
    VideoReferenceInputParam(Box<models::VideoReferenceInputParam>),
}

impl Default for CreateVideoEditMultipartBodyVideo {
    fn default() -> Self {
        Self::String(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateVideoExtendJsonBody {
    /// Reference to the completed video to extend.
    #[serde(rename = "video")]
    pub video: Box<models::VideoReferenceInputParam>,
    /// Updated text prompt that directs the extension generation.
    #[serde(rename = "prompt")]
    pub prompt: String,
    /// Length of the newly generated extension segment in seconds (allowed values: 4, 8, 12, 16, 20).
    #[serde(rename = "seconds")]
    pub seconds: models::VideoSeconds,
}

impl CreateVideoExtendJsonBody {
    /// JSON parameters for extending an existing generated video.
    pub fn new(
        video: models::VideoReferenceInputParam,
        prompt: String,
        seconds: models::VideoSeconds,
    ) -> CreateVideoExtendJsonBody {
        CreateVideoExtendJsonBody { video: Box::new(video), prompt, seconds }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateVideoExtendMultipartBodyVideo {
    VideoReferenceInputParam(Box<models::VideoReferenceInputParam>),
    /// Reference to the completed video to extend.
    String(std::path::PathBuf),
}

impl Default for CreateVideoExtendMultipartBodyVideo {
    fn default() -> Self {
        Self::VideoReferenceInputParam(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateVideoJsonBody {
    /// Text prompt that describes the video to generate.
    #[serde(rename = "prompt")]
    pub prompt: String,
    /// The video generation model to use (allowed values: sora-2, sora-2-pro). Defaults to `sora-2`.
    #[serde(rename = "model", skip_serializing_if = "Option::is_none")]
    pub model: Option<Box<models::VideoModel>>,
    /// Optional reference object that guides generation. Provide exactly one of `image_url` or `file_id`.
    #[serde(rename = "input_reference", skip_serializing_if = "Option::is_none")]
    pub input_reference: Option<Box<models::ImageRefParam2>>,
    /// Clip duration in seconds (allowed values: 4, 8, 12). Defaults to 4 seconds.
    #[serde(rename = "seconds", skip_serializing_if = "Option::is_none")]
    pub seconds: Option<models::VideoSeconds>,
    /// Output resolution formatted as width x height (allowed values: 720x1280, 1280x720, 1024x1792, 1792x1024). Defaults to 720x1280.
    #[serde(rename = "size", skip_serializing_if = "Option::is_none")]
    pub size: Option<models::VideoSize>,
}

impl CreateVideoJsonBody {
    /// JSON parameters for creating a new video generation job.
    pub fn new(prompt: String) -> CreateVideoJsonBody {
        CreateVideoJsonBody {
            prompt,
            model: None,
            input_reference: None,
            seconds: None,
            size: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateVideoMultipartBodyInputReference {
    /// Optional reference asset upload or reference object that guides generation.
    String(std::path::PathBuf),
    ImageRefParam2(Box<models::ImageRefParam2>),
}

impl Default for CreateVideoMultipartBodyInputReference {
    fn default() -> Self {
        Self::String(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateVideoRemixBody {
    /// Updated text prompt that directs the remix generation.
    #[serde(rename = "prompt")]
    pub prompt: String,
}

impl CreateVideoRemixBody {
    /// Parameters for remixing an existing generated video.
    pub fn new(prompt: String) -> CreateVideoRemixBody {
        CreateVideoRemixBody { prompt }
    }
}
