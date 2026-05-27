use super::*;

#[test]
fn models_get_collection_response_deserializes() {
    let list: ListModelsResponse = assert_json_deserializes(MODELS_LIST_RESPONSE);

    assert_eq!(list.data.len(), 1);
}

#[test]
fn models_item_get_response_deserializes() {
    let model: Model = assert_json_deserializes(MODEL_RESPONSE);

    assert_eq!(model.id, "string");
}
