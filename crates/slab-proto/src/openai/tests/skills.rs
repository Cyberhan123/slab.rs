use super::*;

#[test]
fn skills_get_collection_response_deserializes() {
    let list_response: SkillListResource = assert_json_deserializes(SKILL_LIST_RESPONSE);

    assert_eq!(list_response.data.len(), 1);
}

#[test]
fn skills_post_and_item_get_responses_deserialize() {
    let get_response: SkillResource = assert_json_deserializes(SKILL_RESPONSE);
    let create_response: SkillResource = assert_json_deserializes(SKILL_RESPONSE);

    assert_eq!(get_response.id, "string");
    assert_eq!(create_response.latest_version, "string");
}

#[test]
fn skills_item_delete_response_deserializes() {
    let delete_response: DeletedSkillResource = assert_json_deserializes(SKILL_DELETE_RESPONSE);

    assert!(delete_response.deleted);
}

#[test]
fn skills_versions_get_collection_response_deserializes() {
    let version_list: SkillVersionListResource =
        assert_json_deserializes(SKILL_VERSION_LIST_RESPONSE);

    assert_eq!(version_list.data.len(), 1);
}

#[test]
fn skills_versions_post_and_item_get_responses_deserialize() {
    let version_get: SkillVersionResource = assert_json_deserializes(SKILL_VERSION_RESPONSE);
    let version_create: SkillVersionResource = assert_json_deserializes(SKILL_VERSION_RESPONSE);

    assert_eq!(version_get.version, "string");
    assert_eq!(version_create.skill_id, "string");
}

#[test]
fn skills_versions_item_delete_response_deserializes() {
    let version_delete: DeletedSkillVersionResource =
        assert_json_deserializes(SKILL_VERSION_DELETE_RESPONSE);

    assert!(version_delete.deleted);
}
