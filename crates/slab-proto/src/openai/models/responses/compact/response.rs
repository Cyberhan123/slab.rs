use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompactResponseMethodPublicBody {
    #[serde(rename = "model", deserialize_with = "Option::deserialize")]
    pub model: Option<Box<models::ModelIdsCompaction>>,
    #[serde(
        rename = "input",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub input: Option<Option<Box<models::TokenCountsBodyInput>>>,
    /// The unique ID of the previous response to the model. Use this to create multi-turn conversations. Learn more about [conversation state](/docs/guides/conversation-state). Cannot be used in conjunction with `conversation`.
    #[serde(
        rename = "previous_response_id",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub previous_response_id: Option<Option<String>>,
    /// A system (or developer) message inserted into the model's context. When used along with `previous_response_id`, the instructions from a previous response will not be carried over to the next response. This makes it simple to swap out system (or developer) messages in new responses.
    #[serde(
        rename = "instructions",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub instructions: Option<Option<String>>,
    /// A key to use when reading from or writing to the prompt cache.
    #[serde(
        rename = "prompt_cache_key",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub prompt_cache_key: Option<Option<String>>,
    /// How long to retain a prompt cache entry created by this request.
    #[serde(
        rename = "prompt_cache_retention",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub prompt_cache_retention: Option<Option<models::PromptCacheRetentionEnum>>,
    /// The service tier to use for this request.
    #[serde(
        rename = "service_tier",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub service_tier: Option<Option<models::ServiceTierEnum>>,
}

impl CompactResponseMethodPublicBody {
    pub fn new(model: Option<models::ModelIdsCompaction>) -> CompactResponseMethodPublicBody {
        CompactResponseMethodPublicBody {
            model: model.map(Box::new),
            input: None,
            previous_response_id: None,
            instructions: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            service_tier: None,
        }
    }
}
