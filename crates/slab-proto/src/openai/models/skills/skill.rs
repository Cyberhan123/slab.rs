pub mod skill_resource {
    
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SkillResource {
        /// Unique identifier for the skill.
        #[serde(rename = "id")]
        pub id: String,
        /// The object type, which is `skill`.
        #[serde(rename = "object")]
        pub object: Object,
        /// Name of the skill.
        #[serde(rename = "name")]
        pub name: String,
        /// Description of the skill.
        #[serde(rename = "description")]
        pub description: String,
        /// Unix timestamp (seconds) for when the skill was created.
        #[serde(rename = "created_at")]
        pub created_at: i32,
        /// Default version for the skill.
        #[serde(rename = "default_version")]
        pub default_version: String,
        /// Latest version for the skill.
        #[serde(rename = "latest_version")]
        pub latest_version: String,
    }

    impl SkillResource {
        pub fn new(
            id: String,
            object: Object,
            name: String,
            description: String,
            created_at: i32,
            default_version: String,
            latest_version: String,
        ) -> SkillResource {
            SkillResource {
                id,
                object,
                name,
                description,
                created_at,
                default_version,
                latest_version,
            }
        }
    }
    /// The object type, which is `skill`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Object {
        #[serde(rename = "skill")]
        #[default]
        Skill,
    }

    
}

pub mod skill_list_resource {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SkillListResource {
        /// The type of object returned, must be `list`.
        #[serde(rename = "object")]
        pub object: Object,
        /// A list of items
        #[serde(rename = "data")]
        pub data: Vec<models::SkillResource>,
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

    impl SkillListResource {
        pub fn new(
            object: Object,
            data: Vec<models::SkillResource>,
            first_id: Option<String>,
            last_id: Option<String>,
            has_more: bool,
        ) -> SkillListResource {
            SkillListResource { object, data, first_id, last_id, has_more }
        }
    }
    /// The type of object returned, must be `list`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Object {
        #[serde(rename = "list")]
        #[default]
        List,
    }

    
}

pub mod skill_version_resource {
    
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SkillVersionResource {
        /// The object type, which is `skill.version`.
        #[serde(rename = "object")]
        pub object: Object,
        /// Unique identifier for the skill version.
        #[serde(rename = "id")]
        pub id: String,
        /// Identifier of the skill for this version.
        #[serde(rename = "skill_id")]
        pub skill_id: String,
        /// Version number for this skill.
        #[serde(rename = "version")]
        pub version: String,
        /// Unix timestamp (seconds) for when the version was created.
        #[serde(rename = "created_at")]
        pub created_at: i32,
        /// Name of the skill version.
        #[serde(rename = "name")]
        pub name: String,
        /// Description of the skill version.
        #[serde(rename = "description")]
        pub description: String,
    }

    impl SkillVersionResource {
        pub fn new(
            object: Object,
            id: String,
            skill_id: String,
            version: String,
            created_at: i32,
            name: String,
            description: String,
        ) -> SkillVersionResource {
            SkillVersionResource { object, id, skill_id, version, created_at, name, description }
        }
    }
    /// The object type, which is `skill.version`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Object {
        #[serde(rename = "skill.version")]
        #[default]
        SkillVersion,
    }

    
}

pub mod skill_version_list_resource {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SkillVersionListResource {
        /// The type of object returned, must be `list`.
        #[serde(rename = "object")]
        pub object: Object,
        /// A list of items
        #[serde(rename = "data")]
        pub data: Vec<models::SkillVersionResource>,
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

    impl SkillVersionListResource {
        pub fn new(
            object: Object,
            data: Vec<models::SkillVersionResource>,
            first_id: Option<String>,
            last_id: Option<String>,
            has_more: bool,
        ) -> SkillVersionListResource {
            SkillVersionListResource { object, data, first_id, last_id, has_more }
        }
    }
    /// The type of object returned, must be `list`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Object {
        #[serde(rename = "list")]
        #[default]
        List,
    }

    
}

pub mod deleted_skill_resource {
    
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct DeletedSkillResource {
        #[serde(rename = "object")]
        pub object: Object,
        #[serde(rename = "deleted")]
        pub deleted: bool,
        #[serde(rename = "id")]
        pub id: String,
    }

    impl DeletedSkillResource {
        pub fn new(object: Object, deleted: bool, id: String) -> DeletedSkillResource {
            DeletedSkillResource { object, deleted, id }
        }
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Object {
        #[serde(rename = "skill.deleted")]
        #[default]
        SkillDeleted,
    }

    
}

pub mod deleted_skill_version_resource {
    
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct DeletedSkillVersionResource {
        #[serde(rename = "object")]
        pub object: Object,
        #[serde(rename = "deleted")]
        pub deleted: bool,
        #[serde(rename = "id")]
        pub id: String,
        /// The deleted skill version.
        #[serde(rename = "version")]
        pub version: String,
    }

    impl DeletedSkillVersionResource {
        pub fn new(
            object: Object,
            deleted: bool,
            id: String,
            version: String,
        ) -> DeletedSkillVersionResource {
            DeletedSkillVersionResource { object, deleted, id, version }
        }
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Object {
        #[serde(rename = "skill.version.deleted")]
        #[default]
        SkillVersionDeleted,
    }

    
}

pub use deleted_skill_resource::DeletedSkillResource;
pub use deleted_skill_version_resource::DeletedSkillVersionResource;
pub use skill_list_resource::SkillListResource;
pub use skill_resource::SkillResource;
pub use skill_version_list_resource::SkillVersionListResource;
pub use skill_version_resource::SkillVersionResource;
