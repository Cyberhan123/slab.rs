use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionDeleted {
    /// The type of object being deleted.
    #[serde(rename = "object")]
    pub object: DeletedChatCompletionObject,
    /// The ID of the chat completion that was deleted.
    #[serde(rename = "id")]
    pub id: String,
    /// Whether the chat completion was deleted.
    #[serde(rename = "deleted")]
    pub deleted: bool,
}

impl ChatCompletionDeleted {
    pub fn new(object: DeletedChatCompletionObject, id: String, deleted: bool) -> ChatCompletionDeleted {
        ChatCompletionDeleted { object, id, deleted }
    }
}
/// The type of object being deleted.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum DeletedChatCompletionObject {
    #[serde(rename = "chat.completion.deleted")]
    #[default]
    ChatCompletionDeleted,
}

