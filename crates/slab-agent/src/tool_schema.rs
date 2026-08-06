//! JSON-Schema processing pipeline for tool parameter schemas.
//!
//! Tool schemas travel to the LLM on every request. As the tool set grows
//! (built-ins + plugins + MCP proxies), unbounded schema size bloats the prompt
//! and erodes the token budget. This module applies two passes —
//! [`sanitize_tool_schema`] (normalize well-formedness) and
//! [`compact_tool_schema`] (collapse deep schemas, strip documentation) — so the
//! model-facing spec list stays bounded regardless of how many tools register.
//! Mirrors codex's `json_schema.rs` pipeline; pure functions, trivially testable.

use serde_json::{Map, Value};

/// Default depth beyond which nested object/array schemas collapse to a stub.
/// Codex uses 2; deep tool schemas rarely carry actionable detail past that and
/// the model selects the tool from the name + top-level shape anyway.
pub const DEFAULT_MAX_SCHEMA_DEPTH: u8 = 2;

/// Run the full pipeline (sanitize → compact with the default depth limit) on a
/// tool parameter schema, returning a new bounded value. The input is untouched.
pub fn process_tool_schema(schema: &Value) -> Value {
    let mut out = schema.clone();
    sanitize_tool_schema(&mut out);
    compact_tool_schema(&mut out, DEFAULT_MAX_SCHEMA_DEPTH);
    out
}

/// Light normalization: ensure an object schema that declares `properties`
/// advertises `type: "object"`. This does *not* change validation semantics —
/// `additionalProperties` / `required` are left exactly as the tool declared
/// them (forcing `additionalProperties: false` would silently reject valid
/// calls). Only fills in a missing `type` for self-describing object schemas.
pub fn sanitize_tool_schema(schema: &mut Value) {
    let Some(obj) = schema.as_object_mut() else { return };
    if obj.contains_key("properties") && !obj.contains_key("type") {
        obj.insert("type".to_owned(), Value::String("object".to_owned()));
    }
    recurse_children(obj, sanitize_tool_schema);
}

/// Collapse deeply nested schemas and strip documentation keys to bound token
/// usage. Objects/arrays deeper than `max_depth` (counted by `properties`/
/// `items` nesting from the root) become a minimal stub; `description` /
/// `title` / `examples` / `$comment` are removed everywhere. Combinatorial
/// wrappers (`anyOf`/`allOf`/`oneOf`) stay at the same semantic depth.
pub fn compact_tool_schema(schema: &mut Value, max_depth: u8) {
    compact_recursive(schema, 0, max_depth);
}

fn compact_recursive(schema: &mut Value, depth: u8, max_depth: u8) {
    let Some(obj) = schema.as_object_mut() else { return };

    // Strip documentation keys — they cost tokens and the model rarely benefits
    // once a tool is selected by name + top-level shape.
    obj.remove("description");
    obj.remove("$comment");
    obj.remove("title");
    obj.remove("examples");

    // At the depth limit, collapse any further nesting to a minimal stub instead
    // of descending (keeps a `type` hint so the model still knows the shape).
    let has_nested = obj.contains_key("properties") || obj.contains_key("items");
    if depth >= max_depth && has_nested {
        let type_hint = if obj.contains_key("properties") { "object" } else { "array" };
        obj.clear();
        obj.insert("type".to_owned(), Value::String(type_hint.to_owned()));
        return;
    }

    descend_for_compact(obj, depth, max_depth);
}

fn descend_for_compact(obj: &mut Map<String, Value>, depth: u8, max_depth: u8) {
    if let Some(properties) = obj.get_mut("properties").and_then(|v| v.as_object_mut()) {
        for child in properties.values_mut() {
            compact_recursive(child, depth + 1, max_depth);
        }
    }
    if obj.contains_key("items") {
        if let Some(items) = obj.get_mut("items") {
            compact_recursive(items, depth + 1, max_depth);
        }
        if let Some(prefix) = obj.get_mut("prefixItems").and_then(|v| v.as_array_mut()) {
            for child in prefix {
                compact_recursive(child, depth + 1, max_depth);
            }
        }
    }
    for key in ["anyOf", "allOf", "oneOf"] {
        if let Some(arr) = obj.get_mut(key).and_then(|v| v.as_array_mut()) {
            for child in arr {
                compact_recursive(child, depth, max_depth);
            }
        }
    }
}

fn recurse_children<F: Fn(&mut Value)>(obj: &mut Map<String, Value>, f: F) {
    if let Some(properties) = obj.get_mut("properties").and_then(|v| v.as_object_mut()) {
        for child in properties.values_mut() {
            f(child);
        }
    }
    if let Some(items) = obj.get_mut("items") {
        f(items);
    }
    for key in ["anyOf", "allOf", "oneOf"] {
        if let Some(arr) = obj.get_mut(key).and_then(|v| v.as_array_mut()) {
            for child in arr {
                f(child);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sanitize_adds_type_for_property_schema_missing_it() {
        let mut schema = json!({ "properties": { "x": { "type": "string" } } });
        sanitize_tool_schema(&mut schema);
        assert_eq!(schema["type"], "object");
    }

    #[test]
    fn sanitize_leaves_explicit_type_alone() {
        let mut schema = json!({ "type": "string" });
        let before = schema.clone();
        sanitize_tool_schema(&mut schema);
        assert_eq!(schema, before);
    }

    #[test]
    fn sanitize_recurses_into_nested_properties() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "outer": { "properties": { "inner": { "type": "number" } } }
            }
        });
        sanitize_tool_schema(&mut schema);
        assert_eq!(schema["properties"]["outer"]["type"], "object");
    }

    #[test]
    fn compact_strips_descriptions() {
        let mut schema = json!({
            "type": "object",
            "description": "top",
            "properties": {
                "a": { "type": "string", "description": "a field", "title": "A" }
            }
        });
        compact_tool_schema(&mut schema, DEFAULT_MAX_SCHEMA_DEPTH);
        assert!(schema.get("description").is_none());
        assert!(schema["properties"]["a"].get("description").is_none());
        assert!(schema["properties"]["a"].get("title").is_none());
        // Shape preserved within depth budget.
        assert_eq!(schema["properties"]["a"]["type"], "string");
    }

    #[test]
    fn compact_collapses_deep_nesting_beyond_limit() {
        // depth 0: root object -> depth 1: a -> depth 2: b -> depth 3: c (collapsed).
        let mut schema = json!({
            "type": "object",
            "properties": {
                "a": {
                    "type": "object",
                    "properties": {
                        "b": {
                            "type": "object",
                            "properties": { "c": { "type": "string" } }
                        }
                    }
                }
            }
        });
        compact_tool_schema(&mut schema, DEFAULT_MAX_SCHEMA_DEPTH);
        // `b` is at depth 2 == limit and has nested properties -> collapsed to stub.
        assert_eq!(schema["properties"]["a"]["properties"]["b"]["type"], "object");
        assert!(schema["properties"]["a"]["properties"]["b"].get("properties").is_none());
    }

    #[test]
    fn compact_preserves_shallow_arrays() {
        let mut schema = json!({
            "type": "array",
            "items": { "type": "string", "description": "an item" }
        });
        compact_tool_schema(&mut schema, DEFAULT_MAX_SCHEMA_DEPTH);
        assert_eq!(schema["items"]["type"], "string");
        assert!(schema["items"].get("description").is_none());
    }

    #[test]
    fn process_pipeline_composes_sanitize_and_compact() {
        let schema = json!({
            "properties": {
                "a": {
                    "description": "desc",
                    "properties": { "b": { "properties": { "c": { "type": "string" } } } }
                }
            }
        });
        let out = process_tool_schema(&schema);
        // sanitize filled the root type.
        assert_eq!(out["type"], "object");
        // compact stripped the description.
        assert!(out["properties"]["a"].get("description").is_none());
        // compact collapsed the depth-2 nesting.
        assert_eq!(out["properties"]["a"]["properties"]["b"]["type"], "object");
    }

    #[test]
    fn compact_does_not_touch_non_object_schemas() {
        let mut schema = json!("not even an object");
        let before = schema.clone();
        compact_tool_schema(&mut schema, DEFAULT_MAX_SCHEMA_DEPTH);
        assert_eq!(schema, before);
    }
}
