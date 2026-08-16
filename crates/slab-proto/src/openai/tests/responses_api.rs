use super::*;

#[test]
fn responses_post_response_deserializes() {
    let create_response: Response = assert_json_deserializes(RESPONSE_RESOURCE);

    assert_eq!(create_response.id, "resp_67ccd3a9da748190baa7f1570fe91ac604becb25c45c1d41");
    let output = create_response.output.first().expect("response fixture should include output");
    assert!(matches!(
        output,
        OutputItem::OutputMessage(message)
            if matches!(message.content.first(), Some(OutputMessageContent::OutputTextContent(_)))
    ));
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
    let response_value: serde_json::Value =
        serde_json::from_str(RESPONSE_RESOURCE).expect("response fixture should be valid JSON");
    let created_event: ResponseCreatedEvent = serde_json::from_value(json!({
        "type": "response.created",
        "response": response_value.clone(),
        "sequence_number": 1
    }))
    .expect("created response event fixture should deserialize");
    let completed_event: ResponseCompletedEvent = serde_json::from_value(json!({
        "type": "response.completed",
        "response": response_value,
        "sequence_number": 2
    }))
    .expect("completed response event fixture should deserialize");

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
    assert_eq!(error_event.error.message, "stream aborted");
    assert!(error_event.error.code.is_none());
    assert!(error_event.error.param.is_none());
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
    assert_eq!(done_event.name.as_deref(), Some("get_weather"));
    assert!(done_event.arguments.contains("Shanghai"));
}

#[test]
fn responses_stream_event_union_round_trips_known_events() {
    let queued_event: ResponseStreamEvent = assert_json_deserializes(RESPONSE_QUEUED_EVENT);
    let text_delta_event: ResponseStreamEvent =
        assert_json_round_trips(RESPONSE_OUTPUT_TEXT_DELTA_EVENT);
    let function_done_event: ResponsesServerEvent =
        assert_json_round_trips(RESPONSE_FUNCTION_CALL_ARGS_DONE_EVENT);

    assert!(matches!(queued_event, ResponseStreamEvent::ResponseQueuedEvent(_)));
    assert!(matches!(text_delta_event, ResponseStreamEvent::ResponseTextDeltaEvent(_)));
    assert!(matches!(
        function_done_event,
        ResponsesServerEvent::ResponseFunctionCallArgumentsDoneEvent(_)
    ));
}

#[test]
fn responses_stream_event_union_rejects_unknown_missing_and_mismatched_shapes() {
    let unknown_type = json!({
        "type": "response.not_real",
        "sequence_number": 1
    });
    let missing_required_field = json!({
        "type": "response.output_text.delta",
        "item_id": "msg_1",
        "output_index": 0,
        "content_index": 0,
        "sequence_number": 1,
        "logprobs": []
    });
    let mismatched_field_type = json!({
        "type": "response.function_call_arguments.done",
        "item_id": "fc_1",
        "name": "get_weather",
        "output_index": 0,
        "sequence_number": "bad",
        "arguments": "{}"
    });

    assert!(serde_json::from_value::<ResponseStreamEvent>(unknown_type).is_err());
    assert!(serde_json::from_value::<ResponseStreamEvent>(missing_required_field).is_err());
    assert!(serde_json::from_value::<ResponsesServerEvent>(mismatched_field_type).is_err());
}

#[test]
fn tool_enum_round_trips_typed_tool_definitions() {
    // Regression canary for the internally-tagged `Tool` enum fix (Option 2):
    // the inner tool structs no longer carry a `type` field, so the enum tag
    // round-trips without the prior serialize/deserialize conflict that broke
    // tool-config echo. Every declaration variant exercised by the responses
    // fixtures must survive a serialize -> deserialize cycle.
    use crate::openai::{FunctionTool, Tool, ToolSearchExecutionType, ToolSearchToolParam};
    use std::collections::HashMap;

    let function_tool = Tool::FunctionTool(Box::new(FunctionTool {
        name: "get_weather".to_owned(),
        parameters: Some(HashMap::from([("type".to_owned(), serde_json::json!("object"))])),
        strict: Some(true),
        ..Default::default()
    }));
    let tool_search_tool = Tool::ToolSearchToolParam(Box::new(ToolSearchToolParam {
        execution: Some(ToolSearchExecutionType::Server),
        ..Default::default()
    }));

    for original in [function_tool, tool_search_tool] {
        let wire = serde_json::to_value(&original).expect("Tool serializes");
        let round_trip: Tool =
            serde_json::from_value(wire).expect("Tool deserializes back (Option 2 fix)");
        assert_eq!(original, round_trip, "Tool variant must round-trip");
    }

    // Wire-shape sanity: the discriminator comes from the enum tag and the
    // inner struct contributes its own fields (no duplicate/missing `type`).
    let function_wire = serde_json::to_value(Tool::FunctionTool(Box::new(FunctionTool {
        name: "get_weather".to_owned(),
        ..Default::default()
    })))
    .unwrap();
    assert_eq!(function_wire["type"], "function");
    assert_eq!(function_wire["name"], "get_weather");
}

#[test]
fn shell_call_outcome_round_trips() {
    // Regression canary for the internally-tagged `ShellCallOutcome` enum fix
    // (Option 2): the `FunctionShellCallOutputExitOutcome` /
    // `FunctionShellCallOutputTimeoutOutcome` inners no longer carry their own
    // `type` field, so the enum tag round-trips with the canonical `exit` /
    // `timeout` wire strings. Production-relevant: slab-server's
    // `parse_shell_output_content` constructs this enum from inbound JSON.
    use crate::openai::{
        FunctionShellCallOutputExitOutcome, FunctionShellCallOutputTimeoutOutcome, ShellCallOutcome,
    };

    let exit = ShellCallOutcome::FunctionShellCallOutputExitOutcome(Box::new(
        FunctionShellCallOutputExitOutcome::new(0),
    ));
    let timeout = ShellCallOutcome::FunctionShellCallOutputTimeoutOutcome(Box::new(
        FunctionShellCallOutputTimeoutOutcome::new(),
    ));

    for original in [exit, timeout] {
        let wire = serde_json::to_value(&original).expect("ShellCallOutcome serializes");
        let round_trip: ShellCallOutcome = serde_json::from_value(wire)
            .expect("ShellCallOutcome deserializes back (Option 2 fix)");
        assert_eq!(original, round_trip, "ShellCallOutcome variant must round-trip");
    }

    // Wire-shape sanity: the discriminator comes from the enum tag and the
    // inner struct contributes its own fields (no duplicate/missing `type`).
    let exit_wire = serde_json::to_value(ShellCallOutcome::FunctionShellCallOutputExitOutcome(
        Box::new(FunctionShellCallOutputExitOutcome::new(0)),
    ))
    .unwrap();
    assert_eq!(exit_wire["type"], "exit");
    assert_eq!(exit_wire["exit_code"], 0);

    let timeout_wire =
        serde_json::to_value(ShellCallOutcome::FunctionShellCallOutputTimeoutOutcome(Box::new(
            FunctionShellCallOutputTimeoutOutcome::new(),
        )))
        .unwrap();
    assert_eq!(timeout_wire["type"], "timeout");
}

#[test]
fn function_and_custom_tool_output_round_trips() {
    // Regression canary for the internally-tagged
    // `FunctionAndCustomToolCallOutput` / `InputContent` /
    // `FunctionCallOutputItemParamOutputOneOfInner` enum fix (Option 2): the
    // shared `InputTextContent` / `InputImageContent` / `InputFileContent`
    // inners no longer carry their own `type` field, so the enum tag
    // round-trips with the canonical `input_text` / `input_image` /
    // `input_file` wire strings.
    use crate::openai::{FunctionAndCustomToolCallOutput, InputTextContent};

    let text_output = FunctionAndCustomToolCallOutput::InputTextContent(Box::new(
        InputTextContent::new("hello".to_owned()),
    ));

    let original = text_output;
    let wire = serde_json::to_value(&original).expect("FunctionAndCustomToolCallOutput serializes");
    let round_trip: FunctionAndCustomToolCallOutput = serde_json::from_value(wire)
        .expect("FunctionAndCustomToolCallOutput deserializes back (Option 2 fix)");
    assert_eq!(original, round_trip, "FunctionAndCustomToolCallOutput variant must round-trip");

    // Wire-shape sanity: the discriminator comes from the enum tag and the
    // inner struct contributes its own fields (no duplicate/missing `type`).
    let text_wire = serde_json::to_value(FunctionAndCustomToolCallOutput::InputTextContent(
        Box::new(InputTextContent::new("hello".to_owned())),
    ))
    .unwrap();
    assert_eq!(text_wire["type"], "input_text");
    assert_eq!(text_wire["text"], "hello");
}

#[test]
fn chat_response_format_round_trips() {
    // Regression canary for the internally-tagged chat-completions
    // `CreateChatCompletionRequestAllOfResponseFormat` enum fix (Option 2). Its
    // inner structs are dedicated (`ChatResponseFormatText` /
    // `ChatResponseFormatJsonObject`, no `type` field) so they do NOT collide
    // with the untagged Responses `TextResponseFormatConfiguration`, which
    // shares `ResponseFormatText` / `ResponseFormatJsonObject` and needs their
    // inner `type` field to disambiguate.
    use crate::openai::{
        ChatResponseFormatJsonObject, ChatResponseFormatText,
        CreateChatCompletionRequestAllOfResponseFormat,
    };

    let text = CreateChatCompletionRequestAllOfResponseFormat::ResponseFormatText(Box::new(
        ChatResponseFormatText {},
    ));
    let json_object = CreateChatCompletionRequestAllOfResponseFormat::ResponseFormatJsonObject(
        Box::new(ChatResponseFormatJsonObject {}),
    );

    for original in [text, json_object] {
        let wire = serde_json::to_value(&original).expect("chat response format serializes");
        let round_trip: CreateChatCompletionRequestAllOfResponseFormat =
            serde_json::from_value(wire)
                .expect("chat response format deserializes back (Option 2 fix)");
        assert_eq!(original, round_trip, "chat response format variant must round-trip");
    }

    let text_wire =
        serde_json::to_value(CreateChatCompletionRequestAllOfResponseFormat::ResponseFormatText(
            Box::new(ChatResponseFormatText {}),
        ))
        .unwrap();
    assert_eq!(text_wire["type"], "text");

    let json_object_wire = serde_json::to_value(
        CreateChatCompletionRequestAllOfResponseFormat::ResponseFormatJsonObject(Box::new(
            ChatResponseFormatJsonObject {},
        )),
    )
    .unwrap();
    assert_eq!(json_object_wire["type"], "json_object");
}
