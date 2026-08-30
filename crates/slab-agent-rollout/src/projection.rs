//! Bidirectional projection between the two message forms captured by rollout.
//!
//! Rollout stores **both** [`TurnItem`] (full-fidelity UI artifact, from
//! `ItemCompleted`) and [`ConversationMessage`] (LLM-grade, from
//! `TurnContext::MessageAppend`). The two forms overlap for plain user/assistant
//! text but diverge for tool calls: a tool *result* is a `ConversationMessage`
//! with `role:"tool"` + `tool_call_id`, while a tool *invocation* is a
//! `TurnItem::CommandExecution` / `McpToolCall`. This module handles the
//! faithful subset that round-trips.

use slab_agent::protocol::TurnItem;
use slab_agent::protocol::item::UserMessageContent;
use slab_types::chat::{ConversationContentPart, ConversationMessage, ConversationMessageContent};

/// Project a [`TurnItem`] to its LLM-visible [`ConversationMessage`] form, when
/// one exists.
///
/// Returns `None` for UI-only artifacts (`Reasoning`, `CommandExecution`,
/// `FileChange`, `McpToolCall`, `WebSearch`, `ImageView`, `Plan`) — their
/// LLM-visible counterparts flow through `TurnContext::MessageAppend` (tool
/// results) and are not derivable from the UI item alone.
pub fn turn_item_to_conversation_message(item: &TurnItem) -> Option<ConversationMessage> {
    match item {
        TurnItem::UserMessage { content, .. } => {
            let parts = content.iter().map(user_content_to_part).collect::<Vec<_>>();
            let conv_content = if parts.len() == 1 {
                match parts.into_iter().next() {
                    // A lone text part collapses back to the plain-text form so we
                    // round-trip byte-identically to the original ConversationMessage.
                    Some(ConversationContentPart::InputText { text })
                    | Some(ConversationContentPart::Text { text }) => {
                        ConversationMessageContent::Text(text)
                    }
                    other => ConversationMessageContent::Parts(other.into_iter().collect()),
                }
            } else if parts.is_empty() {
                ConversationMessageContent::Text(String::new())
            } else {
                ConversationMessageContent::Parts(parts)
            };
            Some(ConversationMessage {
                role: "user".to_owned(),
                content: conv_content,
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            })
        }
        TurnItem::AgentMessage { text, .. } => Some(ConversationMessage {
            role: "assistant".to_owned(),
            content: ConversationMessageContent::Text(text.clone()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }),
        TurnItem::Reasoning { .. }
        | TurnItem::CommandExecution { .. }
        | TurnItem::FileChange { .. }
        | TurnItem::McpToolCall { .. }
        | TurnItem::ToolCall { .. }
        | TurnItem::WebSearch { .. }
        | TurnItem::ImageView { .. }
        | TurnItem::Plan { .. } => None,
    }
}

/// Project a [`ConversationMessage`] back to its [`TurnItem`] UI form, when the
/// message is a plain user/assistant turn that round-trips losslessly.
///
/// Tool results (`role:"tool"`) and assistant turns carrying `tool_calls` have no
/// single `TurnItem` representation and return `None` — they remain in their
/// `ConversationMessage` form in the rollout file.
pub fn conversation_message_to_turn_item(msg: &ConversationMessage) -> Option<TurnItem> {
    match msg.role.as_str() {
        "user" => {
            let content = message_content_to_user_parts(&msg.content);
            Some(TurnItem::UserMessage { id: uuid::Uuid::new_v4().to_string(), content })
        }
        "assistant" if msg.tool_calls.is_empty() => {
            let text = match &msg.content {
                ConversationMessageContent::Text(t) => t.clone(),
                ConversationMessageContent::Parts(parts) => parts
                    .iter()
                    .filter_map(|p| match p {
                        ConversationContentPart::Text { text }
                        | ConversationContentPart::InputText { text }
                        | ConversationContentPart::OutputText { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(""),
            };
            Some(TurnItem::AgentMessage { id: uuid::Uuid::new_v4().to_string(), text })
        }
        _ => None,
    }
}

/// `true` for tool-result messages (`role:"tool"` carrying a `tool_call_id`).
pub fn is_tool_result(msg: &ConversationMessage) -> bool {
    msg.role == "tool" && msg.tool_call_id.is_some()
}

fn user_content_to_part(content: &UserMessageContent) -> ConversationContentPart {
    match content {
        UserMessageContent::Text { text } => {
            ConversationContentPart::InputText { text: text.clone() }
        }
        UserMessageContent::Image { image_url, base64, mime_type } => {
            // Prefer an explicit URL; otherwise synthesize a data URL from base64.
            let url = image_url.clone().or_else(|| {
                base64.as_ref().map(|b64| {
                    format!(
                        "data:{};base64,{}",
                        mime_type.as_deref().unwrap_or("application/octet-stream"),
                        b64
                    )
                })
            });
            ConversationContentPart::Image {
                image_url: url,
                mime_type: mime_type.clone(),
                detail: None,
            }
        }
    }
}

fn message_content_to_user_parts(content: &ConversationMessageContent) -> Vec<UserMessageContent> {
    match content {
        ConversationMessageContent::Text(t) => vec![UserMessageContent::Text { text: t.clone() }],
        ConversationMessageContent::Parts(parts) => parts
            .iter()
            .filter_map(|p| match p {
                ConversationContentPart::Text { text }
                | ConversationContentPart::InputText { text }
                | ConversationContentPart::OutputText { text } => {
                    Some(UserMessageContent::Text { text: text.clone() })
                }
                ConversationContentPart::Image { image_url, mime_type, .. } => {
                    Some(UserMessageContent::Image {
                        image_url: image_url.clone(),
                        base64: None,
                        mime_type: mime_type.clone(),
                    })
                }
                ConversationContentPart::ToolResult { .. }
                | ConversationContentPart::Json { .. }
                | ConversationContentPart::Refusal { .. } => None,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_agent::protocol::item::ReasoningText;

    #[test]
    fn agent_message_projects_to_assistant() {
        let item = TurnItem::AgentMessage { id: "a1".to_owned(), text: "hi".to_owned() };
        let msg = turn_item_to_conversation_message(&item).unwrap();
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, ConversationMessageContent::Text("hi".to_owned()));
        assert!(msg.tool_calls.is_empty());
        assert!(msg.tool_call_id.is_none());
    }

    #[test]
    fn user_text_collapses_to_plain_text_content() {
        let item = TurnItem::UserMessage {
            id: "u1".to_owned(),
            content: vec![UserMessageContent::Text { text: "hello".to_owned() }],
        };
        let msg = turn_item_to_conversation_message(&item).unwrap();
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, ConversationMessageContent::Text("hello".to_owned()));
    }

    #[test]
    fn user_image_preserves_url_and_mime() {
        let item = TurnItem::UserMessage {
            id: "u2".to_owned(),
            content: vec![
                UserMessageContent::Text { text: "look".to_owned() },
                UserMessageContent::Image {
                    image_url: Some("http://x/y.png".to_owned()),
                    base64: None,
                    mime_type: Some("image/png".to_owned()),
                },
            ],
        };
        let msg = turn_item_to_conversation_message(&item).unwrap();
        let slab_types::ConversationMessageContent::Parts(parts) = &msg.content else {
            panic!("expected parts");
        };
        assert_eq!(parts.len(), 2);
        assert!(matches!(
            parts[1],
            ConversationContentPart::Image { ref image_url, ref mime_type, .. }
                if image_url.as_deref() == Some("http://x/y.png")
                    && mime_url_matches(mime_type, "image/png")
        ));
    }

    fn mime_url_matches(mime: &Option<String>, expected: &str) -> bool {
        mime.as_deref() == Some(expected)
    }

    #[test]
    fn ui_only_items_do_not_project() {
        let reasoning = TurnItem::Reasoning {
            id: "r".to_owned(),
            summary: ReasoningText::one("s"),
            content: ReasoningText::one("c"),
        };
        let cmd = TurnItem::CommandExecution {
            id: "c".to_owned(),
            command: "ls".to_owned(),
            cwd: "/".to_owned(),
            process_id: None,
            status: "completed".to_owned(),
            aggregated_output: None,
            exit_code: None,
            duration_ms: None,
        };
        assert!(turn_item_to_conversation_message(&reasoning).is_none());
        assert!(turn_item_to_conversation_message(&cmd).is_none());
        // Plan is a UI-only artifact (its LLM counterpart is the tool-result text).
        let plan = TurnItem::Plan {
            id: "p".to_owned(),
            plan: serde_json::json!({"plan_id": "plan-0", "items": [], "counts": {
                "pending": 0, "in_progress": 0, "completed": 0, "blocked": 0
            }}),
        };
        assert!(turn_item_to_conversation_message(&plan).is_none());
    }

    #[test]
    fn conversation_message_to_turn_item_round_trips_text() {
        let original = TurnItem::AgentMessage { id: "a".to_owned(), text: "hello back".to_owned() };
        let msg = turn_item_to_conversation_message(&original).unwrap();
        let back = conversation_message_to_turn_item(&msg).unwrap();
        match back {
            TurnItem::AgentMessage { text, .. } => assert_eq!(text, "hello back"),
            other => panic!("expected AgentMessage, got {other:?}"),
        }
    }

    #[test]
    fn tool_result_round_trip_is_none() {
        // Tool results live only in ConversationMessage form.
        let tool_msg = ConversationMessage {
            role: "tool".to_owned(),
            content: ConversationMessageContent::Text("42".to_owned()),
            name: None,
            tool_call_id: Some("call-1".to_owned()),
            tool_calls: vec![],
        };
        assert!(conversation_message_to_turn_item(&tool_msg).is_none());
        assert!(is_tool_result(&tool_msg));
    }

    #[test]
    fn assistant_with_tool_calls_does_not_project_back() {
        let msg = ConversationMessage {
            role: "assistant".to_owned(),
            content: ConversationMessageContent::Text(String::new()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![slab_types::ConversationToolCall {
                id: Some("c1".to_owned()),
                r#type: "function".to_owned(),
                function: slab_types::ConversationToolFunction {
                    name: "f".to_owned(),
                    arguments: "{}".to_owned(),
                },
            }],
        };
        assert!(conversation_message_to_turn_item(&msg).is_none());
    }
}
