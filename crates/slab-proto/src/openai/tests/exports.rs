use super::*;

#[test]
fn flattened_exports_allow_short_paths() {
    let _: CreateEmbeddingRequest = assert_json_deserializes(EMBEDDINGS_REQUEST);
    let _: crate::openai::responses::ResponseItemList =
        assert_json_deserializes(RESPONSE_INPUT_ITEMS);
    let _: crate::openai::ResponseItemList = assert_json_deserializes(RESPONSE_INPUT_ITEMS);
}
