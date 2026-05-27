use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompactResource {
    /// The unique identifier for the compacted response.
    #[serde(rename = "id")]
    pub id: String,
    /// The object type. Always `response.compaction`.
    #[serde(rename = "object")]
    pub object: CompactResponseObject,
    /// The compacted list of output items.
    #[serde(rename = "output")]
    pub output: Vec<models::ItemField>,
    /// Unix timestamp (in seconds) when the compacted conversation was created.
    #[serde(rename = "created_at")]
    pub created_at: i32,
    /// Token accounting for the compaction pass, including cached, reasoning, and total tokens.
    #[serde(rename = "usage")]
    pub usage: Box<models::ResponseUsage>,
}

impl CompactResource {
    pub fn new(
        id: String,
        object: CompactResponseObject,
        output: Vec<models::ItemField>,
        created_at: i32,
        usage: models::ResponseUsage,
    ) -> CompactResource {
        CompactResource { id, object, output, created_at, usage: Box::new(usage) }
    }
}
/// The object type. Always `response.compaction`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum CompactResponseObject {
    #[serde(rename = "response.compaction")]
    #[default]
    ResponseCompaction,
}

