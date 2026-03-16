use serde_json::{json, Map, Value};

use super::SettingValueType;
use crate::error::ServerError;

pub(crate) fn validate_settings_schema_document(document: &Value) -> Result<(), ServerError> {
    let schema = settings_document_validation_schema();
    ensure_json_schema_is_valid(&schema, "settings document schema")?;

    let validator = jsonschema::validator_for(&schema).map_err(|error| {
        ServerError::Internal(format!(
            "failed to compile settings document validation schema: {error}"
        ))
    })?;

    if let Some(error) = validator.iter_errors(document).next() {
        return Err(ServerError::Internal(format!(
            "embedded settings schema is invalid at '{}': {}",
            normalize_json_pointer(error.instance_path().to_string()),
            error
        )));
    }

    Ok(())
}

pub(crate) fn ensure_json_schema_is_valid(schema: &Value, label: &str) -> Result<(), ServerError> {
    jsonschema::draft202012::meta::validate(schema).map_err(|error| {
        ServerError::Internal(format!("{label} is not a valid JSON Schema: {error}"))
    })?;
    jsonschema::validator_for(schema)
        .map(|_| ())
        .map_err(|error| ServerError::Internal(format!("{label} failed to compile: {error}")))
}

pub(crate) fn base_property_validation_schema(
    value_type: SettingValueType,
    allow_null: bool,
) -> Map<String, Value> {
    let type_value = if allow_null {
        json!([setting_value_type_name(value_type), "null"])
    } else {
        json!(setting_value_type_name(value_type))
    };

    let mut schema = Map::new();
    schema.insert(
        "$schema".to_owned(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    schema.insert("type".to_owned(), type_value);
    schema
}

pub(crate) fn chat_providers_validation_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "array",
        "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "name", "api_base", "models"],
            "properties": {
                "id": { "type": "string", "minLength": 1 },
                "name": { "type": "string", "minLength": 1 },
                "api_base": { "type": "string", "minLength": 1 },
                "api_key": { "type": ["string", "null"] },
                "api_key_env": { "type": ["string", "null"] },
                "models": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["id", "display_name"],
                        "properties": {
                            "id": { "type": "string", "minLength": 1 },
                            "display_name": { "type": "string", "minLength": 1 },
                            "remote_model": { "type": ["string", "null"] }
                        }
                    }
                }
            }
        }
    })
}

pub(crate) fn normalize_json_pointer(path: String) -> String {
    if path.is_empty() {
        "/".to_owned()
    } else {
        path
    }
}

fn settings_document_validation_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "sections"],
        "properties": {
            "schema_version": {
                "type": "integer",
                "minimum": 1
            },
            "sections": {
                "type": "array",
                "minItems": 1,
                "items": settings_section_validation_schema()
            }
        }
    })
}

fn settings_section_validation_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "title", "subsections"],
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "title": { "type": "string", "minLength": 1 },
            "description_md": { "type": "string" },
            "order": { "type": "integer", "minimum": 0 },
            "subsections": {
                "type": "array",
                "items": settings_subsection_validation_schema()
            }
        }
    })
}

fn settings_subsection_validation_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "title", "properties"],
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "title": { "type": "string", "minLength": 1 },
            "description_md": { "type": "string" },
            "order": { "type": "integer", "minimum": 0 },
            "properties": {
                "type": "array",
                "items": settings_property_validation_schema()
            }
        }
    })
}

fn settings_property_validation_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["pmid", "label", "storage_kind", "schema"],
        "properties": {
            "pmid": { "type": "string", "minLength": 1 },
            "label": { "type": "string", "minLength": 1 },
            "description_md": { "type": "string" },
            "editable": { "type": "boolean" },
            "search_terms": {
                "type": "array",
                "items": { "type": "string" }
            },
            "storage_kind": {
                "enum": [
                    "boolean",
                    "integer",
                    "string",
                    "path",
                    "array",
                    "object",
                    "chat_providers"
                ]
            },
            "schema": setting_property_schema_validation_schema()
        }
    })
}

fn setting_property_schema_validation_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "default_value"],
        "properties": {
            "type": {
                "enum": ["boolean", "integer", "string", "array", "object"]
            },
            "enum": {
                "type": "array",
                "items": { "type": "string" }
            },
            "minimum": { "type": "integer" },
            "maximum": { "type": "integer" },
            "pattern": { "type": "string" },
            "json_schema": true,
            "default_value": true,
            "secret": { "type": "boolean" },
            "multiline": { "type": "boolean" },
            "order": { "type": "integer", "minimum": 0 }
        }
    })
}

fn setting_value_type_name(value_type: SettingValueType) -> &'static str {
    match value_type {
        SettingValueType::Boolean => "boolean",
        SettingValueType::Integer => "integer",
        SettingValueType::String => "string",
        SettingValueType::Array => "array",
        SettingValueType::Object => "object",
    }
}
