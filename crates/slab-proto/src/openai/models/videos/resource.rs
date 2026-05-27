use crate::openai::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoResource {
    /// Unique identifier for the video job.
    #[serde(rename = "id")]
    pub id: String,
    /// The object type, which is always `video`.
    #[serde(rename = "object")]
    pub object: VideoResourceObject,
    /// The video generation model that produced the job.
    #[serde(rename = "model")]
    pub model: Box<models::VideoModel>,
    /// Current lifecycle status of the video job.
    #[serde(rename = "status")]
    pub status: models::VideoStatus,
    /// Approximate completion percentage for the generation task.
    #[serde(rename = "progress")]
    pub progress: i32,
    /// Unix timestamp (seconds) for when the job was created.
    #[serde(rename = "created_at")]
    pub created_at: i32,
    /// Unix timestamp (seconds) for when the job completed, if finished.
    #[serde(rename = "completed_at", deserialize_with = "Option::deserialize")]
    pub completed_at: Option<i32>,
    /// Unix timestamp (seconds) for when the downloadable assets expire, if set.
    #[serde(rename = "expires_at", deserialize_with = "Option::deserialize")]
    pub expires_at: Option<i32>,
    /// The prompt that was used to generate the video.
    #[serde(rename = "prompt", deserialize_with = "Option::deserialize")]
    pub prompt: Option<String>,
    /// The resolution of the generated video.
    #[serde(rename = "size")]
    pub size: models::VideoSize,
    /// Duration of the generated clip in seconds. For extensions, this is the stitched total duration.
    #[serde(rename = "seconds")]
    pub seconds: String,
    /// Identifier of the source video if this video is a remix.
    #[serde(rename = "remixed_from_video_id", deserialize_with = "Option::deserialize")]
    pub remixed_from_video_id: Option<String>,
    /// Error payload that explains why generation failed, if applicable.
    #[serde(rename = "error", deserialize_with = "Option::deserialize")]
    pub error: Option<Box<models::Error2>>,
}

impl VideoResource {
    /// Structured information describing a generated video job.
    pub fn new(
        id: String,
        object: VideoResourceObject,
        model: models::VideoModel,
        status: models::VideoStatus,
        progress: i32,
        created_at: i32,
        completed_at: Option<i32>,
        expires_at: Option<i32>,
        prompt: Option<String>,
        size: models::VideoSize,
        seconds: String,
        remixed_from_video_id: Option<String>,
        error: Option<models::Error2>,
    ) -> VideoResource {
        VideoResource {
            id,
            object,
            model: Box::new(model),
            status,
            progress,
            created_at,
            completed_at,
            expires_at,
            prompt,
            size,
            seconds,
            remixed_from_video_id,
            error: error.map(Box::new),
        }
    }
}
/// The object type, which is always `video`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum VideoResourceObject {
    #[serde(rename = "video")]
    #[default]
    Video,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeletedVideoResource {
    /// The object type that signals the deletion response.
    #[serde(rename = "object")]
    pub object: DeletedVideoResourceObject,
    /// Indicates that the video resource was deleted.
    #[serde(rename = "deleted")]
    pub deleted: bool,
    /// Identifier of the deleted video.
    #[serde(rename = "id")]
    pub id: String,
}

impl DeletedVideoResource {
    /// Confirmation payload returned after deleting a video.
    pub fn new(
        object: DeletedVideoResourceObject,
        deleted: bool,
        id: String,
    ) -> DeletedVideoResource {
        DeletedVideoResource { object, deleted, id }
    }
}
/// The object type that signals the deletion response.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum DeletedVideoResourceObject {
    #[serde(rename = "video.deleted")]
    #[default]
    VideoDeleted,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoCharacterResource {
    /// Identifier for the character creation cameo.
    #[serde(rename = "id", deserialize_with = "Option::deserialize")]
    pub id: Option<String>,
    /// Display name for the character.
    #[serde(rename = "name", deserialize_with = "Option::deserialize")]
    pub name: Option<String>,
    /// Unix timestamp (in seconds) when the character was created.
    #[serde(rename = "created_at")]
    pub created_at: i32,
}

impl VideoCharacterResource {
    pub fn new(
        id: Option<String>,
        name: Option<String>,
        created_at: i32,
    ) -> VideoCharacterResource {
        VideoCharacterResource { id, name, created_at }
    }
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum VideoContentVariant {
    #[serde(rename = "video")]
    #[default]
    Video,
    #[serde(rename = "thumbnail")]
    Thumbnail,
    #[serde(rename = "spritesheet")]
    Spritesheet,
}

impl std::fmt::Display for VideoContentVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Video => write!(f, "video"),
            Self::Thumbnail => write!(f, "thumbnail"),
            Self::Spritesheet => write!(f, "spritesheet"),
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoListResource {
    /// The type of object returned, must be `list`.
    #[serde(rename = "object")]
    pub object: VideoListResourceObject,
    /// A list of items
    #[serde(rename = "data")]
    pub data: Vec<models::VideoResource>,
    /// The ID of the first item in the list.
    #[serde(rename = "first_id", deserialize_with = "Option::deserialize")]
    pub first_id: Option<String>,
    /// The ID of the last item in the list.
    #[serde(rename = "last_id", deserialize_with = "Option::deserialize")]
    pub last_id: Option<String>,
    /// Whether there are more items available.
    #[serde(rename = "has_more")]
    pub has_more: bool,
}

impl VideoListResource {
    pub fn new(
        object: VideoListResourceObject,
        data: Vec<models::VideoResource>,
        first_id: Option<String>,
        last_id: Option<String>,
        has_more: bool,
    ) -> VideoListResource {
        VideoListResource { object, data, first_id, last_id, has_more }
    }
}
/// The type of object returned, must be `list`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum VideoListResourceObject {
    #[serde(rename = "list")]
    #[default]
    List,
}
