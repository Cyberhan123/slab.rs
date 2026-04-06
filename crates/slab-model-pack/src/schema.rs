use serde_json::{Value, json};

use crate::manifest::ModelPackManifest;

pub fn generate_manifest_schema() -> Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(ModelPackManifest))
        .expect("manifest schema should serialize");
    let root = schema.as_object_mut().expect("manifest schema root should be an object");

    root.insert(
        "$schema".into(),
        Value::String("https://json-schema.org/draft/2020-12/schema".into()),
    );
    root.insert(
        "$id".into(),
        Value::String("https://slab.rs/schemas/slab-manifest.schema.json".into()),
    );
    root.insert("title".into(), Value::String("slab.rs Model Pack Manifest".into()));
    root.insert(
        "description".into(),
        Value::String(
            "Schema for the manifest.json file stored at the root of a .slab model pack.".into(),
        ),
    );

    let properties = root.entry("properties").or_insert_with(|| json!({}));
    let properties = properties
        .as_object_mut()
        .expect("manifest schema properties should be an object");
    properties.insert("$schema".into(), json!({ "type": "string" }));

    let all_of = root.entry("allOf").or_insert_with(|| json!([]));
    let all_of = all_of
        .as_array_mut()
        .expect("manifest schema allOf should be an array");
    all_of.push(json!({
        "if": {
            "properties": {
                "source": {
                    "type": "object",
                    "properties": {
                        "kind": { "const": "cloud" }
                    },
                    "required": ["kind"]
                }
            },
            "required": ["source"]
        },
        "then": {
            "required": ["provider"],
            "properties": {
                "provider": {
                    "pattern": "^cloud\\."
                }
            }
        }
    }));

    schema
}

pub fn render_manifest_schema() -> String {
    let mut rendered =
        serde_json::to_string_pretty(&generate_manifest_schema()).expect("manifest schema should render");
    rendered.push('\n');
    rendered
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::render_manifest_schema;

    #[test]
    fn generated_manifest_schema_matches_checked_in_file() {
        let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../manifests/models/slab-manifest.schema.json");
        let expected = fs::read_to_string(&schema_path).expect("read checked-in schema");

        assert_eq!(render_manifest_schema(), expected);
    }
}