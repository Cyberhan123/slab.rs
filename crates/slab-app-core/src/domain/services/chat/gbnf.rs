use serde_json::{Map, Value};

use crate::domain::models::{StructuredOutput, StructuredOutputJsonSchema};
use crate::error::AppCoreError;

const GENERIC_JSON_VALUE_RULE: &str = "json_value";
const GENERIC_JSON_OBJECT_RULE: &str = "json_object";
const GENERIC_JSON_STRING_RULE: &str = "json_string";
const GENERIC_JSON_NUMBER_RULE: &str = "json_number";
const GENERIC_JSON_INTEGER_RULE: &str = "json_integer";

const BASE_GBNF_RULES: &str = r#"json_value ::= json_string | json_number | json_object | json_array | "true" | "false" | "null"
json_object ::= "{" ws "}" | "{" ws json_object_members ws "}"
json_object_members ::= json_string ws ":" ws json_value (ws "," ws json_string ws ":" ws json_value)*
json_array ::= "[" ws "]" | "[" ws json_array_items ws "]"
json_array_items ::= json_value (ws "," ws json_value)*
json_string ::= "\"" json_chars "\""
json_chars ::= "" | json_char json_chars
json_char ::= [^"\\] | "\\" json_escape
json_escape ::= ["\\/bfnrt] | "u" hex hex hex hex
hex ::= [0-9a-fA-F]
json_number ::= "-"? json_int json_frac? json_exp?
json_integer ::= "-"? json_int
json_int ::= "0" | [1-9] json_digits?
json_digits ::= [0-9] json_digits?
json_frac ::= "." json_digits1
json_digits1 ::= [0-9] json_digits?
json_exp ::= [eE] [+-]? json_digits1
ws ::= [ \t\n\r]*
"#;

pub(super) fn resolve_effective_gbnf(
    request_gbnf: Option<&str>,
    structured_output: Option<&StructuredOutput>,
    default_gbnf: Option<&str>,
) -> Result<Option<String>, AppCoreError> {
    if let Some(gbnf) = normalize_optional_text(request_gbnf) {
        return Ok(Some(gbnf));
    }
    if let Some(structured_output) = structured_output {
        return Ok(Some(compile_structured_output_to_gbnf(structured_output)?));
    }
    Ok(normalize_optional_text(default_gbnf))
}

pub(super) fn compile_structured_output_to_gbnf(
    structured_output: &StructuredOutput,
) -> Result<String, AppCoreError> {
    match structured_output {
        StructuredOutput::JsonObject => Ok(render_root_rule(GENERIC_JSON_OBJECT_RULE)),
        StructuredOutput::JsonSchema(schema) => compile_json_schema_to_gbnf(schema),
    }
}

fn compile_json_schema_to_gbnf(
    schema: &StructuredOutputJsonSchema,
) -> Result<String, AppCoreError> {
    if matches!(schema.strict, Some(false)) {
        return Err(AppCoreError::BadRequest(
            "local structured output requires response_format.json_schema.strict=true".into(),
        ));
    }

    let mut compiler = JsonSchemaGbnfCompiler::new(&schema.schema);
    compiler.compile()
}

fn render_root_rule(root_rule: &str) -> String {
    format!("root ::= ws {root_rule} ws\n{BASE_GBNF_RULES}")
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned)
}

struct JsonSchemaGbnfCompiler<'a> {
    root_schema: &'a Value,
    rules: Vec<(String, String)>,
    next_rule_id: usize,
    ref_stack: Vec<String>,
}

impl<'a> JsonSchemaGbnfCompiler<'a> {
    fn new(root_schema: &'a Value) -> Self {
        Self { root_schema, rules: Vec::new(), next_rule_id: 0, ref_stack: Vec::new() }
    }

    fn compile(&mut self) -> Result<String, AppCoreError> {
        let root_rule = self.compile_schema(self.root_schema)?;
        let mut rendered = format!("root ::= ws {root_rule} ws\n");
        for (name, body) in &self.rules {
            rendered.push_str(name);
            rendered.push_str(" ::= ");
            rendered.push_str(body);
            rendered.push('\n');
        }
        rendered.push_str(BASE_GBNF_RULES);
        Ok(rendered)
    }

    fn compile_schema(&mut self, schema: &Value) -> Result<String, AppCoreError> {
        if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
            return self.compile_ref(reference);
        }
        if let Some(any_of) = schema.get("anyOf").and_then(Value::as_array) {
            return self.compile_union(any_of, "any_of");
        }
        if let Some(one_of) = schema.get("oneOf").and_then(Value::as_array) {
            return self.compile_union(one_of, "one_of");
        }
        if let Some(all_of) = schema.get("allOf").and_then(Value::as_array) {
            return match all_of.as_slice() {
                [single] => self.compile_schema(single),
                _ => Err(unsupported_schema_keyword("allOf")),
            };
        }
        if let Some(values) = schema.get("enum").and_then(Value::as_array) {
            return self.compile_enum(values);
        }
        if let Some(constant) = schema.get("const") {
            return self.compile_const(constant);
        }

        match schema {
            Value::Bool(true) => Ok(GENERIC_JSON_VALUE_RULE.to_owned()),
            Value::Bool(false) => Err(AppCoreError::BadRequest(
                "local structured output cannot compile an always-false JSON Schema".into(),
            )),
            Value::Object(object) => self.compile_object_schema(object),
            _ => Err(AppCoreError::BadRequest(
                "local structured output requires JSON Schema objects or booleans".into(),
            )),
        }
    }

    fn compile_ref(&mut self, reference: &str) -> Result<String, AppCoreError> {
        if !reference.starts_with('#') {
            return Err(AppCoreError::BadRequest(format!(
                "local structured output only supports local JSON Schema $ref values, got '{reference}'"
            )));
        }
        if self.ref_stack.iter().any(|value| value == reference) {
            return Err(AppCoreError::BadRequest(format!(
                "local structured output does not support recursive JSON Schema $ref '{reference}'"
            )));
        }

        let pointer = if reference == "#" { "" } else { &reference[1..] };
        let target = self.root_schema.pointer(pointer).ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "local structured output could not resolve JSON Schema $ref '{reference}'"
            ))
        })?;

        self.ref_stack.push(reference.to_owned());
        let compiled = self.compile_schema(target);
        self.ref_stack.pop();
        compiled
    }

    fn compile_union(&mut self, items: &[Value], label: &str) -> Result<String, AppCoreError> {
        if items.is_empty() {
            return Err(AppCoreError::BadRequest(format!(
                "local structured output cannot compile empty JSON Schema {label}"
            )));
        }
        let alternatives =
            items.iter().map(|item| self.compile_schema(item)).collect::<Result<Vec<_>, _>>()?;
        Ok(self.add_rule(label, alternatives.join(" | ")))
    }

    fn compile_const(&mut self, value: &Value) -> Result<String, AppCoreError> {
        let literal = serde_json::to_string(value).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to serialize structured output const value to JSON: {error}"
            ))
        })?;
        Ok(self.add_rule("const", grammar_literal(&literal)))
    }

    fn compile_enum(&mut self, values: &[Value]) -> Result<String, AppCoreError> {
        if values.is_empty() {
            return Err(AppCoreError::BadRequest(
                "local structured output cannot compile an empty JSON Schema enum".into(),
            ));
        }

        let mut literals = Vec::with_capacity(values.len());
        for value in values {
            let literal = serde_json::to_string(value).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to serialize structured output enum value to JSON: {error}"
                ))
            })?;
            literals.push(grammar_literal(&literal));
        }
        Ok(self.add_rule("enum", literals.join(" | ")))
    }

    fn compile_object_schema(
        &mut self,
        schema: &Map<String, Value>,
    ) -> Result<String, AppCoreError> {
        reject_present_keywords(
            schema,
            &[
                "contains",
                "contentEncoding",
                "contentMediaType",
                "contentSchema",
                "dependentRequired",
                "dependentSchemas",
                "if",
                "maxProperties",
                "minProperties",
                "not",
                "pattern",
                "patternProperties",
                "propertyNames",
                "unevaluatedProperties",
            ],
        )?;

        let explicit_types = schema_types(schema)?;
        if !explicit_types.is_empty() {
            let variants = explicit_types
                .iter()
                .map(|kind| self.compile_typed_schema(kind, schema))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(self.add_rule("typed", variants.join(" | ")));
        }

        if schema.contains_key("properties")
            || schema.contains_key("required")
            || schema.contains_key("additionalProperties")
        {
            return self.compile_json_object_type(schema);
        }
        if schema.contains_key("items") || schema.contains_key("prefixItems") {
            return self.compile_json_array_type(schema);
        }

        Ok(GENERIC_JSON_VALUE_RULE.to_owned())
    }

    fn compile_typed_schema(
        &mut self,
        kind: &str,
        schema: &Map<String, Value>,
    ) -> Result<String, AppCoreError> {
        match kind {
            "string" => {
                reject_present_keywords(
                    schema,
                    &[
                        "format",
                        "maxLength",
                        "minLength",
                        "pattern",
                        "contentEncoding",
                        "contentMediaType",
                        "contentSchema",
                    ],
                )?;
                Ok(GENERIC_JSON_STRING_RULE.to_owned())
            }
            "number" => {
                reject_present_keywords(
                    schema,
                    &["exclusiveMaximum", "exclusiveMinimum", "maximum", "minimum", "multipleOf"],
                )?;
                Ok(GENERIC_JSON_NUMBER_RULE.to_owned())
            }
            "integer" => {
                reject_present_keywords(
                    schema,
                    &["exclusiveMaximum", "exclusiveMinimum", "maximum", "minimum", "multipleOf"],
                )?;
                Ok(GENERIC_JSON_INTEGER_RULE.to_owned())
            }
            "boolean" => Ok(self.add_rule("boolean", "\"true\" | \"false\"".to_owned())),
            "null" => Ok(self.add_rule("null", "\"null\"".to_owned())),
            "object" => self.compile_json_object_type(schema),
            "array" => self.compile_json_array_type(schema),
            other => Err(AppCoreError::BadRequest(format!(
                "local structured output does not support JSON Schema type '{other}'"
            ))),
        }
    }

    fn compile_json_object_type(
        &mut self,
        schema: &Map<String, Value>,
    ) -> Result<String, AppCoreError> {
        let properties =
            schema.get("properties").and_then(Value::as_object).cloned().unwrap_or_default();
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|value| {
                        value.as_str().map(str::to_owned).ok_or_else(|| {
                            AppCoreError::BadRequest(
                                "local structured output requires object.required entries to be strings"
                                    .into(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();

        for field in &required {
            if !properties.contains_key(field) {
                return Err(AppCoreError::BadRequest(format!(
                    "local structured output requires required object field '{field}' to appear in properties"
                )));
            }
        }

        if properties.is_empty() {
            return match schema.get("additionalProperties") {
                Some(Value::Bool(false)) => {
                    Ok(self.add_rule("object", "\"{\" ws \"}\"".to_owned()))
                }
                Some(Value::Bool(true)) | None => {
                    self.compile_generic_object(GENERIC_JSON_VALUE_RULE)
                }
                Some(value) => {
                    let value_rule = self.compile_schema(value)?;
                    self.compile_generic_object(&value_rule)
                }
            };
        }

        let mut fields = Vec::with_capacity(properties.len());
        for (key, value) in properties {
            let value_rule = self.compile_schema(&value)?;
            let pair_rule = self.add_rule(
                "object_field",
                format!(
                    "{} ws \":\" ws {}",
                    grammar_literal(&serde_json::to_string(&key).map_err(|error| {
                        AppCoreError::Internal(format!(
                            "failed to serialize JSON object field name '{key}': {error}"
                        ))
                    })?),
                    value_rule
                ),
            );
            fields.push(ObjectField {
                pair_rule,
                required: required.iter().any(|field| field == &key),
            });
        }

        let members_rule = self.compile_object_members(&fields, 0, false)?;
        Ok(self.add_rule("object", format!("\"{{\" ws {} ws \"}}\"", members_rule)))
    }

    fn compile_generic_object(&mut self, value_rule: &str) -> Result<String, AppCoreError> {
        let member_rule = self.add_rule(
            "object_member",
            format!("{GENERIC_JSON_STRING_RULE} ws \":\" ws {value_rule}"),
        );
        Ok(self.add_rule(
            "object",
            format!(
                "\"{{\" ws \"}}\" | \"{{\" ws {} (ws \",\" ws {})* ws \"}}\"",
                member_rule, member_rule
            ),
        ))
    }

    fn compile_object_members(
        &mut self,
        fields: &[ObjectField],
        index: usize,
        emitted_any: bool,
    ) -> Result<String, AppCoreError> {
        if index >= fields.len() {
            return Ok(self.empty_rule());
        }

        let field = &fields[index];
        let next_skip = self.compile_object_members(fields, index + 1, emitted_any)?;
        let next_include = self.compile_object_members(fields, index + 1, true)?;
        let include = if emitted_any {
            format!("\",\" ws {} ws {}", field.pair_rule, next_include)
        } else {
            format!("{} ws {}", field.pair_rule, next_include)
        };

        if field.required {
            Ok(self.add_rule("object_members", include))
        } else {
            Ok(self.add_rule("object_members", format!("{next_skip} | {include}")))
        }
    }

    fn compile_json_array_type(
        &mut self,
        schema: &Map<String, Value>,
    ) -> Result<String, AppCoreError> {
        reject_present_keywords(
            schema,
            &["contains", "maxContains", "minContains", "uniqueItems", "unevaluatedItems"],
        )?;

        let min_items = schema
            .get("minItems")
            .and_then(Value::as_u64)
            .map(usize::try_from)
            .transpose()
            .map_err(|error| {
                AppCoreError::BadRequest(format!(
                    "local structured output could not convert minItems into usize: {error}"
                ))
            })?
            .unwrap_or(0);
        let max_items = schema
            .get("maxItems")
            .and_then(Value::as_u64)
            .map(usize::try_from)
            .transpose()
            .map_err(|error| {
                AppCoreError::BadRequest(format!(
                    "local structured output could not convert maxItems into usize: {error}"
                ))
            })?;

        if max_items.is_some_and(|max_items| max_items < min_items) {
            return Err(AppCoreError::BadRequest(
                "local structured output requires maxItems to be greater than or equal to minItems"
                    .into(),
            ));
        }

        if let Some(prefix_items) = schema.get("prefixItems").and_then(Value::as_array) {
            let prefix_rules = prefix_items
                .iter()
                .map(|item| self.compile_schema(item))
                .collect::<Result<Vec<_>, _>>()?;
            let item_rule = match schema.get("items") {
                Some(Value::Bool(false)) => None,
                Some(Value::Bool(true)) | None => Some(GENERIC_JSON_VALUE_RULE.to_owned()),
                Some(value) => Some(self.compile_schema(value)?),
            };
            let items_rule = self.compile_prefix_array_items(
                &prefix_rules,
                item_rule.as_deref(),
                0,
                false,
                min_items,
                max_items,
            )?;
            return Ok(self.add_rule("array", format!("\"[\" ws {} ws \"]\"", items_rule)));
        }

        let item_rule = match schema.get("items") {
            Some(Value::Bool(false)) => {
                if min_items > 0 {
                    return Err(AppCoreError::BadRequest(
                        "local structured output cannot satisfy minItems when items=false".into(),
                    ));
                }
                return Ok(self.add_rule("array", "\"[\" ws \"]\"".to_owned()));
            }
            Some(Value::Bool(true)) | None => GENERIC_JSON_VALUE_RULE.to_owned(),
            Some(value) => self.compile_schema(value)?,
        };

        let items_rule =
            self.compile_uniform_array_items(&item_rule, false, min_items, max_items)?;
        Ok(self.add_rule("array", format!("\"[\" ws {} ws \"]\"", items_rule)))
    }

    fn compile_prefix_array_items(
        &mut self,
        prefix_rules: &[String],
        tail_rule: Option<&str>,
        index: usize,
        emitted_any: bool,
        min_items: usize,
        max_items: Option<usize>,
    ) -> Result<String, AppCoreError> {
        if max_items.is_some_and(|max_items| index >= max_items) {
            if index < min_items {
                return Err(AppCoreError::BadRequest(
                    "local structured output cannot satisfy minItems with the configured prefixItems/maxItems combination".into(),
                ));
            }
            return Ok(self.empty_rule());
        }

        if index >= prefix_rules.len() {
            let remaining_min = min_items.saturating_sub(index);
            let remaining_max = max_items.map(|max_items| max_items.saturating_sub(index));
            return match tail_rule {
                Some(tail_rule) => self.compile_uniform_array_items(
                    tail_rule,
                    emitted_any,
                    remaining_min,
                    remaining_max,
                ),
                None => {
                    if remaining_min > 0 {
                        Err(AppCoreError::BadRequest(
                            "local structured output cannot satisfy minItems because no array item schema remains".into(),
                        ))
                    } else {
                        Ok(self.empty_rule())
                    }
                }
            };
        }

        let next_rule = self.compile_prefix_array_items(
            prefix_rules,
            tail_rule,
            index + 1,
            true,
            min_items,
            max_items,
        )?;
        let include = if emitted_any {
            format!("\",\" ws {} ws {}", prefix_rules[index], next_rule)
        } else {
            format!("{} ws {}", prefix_rules[index], next_rule)
        };

        if index >= min_items {
            let end_rule = self.empty_rule();
            Ok(self.add_rule("array_items", format!("{end_rule} | {include}")))
        } else {
            Ok(self.add_rule("array_items", include))
        }
    }

    fn compile_uniform_array_items(
        &mut self,
        item_rule: &str,
        emitted_any: bool,
        min_items: usize,
        max_items: Option<usize>,
    ) -> Result<String, AppCoreError> {
        match max_items {
            Some(max_items) => self.compile_finite_uniform_array_items(
                item_rule,
                emitted_any,
                min_items,
                max_items,
            ),
            None => self.compile_unbounded_uniform_array_items(item_rule, emitted_any, min_items),
        }
    }

    fn compile_finite_uniform_array_items(
        &mut self,
        item_rule: &str,
        emitted_any: bool,
        min_items: usize,
        max_items: usize,
    ) -> Result<String, AppCoreError> {
        if max_items == 0 {
            return if min_items == 0 {
                Ok(self.empty_rule())
            } else {
                Err(AppCoreError::BadRequest(
                    "local structured output cannot satisfy the configured array length bounds"
                        .into(),
                ))
            };
        }

        let next_rule = self.compile_finite_uniform_array_items(
            item_rule,
            true,
            min_items.saturating_sub(1),
            max_items - 1,
        )?;
        let include = if emitted_any {
            format!("\",\" ws {} ws {}", item_rule, next_rule)
        } else {
            format!("{} ws {}", item_rule, next_rule)
        };

        if min_items == 0 {
            let end_rule = self.empty_rule();
            Ok(self.add_rule("array_items", format!("{end_rule} | {include}")))
        } else {
            Ok(self.add_rule("array_items", include))
        }
    }

    fn compile_unbounded_uniform_array_items(
        &mut self,
        item_rule: &str,
        emitted_any: bool,
        min_items: usize,
    ) -> Result<String, AppCoreError> {
        if min_items == 0 {
            let repeated = if emitted_any {
                format!("\",\" ws {} (ws \",\" ws {})*", item_rule, item_rule)
            } else {
                format!("{} (ws \",\" ws {})*", item_rule, item_rule)
            };
            let end_rule = self.empty_rule();
            return Ok(self.add_rule("array_items", format!("{end_rule} | {repeated}")));
        }

        let next_rule =
            self.compile_unbounded_uniform_array_items(item_rule, true, min_items - 1)?;
        let include = if emitted_any {
            format!("\",\" ws {} ws {}", item_rule, next_rule)
        } else {
            format!("{} ws {}", item_rule, next_rule)
        };
        Ok(self.add_rule("array_items", include))
    }

    fn add_rule(&mut self, prefix: &str, body: String) -> String {
        let rule_name = format!("{prefix}_{}", self.next_rule_id);
        self.next_rule_id += 1;
        self.rules.push((rule_name.clone(), body));
        rule_name
    }

    fn empty_rule(&mut self) -> String {
        self.add_rule("empty", "\"\"".to_owned())
    }
}

#[derive(Clone)]
struct ObjectField {
    pair_rule: String,
    required: bool,
}

fn grammar_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

fn schema_types(schema: &Map<String, Value>) -> Result<Vec<String>, AppCoreError> {
    let Some(type_value) = schema.get("type") else {
        return Ok(Vec::new());
    };

    match type_value {
        Value::String(kind) => Ok(vec![kind.clone()]),
        Value::Array(values) => values
            .iter()
            .map(|value| {
                value.as_str().map(str::to_owned).ok_or_else(|| {
                    AppCoreError::BadRequest(
                        "local structured output requires JSON Schema type arrays to contain only strings".into(),
                    )
                })
            })
            .collect(),
        _ => Err(AppCoreError::BadRequest(
            "local structured output requires JSON Schema type to be a string or string array"
                .into(),
        )),
    }
}

fn reject_present_keywords(
    schema: &Map<String, Value>,
    keywords: &[&str],
) -> Result<(), AppCoreError> {
    if let Some(keyword) = keywords.iter().find(|keyword| schema.contains_key(**keyword)) {
        return Err(unsupported_schema_keyword(keyword));
    }
    Ok(())
}

fn unsupported_schema_keyword(keyword: &str) -> AppCoreError {
    AppCoreError::BadRequest(format!(
        "local structured output does not support compiling JSON Schema keyword '{keyword}' to GBNF"
    ))
}

#[cfg(test)]
mod tests {
    use super::{compile_structured_output_to_gbnf, resolve_effective_gbnf};
    use crate::domain::models::{StructuredOutput, StructuredOutputJsonSchema};
    use serde_json::json;

    #[test]
    fn request_gbnf_wins_over_structured_output_and_defaults() {
        let resolved = resolve_effective_gbnf(
            Some(" root ::= \"ok\" "),
            Some(&StructuredOutput::JsonObject),
            Some("root ::= \"fallback\""),
        )
        .expect("resolve gbnf");

        assert_eq!(resolved.as_deref(), Some("root ::= \"ok\""));
    }

    #[test]
    fn json_object_structured_output_compiles_to_object_root() {
        let rendered =
            compile_structured_output_to_gbnf(&StructuredOutput::JsonObject).expect("gbnf");

        assert!(rendered.contains("root ::= ws json_object ws"));
        assert!(rendered.contains("json_object ::= "));
    }

    #[test]
    fn json_schema_const_compiles_to_exact_literal() {
        let rendered = compile_structured_output_to_gbnf(&StructuredOutput::JsonSchema(
            StructuredOutputJsonSchema::new(
                None,
                None,
                Some(true),
                json!({ "const": { "ok": 1 } }),
            ),
        ))
        .expect("gbnf");

        assert!(rendered.contains("\\\"ok\\\""));
    }
}
