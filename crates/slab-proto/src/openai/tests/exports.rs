use super::*;

#[test]
fn flattened_exports_allow_short_paths() {
    let _: CreateCompletionRequest = assert_json_deserializes(COMPLETIONS_REQUEST);
    let _: CreateCompletionResponse = assert_json_deserializes(COMPLETIONS_RESPONSE);
    let _: CreateEmbeddingRequest = assert_json_deserializes(EMBEDDINGS_REQUEST);
    let _: Response = assert_json_deserializes(RESPONSE_RESOURCE);
    let _: CompactResource = assert_json_deserializes(RESPONSE_COMPACT);
    let _: SkillResource = assert_json_deserializes(SKILL_RESPONSE);
    let _: SkillVersionResource = assert_json_deserializes(SKILL_VERSION_RESPONSE);
    let _: VideoResource = assert_json_deserializes(VIDEO_RESOURCE_RESPONSE);
    let _: crate::openai::responses::ResponseItemList =
        assert_json_deserializes(RESPONSE_INPUT_ITEMS);
    let _: crate::openai::ResponseItemList = assert_json_deserializes(RESPONSE_INPUT_ITEMS);
}
