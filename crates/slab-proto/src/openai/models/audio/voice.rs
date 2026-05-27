use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceConsentDeletedResource {
    /// The consent recording identifier.
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "object")]
    pub object: VoiceConsentDeletedResourceObject,
    #[serde(rename = "deleted")]
    pub deleted: bool,
}

impl VoiceConsentDeletedResource {
    pub fn new(
        id: String,
        object: VoiceConsentDeletedResourceObject,
        deleted: bool,
    ) -> VoiceConsentDeletedResource {
        VoiceConsentDeletedResource { id, object, deleted }
    }
}
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum VoiceConsentDeletedResourceObject {
    #[serde(rename = "audio.voice_consent")]
    #[default]
    AudioVoiceConsent,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceConsentListResource {
    #[serde(rename = "object")]
    pub object: VoiceConsentListResourceObject,
    #[serde(rename = "data")]
    pub data: Vec<models::VoiceConsentResource>,
    #[serde(rename = "has_more")]
    pub has_more: bool,
    #[serde(
        rename = "first_id",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub first_id: Option<Option<String>>,
    #[serde(
        rename = "last_id",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_id: Option<Option<String>>,
}

impl VoiceConsentListResource {
    pub fn new(
        object: VoiceConsentListResourceObject,
        data: Vec<models::VoiceConsentResource>,
        has_more: bool,
    ) -> VoiceConsentListResource {
        VoiceConsentListResource { object, data, has_more, first_id: None, last_id: None }
    }
}
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum VoiceConsentListResourceObject {
    #[serde(rename = "list")]
    #[default]
    List,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceConsentResource {
    /// The object type, which is always `audio.voice_consent`.
    #[serde(rename = "object")]
    pub object: VoiceConsentResourceObject,
    /// The consent recording identifier.
    #[serde(rename = "id")]
    pub id: String,
    /// The label provided when the consent recording was uploaded.
    #[serde(rename = "name")]
    pub name: String,
    /// The BCP 47 language tag for the consent phrase (for example, `en-US`).
    #[serde(rename = "language")]
    pub language: String,
    /// The Unix timestamp (in seconds) for when the consent recording was created.
    #[serde(rename = "created_at")]
    pub created_at: i32,
}

impl VoiceConsentResource {
    /// A consent recording used to authorize creation of a custom voice.
    pub fn new(
        object: VoiceConsentResourceObject,
        id: String,
        name: String,
        language: String,
        created_at: i32,
    ) -> VoiceConsentResource {
        VoiceConsentResource { object, id, name, language, created_at }
    }
}
/// The object type, which is always `audio.voice_consent`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum VoiceConsentResourceObject {
    #[serde(rename = "audio.voice_consent")]
    #[default]
    AudioVoiceConsent,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceIdsOrCustomVoice {
    /// The custom voice ID, e.g. `voice_1234`.
    #[serde(rename = "id")]
    pub id: String,
}

impl VoiceIdsOrCustomVoice {
    /// A built-in voice name or a custom voice reference.
    pub fn new(id: String) -> VoiceIdsOrCustomVoice {
        VoiceIdsOrCustomVoice { id }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceIdsOrCustomVoiceAnyOf {
    /// The custom voice ID, e.g. `voice_1234`.
    #[serde(rename = "id")]
    pub id: String,
}

impl VoiceIdsOrCustomVoiceAnyOf {
    /// Custom voice reference.
    pub fn new(id: String) -> VoiceIdsOrCustomVoiceAnyOf {
        VoiceIdsOrCustomVoiceAnyOf { id }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceIdsShared {}

impl VoiceIdsShared {
    pub fn new() -> VoiceIdsShared {
        VoiceIdsShared {}
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceResource {
    /// The object type, which is always `audio.voice`.
    #[serde(rename = "object")]
    pub object: VoiceResourceObject,
    /// The voice identifier, which can be referenced in API endpoints.
    #[serde(rename = "id")]
    pub id: String,
    /// The name of the voice.
    #[serde(rename = "name")]
    pub name: String,
    /// The Unix timestamp (in seconds) for when the voice was created.
    #[serde(rename = "created_at")]
    pub created_at: i32,
}

impl VoiceResource {
    /// A custom voice that can be used for audio output.
    pub fn new(
        object: VoiceResourceObject,
        id: String,
        name: String,
        created_at: i32,
    ) -> VoiceResource {
        VoiceResource { object, id, name, created_at }
    }
}
/// The object type, which is always `audio.voice`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum VoiceResourceObject {
    #[serde(rename = "audio.voice")]
    #[default]
    AudioVoice,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct UpdateVoiceConsentRequest {
    /// The updated label for this consent recording.
    #[serde(rename = "name")]
    pub name: String,
}

impl UpdateVoiceConsentRequest {
    pub fn new(name: String) -> UpdateVoiceConsentRequest {
        UpdateVoiceConsentRequest { name }
    }
}
