use super::*;

#[test]
fn completions_post_request_deserializes() {
    let request: CreateCompletionRequest = assert_json_deserializes(COMPLETIONS_REQUEST);

    assert_eq!(request.prompt.as_ref().and_then(|value| value.as_str()), Some("Write a haiku about tests."));
}

#[test]
fn completions_post_response_deserializes() {
    let response: CreateCompletionResponse = assert_json_deserializes(COMPLETIONS_RESPONSE);

    assert_eq!(response.id, "cmpl_123");
    assert_eq!(response.choices.len(), 1);
}

#[test]
fn completions_post_request_with_custom_model_deserializes() {
    let request: CreateCompletionRequest =
        assert_json_deserializes(COMPLETIONS_REQUEST_CUSTOM_MODEL);

    assert!(matches!(
        request.model,
        CreateCompletionRequestModel::StringValue(ref model) if model == "my-custom-model"
    ));
}

#[test]
fn completions_post_response_with_content_filter_deserializes() {
    let response: CreateCompletionResponse =
        assert_json_deserializes(COMPLETIONS_RESPONSE_CONTENT_FILTER);

    assert_eq!(response.id, "cmpl_124");
    assert!(matches!(
        response.choices[0].finish_reason,
        Some(crate::openai::CompletionFinishReason::ContentFilter)
    ));
}
