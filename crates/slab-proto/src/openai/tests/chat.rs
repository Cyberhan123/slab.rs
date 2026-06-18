use super::*;

#[test]
fn chat_completions_post_response_deserializes() {
    let create_response: CreateChatCompletionResponse =
        assert_json_deserializes(CHAT_COMPLETION_RESPONSE);

    assert_eq!(create_response.id, "string");
}

#[test]
fn chat_completions_get_response_deserializes() {
    let get_response: CreateChatCompletionResponse =
        assert_json_deserializes(CHAT_COMPLETION_RESPONSE);

    assert_eq!(get_response.choices.len(), 1);
}

#[test]
fn chat_completions_item_post_update_request_and_response_deserialize() {
    let update_response: CreateChatCompletionResponse =
        assert_json_deserializes(CHAT_COMPLETION_RESPONSE);
    let update_request: UpdateChatCompletionRequest =
        assert_json_deserializes(CHAT_COMPLETION_UPDATE_REQUEST);

    assert_eq!(update_response.model, "string");
    assert_eq!(
        update_request.metadata.expect("chat completion update fixture should include metadata")["foo"],
        "string"
    );
}

#[test]
fn chat_completions_item_delete_response_deserializes() {
    let delete_response: ChatCompletionDeleted =
        assert_json_deserializes(CHAT_COMPLETION_DELETE_RESPONSE);

    assert!(delete_response.deleted);
}

#[test]
fn chat_completions_post_sse_chunk_deserializes() {
    let stream_chunk: CreateChatCompletionStreamResponse =
        assert_json_deserializes(CHAT_COMPLETION_STREAM_CHUNK);

    assert_eq!(stream_chunk.choices.len(), 1);
}

#[test]
fn chat_completions_post_request_with_optionals_deserializes() {
    let request: CreateChatCompletionRequest =
        assert_json_deserializes(CHAT_COMPLETION_REQUEST_OPTIONALS);

    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.temperature, Some(0.2));
    assert_eq!(request.stream, Some(true));
    assert_eq!(request.logprobs, Some(true));
    assert!(matches!(request.metadata, Some(None)));
    assert!(matches!(request.service_tier, Some(None)));
    assert!(matches!(request.modalities, Some(None)));
    assert!(matches!(request.stream_options, Some(None)));
}

#[test]
fn chat_completions_post_response_with_content_filter_deserializes() {
    let response: CreateChatCompletionResponse =
        assert_json_deserializes(CHAT_COMPLETION_RESPONSE_CONTENT_FILTER);

    assert_eq!(response.id, "chatcmpl_filter");
    assert!(matches!(
        response.choices[0].finish_reason,
        crate::openai::FinishReason::ContentFilter
    ));
    assert!(matches!(response.service_tier, Some(None)));
}

#[test]
fn chat_completion_tool_call_union_round_trips_function_and_custom_calls() {
    let function_call = json!({
        "id": "call_1",
        "type": "function",
        "function": {
            "name": "get_weather",
            "arguments": "{\"city\":\"Shanghai\"}"
        }
    });
    let custom_call = json!({
        "id": "call_2",
        "type": "custom",
        "custom": {
            "name": "shell",
            "input": "pwd"
        }
    });

    let parsed_function: ChatCompletionMessageToolCallsInner =
        serde_json::from_value(function_call.clone()).expect("function tool call");
    let parsed_custom: ChatCompletionMessageToolCallsInner =
        serde_json::from_value(custom_call.clone()).expect("custom tool call");

    assert!(matches!(
        parsed_function,
        ChatCompletionMessageToolCallsInner::ChatCompletionMessageToolCall(_)
    ));
    assert!(matches!(
        parsed_custom,
        ChatCompletionMessageToolCallsInner::ChatCompletionMessageCustomToolCall(_)
    ));
    assert_eq!(serde_json::to_value(parsed_function).expect("serialize"), function_call);
    assert_eq!(serde_json::to_value(parsed_custom).expect("serialize"), custom_call);
}

#[test]
fn chat_completion_tool_call_union_rejects_unknown_missing_and_mismatched_shapes() {
    let unknown_type = json!({
        "id": "call_1",
        "type": "web_search",
        "web_search": {}
    });
    let missing_function = json!({
        "id": "call_1",
        "type": "function"
    });
    let mismatched_function = json!({
        "id": "call_1",
        "type": "function",
        "function": "not an object"
    });

    assert!(serde_json::from_value::<ChatCompletionMessageToolCallsInner>(unknown_type).is_err());
    assert!(
        serde_json::from_value::<ChatCompletionMessageToolCallsInner>(missing_function).is_err()
    );
    assert!(
        serde_json::from_value::<ChatCompletionMessageToolCallsInner>(mismatched_function).is_err()
    );
}

#[test]
fn chat_completion_request_message_content_unions_round_trip() {
    let assistant_message = json!({
        "role": "assistant",
        "content": [
            {
                "type": "text",
                "text": "partial answer"
            },
            {
                "type": "refusal",
                "refusal": "I cannot comply"
            }
        ]
    });
    let user_content = json!([
        {
            "type": "text",
            "text": "what is in this image?"
        },
        {
            "type": "image_url",
            "image_url": {
                "url": "https://example.test/image.png"
            }
        }
    ]);

    let assistant: ChatCompletionRequestAssistantMessage =
        serde_json::from_value(assistant_message.clone()).expect("assistant message content");
    let message: ChatCompletionRequestMessage =
        serde_json::from_value(assistant_message.clone()).expect("assistant request message");
    let user: ChatCompletionRequestUserMessageContent =
        serde_json::from_value(user_content.clone()).expect("user multimodal content");

    let assistant_content = assistant.content.as_ref().and_then(|content| content.as_ref());
    assert!(matches!(
        assistant_content,
        Some(content) if matches!(content.as_ref(), ChatCompletionRequestAssistantMessageContent::ArrayContentParts(parts) if parts.len() == 2)
    ));
    assert!(matches!(
        message,
        ChatCompletionRequestMessage::ChatCompletionRequestAssistantMessage(_)
    ));
    assert!(matches!(
        &user,
        ChatCompletionRequestUserMessageContent::ArrayContentParts(parts) if parts.len() == 2
    ));
    assert_eq!(serde_json::to_value(assistant).expect("serialize assistant"), assistant_message);
    assert_eq!(serde_json::to_value(user).expect("serialize user content"), user_content);
}

#[test]
fn chat_completion_request_message_content_unions_reject_invalid_parts() {
    let unknown_user_part = json!({
        "type": "video",
        "video": {
            "url": "https://example.test/video.mp4"
        }
    });
    let missing_assistant_text = json!({
        "type": "text",
        "refusal": "wrong field for text part"
    });
    let mismatched_user_content = json!([
        {
            "type": "image_url",
            "image_url": "not an object"
        }
    ]);

    assert!(
        serde_json::from_value::<ChatCompletionRequestUserMessageContentPart>(unknown_user_part)
            .is_err()
    );
    assert!(
        serde_json::from_value::<ChatCompletionRequestAssistantMessageContentPart>(
            missing_assistant_text
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<ChatCompletionRequestUserMessageContent>(mismatched_user_content)
            .is_err()
    );
}
