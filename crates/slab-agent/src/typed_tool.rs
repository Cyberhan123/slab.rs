//! Strongly-typed tool authoring: [`TypedTool`] plus shared helpers.
//!
//! A tool declares its argument shape once as a struct deriving
//! `Deserialize` + `JsonSchema`; [`typed_input_schema`] renders the
//! model-facing JSON Schema from that struct and [`parse_tool_input`]
//! deserializes call arguments with the tool-error wording the dispatch
//! layer and tests already know. Schema and parsing stay derived from one
//! type instead of drifting apart as a hand-written `json!` schema and a
//! manual `Value` extraction would.
//!
//! The blanket `ToolHandler` impl below adapts every `TypedTool` to the
//! router's dispatch trait, parsing the raw arguments once before the typed
//! `execute` runs. The metadata methods deliberately keep the RAW arguments
//! value: the dispatch layer calls them around execution (risk analysis,
//! batch partitioning, rendering) where a full-struct parse would be
//! all-or-nothing and would change behavior — e.g. `describe_operation`
//! must still extract `command` from arguments whose other fields are
//! invalid, and hooks may rewrite arguments between partitioning and
//! execution.

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::AgentError;
use crate::protocol::TurnItem;
use crate::tool::{
    ToolCallRender, ToolContext, ToolHandler, ToolNamespace, ToolOutput, ToolVisibility,
    default_tool_turn_item,
};

/// JSON Schema for a typed tool input, normalized to the shape the previous
/// hand-written schemas had: schemars' root meta keys (`$schema`, `title`,
/// and the struct doc comment leaking in as `description`) are stripped —
/// the tool's prose lives on [`crate::ToolHandler::description`], not the
/// parameter schema — a non-object result becomes the empty object schema
/// (so the provider adapter keeps seeing an object schema), and an object
/// schema without composition/`properties` gets an explicit empty
/// `properties` map (schemars omits it for field-less structs; hand-written
/// schemas spelled it out, and tests pin that shape).
pub fn typed_input_schema<T: JsonSchema>() -> Value {
    let schema = serde_json::to_value(schemars::schema_for!(T))
        .unwrap_or_else(|_| serde_json::json!({"type": "object", "properties": {}}));
    let Value::Object(mut map) = schema else {
        return serde_json::json!({"type": "object", "properties": {}});
    };
    map.remove("$schema");
    map.remove("title");
    map.remove("description");
    let is_object = matches!(map.get("type"), Some(Value::String(t)) if t == "object");
    if !map.contains_key("properties")
        && !map.contains_key("anyOf")
        && !map.contains_key("oneOf")
        && !map.contains_key("allOf")
        && !map.contains_key("$ref")
        && (is_object || map.is_empty())
    {
        if map.is_empty() {
            map.insert("type".to_owned(), Value::String("object".to_owned()));
        }
        map.insert("properties".to_owned(), serde_json::Map::new().into());
    }
    let mut schema = Value::Object(map);
    strip_null_defaults(&mut schema);
    schema
}

/// Drop `"default": null` entries anywhere in the schema. `#[serde(default)]`
/// on an `Option` field (needed when the field also carries a custom
/// deserializer) makes schemars emit that noise; `null` is already the
/// implied default of every nullable property, so removing it keeps the
/// schema free of meaningless keywords.
fn strip_null_defaults(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|key, value| !(key == "default" && value.is_null()));
            for value in map.values_mut() {
                strip_null_defaults(value);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(strip_null_defaults),
        _ => {}
    }
}

/// Deserialize tool arguments into a typed input, mapping a missing required
/// field to the `missing '<field>' argument` error the existing tools emit
/// (see the `string_arg` helper in `slab-agent-tools`).
pub fn parse_tool_input<T: DeserializeOwned>(arguments: &Value) -> Result<T, AgentError> {
    serde_json::from_value(arguments.clone()).map_err(input_parse_error)
}

/// An individual tool whose arguments are described by a strongly-typed
/// input struct — the single source of truth for both the model-facing
/// parameter schema and argument parsing.
///
/// Tools implement this instead of [`ToolHandler`]; the blanket impl below
/// supplies the `ToolHandler` the [`crate::ToolRouter`] dispatches on.
/// Dynamic proxies that carry a remote/plugin-authored schema (MCP, plugin
/// capabilities) use `Input = serde_json::Value` and override
/// [`TypedTool::parameters_schema`] with the stored schema.
#[async_trait::async_trait]
pub trait TypedTool: Send + Sync {
    /// Parsed argument type; also drives the generated parameters schema.
    type Input: JsonSchema + DeserializeOwned + Send + Sync;

    /// Canonical tool name, matched against LLM tool-call names.
    fn name(&self) -> &str;

    /// Human-readable description shown to the model in the tool list.
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's parameter object. Defaults to the
    /// schema generated from [`Self::Input`]; dynamic proxies override to
    /// return their remote-authored schema.
    fn parameters_schema(&self) -> Value {
        typed_input_schema::<Self::Input>()
    }

    /// Describe the operation this invocation performs, for the unified
    /// policy engine. Returning `None` (the default) lets the kernel infer
    /// the category from the tool name. Tools that carry a meaningful
    /// subject (command / path / query) should override this.
    fn describe_operation(
        &self,
        _arguments: &Value,
    ) -> Option<slab_exec_policy::OperationDescriptor> {
        None
    }

    /// Coarse operation category used for progressive tool exposure.
    fn category(&self) -> slab_exec_policy::OperationCategory {
        slab_exec_policy::OperationCategory::ReadOnly
    }

    /// Whether THIS invocation may run concurrently with other
    /// concurrency-safe invocations in the same assistant tool batch.
    fn is_concurrency_safe(&self, _arguments: &Value) -> bool {
        false
    }

    /// When/how the tool appears in the model-facing tool list.
    fn visibility(&self) -> ToolVisibility {
        ToolVisibility::Direct
    }

    /// Namespace the tool belongs to.
    fn namespace(&self) -> ToolNamespace {
        ToolNamespace::builtin()
    }

    /// Build the harness [`TurnItem`] for a call to this tool.
    fn render_turn_item(&self, render: &ToolCallRender<'_>) -> TurnItem {
        default_tool_turn_item(render)
    }

    /// Execute the tool with the parsed input.
    async fn execute(
        &self,
        ctx: &ToolContext,
        input: Self::Input,
    ) -> Result<ToolOutput, AgentError>;
}

/// Every [`TypedTool`] is a [`ToolHandler`]: the adapter parses the raw
/// arguments once (through [`parse_tool_input`]) and forwards the metadata
/// methods verbatim. `capability()` is intentionally not overridden — the
/// `ToolHandler` default derives it from `category()`/`visibility()`/
/// `namespace()`, which route through the forwards below.
#[async_trait::async_trait]
impl<T> ToolHandler for T
where
    T: TypedTool,
{
    fn name(&self) -> &str {
        TypedTool::name(self)
    }

    fn description(&self) -> &str {
        TypedTool::description(self)
    }

    fn parameters_schema(&self) -> Value {
        TypedTool::parameters_schema(self)
    }

    fn describe_operation(
        &self,
        arguments: &Value,
    ) -> Option<slab_exec_policy::OperationDescriptor> {
        TypedTool::describe_operation(self, arguments)
    }

    fn category(&self) -> slab_exec_policy::OperationCategory {
        TypedTool::category(self)
    }

    fn is_concurrency_safe(&self, arguments: &Value) -> bool {
        TypedTool::is_concurrency_safe(self, arguments)
    }

    fn visibility(&self) -> ToolVisibility {
        TypedTool::visibility(self)
    }

    fn namespace(&self) -> ToolNamespace {
        TypedTool::namespace(self)
    }

    fn render_turn_item(&self, render: &ToolCallRender<'_>) -> TurnItem {
        TypedTool::render_turn_item(self, render)
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        arguments: &Value,
    ) -> Result<ToolOutput, AgentError> {
        let input = parse_tool_input::<T::Input>(arguments)?;
        TypedTool::execute(self, ctx, input).await
    }
}

/// Map a serde parse failure to the tool-error convention: a missing
/// required field reads as `missing '<field>' argument`; other failures
/// surface the serde message.
fn input_parse_error(error: serde_json::Error) -> AgentError {
    let message = error.to_string();
    // `from_value` errors carry no location, but strip one defensively so
    // messages stay stable across serde_json versions.
    let message = match message.find(" at line ") {
        Some(idx) => message[..idx].to_owned(),
        None => message,
    };
    if let Some(field) =
        message.strip_prefix("missing field `").and_then(|rest| rest.strip_suffix('`'))
    {
        return AgentError::ToolExecution(format!("missing '{field}' argument"));
    }
    AgentError::ToolExecution(format!("invalid arguments: {message}"))
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize, JsonSchema)]
    struct SampleArgs {
        /// Command description.
        command: String,
        #[serde(default = "default_count")]
        #[schemars(range(min = 1, max = 3))]
        count: u32,
        #[serde(default)]
        flag: bool,
        label: Option<String>,
        /// A field whose serde default (needed alongside a custom
        /// deserializer) would otherwise leak `"default": null`.
        #[serde(default)]
        window: Option<u64>,
    }

    fn default_count() -> u32 {
        2
    }

    #[derive(Debug, Deserialize, JsonSchema)]
    struct EmptyArgs {}

    #[test]
    fn schema_strips_meta_keys_and_keeps_object_shape() {
        let schema = typed_input_schema::<SampleArgs>();
        assert_eq!(schema["$schema"], Value::Null);
        assert_eq!(schema["title"], Value::Null);
        assert_eq!(schema["type"], "object");
    }

    #[test]
    fn schema_derives_keywords_from_attributes() {
        let schema = typed_input_schema::<SampleArgs>();
        // Doc comments become descriptions; declaration order is preserved.
        assert_eq!(schema["properties"]["command"]["description"], "Command description.");
        // `#[serde(default = "…")]` emits the default keyword …
        assert_eq!(schema["properties"]["count"]["default"], 2);
        assert_eq!(schema["properties"]["flag"]["default"], false);
        // … schemars range attributes emit minimum/maximum …
        assert_eq!(schema["properties"]["count"]["minimum"], 1);
        assert_eq!(schema["properties"]["count"]["maximum"], 3);
        // … and a plain `Option` field stays optional without `default: null` …
        assert!(schema["properties"]["label"].get("default").is_none());
        // … including when `#[serde(default)]` is present for deserialize
        // reasons (the null default is stripped everywhere).
        assert!(schema["properties"]["window"].get("default").is_none());
        // Only fields without a default are required.
        assert_eq!(schema["required"], serde_json::json!(["command"]));
    }

    #[test]
    fn schema_of_empty_struct_is_empty_object() {
        // Hand-written no-arg tool schemas spell out the empty properties map;
        // tests pin that exact shape (e.g. mcp_list_tools).
        assert_eq!(
            typed_input_schema::<EmptyArgs>(),
            serde_json::json!({"type": "object", "properties": {}})
        );
    }

    #[test]
    fn value_input_normalizes_to_empty_object_schema() {
        // schemars renders `serde_json::Value` as an empty schema; the provider
        // adapter only forwards object schemas, so normalize to the empty
        // object schema.
        assert_eq!(
            typed_input_schema::<Value>(),
            serde_json::json!({"type": "object", "properties": {}})
        );
    }

    #[test]
    fn parse_maps_missing_field_to_tool_argument_error() {
        let error = parse_tool_input::<SampleArgs>(&serde_json::json!({}))
            .expect_err("command is required");
        assert!(matches!(
            error,
            AgentError::ToolExecution(message) if message == "missing 'command' argument"
        ));
    }

    #[test]
    fn parse_applies_serde_defaults() {
        let args = parse_tool_input::<SampleArgs>(&serde_json::json!({"command": "ls"}))
            .expect("sample args");
        assert_eq!(args.command, "ls");
        assert_eq!(args.count, 2);
        assert!(!args.flag);
        assert_eq!(args.label, None);
    }

    #[test]
    fn parse_maps_type_mismatch_onto_serde_message() {
        let error = parse_tool_input::<SampleArgs>(&serde_json::json!({"command": 3}))
            .expect_err("command must be a string");
        assert!(matches!(
            error,
            AgentError::ToolExecution(message) if message.starts_with("invalid arguments: invalid type")
        ));
    }

    // ── blanket ToolHandler adapter ──────────────────────────────────────────

    struct SampleTypedTool;

    #[async_trait]
    impl TypedTool for SampleTypedTool {
        type Input = SampleArgs;

        fn name(&self) -> &str {
            "sample"
        }

        fn description(&self) -> &str {
            "sample typed tool"
        }

        async fn execute(
            &self,
            _ctx: &ToolContext,
            input: Self::Input,
        ) -> Result<ToolOutput, AgentError> {
            Ok(ToolOutput {
                content: format!("{}:{}:{}", input.command, input.count, input.flag),
                metadata: None,
            })
        }
    }

    #[tokio::test]
    async fn blanket_handler_parses_and_forwards_typed_execute() {
        let tool = SampleTypedTool;
        let ctx = ToolContext::for_thread("t").build();
        let output = ToolHandler::execute(&tool, &ctx, &serde_json::json!({"command": "ls"}))
            .await
            .expect("typed execute");
        assert_eq!(output.content, "ls:2:false");

        // Raw-argument metadata methods route through the adapter unchanged.
        assert!(
            ToolHandler::describe_operation(&tool, &serde_json::json!({"command": "ls"})).is_none()
        );
        assert!(!ToolHandler::is_concurrency_safe(&tool, &serde_json::json!({})));
        assert_eq!(ToolHandler::parameters_schema(&tool), typed_input_schema::<SampleArgs>());
        assert_eq!(ToolHandler::name(&tool), "sample");
    }

    #[tokio::test]
    async fn blanket_handler_maps_parse_failure_before_execute() {
        let tool = SampleTypedTool;
        let ctx = ToolContext::for_thread("t").build();
        let error =
            ToolHandler::execute(&tool, &ctx, &serde_json::json!({})).await.expect_err("missing");
        assert!(matches!(
            error,
            AgentError::ToolExecution(message) if message == "missing 'command' argument"
        ));
    }
}
