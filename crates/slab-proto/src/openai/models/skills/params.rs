pub mod inline_skill_param {
    use crate::openai::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct InlineSkillParam {
        /// Defines an inline skill for this request.
        #[serde(rename = "type")]
        pub r#type: Type,
        /// The name of the skill.
        #[serde(rename = "name")]
        pub name: String,
        /// The description of the skill.
        #[serde(rename = "description")]
        pub description: String,
        /// Inline skill payload
        #[serde(rename = "source")]
        pub source: Box<models::InlineSkillSourceParam>,
    }

    impl InlineSkillParam {
        pub fn new(
            r#type: Type,
            name: String,
            description: String,
            source: models::InlineSkillSourceParam,
        ) -> InlineSkillParam {
            InlineSkillParam { r#type, name, description, source: Box::new(source) }
        }
    }
    /// Defines an inline skill for this request.
    #[derive(
        Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
    )]
    pub enum Type {
        #[serde(rename = "inline")]
        #[default]
        Inline,
    }
}

pub mod inline_skill_source_param {

    use serde::{Deserialize, Serialize};

    use super::media_type::MediaType;
    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct InlineSkillSourceParam {
        /// The type of the inline skill source. Must be `base64`.
        #[serde(rename = "type")]
        pub r#type: Type,
        /// The media type of the inline skill payload. Must be `application/zip`.
        #[serde(rename = "media_type")]
        pub media_type: MediaType,
        /// Base64-encoded skill zip bundle.
        #[serde(rename = "data")]
        pub data: String,
    }

    impl InlineSkillSourceParam {
        /// Inline skill payload
        pub fn new(r#type: Type, media_type: MediaType, data: String) -> InlineSkillSourceParam {
            InlineSkillSourceParam { r#type, media_type, data }
        }
    }
    /// The type of the inline skill source. Must be `base64`.
    #[derive(
        Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
    )]
    pub enum Type {
        #[serde(rename = "base64")]
        #[default]
        Base64,
    }

    // The media type of the inline skill payload. Must be `application/zip`.
}

pub mod local_skill_param {

    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct LocalSkillParam {
        /// The name of the skill.
        #[serde(rename = "name")]
        pub name: String,
        /// The description of the skill.
        #[serde(rename = "description")]
        pub description: String,
        /// The path to the directory containing the skill.
        #[serde(rename = "path")]
        pub path: String,
    }

    impl LocalSkillParam {
        pub fn new(name: String, description: String, path: String) -> LocalSkillParam {
            LocalSkillParam { name, description, path }
        }
    }
}

pub mod skill_reference_param {

    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SkillReferenceParam {
        /// References a skill created with the /v1/skills endpoint.
        #[serde(rename = "type")]
        pub r#type: Type,
        /// The ID of the referenced skill.
        #[serde(rename = "skill_id")]
        pub skill_id: String,
        /// Optional skill version. Use a positive integer or 'latest'. Omit for default.
        #[serde(rename = "version", skip_serializing_if = "Option::is_none")]
        pub version: Option<String>,
    }

    impl SkillReferenceParam {
        pub fn new(r#type: Type, skill_id: String) -> SkillReferenceParam {
            SkillReferenceParam { r#type, skill_id, version: None }
        }
    }
    /// References a skill created with the /v1/skills endpoint.
    #[derive(
        Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
    )]
    pub enum Type {
        #[serde(rename = "skill_reference")]
        #[default]
        SkillReference,
    }
}

pub mod media_type {

    use serde::{Deserialize, Serialize};

    #[derive(
        Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
    )]
    pub enum MediaType {
        #[serde(rename = "application/zip")]
        #[default]
        ApplicationSlashZip,
    }
}

pub use inline_skill_param::InlineSkillParam;
pub use inline_skill_source_param::InlineSkillSourceParam;
pub use local_skill_param::LocalSkillParam;
pub use media_type::MediaType;
pub use skill_reference_param::SkillReferenceParam;
