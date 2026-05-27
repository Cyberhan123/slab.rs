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
    assert_eq!(update_request.metadata.unwrap()["foo"], "string");
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
