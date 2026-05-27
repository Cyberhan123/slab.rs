use super::*;

#[test]
fn images_edits_post_response_deserializes() {
    let image_edit: ImagesResponse = assert_json_deserializes(IMAGES_RESPONSE);

    assert_eq!(image_edit.created, 0);
}

#[test]
fn images_generations_post_response_deserializes() {
    let image_generation: ImagesResponse = assert_json_deserializes(IMAGES_RESPONSE);

    assert_eq!(image_generation.data.unwrap().len(), 1);
}

#[test]
fn images_variations_post_response_deserializes() {
    let image_variation: ImagesResponse = assert_json_deserializes(IMAGES_RESPONSE);

    assert_eq!(image_variation.usage.unwrap().total_tokens, 0);
}
