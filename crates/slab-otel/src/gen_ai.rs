use serde::Serialize;
use serde_json::{Value, json};
use slab_types::chat::{ConversationMessage, ConversationMessageContent};

pub const ATTR_PROVIDER_NAME: &str = "gen_ai.provider.name";
pub const ATTR_OPERATION_NAME: &str = "gen_ai.operation.name";
pub const ATTR_REQUEST_MODEL: &str = "gen_ai.request.model";
pub const ATTR_RESPONSE_MODEL: &str = "gen_ai.response.model";
pub const ATTR_RESPONSE_ID: &str = "gen_ai.response.id";
pub const ATTR_USAGE_INPUT_TOKENS: &str = "gen_ai.usage.input_tokens";
pub const ATTR_USAGE_OUTPUT_TOKENS: &str = "gen_ai.usage.output_tokens";
pub const ATTR_RESPONSE_FINISH_REASONS: &str = "gen_ai.response.finish_reasons";
pub const ATTR_INPUT_MESSAGES: &str = "gen_ai.input.messages";
pub const ATTR_OUTPUT_MESSAGES: &str = "gen_ai.output.messages";
pub const ATTR_TOOL_DEFINITIONS: &str = "gen_ai.tool.definitions";
pub const ATTR_TOOL_CALL_ID: &str = "gen_ai.tool.call.id";
pub const ATTR_TOOL_NAME: &str = "gen_ai.tool.name";
pub const ATTR_TOOL_TYPE: &str = "gen_ai.tool.type";

pub const OPERATION_CHAT: &str = "chat";
pub const OPERATION_EXECUTE_TOOL: &str = "execute_tool";
pub const TOOL_TYPE_FUNCTION: &str = "function";

pub fn messages_attribute(
    messages: &[ConversationMessage],
    capture_content: bool,
) -> Option<Value> {
    capture_content
        .then(|| Value::Array(messages.iter().map(message_attribute).collect::<Vec<_>>()))
}

pub fn tool_definitions_attribute<T>(tools: &[T], capture_content: bool) -> Option<Value>
where
    T: Serialize,
{
    capture_content
        .then(|| serde_json::to_value(tools).unwrap_or_else(|_| Value::Array(Vec::new())))
}

pub fn finish_reasons_attribute(reasons: impl IntoIterator<Item = impl Into<String>>) -> Value {
    Value::Array(reasons.into_iter().map(|reason| Value::String(reason.into())).collect())
}

fn message_attribute(message: &ConversationMessage) -> Value {
    let mut value = json!({
        "role": message.role,
        "content": content_attribute(&message.content),
    });
    let object = value.as_object_mut().expect("message value is object");
    if let Some(name) = message.name.as_deref() {
        object.insert("name".to_owned(), Value::String(name.to_owned()));
    }
    if let Some(tool_call_id) = message.tool_call_id.as_deref() {
        object.insert("tool_call_id".to_owned(), Value::String(tool_call_id.to_owned()));
    }
    if !message.tool_calls.is_empty() {
        object.insert(
            "tool_calls".to_owned(),
            serde_json::to_value(&message.tool_calls).unwrap_or_else(|_| Value::Array(Vec::new())),
        );
    }
    value
}

fn content_attribute(content: &ConversationMessageContent) -> Value {
    match content {
        ConversationMessageContent::Text(text) => Value::String(text.clone()),
        ConversationMessageContent::Parts(parts) => {
            serde_json::to_value(parts).unwrap_or_else(|_| Value::Array(Vec::new()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_types::chat::{
        ConversationMessageContent, ConversationToolCall, ConversationToolFunction,
    };

    #[test]
    fn messages_attribute_is_absent_when_content_capture_is_disabled() {
        let messages = vec![ConversationMessage {
            role: "user".to_owned(),
            content: ConversationMessageContent::Text("secret".to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }];

        assert!(messages_attribute(&messages, false).is_none());
    }

    #[test]
    fn messages_attribute_includes_content_when_enabled() {
        let messages = vec![ConversationMessage {
            role: "assistant".to_owned(),
            content: ConversationMessageContent::Text("done".to_owned()),
            name: Some("assistant-1".to_owned()),
            tool_call_id: None,
            tool_calls: vec![ConversationToolCall {
                id: Some("call_1".to_owned()),
                r#type: "function".to_owned(),
                function: ConversationToolFunction {
                    name: "read_file".to_owned(),
                    arguments: "{}".to_owned(),
                },
            }],
        }];

        let value = messages_attribute(&messages, true).expect("messages");

        assert_eq!(value[0]["role"], "assistant");
        assert_eq!(value[0]["content"], "done");
        assert_eq!(value[0]["name"], "assistant-1");
        assert_eq!(value[0]["tool_calls"][0]["function"]["name"], "read_file");
    }

    #[test]
    fn tool_definitions_are_absent_when_content_capture_is_disabled() {
        let tools = [json!({ "name": "read_file" })];

        assert!(tool_definitions_attribute(&tools, false).is_none());
        assert_eq!(
            tool_definitions_attribute(&tools, true).expect("tools")[0]["name"],
            "read_file"
        );
    }
}
