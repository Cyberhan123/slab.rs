use super::*;

#[test]
fn responses_post_response_deserializes() {
    let create_response: Response = assert_json_deserializes(RESPONSE_RESOURCE);

    assert_eq!(create_response.id, "resp_67ccd3a9da748190baa7f1570fe91ac604becb25c45c1d41");
}

#[test]
fn responses_item_get_and_delete_payloads_deserialize() {
    let get_response: Response = assert_json_deserializes(RESPONSE_RESOURCE);

    assert_eq!(get_response.output.len(), 1);
    assert!(RESPONSE_DELETE_BODY.is_empty());
}

#[test]
fn responses_item_cancel_post_response_deserializes() {
    let cancel_response: Response = assert_json_deserializes(RESPONSE_RESOURCE);

    assert_eq!(cancel_response.status, Some(ResponseStatus::Completed));
}

#[test]
fn responses_input_items_get_response_deserializes() {
    let input_items: ResponseItemList = assert_json_deserializes(RESPONSE_INPUT_ITEMS);

    assert_eq!(input_items.data.len(), 1);
}

#[test]
fn responses_input_tokens_post_response_deserializes() {
    let input_tokens: InputTokensResponse = assert_json_deserializes(RESPONSE_INPUT_TOKENS);

    assert_eq!(input_tokens.object, "response.input_tokens");
    assert_eq!(input_tokens.input_tokens, 123);
}

#[test]
fn responses_compact_post_response_deserializes() {
    let compact: CompactResource = assert_json_deserializes(RESPONSE_COMPACT);

    assert_eq!(compact.id, "resp_001");
}

#[test]
fn responses_post_sse_lifecycle_events_deserialize() {
    let response_value: serde_json::Value = serde_json::from_str(RESPONSE_RESOURCE).unwrap();
    let created_event: ResponseCreatedEvent = serde_json::from_value(json!({
        "type": "response.created",
        "response": response_value.clone(),
        "sequence_number": 1
    }))
    .unwrap();
    let completed_event: ResponseCompletedEvent = serde_json::from_value(json!({
        "type": "response.completed",
        "response": response_value,
        "sequence_number": 2
    }))
    .unwrap();

    assert_eq!(created_event.sequence_number, 1);
    assert_eq!(completed_event.sequence_number, 2);
}

#[test]
fn responses_post_sse_non_terminal_lifecycle_events_deserialize() {
    let queued_event: ResponseQueuedEvent = assert_json_deserializes(RESPONSE_QUEUED_EVENT);
    let in_progress_event: ResponseInProgressEvent =
        assert_json_deserializes(RESPONSE_IN_PROGRESS_EVENT);
    let incomplete_event: ResponseIncompleteEvent =
        assert_json_deserializes(RESPONSE_INCOMPLETE_EVENT);
    let failed_event: ResponseFailedEvent = assert_json_deserializes(RESPONSE_FAILED_EVENT);

    assert_eq!(queued_event.sequence_number, 3);
    assert_eq!(in_progress_event.sequence_number, 4);
    assert_eq!(incomplete_event.sequence_number, 5);
    assert_eq!(failed_event.sequence_number, 6);
}

#[test]
fn responses_post_sse_error_event_deserializes() {
    let error_event: ResponseErrorEvent = assert_json_deserializes(RESPONSE_ERROR_EVENT);

    assert_eq!(error_event.sequence_number, 7);
    assert_eq!(error_event.message, "stream aborted");
    assert!(error_event.code.is_none());
    assert!(error_event.param.is_none());
}

#[test]
fn responses_post_sse_output_text_events_deserialize() {
    let delta_event: ResponseTextDeltaEvent =
        assert_json_deserializes(RESPONSE_OUTPUT_TEXT_DELTA_EVENT);
    let done_event: ResponseTextDoneEvent =
        assert_json_deserializes(RESPONSE_OUTPUT_TEXT_DONE_EVENT);

    assert_eq!(delta_event.delta, "hel");
    assert_eq!(delta_event.sequence_number, 8);
    assert_eq!(done_event.text, "hello");
    assert_eq!(done_event.sequence_number, 9);
}

#[test]
fn responses_post_sse_function_call_events_deserialize() {
    let delta_event: ResponseFunctionCallArgumentsDeltaEvent =
        assert_json_deserializes(RESPONSE_FUNCTION_CALL_ARGS_DELTA_EVENT);
    let done_event: ResponseFunctionCallArgumentsDoneEvent =
        assert_json_deserializes(RESPONSE_FUNCTION_CALL_ARGS_DONE_EVENT);

    assert_eq!(delta_event.sequence_number, 10);
    assert!(delta_event.delta.contains("city"));
    assert_eq!(done_event.name, "get_weather");
    assert!(done_event.arguments.contains("Shanghai"));
}
