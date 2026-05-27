/*
 * OpenAI API - Merged type definitions
 */

use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateSkillBody {
    #[serde(rename = "files")]
    pub files: Box<models::CreateSkillBodyFiles>,
}

impl CreateSkillBody {
    /// Uploads a skill either as a directory (multipart `files[]`) or as a single zip file.
    pub fn new(files: models::CreateSkillBodyFiles) -> CreateSkillBody {
        CreateSkillBody { files: Box::new(files) }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateSkillBodyFiles {
    /// Skill files to upload (directory upload) or a single zip file.
    ArrayPathBuf(Vec<std::path::PathBuf>),
    /// Skill zip file to upload.
    String(std::path::PathBuf),
}

impl Default for CreateSkillBodyFiles {
    fn default() -> Self {
        Self::ArrayPathBuf(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateSkillVersionBody {
    #[serde(rename = "files")]
    pub files: Box<models::CreateSkillBodyFiles>,
    /// Whether to set this version as the default.
    #[serde(rename = "default", skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
}

impl CreateSkillVersionBody {
    /// Uploads a new immutable version of a skill.
    pub fn new(files: models::CreateSkillBodyFiles) -> CreateSkillVersionBody {
        CreateSkillVersionBody { files: Box::new(files), default: None }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct SetDefaultSkillVersionBody {
    /// The skill version number to set as default.
    #[serde(rename = "default_version")]
    pub default_version: String,
}

impl SetDefaultSkillVersionBody {
    /// Updates the default version pointer for a skill.
    pub fn new(default_version: String) -> SetDefaultSkillVersionBody {
        SetDefaultSkillVersionBody { default_version }
    }
}
