use super::*;

#[test]
fn embeddings_post_response_deserializes() {
    let embeddings: CreateEmbeddingResponse = assert_json_deserializes(EMBEDDINGS_RESPONSE);

    assert_eq!(embeddings.data.len(), 1);
}
