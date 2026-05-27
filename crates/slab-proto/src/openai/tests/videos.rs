use super::*;

#[test]
fn videos_item_get_response_deserializes() {
    let video: VideoResource = assert_json_deserializes(VIDEO_RESOURCE_RESPONSE);

    assert_eq!(video.id, "video_123");
    assert_eq!(video.progress, 100);
}

#[test]
fn videos_list_get_response_deserializes() {
    let list: VideoListResource = assert_json_deserializes(VIDEO_LIST_RESPONSE);

    assert_eq!(list.data.len(), 1);
    assert!(!list.has_more);
}

#[test]
fn videos_post_create_request_deserializes() {
    let create_body: CreateVideoJsonBody = assert_json_deserializes(VIDEO_CREATE_REQUEST);

    assert_eq!(create_body.prompt, "Create a cinematic drone shot.");
}

#[test]
fn videos_edits_post_request_deserializes() {
    let edit_body: CreateVideoEditJsonBody = assert_json_deserializes(VIDEO_EDIT_REQUEST);

    assert_eq!(edit_body.prompt, "Make the scene at sunset.");
    assert_eq!(edit_body.video.id, "video_123");
}

#[test]
fn videos_extensions_post_request_deserializes() {
    let extend_body: CreateVideoExtendJsonBody = assert_json_deserializes(VIDEO_EXTEND_REQUEST);

    assert_eq!(extend_body.prompt, "Continue the shot to the right.");
    assert_eq!(extend_body.video.id, "video_123");
}

#[test]
fn videos_remix_post_request_deserializes() {
    let remix_body: CreateVideoRemixBody = assert_json_deserializes(VIDEO_REMIX_REQUEST);

    assert_eq!(remix_body.prompt, "Remix with watercolor style.");
}

#[test]
fn videos_characters_response_deserializes() {
    let character: VideoCharacterResource = assert_json_deserializes(VIDEO_CHARACTER_RESPONSE);

    assert_eq!(character.id.as_deref(), Some("char_001"));
    assert_eq!(character.name.as_deref(), Some("Ava"));
}

#[test]
fn videos_content_variant_deserializes() {
    let variant: VideoContentVariant = assert_json_deserializes(VIDEO_CONTENT_VARIANT);

    assert_eq!(variant.to_string(), "thumbnail");
}
