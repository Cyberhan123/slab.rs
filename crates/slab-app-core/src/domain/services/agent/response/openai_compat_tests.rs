//! Golden-fixture tests for the OpenAI-Responses projection
//! ([`super::build_response`] / [`super::envelope_to_events`]).
//!
//! Each test loads a golden fixture from
//! `testdata/fixtures/openai-compatible/responses/`, builds the matching slab
//! domain event sequence, runs it through [`super::build_response`], and
//! asserts equality AFTER normalizing both sides via
//! [`redact_dynamic_fields`].

use serde_json::Value;

use super::event::{AgentEventEnvelope, AgentEventKind, AgentResponseRef, TurnEvent};
use super::projection::{AdapterInput, build_response};
use super::stream::{StreamCtx, envelope_to_events};

const PHASE_1_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-phase.1.json"
);

/// Normalize a JSON value for golden-fixture comparison.
///
/// Two jobs:
/// 1. **Redact dynamic fields** — replace values whose key is a known dynamic
///    field (`id`, `created_at`, `usage.*` counts, `encrypted_content`, ...)
///    with a fixed mock so run-to-run variation doesn't surface as a diff.
/// 2. **Drop optional-absent noise** — remove keys whose value is `null` or an
///    empty array. The fixtures emit `null`/`[]` for absent optional fields
///    while serde `skip_serializing_if` omits them on our side; treating the
///    two as equivalent keeps the comparison structural without string fuzzing.
///
/// Applied to BOTH the expected fixture and the actual adapter output before
/// `assert_eq!`.
pub(crate) fn redact_dynamic_fields(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                let redacted = redact_value(&k, redact_dynamic_fields(v));
                match &redacted {
                    // Drop null and empty-array optional noise.
                    Value::Null => {}
                    Value::Array(arr) if arr.is_empty() => {}
                    _ => {
                        out.insert(k, redacted);
                    }
                }
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(redact_dynamic_fields).collect()),
        Value::Number(n) => canonicalize_number(n),
        other => other,
    }
}

/// Canonicalize JSON numbers so `1` (parsed from a fixture as an integer) and
/// `1.0` (serialized from an `f64`) compare equal: integral floats collapse to
/// their `i64`/`u64` form. Non-integral floats are left untouched. serde_json's
/// `Number::as_i64()` returns `None` for the f64 variant, so we route through
/// `as_f64()` and a checked cast.
fn canonicalize_number(n: serde_json::Number) -> Value {
    let f = match n.as_f64() {
        Some(f) if f.is_finite() && f.fract() == 0.0 => f,
        _ => return Value::Number(n),
    };
    if f >= 0.0 && f <= u64::MAX as f64 {
        Value::from(f as u64)
    } else if f < 0.0 && f >= i64::MIN as f64 {
        Value::from(f as i64)
    } else {
        Value::Number(n)
    }
}

/// Replace a value whose key name is a known dynamic field with its mock.
fn redact_value(key: &str, value: Value) -> Value {
    // Null values are left untouched so the caller's drop-null rule wins: a
    // fixture's `previous_response_id: null` is semantically absent and must
    // not be promoted to the redacted mock (the adapter cannot emit `null` for
    // these `Option<String>` echo fields, only omit them).
    if value.is_null() {
        return value;
    }
    // Null / non-scalar values are only redacted for keys that are always
    // scalar dynamic fields (timestamps, token counts).
    match key {
        // IDs
        "id"
        | "item_id"
        | "previous_response_id"
        | "prompt_cache_key"
        | "safety_identifier"
        | "response_id" => return Value::String("redacted_id".to_owned()),
        "call_id" => return Value::String("redacted_call_id".to_owned()),
        "container_id" => return Value::String("redacted_container_id".to_owned()),
        "file_id" => return Value::String("redacted_file_id".to_owned()),
        // Timestamps
        "created_at" | "completed_at" => {
            return Value::from(0);
        }
        // `sequence_number` is a monotonic per-stream counter that varies
        // between fixtures (some emit it, some omit it entirely). Drop it on
        // both sides so the comparison is structural.
        "sequence_number" => return Value::Null,
        // Usage counts
        "input_tokens" | "output_tokens" | "total_tokens" | "cached_tokens"
        | "reasoning_tokens" => {
            return Value::from(0);
        }
        // Opaque blobs / model output text
        "encrypted_content" | "delta" | "result" | "revised_prompt" => {
            return Value::String("redacted".to_owned());
        }
        // `output` is ambiguous: it's the Response's output ARRAY (keep as-is)
        // and also the opaque string blob on mcp_call / function_call_output
        // (redact to a fixed mock). Only redact the string form.
        "output" if value.is_string() => {
            return Value::String("redacted".to_owned());
        }
        // `obfuscation` is a dynamic per-delta token emitted by OpenAI on
        // `function_call_arguments.delta` events. slab-proto does not model it
        // (it carries no semantic content), so drop it entirely from both sides.
        "obfuscation" => return Value::Null,
        "vector_store_ids" => {
            return Value::Array(vec![Value::String("redacted_id".to_owned())]);
        }
        _ => {}
    }
    value
}

fn parse_fixture(raw: &str) -> Value {
    redact_dynamic_fields(serde_json::from_str(raw).expect("fixture parses as JSON"))
}

/// Strip `annotations` from every `output_text` content part (recursively).
/// slab-agent does not model annotation variants (`file_citation`,
/// `url_citation`, `container_file_citation`), so tests that exercise fixtures
/// carrying them normalize annotations away on BOTH the expected fixture and
/// the actual adapter output before `assert_eq!`.
fn strip_message_annotations(v: Value) -> Value {
    fn walk(v: Value) -> Value {
        match v {
            Value::Object(mut map) => {
                if map.get("type").and_then(|t| t.as_str()) == Some("output_text") {
                    map.remove("annotations");
                }
                Value::Object(map.into_iter().map(|(k, v)| (k, walk(v))).collect())
            }
            Value::Array(arr) => Value::Array(arr.into_iter().map(walk).collect()),
            other => other,
        }
    }
    walk(v)
}

/// Non-streaming `Response` with two `message` output items that
/// carry the `phase` discriminator (`commentary` / `final_answer`).
#[test]
fn non_streaming_message_with_phase_round_trips() {
    let expected = parse_fixture(PHASE_1_JSON);

    let envelopes = vec![
        envelope(AgentEventKind::ResponseOutputTextDone {
            item_id: "msg-1".to_owned(),
            output_index: 0,
            content_index: 0,
            text: COMMENTARY_TEXT.to_owned(),
            artifact_refs: vec![],
            reason: None,
            phase: Some("commentary".to_owned()),
        }),
        envelope(AgentEventKind::ResponseOutputTextDone {
            item_id: "msg-2".to_owned(),
            output_index: 1,
            content_index: 0,
            text: FINAL_ANSWER_TEXT.to_owned(),
            artifact_refs: vec![],
            reason: None,
            phase: Some("final_answer".to_owned()),
        }),
    ];

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5.3-codex",
            created_at_unix: 0.0,
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(expected, actual);
}

fn envelope(event: AgentEventKind) -> AgentEventEnvelope {
    AgentEventEnvelope { id: 0, event: TurnEvent::Response { turn_index: Some(0), event } }
}

// ---------------------------------------------------------------------------
// reasoning (encrypted_content + summary)
// ---------------------------------------------------------------------------

const REASONING_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-reasoning-encrypted-content.1.json"
);

/// Shared `{format:{type:text}, verbosity:medium}` text param used by the
/// reasoning and function-call fixtures.
fn text_format_param() -> slab_proto::openai::ResponseTextParam {
    slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: Some(Some(slab_proto::openai::Verbosity::Medium)),
    }
}

/// Build the `calculator` function-tool definition matching the reasoning
/// fixture. Constructed via JSON-parsed `HashMap<String, Value>` so the
/// nested JSON-schema `parameters` round-trips verbatim.
fn calculator_tool() -> slab_proto::openai::Tool {
    let parameters: std::collections::HashMap<String, Value> = serde_json::from_str(
        r#"{
            "type":"object",
            "properties":{
                "a":{"type":"number","description":"First operand."},
                "b":{"type":"number","description":"Second operand."},
                "op":{"type":"string","enum":["add","subtract","multiply","divide"],"default":"add","description":"Arithmetic operation to perform."}
            },
            "required":["a","b","op"],
            "additionalProperties":false
        }"#,
    )
    .expect("calculator params parse");

    slab_proto::openai::Tool::FunctionTool(Box::new(slab_proto::openai::FunctionTool {
        name: "calculator".to_owned(),
        strict: Some(true),
        description: Some(Some(
            "A minimal calculator for basic arithmetic. Call it once per step.".to_owned(),
        )),
        parameters: Some(parameters),
        ..Default::default()
    }))
}

#[test]
fn non_streaming_reasoning_round_trips() {
    let expected = parse_fixture(REASONING_JSON);

    let reasoning_text = "step one";
    let reasoning_summary = "**Reporting final result**\n\nThe tool returned 570, and now I need to report this final \
         result. The user asked for a clear breakdown, so I'll include the steps taken: \
         first, I added 12 and 7 to get 19; then, I multiplied 19 by 3 for 57; finally, I \
         multiplied 57 by 10 to arrive at 570. I want to keep it concise, so I'll simply \
         say, \"Final result: 570,\" without heavy formatting. Let's finalize \
         that!";
    let message_text = "12 + 7 = 19\n19 \u{d7} 3 = 57\n57 \u{d7} 10 = 570\n\nFinal result: 570";

    let envelopes = vec![
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_1".to_owned(),
            output_index: 0,
            content_index: 0,
            text: reasoning_text.to_owned(),
            encrypted_content: Some("opaque".to_owned()),
            summary: Some(reasoning_summary.to_owned()),
        }),
        envelope(AgentEventKind::ResponseOutputTextDone {
            item_id: "msg_1".to_owned(),
            output_index: 1,
            content_index: 0,
            text: message_text.to_owned(),
            artifact_refs: vec![],
            reason: None,
            phase: None,
        }),
    ];

    let billing = serde_json::json!({ "payer": "developer" });
    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::High)),
        summary: Some(Some(slab_proto::openai::Summary::Detailed)),
        ..Default::default()
    };

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5-mini-2025-08-07",
            created_at_unix: 0.0,
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(billing),
            store: Some(false),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(vec![calculator_tool()]),
            text: Some(text_format_param()),
            reasoning: Some(reasoning),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(expected, actual);
}

const COMMENTARY_TEXT: &str = "I\u{2019}ll quickly check reliable, up-to-date sources (major tech/news outlets and company blogs) to pull the most recent AI headlines for today, then summarize them for you with links.";

// ---------------------------------------------------------------------------
// Function call output item
// ---------------------------------------------------------------------------

const FUNCTION_CALL_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-client-tool-search.2.json"
);

/// Parse a JSON object literal into a `HashMap<String, Value>` for a tool's
/// free-form `parameters` schema.
fn params_map(raw: &str) -> std::collections::HashMap<String, Value> {
    serde_json::from_str(raw).expect("tool parameters parse as object")
}

/// `search_files` function tool with `defer_loading: true` (fixture mirror).
fn search_files_tool() -> slab_proto::openai::Tool {
    slab_proto::openai::Tool::FunctionTool(Box::new(slab_proto::openai::FunctionTool {
        name: "search_files".to_owned(),
        strict: Some(true),
        description: Some(Some("Search through files in the workspace".to_owned())),
        defer_loading: Some(true),
        parameters: Some(params_map(
            r#"{
                "type":"object",
                "properties":{
                    "query":{"type":"string","description":"The search query"},
                    "file_types":{"type":"array","items":{"type":"string"},"description":"Filter by file types"}
                },
                "required":["query","file_types"],
                "additionalProperties":false
            }"#,
        )),
    }))
}

/// `get_weather` function tool (fixture mirror). `defer_loading` varies by
/// fixture: `Some(true)` in `tool_search.1` / `client-tool-search.1`, absent
/// (`None`) in `client-tool-search.2`.
fn get_weather_function_tool(defer_loading: Option<bool>) -> slab_proto::openai::Tool {
    slab_proto::openai::Tool::FunctionTool(Box::new(slab_proto::openai::FunctionTool {
        name: "get_weather".to_owned(),
        strict: Some(true),
        defer_loading,
        description: Some(Some("Get the current weather at a specific location".to_owned())),
        parameters: Some(params_map(
            r#"{
                "type":"object",
                "properties":{
                    "location":{"type":"string","description":"The city and state, e.g. San Francisco, CA"},
                    "unit":{"type":"string","enum":["celsius","fahrenheit"],"description":"Temperature unit"}
                },
                "required":["location","unit"],
                "additionalProperties":false
            }"#,
        )),
    }))
}

/// `tool_search` tool param executed by the client (fixture mirror).
fn tool_search_param() -> slab_proto::openai::Tool {
    slab_proto::openai::Tool::ToolSearchToolParam(Box::new(
        slab_proto::openai::ToolSearchToolParam {
            execution: Some(slab_proto::openai::ToolSearchExecutionType::Client),
            description: Some(Some(
                "Search for available tools based on what the user needs.".to_owned(),
            )),
            parameters: Some(Some(serde_json::json!({
                "type":"object",
                "properties":{
                    "goal":{"type":"string","description":"What the user is trying to accomplish"}
                },
                "required":["goal"],
                "additionalProperties":false
            }))),
        },
    ))
}

/// `namespace` tool param grouping a single `get_weather` function (fixture mirror).
fn namespace_param() -> slab_proto::openai::Tool {
    slab_proto::openai::Tool::NamespaceToolParam(Box::new(slab_proto::openai::NamespaceToolParam {
        name: "get_weather".to_owned(),
        description: "Get the current weather at a specific location".to_owned(),
        tools: vec![slab_proto::openai::NamespaceToolParamToolsInner::FunctionToolParam(Box::new(
            slab_proto::openai::FunctionToolParam {
                name: "get_weather".to_owned(),
                description: Some(Some(
                    "Get the current weather at a specific location".to_owned(),
                )),
                parameters: Some(Some(serde_json::json!({
                    "type":"object",
                    "properties":{
                        "location":{"type":"string","description":"The city and state, e.g. San Francisco, CA"},
                        "unit":{"type":"string","enum":["celsius","fahrenheit"],"description":"Temperature unit"}
                    },
                    "required":["location","unit"],
                    "additionalProperties":false
                }))),
                strict: Some(Some(true)),
                ..Default::default()
            },
        ))],
    }))
}

#[test]
fn non_streaming_function_call_round_trips() {
    let expected = parse_fixture(FUNCTION_CALL_JSON);

    let arguments = "{\"location\":\"San Francisco, CA\",\"unit\":\"fahrenheit\"}";
    let envelopes = vec![envelope(AgentEventKind::ResponseFunctionCallArgumentsDone {
        item_id: "fc_1".to_owned(),
        call_id: "call_test".to_owned(),
        name: "get_weather".to_owned(),
        output_index: 0,
        arguments: arguments.to_owned(),
        namespace: None,
        risk: None,
    })];

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::None)),
        ..Default::default()
    };

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5.4-2026-03-05",
            created_at_unix: 0.0,
            completed_at: Some(0.0),
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(false),
            temperature: Some(1.0),
            top_p: Some(0.98),
            top_logprobs: Some(0),
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(vec![
                search_files_tool(),
                get_weather_function_tool(None),
                tool_search_param(),
                namespace_param(),
            ]),
            text: Some(text_format_param()),
            reasoning: Some(reasoning),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(expected, actual);
}

// ---------------------------------------------------------------------------
// Streaming (envelope_to_events + StreamCtx)
// ---------------------------------------------------------------------------

const PHASE_1_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-phase.1.chunks.txt"
);

/// Parse each line of a `.chunks.txt` into a redacted `Value`, collecting the
/// fixture's expected SSE event sequence.
fn redacted_chunks(raw: &str) -> Vec<Value> {
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| redact_dynamic_fields(serde_json::from_str(l).expect("chunk line parses")))
        .collect()
}

/// Drive a sequence of slab envelopes through [`envelope_to_events`] and collect
/// the redacted serialized events.
fn redacted_stream_events(envelopes: &[AgentEventEnvelope], ctx: &mut StreamCtx) -> Vec<Value> {
    let mut out = Vec::new();
    for env in envelopes {
        for ev in envelope_to_events(env, ctx) {
            let v = serde_json::to_value(&ev).expect("event serializes");
            out.push(redact_dynamic_fields(v));
        }
    }
    out
}

/// Slab's `ResponseOutputTextDelta` carries no `phase` discriminator, so the
/// adapter's synthesized `response.output_item.added` skeleton (fired on the
/// first delta) cannot include `phase` even though the OpenAI fixture carries
/// it there. Strip `phase` from the added skeleton on both sides so the
/// comparison exercises the streaming state machine rather than this slab-agent
/// modeling gap.
fn normalize_added_skeleton_phase(events: Vec<Value>) -> Vec<Value> {
    events
        .into_iter()
        .map(|mut ev| {
            let is_added =
                ev.get("type").and_then(|t| t.as_str()) == Some("response.output_item.added");
            if is_added {
                let item_obj = ev.get_mut("item").and_then(|i| i.as_object_mut());
                if let Some(item) = item_obj {
                    item.remove("phase");
                }
            }
            ev
        })
        .collect()
}

fn str_field<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("")
}

fn i32_field(v: &Value, key: &str) -> i32 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0) as i32
}

/// Derive the slab envelope sequence that feeds [`envelope_to_events`] from a
/// fixture's `.chunks.txt`. Maps lifecycle events 1:1, text deltas/done to the
/// matching slab events, and skips the wrapper events the adapter synthesizes
/// (`output_item.added`, `content_part.added/done`, `output_item.done`).
///
/// `phase` for each message item is recovered from the `output_item.added`
/// wrapper (the slab `ResponseOutputTextDelta` event does not carry it).
fn envelopes_from_chunks(raw: &str) -> Vec<AgentEventEnvelope> {
    use slab_types::agent::AgentThreadStatus;

    let mut item_phase: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut envelopes: Vec<AgentEventEnvelope> = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(line).expect("chunk line parses");
        let ty = str_field(&v, "type");
        let event = match ty {
            "response.created" => Some(AgentEventKind::ResponseQueued {
                response: AgentResponseRef {
                    id: "resp_test".to_owned(),
                    status: AgentThreadStatus::Pending,
                },
            }),
            "response.in_progress" => Some(AgentEventKind::ResponseInProgress {
                response: AgentResponseRef {
                    id: "resp_test".to_owned(),
                    status: AgentThreadStatus::Running,
                },
            }),
            "response.completed" => Some(AgentEventKind::ResponseCompleted {
                response: AgentResponseRef {
                    id: "resp_test".to_owned(),
                    status: AgentThreadStatus::Running,
                },
            }),
            "response.output_item.added" => {
                if let Some(item) = v.get("item") {
                    let item_id = str_field(item, "id");
                    if let Some(phase) = item.get("phase").and_then(|p| p.as_str()) {
                        item_phase.insert(item_id.to_owned(), phase.to_owned());
                    }
                }
                None
            }
            "response.output_text.delta" => {
                let item_id = str_field(&v, "item_id").to_owned();
                Some(AgentEventKind::ResponseOutputTextDelta {
                    item_id,
                    output_index: i32_field(&v, "output_index"),
                    content_index: i32_field(&v, "content_index"),
                    delta: v.get("delta").and_then(|d| d.as_str()).unwrap_or("").to_owned(),
                })
            }
            "response.output_text.done" => {
                let item_id = str_field(&v, "item_id").to_owned();
                let phase = item_phase.get(&item_id).cloned();
                Some(AgentEventKind::ResponseOutputTextDone {
                    item_id,
                    output_index: i32_field(&v, "output_index"),
                    content_index: i32_field(&v, "content_index"),
                    text: v.get("text").and_then(|t| t.as_str()).unwrap_or("").to_owned(),
                    artifact_refs: vec![],
                    reason: None,
                    phase,
                })
            }
            _ => None,
        };
        if let Some(event) = event {
            envelopes.push(AgentEventEnvelope {
                id: 0,
                event: TurnEvent::Response { turn_index: Some(0), event },
            });
        }
    }
    envelopes
}

/// Streaming text round-trip against `openai-phase.1.chunks.txt`.
///
/// Slab's `ResponseOutputTextDelta` carries no `phase` discriminator, so the
/// `phase` shown on the fixture's `response.output_item.added` wrapper is
/// recovered from that wrapper line in the chunks and threaded onto the
/// slab `ResponseOutputTextDone` event. The adapter then emits it on the
/// `output_item.done` and `response.completed` payloads.
#[test]
fn streaming_text_round_trips() {
    let expected = normalize_added_skeleton_phase(redacted_chunks(PHASE_1_CHUNKS));
    let envelopes = envelopes_from_chunks(PHASE_1_CHUNKS);

    let mut ctx = StreamCtx::new(
        "resp_test".to_owned(),
        "gpt-5.3-codex".to_owned(),
        0.0,
        Some(slab_proto::openai::ServiceTier::Auto),
    );
    // The created/in_progress skeleton echoes the unresolved request tier
    // (`auto`); the completed event echoes the resolved tier (`default`).
    ctx.set_completed_service_tier(Some(slab_proto::openai::ServiceTier::Default));

    let actual = normalize_added_skeleton_phase(redacted_stream_events(&envelopes, &mut ctx));

    assert_eq!(expected, actual);
}

const FINAL_ANSWER_TEXT: &str = "Here are some **latest AI updates for today (Wednesday, February 25, 2026)** based on recent postings:\n\n### Top headline\n- **Anthropic announced it is acquiring Vercept** to improve Claude\u{2019}s computer-use/agent capabilities (posted **Feb 25, 2026**).  \n  Source: Anthropic Newsroom: https://www.anthropic.com/news\n\n### Other very recent AI items (this week)\n- **Anthropic updated its Responsible Scaling Policy to v3.0** (**Feb 24, 2026**).  \n- **Anthropic posted work on detecting/preventing model distillation attacks** (**Feb 23, 2026**).  \n- **Anthropic announced cybersecurity capability access for defenders** (**Feb 20, 2026**).  \n- **Claude Sonnet 4.6 introduced** (**Feb 17, 2026**).  \n  Source for all above: https://www.anthropic.com/news\n\n### Bigger recent context\n- **Anthropic disclosed a major Series G funding round** (**Feb 12, 2026**) with a very large valuation, signaling continued heavy investment in frontier AI.  \n  Source: https://www.anthropic.com/news\n\n---\n\nIf you want, I can also give you a **broader \u{201C}today in AI\u{201D} roundup** across OpenAI, Google DeepMind, Microsoft, Meta, xAI, NVIDIA, and major regulators (US/EU/UK) with only same-day / last-48-hours items.";

// ---------------------------------------------------------------------------
// Streaming reasoning with summary text deltas
// (openai-reasoning-encrypted-content.1.chunks.txt)
//
// The slab `ResponseReasoningTextDelta`/`.done` events are mapped onto the
// canonical `response.reasoning_summary_text.delta`/`.done` stream wrapped by
// `response.reasoning_summary_part.added`/`.done`. The fixture is large
// (~100 deltas); reasoning skeleton's `encrypted_content` arrives with the
// `Done` event in slab (not on deltas), so the eager skeleton in
// `output_item.added` is normalized away on both sides.
// ---------------------------------------------------------------------------

const REASONING_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-reasoning-encrypted-content.1.chunks.txt"
);

/// Slab's `ResponseReasoningTextDelta` does not carry `encrypted_content`, so
/// the adapter's `output_item.added` reasoning skeleton cannot include it
/// (even though the OpenAI fixture does). Strip `encrypted_content` from the
/// reasoning item in `output_item.added` on both sides.
fn normalize_reasoning_added_encrypted(events: Vec<Value>) -> Vec<Value> {
    events
        .into_iter()
        .map(|mut ev| {
            let is_added =
                ev.get("type").and_then(|t| t.as_str()) == Some("response.output_item.added");
            if is_added {
                let item_obj = ev.get_mut("item").and_then(|i| i.as_object_mut());
                if let Some(item) = item_obj
                    && item.get("type").and_then(|t| t.as_str()) == Some("reasoning")
                {
                    item.remove("encrypted_content");
                }
            }
            ev
        })
        .collect()
}

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// custom_tool_call (openai-custom-tool.1.{json,chunks.txt})
// ---------------------------------------------------------------------------

const CUSTOM_TOOL_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-custom-tool.1.json"
);
const CUSTOM_TOOL_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-custom-tool.1.chunks.txt"
);

/// The `write_sql` custom tool param with the grammar/regex format (fixture
/// mirror).
fn write_sql_tool() -> slab_proto::openai::Tool {
    use slab_proto::openai::{CustomGrammarFormatParam, CustomToolParam, CustomToolParamFormat};

    slab_proto::openai::Tool::CustomToolParam(Box::new(CustomToolParam {
        name: "write_sql".to_owned(),
        description: Some("Write a SQL SELECT query to answer the user question.".to_owned()),
        format: Some(Box::new(CustomToolParamFormat::CustomGrammarFormatParam(Box::new(
            CustomGrammarFormatParam {
                syntax: slab_proto::openai::GrammarSyntax1::Regex,
                definition: "SELECT .+".to_owned(),
            },
        )))),
        ..Default::default()
    }))
}

#[test]
fn custom_tool_non_streaming_round_trips() {
    let expected = parse_fixture(CUSTOM_TOOL_JSON);

    let envelopes = vec![envelope(AgentEventKind::ResponseCustomToolCallInputDone {
        item_id: "ct_1".to_owned(),
        call_id: "call_test".to_owned(),
        name: "write_sql".to_owned(),
        output_index: 0,
        input: "SELECT * FROM users WHERE age > 25".to_owned(),
        namespace: None,
    })];

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::Low)),
        ..Default::default()
    };
    let text = slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: None,
    };

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5.2-codex",
            created_at_unix: 0.0,
            service_tier: None,
            envelopes: &envelopes,
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Required,
            )),
            tools: Some(vec![write_sql_tool()]),
            text: Some(text),
            reasoning: Some(reasoning),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            input: Some(Vec::new()),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(expected, actual);
}

#[test]
fn custom_tool_streaming_round_trips() {
    let expected = redacted_chunks(CUSTOM_TOOL_CHUNKS);

    let deltas = vec!["SELECT * ", "FROM users ", "WHERE age > 25"];
    let final_input = "SELECT * FROM users WHERE age > 25";

    let mut envelopes = vec![
        envelope(AgentEventKind::ResponseQueued {
            response: AgentResponseRef {
                id: "resp_test".to_owned(),
                status: slab_types::agent::AgentThreadStatus::Pending,
            },
        }),
        envelope(AgentEventKind::ResponseInProgress {
            response: AgentResponseRef {
                id: "resp_test".to_owned(),
                status: slab_types::agent::AgentThreadStatus::Running,
            },
        }),
    ];
    for d in &deltas {
        envelopes.push(envelope(AgentEventKind::ResponseCustomToolCallInputDelta {
            item_id: "ct_1".to_owned(),
            call_id: "call_test".to_owned(),
            name: "write_sql".to_owned(),
            output_index: 0,
            delta: (*d).to_owned(),
        }));
    }
    envelopes.push(envelope(AgentEventKind::ResponseCustomToolCallInputDone {
        item_id: "ct_1".to_owned(),
        call_id: "call_test".to_owned(),
        name: "write_sql".to_owned(),
        output_index: 0,
        input: final_input.to_owned(),
        namespace: None,
    }));
    envelopes.push(envelope(AgentEventKind::ResponseCompleted {
        response: AgentResponseRef {
            id: "resp_test".to_owned(),
            status: slab_types::agent::AgentThreadStatus::Completed,
        },
    }));

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::Low)),
        ..Default::default()
    };
    let text = slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: None,
    };

    let mut ctx = StreamCtx::new("resp_test".to_owned(), "gpt-5.2-codex".to_owned(), 0.0, None);
    ctx.set_skeleton(slab_proto::openai::Response {
        id: "resp_test".to_owned(),
        object: slab_proto::openai::ResponseObject::Response,
        created_at: 0.0,
        status: Some(slab_proto::openai::ResponseStatus::InProgress),
        model: Some(Box::new(slab_proto::openai::ModelIdsResponses::StringValue(
            "gpt-5.2-codex".to_owned(),
        ))),
        store: Some(true),
        temperature: Some(1.0),
        top_p: Some(1.0),
        truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
        tool_choice: Some(Box::new(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
            slab_proto::openai::ToolChoiceOptions::Required,
        ))),
        tools: Some(vec![write_sql_tool()]),
        text: Some(Box::new(text)),
        reasoning: Some(Box::new(reasoning)),
        metadata: Some(std::collections::HashMap::new()),
        parallel_tool_calls: Some(true),
        ..Default::default()
    });

    let actual = redacted_stream_events(&envelopes, &mut ctx);

    assert_eq!(expected, actual);
}

// ---------------------------------------------------------------------------
// Local shell call (openai-local-shell-tool.1.{json,chunks.txt})
// ---------------------------------------------------------------------------

const LOCAL_SHELL_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-local-shell-tool.1.json"
);
const LOCAL_SHELL_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-local-shell-tool.1.chunks.txt"
);

#[test]
fn local_shell_non_streaming_round_trips() {
    let expected = parse_fixture(LOCAL_SHELL_JSON);

    let envelopes = vec![
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_1".to_owned(),
            output_index: 0,
            content_index: 0,
            text: String::new(),
            encrypted_content: None,
            summary: None,
        }),
        envelope(AgentEventKind::ResponseLocalShellCallDone {
            item_id: "lsh_1".to_owned(),
            call_id: "call_test".to_owned(),
            output_index: 1,
            command: vec!["ls".to_owned()],
            env: std::collections::HashMap::new(),
            working_directory: Some("/root".to_owned()),
        }),
    ];

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::Medium)),
        ..Default::default()
    };
    let text = slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: Some(Some(slab_proto::openai::Verbosity::Medium)),
    };

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5-codex",
            created_at_unix: 0.0,
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(Vec::new()),
            text: Some(text),
            reasoning: Some(reasoning),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(expected, actual);
}

#[test]
fn local_shell_streaming_round_trips() {
    let expected = redacted_chunks(LOCAL_SHELL_CHUNKS);

    let envelopes = vec![
        envelope(AgentEventKind::ResponseQueued {
            response: AgentResponseRef {
                id: "resp_test".to_owned(),
                status: slab_types::agent::AgentThreadStatus::Pending,
            },
        }),
        envelope(AgentEventKind::ResponseInProgress {
            response: AgentResponseRef {
                id: "resp_test".to_owned(),
                status: slab_types::agent::AgentThreadStatus::Running,
            },
        }),
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_1".to_owned(),
            output_index: 0,
            content_index: 0,
            text: String::new(),
            encrypted_content: None,
            summary: None,
        }),
        envelope(AgentEventKind::ResponseLocalShellCallDone {
            item_id: "lsh_1".to_owned(),
            call_id: "call_test".to_owned(),
            output_index: 1,
            command: vec!["ls".to_owned(), "-a".to_owned(), "~".to_owned()],
            env: std::collections::HashMap::new(),
            working_directory: None,
        }),
        envelope(AgentEventKind::ResponseCompleted {
            response: AgentResponseRef {
                id: "resp_test".to_owned(),
                status: slab_types::agent::AgentThreadStatus::Completed,
            },
        }),
    ];

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::Medium)),
        ..Default::default()
    };
    let text = slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: Some(Some(slab_proto::openai::Verbosity::Medium)),
    };

    let mut ctx = StreamCtx::new(
        "resp_test".to_owned(),
        "gpt-5-codex".to_owned(),
        0.0,
        Some(slab_proto::openai::ServiceTier::Auto),
    );
    ctx.set_completed_service_tier(Some(slab_proto::openai::ServiceTier::Default));
    ctx.set_skeleton(slab_proto::openai::Response {
        id: "resp_test".to_owned(),
        object: slab_proto::openai::ResponseObject::Response,
        created_at: 0.0,
        status: Some(slab_proto::openai::ResponseStatus::InProgress),
        model: Some(Box::new(slab_proto::openai::ModelIdsResponses::StringValue(
            "gpt-5-codex".to_owned(),
        ))),
        service_tier: Some(Some(slab_proto::openai::ServiceTier::Auto)),
        background: Some(false),
        store: Some(true),
        temperature: Some(1.0),
        top_p: Some(1.0),
        top_logprobs: Some(0),
        truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
        tool_choice: Some(Box::new(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
            slab_proto::openai::ToolChoiceOptions::Auto,
        ))),
        tools: Some(Vec::new()),
        text: Some(Box::new(text)),
        reasoning: Some(Box::new(reasoning)),
        metadata: Some(std::collections::HashMap::new()),
        parallel_tool_calls: Some(true),
        ..Default::default()
    });

    let actual = redacted_stream_events(&envelopes, &mut ctx);

    assert_eq!(expected, actual);
}
//                              openai-apply-patch-tool-delete.1.chunks.txt)
// ---------------------------------------------------------------------------

const APPLY_PATCH_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-apply-patch-tool.1.json"
);
const APPLY_PATCH_DELETE_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-apply-patch-tool-delete.1.chunks.txt"
);

#[test]
fn apply_patch_non_streaming_round_trips() {
    let expected = parse_fixture(APPLY_PATCH_JSON);

    let diff = "+## Shopping Checklist\n+\n+- [ ] Milk\n+- [ ] Bread\n+- [ ] Eggs\n+- [ ] Apples\n+- [ ] Coffee\n+\n";
    let envelopes = vec![envelope(AgentEventKind::ResponseApplyPatchCallDone {
        item_id: "apc_1".to_owned(),
        call_id: "call_test".to_owned(),
        output_index: 0,
        operation_type: "create_file".to_owned(),
        path: "shopping-checklist.md".to_owned(),
        diff: Some(diff.to_owned()),
    })];

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::None)),
        ..Default::default()
    };
    let text = slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: Some(Some(slab_proto::openai::Verbosity::Medium)),
    };

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5.1-2025-11-13",
            created_at_unix: 0.0,
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(vec![slab_proto::openai::Tool::ApplyPatchToolParam(Box::new(
                slab_proto::openai::ApplyPatchToolParam::new(),
            ))]),
            text: Some(text),
            reasoning: Some(reasoning),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(expected, actual);
}

#[test]
fn apply_patch_delete_streaming_round_trips() {
    let expected = redacted_chunks(APPLY_PATCH_DELETE_CHUNKS);

    let envelopes = vec![
        envelope(AgentEventKind::ResponseQueued {
            response: AgentResponseRef {
                id: "resp_test".to_owned(),
                status: slab_types::agent::AgentThreadStatus::Pending,
            },
        }),
        envelope(AgentEventKind::ResponseInProgress {
            response: AgentResponseRef {
                id: "resp_test".to_owned(),
                status: slab_types::agent::AgentThreadStatus::Running,
            },
        }),
        envelope(AgentEventKind::ResponseApplyPatchCallDone {
            item_id: "apc_delete_001".to_owned(),
            call_id: "call_delete_1".to_owned(),
            output_index: 0,
            operation_type: "delete_file".to_owned(),
            path: "obsolete.txt".to_owned(),
            diff: None,
        }),
        envelope(AgentEventKind::ResponseCompleted {
            response: AgentResponseRef {
                id: "resp_test".to_owned(),
                status: slab_types::agent::AgentThreadStatus::Completed,
            },
        }),
    ];

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::None)),
        ..Default::default()
    };
    let text = slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: Some(Some(slab_proto::openai::Verbosity::Medium)),
    };

    let mut ctx = StreamCtx::new(
        "resp_test".to_owned(),
        "gpt-5.1-2025-11-13".to_owned(),
        0.0,
        Some(slab_proto::openai::ServiceTier::Default),
    );
    ctx.set_skeleton(slab_proto::openai::Response {
        id: "resp_test".to_owned(),
        object: slab_proto::openai::ResponseObject::Response,
        created_at: 0.0,
        status: Some(slab_proto::openai::ResponseStatus::InProgress),
        model: Some(Box::new(slab_proto::openai::ModelIdsResponses::StringValue(
            "gpt-5.1-2025-11-13".to_owned(),
        ))),
        service_tier: Some(Some(slab_proto::openai::ServiceTier::Default)),
        background: Some(false),
        store: Some(true),
        temperature: Some(1.0),
        top_p: Some(1.0),
        top_logprobs: Some(0),
        truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
        tool_choice: Some(Box::new(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
            slab_proto::openai::ToolChoiceOptions::Auto,
        ))),
        tools: Some(vec![slab_proto::openai::Tool::ApplyPatchToolParam(Box::new(
            slab_proto::openai::ApplyPatchToolParam::new(),
        ))]),
        text: Some(Box::new(text)),
        reasoning: Some(Box::new(reasoning)),
        metadata: Some(std::collections::HashMap::new()),
        parallel_tool_calls: Some(true),
        ..Default::default()
    });

    let actual = redacted_stream_events(&envelopes, &mut ctx);

    assert_eq!(expected, actual);
}

// ---------------------------------------------------------------------------
// Streaming error (openai-error.1.chunks.txt)
//
// Skeleton echoes the full request config; the standalone nested `error` event
// fires before `response.failed`, and `response.failed` carries the simplified
// `{code,message}` error (no `type`/`param`).
// ---------------------------------------------------------------------------

const ERROR_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-error.1.chunks.txt"
);

/// Build the full-form skeleton `Response` mirroring the error fixture's echoed
/// request config (background/reasoning/service_tier/store/temperature/text/
/// tool_choice/tools/top_logprobs/top_p/truncation/metadata/parallel_tool_calls).
fn error_fixture_skeleton() -> slab_proto::openai::Response {
    use slab_proto::openai::{
        Reasoning, ReasoningEffort, ResponseTextParam, ResponseTruncation, ServiceTier,
        TextResponseFormatConfiguration, ToolChoiceOptions, ToolChoiceParam, Verbosity,
    };

    let reasoning = Reasoning { effort: Some(Some(ReasoningEffort::Medium)), ..Default::default() };
    let text = ResponseTextParam {
        format: Some(Box::new(TextResponseFormatConfiguration::ResponseFormatText(Box::default()))),
        verbosity: Some(Some(Verbosity::Medium)),
    };

    slab_proto::openai::Response {
        id: "resp_test".to_owned(),
        object: slab_proto::openai::ResponseObject::Response,
        created_at: 0.0,
        status: Some(slab_proto::openai::ResponseStatus::InProgress),
        model: Some(Box::new(slab_proto::openai::ModelIdsResponses::StringValue(
            "gpt-5-nano-2025-08-07".to_owned(),
        ))),
        service_tier: Some(Some(ServiceTier::Auto)),
        background: Some(false),
        reasoning: Some(Box::new(reasoning)),
        store: Some(true),
        temperature: Some(1.0),
        text: Some(Box::new(text)),
        tool_choice: Some(Box::new(ToolChoiceParam::ToolChoiceOptions(ToolChoiceOptions::Auto))),
        tools: Some(Vec::new()),
        top_logprobs: Some(0),
        top_p: Some(1.0),
        truncation: Some(ResponseTruncation::Disabled),
        metadata: Some(std::collections::HashMap::new()),
        parallel_tool_calls: Some(true),
        ..Default::default()
    }
}

#[test]
fn streaming_error_round_trips() {
    let expected = redacted_chunks(ERROR_CHUNKS);

    let envelopes = vec![
        envelope(AgentEventKind::ResponseQueued {
            response: AgentResponseRef {
                id: "resp_test".to_owned(),
                status: slab_types::agent::AgentThreadStatus::Pending,
            },
        }),
        envelope(AgentEventKind::ResponseInProgress {
            response: AgentResponseRef {
                id: "resp_test".to_owned(),
                status: slab_types::agent::AgentThreadStatus::Running,
            },
        }),
        envelope(AgentEventKind::ResponseFailed {
            response: AgentResponseRef {
                id: "resp_test".to_owned(),
                status: slab_types::agent::AgentThreadStatus::Errored,
            },
            error: "You exceeded your current quota, please check your plan and billing details. \
                    For more information on this error, read the docs: \
                    https://platform.openai.com/docs/guides/error-codes/api-errors."
                .to_owned(),
            error_code: Some("insufficient_quota".to_owned()),
            error_type: Some("insufficient_quota".to_owned()),
        }),
    ];

    let mut ctx = StreamCtx::new(
        "resp_test".to_owned(),
        "gpt-5-nano-2025-08-07".to_owned(),
        0.0,
        Some(slab_proto::openai::ServiceTier::Auto),
    );
    ctx.set_skeleton(error_fixture_skeleton());

    let actual = redacted_stream_events(&envelopes, &mut ctx);

    assert_eq!(expected, actual);
}

// ---------------------------------------------------------------------------
// Streaming function_call with arguments deltas
// (openai-client-tool-search.2.chunks.txt)
//
// `response.function_call_arguments.delta` events are synthesized from the
// slab-agent `ResponseFunctionCallArgumentsDelta` variant. The
// `output_item.added` wrapper fires on the first delta with the in_progress
// function-call skeleton.
// ---------------------------------------------------------------------------

const FUNCTION_CALL_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-client-tool-search.2.chunks.txt"
);

#[test]
fn streaming_function_call_arguments_delta_round_trips() {
    let expected = redacted_chunks(FUNCTION_CALL_CHUNKS);

    let arguments = "{\"location\":\"San Francisco, CA\",\"unit\":\"fahrenheit\"}";
    let item_id = "fc_test".to_owned();
    let call_id = "call_test".to_owned();
    let name = "get_weather".to_owned();

    // Split the arguments into the same delta sequence the fixture emits.
    let deltas = vec![
        "{\"",
        "location",
        "\":\"",
        "San",
        " Francisco",
        ",",
        " CA",
        "\",\"",
        "unit",
        "\":\"",
        "fahren",
        "heit",
        "\"}",
    ];

    let mut envelopes = vec![
        envelope(AgentEventKind::ResponseQueued {
            response: AgentResponseRef {
                id: "resp_test".to_owned(),
                status: slab_types::agent::AgentThreadStatus::Pending,
            },
        }),
        envelope(AgentEventKind::ResponseInProgress {
            response: AgentResponseRef {
                id: "resp_test".to_owned(),
                status: slab_types::agent::AgentThreadStatus::Running,
            },
        }),
    ];
    for d in &deltas {
        envelopes.push(envelope(AgentEventKind::ResponseFunctionCallArgumentsDelta {
            item_id: item_id.clone(),
            call_id: call_id.clone(),
            name: name.clone(),
            output_index: 0,
            delta: (*d).to_owned(),
        }));
    }
    envelopes.push(envelope(AgentEventKind::ResponseFunctionCallArgumentsDone {
        item_id: item_id.clone(),
        call_id,
        name,
        output_index: 0,
        arguments: arguments.to_owned(),
        namespace: None,
        risk: None,
    }));
    envelopes.push(envelope(AgentEventKind::ResponseCompleted {
        response: AgentResponseRef {
            id: "resp_test".to_owned(),
            status: slab_types::agent::AgentThreadStatus::Completed,
        },
    }));

    let mut ctx = StreamCtx::new(
        "resp_test".to_owned(),
        "gpt-5.4-2026-03-05".to_owned(),
        0.0,
        Some(slab_proto::openai::ServiceTier::Auto),
    );
    ctx.set_completed_service_tier(Some(slab_proto::openai::ServiceTier::Default));
    ctx.set_completed_at(Some(0.0));
    ctx.set_skeleton(function_call_fixture_skeleton());

    let actual = redacted_stream_events(&envelopes, &mut ctx);

    assert_eq!(expected, actual);
}

/// Build the full-form skeleton `Response` mirroring the function-call
/// fixture's echoed request config (background/store/temperature/text/
/// tool_choice/tools/top_logprobs/top_p/truncation/metadata/parallel_tool_calls
/// /reasoning=none/frequency_penalty/presence_penalty).
fn function_call_fixture_skeleton() -> slab_proto::openai::Response {
    use slab_proto::openai::{
        Reasoning, ReasoningEffort, ResponseTextParam, ResponseTruncation, ServiceTier,
        TextResponseFormatConfiguration, ToolChoiceOptions, ToolChoiceParam, Verbosity,
    };

    let reasoning = Reasoning { effort: Some(Some(ReasoningEffort::None)), ..Default::default() };
    let text = ResponseTextParam {
        format: Some(Box::new(TextResponseFormatConfiguration::ResponseFormatText(Box::default()))),
        verbosity: Some(Some(Verbosity::Medium)),
    };

    slab_proto::openai::Response {
        id: "resp_test".to_owned(),
        object: slab_proto::openai::ResponseObject::Response,
        created_at: 0.0,
        status: Some(slab_proto::openai::ResponseStatus::InProgress),
        model: Some(Box::new(slab_proto::openai::ModelIdsResponses::StringValue(
            "gpt-5.4-2026-03-05".to_owned(),
        ))),
        service_tier: Some(Some(ServiceTier::Auto)),
        background: Some(false),
        reasoning: Some(Box::new(reasoning)),
        store: Some(true),
        temperature: Some(1.0),
        text: Some(Box::new(text)),
        tool_choice: Some(Box::new(ToolChoiceParam::ToolChoiceOptions(ToolChoiceOptions::Auto))),
        tools: Some(vec![
            search_files_tool(),
            get_weather_function_tool(None),
            tool_search_param(),
            namespace_param(),
        ]),
        top_logprobs: Some(0),
        top_p: Some(0.98),
        truncation: Some(ResponseTruncation::Disabled),
        metadata: Some(std::collections::HashMap::new()),
        parallel_tool_calls: Some(true),
        frequency_penalty: Some(0.0),
        presence_penalty: Some(0.0),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// compaction (openai-compaction.1.{json,chunks.txt})
//
// Output is [message, compaction]. The compaction item carries an opaque
// `encrypted_content` blob. slab-agent's new `ResponseCompactionDone` variant
// maps to `CompactionBody`. The fixture's message text is large (~815 deltas
// worth), so the test extracts it from the fixture JSON at runtime rather than
// hardcoding.
// ---------------------------------------------------------------------------

const COMPACTION_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-compaction.1.json"
);

#[test]
fn compaction_non_streaming_round_trips() {
    let expected = parse_fixture(COMPACTION_JSON);

    // Extract the large text + encrypted blob from the fixture at runtime so
    // the test asserts structural round-trip rather than hardcoding megabytes.
    let fixture_value: Value = serde_json::from_str(COMPACTION_JSON).expect("fixture parses");
    let message_text = fixture_value
        .get("output")
        .and_then(|o| o.get(0))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .expect("fixture has message text")
        .to_owned();
    let encrypted = fixture_value
        .get("output")
        .and_then(|o| o.get(1))
        .and_then(|c| c.get("encrypted_content"))
        .and_then(|t| t.as_str())
        .expect("fixture has encrypted_content")
        .to_owned();

    let envelopes = vec![
        envelope(AgentEventKind::ResponseOutputTextDone {
            item_id: "msg_1".to_owned(),
            output_index: 0,
            content_index: 0,
            text: message_text,
            artifact_refs: vec![],
            reason: None,
            phase: None,
        }),
        envelope(AgentEventKind::ResponseCompactionDone {
            item_id: "cmp_1".to_owned(),
            output_index: 1,
            encrypted_content: encrypted,
        }),
    ];

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::None)),
        ..Default::default()
    };
    let text = slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: Some(Some(slab_proto::openai::Verbosity::Medium)),
    };

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5.2-2025-12-11",
            created_at_unix: 0.0,
            completed_at: Some(0.0),
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(false),
            temperature: Some(1.0),
            top_p: Some(0.98),
            top_logprobs: Some(0),
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(Vec::new()),
            text: Some(text),
            reasoning: Some(reasoning),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(expected, actual);
}

// ---------------------------------------------------------------------------
// tool_search (openai-tool-search.1 / openai-client-tool-search.1)
// ---------------------------------------------------------------------------

const TOOL_SEARCH_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-tool-search.1.json"
);
const CLIENT_TOOL_SEARCH_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-client-tool-search.1.json"
);

/// `send_email` function tool (fixture mirror for tool_search.1).
fn send_email_tool() -> slab_proto::openai::Tool {
    slab_proto::openai::Tool::FunctionTool(Box::new(slab_proto::openai::FunctionTool {
        name: "send_email".to_owned(),
        strict: Some(true),
        description: Some(Some("Send an email to a recipient".to_owned())),
        defer_loading: Some(true),
        parameters: Some(params_map(
            r#"{
                "type":"object",
                "properties":{
                    "to":{"type":"string","description":"Recipient email address"},
                    "subject":{"type":"string","description":"Email subject"},
                    "body":{"type":"string","description":"Email body content"}
                },
                "required":["to","subject","body"],
                "additionalProperties":false
            }"#,
        )),
    }))
}

#[test]
fn tool_search_non_streaming_round_trips() {
    let expected = parse_fixture(TOOL_SEARCH_JSON);

    // Extract the tool_search_output's `tools` array from the fixture so the
    // test round-trips the exact tool definitions the search resolved.
    let fixture: Value = serde_json::from_str(TOOL_SEARCH_JSON).expect("fixture parses");
    let tso_tools = fixture
        .get("output")
        .and_then(|o| o.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("tool_search_output"))
        })
        .and_then(|i| i.get("tools"))
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();

    let envelopes = vec![
        envelope(AgentEventKind::ResponseToolSearchCallDone {
            item_id: "tsc_1".to_owned(),
            output_index: 0,
            execution: "server".to_owned(),
            call_id: None,
            arguments: serde_json::json!({ "paths": ["get_weather"] }),
        }),
        envelope(AgentEventKind::ResponseToolSearchOutputDone {
            item_id: "tso_1".to_owned(),
            output_index: 1,
            execution: "server".to_owned(),
            call_id: None,
            tools: tso_tools,
        }),
        envelope(AgentEventKind::ResponseFunctionCallArgumentsDone {
            item_id: "fc_1".to_owned(),
            call_id: "call_test".to_owned(),
            name: "get_weather".to_owned(),
            output_index: 2,
            arguments: "{\"location\":\"San Francisco, CA\",\"unit\":\"fahrenheit\"}".to_owned(),
            namespace: Some("get_weather".to_owned()),
            risk: None,
        }),
    ];

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::None)),
        ..Default::default()
    };
    let text = slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: Some(Some(slab_proto::openai::Verbosity::Medium)),
    };

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5.4-2026-03-05",
            created_at_unix: 0.0,
            completed_at: Some(0.0),
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(0.98),
            top_logprobs: Some(0),
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(vec![
                get_weather_function_tool(Some(true)),
                search_files_tool(),
                send_email_tool(),
                slab_proto::openai::Tool::ToolSearchToolParam(Box::new(
                    slab_proto::openai::ToolSearchToolParam { ..Default::default() },
                )),
            ]),
            text: Some(text),
            reasoning: Some(reasoning),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(expected, actual);
}

#[test]
fn client_tool_search_non_streaming_round_trips() {
    let expected = parse_fixture(CLIENT_TOOL_SEARCH_JSON);

    let envelopes = vec![envelope(AgentEventKind::ResponseToolSearchCallDone {
        item_id: "tsc_1".to_owned(),
        output_index: 0,
        execution: "client".to_owned(),
        call_id: Some("call_test".to_owned()),
        arguments: serde_json::json!({
            "goal": "Find a tool to get current weather for San Francisco"
        }),
    })];

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::None)),
        ..Default::default()
    };
    let text = slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: Some(Some(slab_proto::openai::Verbosity::Medium)),
    };

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5.4-2026-03-05",
            created_at_unix: 0.0,
            completed_at: Some(0.0),
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(false),
            temperature: Some(1.0),
            top_p: Some(0.98),
            top_logprobs: Some(0),
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(vec![
                get_weather_function_tool(Some(true)),
                search_files_tool(),
                tool_search_param(),
            ]),
            text: Some(text),
            reasoning: Some(reasoning),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(expected, actual);
}

// ---------------------------------------------------------------------------
// MCP (openai-mcp-tool.1 — mcp_list_tools + reasoning + mcp_call)
// ---------------------------------------------------------------------------

const MCP_TOOL_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-mcp-tool.1.json"
);

#[test]
fn mcp_tool_non_streaming_round_trips() {
    let fixture: Value = serde_json::from_str(MCP_TOOL_JSON).expect("fixture parses");
    let output = fixture.get("output").and_then(|o| o.as_array()).expect("output array");

    let list = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("mcp_list_tools"))
        .expect("mcp_list_tools item");
    let list_tools = list.get("tools").and_then(|t| t.as_array()).cloned().unwrap_or_default();
    let list_server_label = list["server_label"].as_str().unwrap().to_owned();

    let call = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("mcp_call"))
        .expect("mcp_call item");
    let call_name = call["name"].as_str().unwrap().to_owned();
    let call_arguments = call["arguments"].as_str().unwrap().to_owned();
    let call_output_val = call.get("output").and_then(|v| v.as_str()).map(|s| s.to_owned());
    let call_server_label = call["server_label"].as_str().unwrap().to_owned();

    let message_text = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_owned();

    let envelopes = vec![
        envelope(AgentEventKind::ResponseMcpListToolsDone {
            item_id: "mcpl_1".to_owned(),
            output_index: 0,
            server_label: list_server_label,
            tools: list_tools,
            error: None,
        }),
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_1".to_owned(),
            output_index: 1,
            content_index: 0,
            text: String::new(),
            encrypted_content: None,
            summary: None,
        }),
        envelope(AgentEventKind::ResponseMcpCallDone {
            item_id: "mcp_1".to_owned(),
            output_index: 2,
            server_label: call_server_label,
            name: call_name,
            arguments: call_arguments,
            output: call_output_val,
            error: None,
            status: Some("completed".to_owned()),
            approval_request_id: None,
        }),
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_2".to_owned(),
            output_index: 3,
            content_index: 0,
            text: String::new(),
            encrypted_content: None,
            summary: None,
        }),
        envelope(AgentEventKind::ResponseOutputTextDone {
            item_id: "msg_1".to_owned(),
            output_index: 4,
            content_index: 0,
            text: message_text,
            artifact_refs: vec![],
            reason: None,
            phase: None,
        }),
    ];

    let fixture_tools =
        fixture.get("tools").and_then(|t| t.as_array()).cloned().unwrap_or_default();
    let tools_config: Vec<slab_proto::openai::Tool> = fixture_tools
        .iter()
        .filter_map(|v| serde_json::from_value::<slab_proto::openai::Tool>(v.clone()).ok())
        .collect();

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::Medium)),
        ..Default::default()
    };
    let text = slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: Some(Some(slab_proto::openai::Verbosity::Medium)),
    };

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5-mini-2025-08-07",
            created_at_unix: 0.0,
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(tools_config),
            text: Some(text),
            reasoning: Some(reasoning),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(parse_fixture(MCP_TOOL_JSON), actual);
}

// ---------------------------------------------------------------------------
// Function shell (openai-shell-tool.1 — no environment)
// ---------------------------------------------------------------------------

const SHELL_TOOL_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-shell-tool.1.json"
);

#[test]
fn shell_tool_non_streaming_round_trips() {
    let expected = parse_fixture(SHELL_TOOL_JSON);

    let envelopes = vec![envelope(AgentEventKind::ResponseFunctionShellCallDone {
        item_id: "sh_1".to_owned(),
        call_id: "call_test".to_owned(),
        output_index: 0,
        commands: vec![
            "cd ~ && pwd".to_owned(),
            "cd ~/Desktop && pwd".to_owned(),
            "cd ~/Desktop && echo 'THIS WORKS!' > dec1.txt && ls -l dec1.txt && cat dec1.txt"
                .to_owned(),
        ],
        max_output_length: Some(9907),
        timeout_ms: None,
        environment_type: None,
        container_id: None,
    })];

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::None)),
        ..Default::default()
    };
    let text = slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: Some(Some(slab_proto::openai::Verbosity::Medium)),
    };

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5.1-2025-11-13",
            created_at_unix: 0.0,
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(vec![slab_proto::openai::Tool::FunctionShellToolParam(Box::new(
                slab_proto::openai::FunctionShellToolParam::new(),
            ))]),
            text: Some(text),
            reasoning: Some(reasoning),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(expected, actual);
}

// ---------------------------------------------------------------------------
// File search (openai-file-search-tool.1 — results=null)
// ---------------------------------------------------------------------------

const FILE_SEARCH_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-file-search-tool.1.json"
);

/// `file_search` tool param matching the fixture (max_num_results=20,
/// ranking_options={ranker:auto, score_threshold:0},
/// vector_store_ids=["vs_..."]).
fn file_search_tool() -> slab_proto::openai::Tool {
    slab_proto::openai::Tool::FileSearchTool(Box::new(slab_proto::openai::FileSearchTool {
        filters: None,
        max_num_results: Some(20),
        ranking_options: Some(serde_json::json!({
            "ranker": "auto",
            "score_threshold": 0
        })),
        vector_store_ids: Some(vec!["vs_redacted".to_owned()]),
    }))
}

#[test]
fn file_search_non_streaming_round_trips() {
    // Extract the message text from the fixture (it's long and carries
    // file_citation annotations that slab-agent doesn't model — normalize
    // annotations away on both sides).
    let fixture: Value = serde_json::from_str(FILE_SEARCH_JSON).expect("fixture parses");
    let message_text = fixture
        .get("output")
        .and_then(|o| o.as_array())
        .and_then(|arr| {
            arr.iter().find(|i| i.get("type").and_then(|t| t.as_str()) == Some("message"))
        })
        .and_then(|m| m.get("content"))
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_owned();

    // Strip annotations from message content parts on both sides (slab-agent
    // doesn't model file_citation annotations).
    let strip_annotations = |v: Value| -> Value {
        fn walk(v: Value) -> Value {
            match v {
                Value::Object(mut map) => {
                    if map.get("type").and_then(|t| t.as_str()) == Some("output_text") {
                        map.remove("annotations");
                    }
                    Value::Object(map.into_iter().map(|(k, v)| (k, walk(v))).collect())
                }
                Value::Array(arr) => Value::Array(arr.into_iter().map(walk).collect()),
                other => other,
            }
        }
        walk(v)
    };

    let expected = strip_annotations(parse_fixture(FILE_SEARCH_JSON));

    let envelopes = vec![
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_1".to_owned(),
            output_index: 0,
            content_index: 0,
            text: String::new(),
            encrypted_content: None,
            summary: None,
        }),
        envelope(AgentEventKind::ResponseFileSearchCallDone {
            item_id: "fs_1".to_owned(),
            output_index: 1,
            queries: vec![
                "What is an embedding model according to this document?".to_owned(),
                "What is an embedding model?".to_owned(),
                "definition of embedding model in the document".to_owned(),
                "embedding model description".to_owned(),
            ],
            results: None,
        }),
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_2".to_owned(),
            output_index: 2,
            content_index: 0,
            text: String::new(),
            encrypted_content: None,
            summary: None,
        }),
        envelope(AgentEventKind::ResponseOutputTextDone {
            item_id: "msg_1".to_owned(),
            output_index: 3,
            content_index: 0,
            text: message_text,
            artifact_refs: vec![],
            reason: None,
            phase: None,
        }),
    ];

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::Medium)),
        ..Default::default()
    };
    let text = slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: Some(Some(slab_proto::openai::Verbosity::Medium)),
    };

    let actual = strip_annotations(redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5-mini-2025-08-07",
            created_at_unix: 0.0,
            background: Some(false),
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(vec![file_search_tool()]),
            text: Some(text),
            reasoning: Some(reasoning),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    ));

    assert_eq!(expected, actual);
}

// ---------------------------------------------------------------------------
// Image generation (openai-image-generation-tool.1)
// ---------------------------------------------------------------------------

const IMAGE_GEN_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-image-generation-tool.1.json"
);

#[test]
fn image_generation_non_streaming_round_trips() {
    let expected = parse_fixture(IMAGE_GEN_JSON);

    // Extract the result/revised_prompt from the fixture (opaque blobs).
    let fixture: Value = serde_json::from_str(IMAGE_GEN_JSON).expect("fixture parses");
    let ig = fixture
        .get("output")
        .and_then(|o| o.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("image_generation_call"))
        })
        .expect("fixture has image_generation_call");
    let result = ig.get("result").and_then(|v| v.as_str()).unwrap_or("").to_owned();
    let revised_prompt = ig.get("revised_prompt").and_then(|v| v.as_str()).map(|s| s.to_owned());

    let envelopes = vec![
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_1".to_owned(),
            output_index: 0,
            content_index: 0,
            text: String::new(),
            encrypted_content: None,
            summary: None,
        }),
        envelope(AgentEventKind::ResponseImageGenCallDone {
            item_id: "ig_1".to_owned(),
            output_index: 1,
            result,
            revised_prompt,
            background: "opaque".to_owned(),
            output_format: "webp".to_owned(),
            quality: "low".to_owned(),
            size: "1024x1024".to_owned(),
        }),
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_2".to_owned(),
            output_index: 2,
            content_index: 0,
            text: String::new(),
            encrypted_content: None,
            summary: None,
        }),
        envelope(AgentEventKind::ResponseOutputTextDone {
            item_id: "msg_1".to_owned(),
            output_index: 3,
            content_index: 0,
            text: String::new(),
            artifact_refs: vec![],
            reason: None,
            phase: None,
        }),
    ];

    let reasoning = slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::Medium)),
        ..Default::default()
    };
    let text = slab_proto::openai::ResponseTextParam {
        format: Some(Box::new(
            slab_proto::openai::TextResponseFormatConfiguration::ResponseFormatText(Box::default()),
        )),
        verbosity: Some(Some(slab_proto::openai::Verbosity::Medium)),
    };

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5-nano-2025-08-07",
            created_at_unix: 0.0,
            background: Some(false),
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(vec![slab_proto::openai::Tool::ImageGenTool(Box::new(
                slab_proto::openai::ImageGenTool {
                    background: Some(slab_proto::openai::ImageGenToolBackground::Auto),
                    moderation: Some(slab_proto::openai::ImageParamsModeration::Auto),
                    n: Some(1),
                    output_compression: Some(100),
                    output_format: Some(slab_proto::openai::ImageGenToolOutputFormat::Webp),
                    quality: Some(slab_proto::openai::ImageGenToolQuality::Low),
                    size: Some(Box::new(slab_proto::openai::ImageGenToolSize::from_string(
                        "1024x1024".to_owned(),
                    ))),
                    ..Default::default()
                },
            ))]),
            text: Some(text),
            reasoning: Some(reasoning),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(expected, actual);
}

// ---------------------------------------------------------------------------
// mcp approval family (openai-mcp-tool-approval.{1,2,3,4})
//
// All four fixtures share the same echoed request config (single `mcp` tool
// with `require_approval: "always"`, model `gpt-5-mini-2025-08-07`, reasoning
// `effort=medium`, service_tier `default`, store=true, non-null `completed_at`).
// The MCP adapter arms (`ResponseMcpListToolsDone` / `ResponseMcpCallDone` /
// `ResponseMcpApprovalRequestDone`) already exist; these tests exercise them.
// ---------------------------------------------------------------------------

const MCP_APPROVAL_1_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-mcp-tool-approval.1.json"
);
const MCP_APPROVAL_2_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-mcp-tool-approval.2.json"
);
const MCP_APPROVAL_3_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-mcp-tool-approval.3.json"
);
const MCP_APPROVAL_4_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-mcp-tool-approval.4.json"
);

/// `reasoning: { effort: "medium" }` echoed by every mcp-approval fixture.
fn medium_reasoning() -> slab_proto::openai::Reasoning {
    slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::Medium)),
        ..Default::default()
    }
}

/// `reasoning: { effort: "none" }` echoed by every shell fixture.
fn none_reasoning() -> slab_proto::openai::Reasoning {
    slab_proto::openai::Reasoning {
        effort: Some(Some(slab_proto::openai::ReasoningEffort::None)),
        ..Default::default()
    }
}

/// Walk a fixture `output` array of `shell_call` / `shell_call_output` /
/// `message` items and build the matching slab envelope sequence. Dynamic
/// fields (commands, call_id, environment, output blobs, message text) are
/// extracted verbatim so they round-trip exactly. Used by the shell-skills and
/// shell-container fixtures (both interleave shell_call + shell_call_output
/// pairs before a final message).
fn shell_output_envelopes(output: &[Value]) -> Vec<AgentEventEnvelope> {
    let mut envelopes = Vec::new();
    let mut call_counter = 0;
    let mut out_counter = 0;
    for (idx, item) in output.iter().enumerate() {
        let ty = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match ty {
            "shell_call" => {
                call_counter += 1;
                let call_id =
                    item.get("call_id").and_then(|c| c.as_str()).unwrap_or_default().to_owned();
                let commands = item
                    .get("action")
                    .and_then(|a| a.get("commands"))
                    .and_then(|c| c.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_owned())).collect())
                    .unwrap_or_default();
                let environment_type = item
                    .get("environment")
                    .and_then(|e| e.get("type"))
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_owned());
                let container_id = item
                    .get("environment")
                    .and_then(|e| e.get("container_id"))
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_owned());
                envelopes.push(envelope(AgentEventKind::ResponseFunctionShellCallDone {
                    item_id: format!("sh_{call_counter}"),
                    call_id,
                    output_index: idx as i32,
                    commands,
                    max_output_length: None,
                    timeout_ms: None,
                    environment_type,
                    container_id,
                }));
            }
            "shell_call_output" => {
                out_counter += 1;
                let call_id =
                    item.get("call_id").and_then(|c| c.as_str()).unwrap_or_default().to_owned();
                let outputs =
                    item.get("output").and_then(|o| o.as_array()).cloned().unwrap_or_default();
                envelopes.push(envelope(AgentEventKind::ResponseShellCallOutputContentDone {
                    item_id: format!("sho_{out_counter}"),
                    call_id,
                    output_index: idx as i32,
                    outputs,
                }));
            }
            "message" => {
                let text = item
                    .get("content")
                    .and_then(|c| c.as_array())
                    .and_then(|c| c.first())
                    .and_then(|p| p.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or_default()
                    .to_owned();
                envelopes.push(envelope(AgentEventKind::ResponseOutputTextDone {
                    item_id: "msg_1".to_owned(),
                    output_index: idx as i32,
                    content_index: 0,
                    text,
                    artifact_refs: vec![],
                    reason: None,
                    phase: None,
                }));
            }
            _ => {}
        }
    }
    envelopes
}

/// Parse the fixture's top-level `tools` array into typed `Tool` definitions
/// (the internally-tagged `Tool` enum round-trips, so each element survives the
/// `serde_json::from_value` parse).
fn fixture_tools(fixture: &Value) -> Vec<slab_proto::openai::Tool> {
    fixture
        .get("tools")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| serde_json::from_value::<slab_proto::openai::Tool>(v.clone()).ok())
        .collect()
}

/// MCP (openai-mcp-approval.1: output is [mcp_list_tools, reasoning,
/// mcp_approval_request]. Exercises the `ResponseMcpApprovalRequestDone` arm.
#[test]
fn mcp_approval_1_non_streaming_round_trips() {
    let fixture: Value = serde_json::from_str(MCP_APPROVAL_1_JSON).expect("fixture parses");
    let output = fixture.get("output").and_then(|o| o.as_array()).expect("output array");

    let list = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("mcp_list_tools"))
        .expect("mcp_list_tools item");
    let list_tools = list.get("tools").and_then(|t| t.as_array()).cloned().unwrap_or_default();
    let list_server_label = list["server_label"].as_str().unwrap().to_owned();

    let approval = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("mcp_approval_request"))
        .expect("mcp_approval_request item");
    let ap_server_label = approval["server_label"].as_str().unwrap().to_owned();
    let ap_name = approval["name"].as_str().unwrap().to_owned();
    let ap_arguments = approval["arguments"].as_str().unwrap().to_owned();

    let envelopes = vec![
        envelope(AgentEventKind::ResponseMcpListToolsDone {
            item_id: "mcpl_1".to_owned(),
            output_index: 0,
            server_label: list_server_label,
            tools: list_tools,
            error: None,
        }),
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_1".to_owned(),
            output_index: 1,
            content_index: 0,
            text: String::new(),
            encrypted_content: None,
            summary: None,
        }),
        envelope(AgentEventKind::ResponseMcpApprovalRequestDone {
            item_id: "mcpr_1".to_owned(),
            output_index: 2,
            server_label: ap_server_label,
            name: ap_name,
            arguments: ap_arguments,
        }),
    ];

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5-mini-2025-08-07",
            created_at_unix: 0.0,
            completed_at: Some(0.0),
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(fixture_tools(&fixture)),
            text: Some(text_format_param()),
            reasoning: Some(medium_reasoning()),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(parse_fixture(MCP_APPROVAL_1_JSON), actual);
}

/// MCP (openai-mcp-approval.2: output is [mcp_list_tools, reasoning, message]
/// (rejection turn — no tool call executed). Message text extracted verbatim.
#[test]
fn mcp_approval_2_non_streaming_round_trips() {
    let fixture: Value = serde_json::from_str(MCP_APPROVAL_2_JSON).expect("fixture parses");
    let output = fixture.get("output").and_then(|o| o.as_array()).expect("output array");

    let list = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("mcp_list_tools"))
        .expect("mcp_list_tools item");
    let list_tools = list.get("tools").and_then(|t| t.as_array()).cloned().unwrap_or_default();
    let list_server_label = list["server_label"].as_str().unwrap().to_owned();

    let message_text = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_owned();

    let envelopes = vec![
        envelope(AgentEventKind::ResponseMcpListToolsDone {
            item_id: "mcpl_1".to_owned(),
            output_index: 0,
            server_label: list_server_label,
            tools: list_tools,
            error: None,
        }),
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_1".to_owned(),
            output_index: 1,
            content_index: 0,
            text: String::new(),
            encrypted_content: None,
            summary: None,
        }),
        envelope(AgentEventKind::ResponseOutputTextDone {
            item_id: "msg_1".to_owned(),
            output_index: 2,
            content_index: 0,
            text: message_text,
            artifact_refs: vec![],
            reason: None,
            phase: None,
        }),
    ];

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5-mini-2025-08-07",
            created_at_unix: 0.0,
            completed_at: Some(0.0),
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(fixture_tools(&fixture)),
            text: Some(text_format_param()),
            reasoning: Some(medium_reasoning()),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(parse_fixture(MCP_APPROVAL_2_JSON), actual);
}

/// MCP (openai-mcp-approval.3: same shape as .1 (list_tools → reasoning →
/// mcp_approval_request).
#[test]
fn mcp_approval_3_non_streaming_round_trips() {
    let fixture: Value = serde_json::from_str(MCP_APPROVAL_3_JSON).expect("fixture parses");
    let output = fixture.get("output").and_then(|o| o.as_array()).expect("output array");

    let list = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("mcp_list_tools"))
        .expect("mcp_list_tools item");
    let list_tools = list.get("tools").and_then(|t| t.as_array()).cloned().unwrap_or_default();
    let list_server_label = list["server_label"].as_str().unwrap().to_owned();

    let approval = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("mcp_approval_request"))
        .expect("mcp_approval_request item");
    let ap_server_label = approval["server_label"].as_str().unwrap().to_owned();
    let ap_name = approval["name"].as_str().unwrap().to_owned();
    let ap_arguments = approval["arguments"].as_str().unwrap().to_owned();

    let envelopes = vec![
        envelope(AgentEventKind::ResponseMcpListToolsDone {
            item_id: "mcpl_1".to_owned(),
            output_index: 0,
            server_label: list_server_label,
            tools: list_tools,
            error: None,
        }),
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_1".to_owned(),
            output_index: 1,
            content_index: 0,
            text: String::new(),
            encrypted_content: None,
            summary: None,
        }),
        envelope(AgentEventKind::ResponseMcpApprovalRequestDone {
            item_id: "mcpr_1".to_owned(),
            output_index: 2,
            server_label: ap_server_label,
            name: ap_name,
            arguments: ap_arguments,
        }),
    ];

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5-mini-2025-08-07",
            created_at_unix: 0.0,
            completed_at: Some(0.0),
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(fixture_tools(&fixture)),
            text: Some(text_format_param()),
            reasoning: Some(medium_reasoning()),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(parse_fixture(MCP_APPROVAL_3_JSON), actual);
}

/// MCP (openai-mcp-approval.4: output is [mcp_list_tools, mcp_call (with
/// `approval_request_id` set), message]. Exercises the `ResponseMcpCallDone`
/// arm with `approval_request_id: Some(..)`.
#[test]
fn mcp_approval_4_non_streaming_round_trips() {
    let fixture: Value = serde_json::from_str(MCP_APPROVAL_4_JSON).expect("fixture parses");
    let output = fixture.get("output").and_then(|o| o.as_array()).expect("output array");

    let list = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("mcp_list_tools"))
        .expect("mcp_list_tools item");
    let list_tools = list.get("tools").and_then(|t| t.as_array()).cloned().unwrap_or_default();
    let list_server_label = list["server_label"].as_str().unwrap().to_owned();

    let call = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("mcp_call"))
        .expect("mcp_call item");
    let call_server_label = call["server_label"].as_str().unwrap().to_owned();
    let call_name = call["name"].as_str().unwrap().to_owned();
    let call_arguments = call["arguments"].as_str().unwrap().to_owned();
    let call_output_val = call.get("output").and_then(|v| v.as_str()).map(|s| s.to_owned());
    let call_approval_id =
        call.get("approval_request_id").and_then(|v| v.as_str()).map(|s| s.to_owned());

    let message_text = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_owned();

    let envelopes = vec![
        envelope(AgentEventKind::ResponseMcpListToolsDone {
            item_id: "mcpl_1".to_owned(),
            output_index: 0,
            server_label: list_server_label,
            tools: list_tools,
            error: None,
        }),
        envelope(AgentEventKind::ResponseMcpCallDone {
            item_id: "mcp_1".to_owned(),
            output_index: 1,
            server_label: call_server_label,
            name: call_name,
            arguments: call_arguments,
            output: call_output_val,
            error: None,
            status: Some("completed".to_owned()),
            approval_request_id: call_approval_id,
        }),
        envelope(AgentEventKind::ResponseOutputTextDone {
            item_id: "msg_1".to_owned(),
            output_index: 2,
            content_index: 0,
            text: message_text,
            artifact_refs: vec![],
            reason: None,
            phase: None,
        }),
    ];

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5-mini-2025-08-07",
            created_at_unix: 0.0,
            completed_at: Some(0.0),
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(fixture_tools(&fixture)),
            text: Some(text_format_param()),
            reasoning: Some(medium_reasoning()),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(parse_fixture(MCP_APPROVAL_4_JSON), actual);
}

// ---------------------------------------------------------------------------
// Code Interpreter (openai-code-interpreter-tool.1)
//
// Output interleaves reasoning + code_interpreter_call items (×3). Each call
// carries `code`, `container_id`, and an opaque `outputs` array
// ({type:"logs", logs}). The message carries a `container_file_citation`
// annotation slab-agent doesn't model — strip on both sides.
// ---------------------------------------------------------------------------

const CODE_INTERPRETER_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-code-interpreter-tool.1.json"
);

#[test]
fn code_interpreter_non_streaming_round_trips() {
    let fixture: Value = serde_json::from_str(CODE_INTERPRETER_JSON).expect("fixture parses");
    let output = fixture.get("output").and_then(|o| o.as_array()).expect("output array");

    // Walk the fixture's output array in order, emitting one slab envelope per
    // item. Dynamic fields (code, container_id, outputs, message text) are
    // extracted verbatim so they round-trip exactly.
    let mut envelopes = Vec::new();
    for (idx, item) in output.iter().enumerate() {
        let ty = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match ty {
            "reasoning" => envelopes.push(envelope(AgentEventKind::ResponseReasoningTextDone {
                item_id: format!("rs_{idx}"),
                output_index: idx as i32,
                content_index: 0,
                text: String::new(),
                encrypted_content: None,
                summary: None,
            })),
            "code_interpreter_call" => {
                let code = item.get("code").and_then(|c| c.as_str()).unwrap_or_default().to_owned();
                let container_id =
                    item.get("container_id").and_then(|c| c.as_str()).map(|s| s.to_owned());
                let ci_outputs =
                    item.get("outputs").and_then(|o| o.as_array()).cloned().unwrap_or_default();
                envelopes.push(envelope(AgentEventKind::ResponseCodeInterpreterCallDone {
                    item_id: format!("ci_{idx}"),
                    output_index: idx as i32,
                    code,
                    container_id,
                    outputs: ci_outputs,
                }));
            }
            "message" => {
                let text = item
                    .get("content")
                    .and_then(|c| c.as_array())
                    .and_then(|c| c.first())
                    .and_then(|p| p.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or_default()
                    .to_owned();
                envelopes.push(envelope(AgentEventKind::ResponseOutputTextDone {
                    item_id: format!("msg_{idx}"),
                    output_index: idx as i32,
                    content_index: 0,
                    text,
                    artifact_refs: vec![],
                    reason: None,
                    phase: None,
                }));
            }
            _ => {}
        }
    }

    let actual = strip_message_annotations(redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5-nano-2025-08-07",
            created_at_unix: 0.0,
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(fixture_tools(&fixture)),
            text: Some(text_format_param()),
            reasoning: Some(medium_reasoning()),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    ));

    assert_eq!(strip_message_annotations(parse_fixture(CODE_INTERPRETER_JSON)), actual);
}

// ---------------------------------------------------------------------------
// Web Search (openai-web-search-tool.1)
//
// Output interleaves reasoning + web_search_call items (×3: search /
// open_page / find_in_page actions). The action JSON round-trips through the
// typed `WebSearchToolCallAction` (internally tagged, snake_case variants).
// The message carries `url_citation` annotations slab-agent doesn't model —
// strip on both sides.
// ---------------------------------------------------------------------------

const WEB_SEARCH_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-web-search-tool.1.json"
);

#[test]
fn web_search_non_streaming_round_trips() {
    let fixture: Value = serde_json::from_str(WEB_SEARCH_JSON).expect("fixture parses");
    let output = fixture.get("output").and_then(|o| o.as_array()).expect("output array");

    let mut envelopes = Vec::new();
    for (idx, item) in output.iter().enumerate() {
        let ty = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match ty {
            "reasoning" => envelopes.push(envelope(AgentEventKind::ResponseReasoningTextDone {
                item_id: format!("rs_{idx}"),
                output_index: idx as i32,
                content_index: 0,
                text: String::new(),
                encrypted_content: None,
                summary: None,
            })),
            "web_search_call" => {
                let action = item.get("action").cloned().unwrap_or(Value::Null);
                envelopes.push(envelope(AgentEventKind::ResponseWebSearchCallDone {
                    item_id: format!("ws_{idx}"),
                    output_index: idx as i32,
                    action,
                }));
            }
            "message" => {
                let text = item
                    .get("content")
                    .and_then(|c| c.as_array())
                    .and_then(|c| c.first())
                    .and_then(|p| p.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or_default()
                    .to_owned();
                envelopes.push(envelope(AgentEventKind::ResponseOutputTextDone {
                    item_id: format!("msg_{idx}"),
                    output_index: idx as i32,
                    content_index: 0,
                    text,
                    artifact_refs: vec![],
                    reason: None,
                    phase: None,
                }));
            }
            _ => {}
        }
    }

    let actual = strip_message_annotations(redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5-mini-2025-08-07",
            created_at_unix: 0.0,
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(fixture_tools(&fixture)),
            text: Some(text_format_param()),
            reasoning: Some(medium_reasoning()),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    ));

    assert_eq!(strip_message_annotations(parse_fixture(WEB_SEARCH_JSON)), actual);
}

// ---------------------------------------------------------------------------
// File Search (openai-file-search-tool.2)
//
// Output is [reasoning, file_search_call (results[] with score), reasoning,
// message]. The existing `ResponseFileSearchCallDone` arm already maps the
// opaque `results` (`Vec<Value>`); pass `results: Some(<extracted>)`. The
// message carries a `file_citation` annotation slab-agent doesn't model —
// strip on both sides.
// ---------------------------------------------------------------------------

const FILE_SEARCH_2_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-file-search-tool.2.json"
);

#[test]
fn file_search_2_non_streaming_round_trips() {
    let fixture: Value = serde_json::from_str(FILE_SEARCH_2_JSON).expect("fixture parses");
    let output = fixture.get("output").and_then(|o| o.as_array()).expect("output array");

    let fs = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("file_search_call"))
        .expect("file_search_call item");
    let queries = fs
        .get("queries")
        .and_then(|q| q.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_owned())).collect())
        .unwrap_or_default();
    let results = fs.get("results").and_then(|r| r.as_array()).cloned();

    let message_text = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_owned();

    let envelopes = vec![
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_1".to_owned(),
            output_index: 0,
            content_index: 0,
            text: String::new(),
            encrypted_content: None,
            summary: None,
        }),
        envelope(AgentEventKind::ResponseFileSearchCallDone {
            item_id: "fs_1".to_owned(),
            output_index: 1,
            queries,
            results,
        }),
        envelope(AgentEventKind::ResponseReasoningTextDone {
            item_id: "rs_2".to_owned(),
            output_index: 2,
            content_index: 0,
            text: String::new(),
            encrypted_content: None,
            summary: None,
        }),
        envelope(AgentEventKind::ResponseOutputTextDone {
            item_id: "msg_1".to_owned(),
            output_index: 3,
            content_index: 0,
            text: message_text,
            artifact_refs: vec![],
            reason: None,
            phase: None,
        }),
    ];

    let actual = strip_message_annotations(redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5-mini-2025-08-07",
            created_at_unix: 0.0,
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(fixture_tools(&fixture)),
            text: Some(text_format_param()),
            reasoning: Some(medium_reasoning()),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    ));

    assert_eq!(strip_message_annotations(parse_fixture(FILE_SEARCH_2_JSON)), actual);
}

// ---------------------------------------------------------------------------
// Function Shell (openai-shell-skills.1, openai-shell-container.1)
//
// Output interleaves shell_call + shell_call_output pairs, then a final
// message. The new `ResponseShellCallOutputContentDone` arm maps each output
// item; the existing `ResponseFunctionShellCallDone` arm maps each call item
// (with `environment: {type:"container_reference", container_id}`).
// ---------------------------------------------------------------------------

const SHELL_SKILLS_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-shell-skills.1.json"
);
const SHELL_CONTAINER_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-shell-container.1.json"
);

#[test]
fn shell_skills_non_streaming_round_trips() {
    let fixture: Value = serde_json::from_str(SHELL_SKILLS_JSON).expect("fixture parses");
    let output = fixture.get("output").and_then(|o| o.as_array()).expect("output array");
    let envelopes = shell_output_envelopes(output);

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5.2-2025-12-11",
            created_at_unix: 0.0,
            completed_at: Some(0.0),
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(0.98),
            top_logprobs: Some(0),
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(fixture_tools(&fixture)),
            text: Some(text_format_param()),
            reasoning: Some(none_reasoning()),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(parse_fixture(SHELL_SKILLS_JSON), actual);
}

#[test]
fn shell_container_non_streaming_round_trips() {
    let fixture: Value = serde_json::from_str(SHELL_CONTAINER_JSON).expect("fixture parses");
    let output = fixture.get("output").and_then(|o| o.as_array()).expect("output array");
    let envelopes = shell_output_envelopes(output);

    let actual = redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5.2-2025-12-11",
            created_at_unix: 0.0,
            completed_at: Some(0.0),
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(1.0),
            top_logprobs: Some(0),
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(fixture_tools(&fixture)),
            text: Some(text_format_param()),
            reasoning: Some(none_reasoning()),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    );

    assert_eq!(parse_fixture(SHELL_CONTAINER_JSON), actual);
}

// ---------------------------------------------------------------------------
// Function Shell Multiturn (text-only final turn)
// (openai-shell-local-multiturn.1, openai-shell-container-multiturn.1)
//
// Both fixtures carry a single `message` output item (the final turn of a
// multi-turn shell session — the earlier tool I/O lives in prior responses).
// Only the `environment` on the echoed `shell` tool config differs (`local` vs
// `container_reference`).
// ---------------------------------------------------------------------------

const SHELL_LOCAL_MULTITURN_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-shell-local-multiturn.1.json"
);
const SHELL_CONTAINER_MULTITURN_JSON: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-shell-container-multiturn.1.json"
);

/// Build the envelope + actual response for a text-only multiturn fixture,
/// echoing the fixture's `tools` config verbatim and extracting the message
/// text. `top_p` is the only field that varies across the two fixtures.
fn run_shell_multiturn_round_trip(raw: &'static str, top_p: f64) -> Value {
    let fixture: Value = serde_json::from_str(raw).expect("fixture parses");
    let output = fixture.get("output").and_then(|o| o.as_array()).expect("output array");
    let message_text = output
        .iter()
        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_owned();

    let envelopes = vec![envelope(AgentEventKind::ResponseOutputTextDone {
        item_id: "msg_1".to_owned(),
        output_index: 0,
        content_index: 0,
        text: message_text,
        artifact_refs: vec![],
        reason: None,
        phase: None,
    })];

    redact_dynamic_fields(
        serde_json::to_value(build_response(AdapterInput {
            response_id: "resp_test",
            model: "gpt-5.2-2025-12-11",
            created_at_unix: 0.0,
            completed_at: Some(0.0),
            service_tier: Some(slab_proto::openai::ServiceTier::Default),
            envelopes: &envelopes,
            background: Some(false),
            billing: Some(serde_json::json!({ "payer": "developer" })),
            store: Some(true),
            temperature: Some(1.0),
            top_p: Some(top_p),
            top_logprobs: Some(0),
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.0),
            truncation: Some(slab_proto::openai::ResponseTruncation::Disabled),
            tool_choice: Some(slab_proto::openai::ToolChoiceParam::ToolChoiceOptions(
                slab_proto::openai::ToolChoiceOptions::Auto,
            )),
            tools: Some(fixture_tools(&fixture)),
            text: Some(text_format_param()),
            reasoning: Some(none_reasoning()),
            metadata: Some(std::collections::HashMap::new()),
            parallel_tool_calls: Some(true),
            ..Default::default()
        }))
        .expect("serialize response"),
    )
}

#[test]
fn shell_local_multiturn_non_streaming_round_trips() {
    let actual = run_shell_multiturn_round_trip(SHELL_LOCAL_MULTITURN_JSON, 0.98);
    assert_eq!(parse_fixture(SHELL_LOCAL_MULTITURN_JSON), actual);
}

#[test]
fn shell_container_multiturn_non_streaming_round_trips() {
    let actual = run_shell_multiturn_round_trip(SHELL_CONTAINER_MULTITURN_JSON, 0.98);
    assert_eq!(parse_fixture(SHELL_CONTAINER_MULTITURN_JSON), actual);
}

// ===========================================================================
// Streaming long tail (mcp / shell / code-interpreter / web-search /
// file-search / multi-response reasoning)
//
// Each `.chunks.txt` is driven through `envelope_to_events` via a general
// chunks→envelopes walker (`streaming_envelopes_from_chunks`). The skeleton
// echoed by lifecycle events is recovered by deserializing the fixture's own
// `response.created` `response` object, so every echoed request-config field
// (tools/reasoning/text/tool_choice/...) round-trips without hand-mirroring.
// ===========================================================================

const MCP_TOOL_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-mcp-tool.1.chunks.txt"
);
const MCP_APPROVAL_1_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-mcp-tool-approval.1.chunks.txt"
);
const MCP_APPROVAL_2_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-mcp-tool-approval.2.chunks.txt"
);
const MCP_APPROVAL_3_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-mcp-tool-approval.3.chunks.txt"
);
const MCP_APPROVAL_4_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-mcp-tool-approval.4.chunks.txt"
);
const SHELL_SKILLS_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-shell-skills.1.chunks.txt"
);
const SHELL_CONTAINER_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-shell-container.1.chunks.txt"
);
const SHELL_LOCAL_MULTITURN_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-shell-local-multiturn.1.chunks.txt"
);
const SHELL_CONTAINER_MULTITURN_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-shell-container-multiturn.1.chunks.txt"
);
const CODE_INTERPRETER_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-code-interpreter-tool.1.chunks.txt"
);
const WEB_SEARCH_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-web-search-tool.1.chunks.txt"
);
const FILE_SEARCH_2_CHUNKS: &str = include_str!(
    "../../../../../../../testdata/fixtures/openai-compatible/responses/openai-file-search-tool.2.chunks.txt"
);

/// Deserialize the fixture's first `response.created` `response` object into a
/// full [`slab_proto::openai::Response`] skeleton. Every echoed request-config
/// field round-trips, so lifecycle events match the fixture without
/// hand-mirroring each knob. Unmodeled fixture keys are null-valued and the
/// redactor drops them on both sides.
fn skeleton_from_chunks(raw: &str) -> slab_proto::openai::Response {
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(line).expect("chunk line parses");
        if v.get("type").and_then(|t| t.as_str()) == Some("response.created") {
            let resp = v.get("response").cloned().unwrap_or(Value::Null);
            return serde_json::from_value::<slab_proto::openai::Response>(resp)
                .expect("response.created skeleton deserializes into Response");
        }
    }
    panic!("no response.created event found in chunks");
}

/// Per-item metadata captured while walking a `.chunks.txt`, used to reconstruct
/// the slab Done envelope for item types whose lifecycle the adapter synthesizes
/// from a single Done event (mcp / code-interpreter / web-search / file-search).
#[derive(Default)]
struct StreamItemMeta {
    call_id: String,
    name: String,
    server_label: String,
    arguments: String,
    output: Option<String>,
    error: Option<String>,
    status: Option<String>,
    approval_request_id: Option<String>,
    encrypted_content: Option<String>,
    code: String,
    container_id: Option<String>,
    outputs: Vec<Value>,
    queries: Vec<String>,
    results: Option<Vec<Value>>,
    action: Value,
    tools: Vec<Value>,
    phase: Option<String>,
}

/// Walk a `.chunks.txt` NDJSON stream and derive the slab
/// [`AgentEventKind`] envelope sequence whose `envelope_to_events`
/// expansion reproduces the fixture. Lifecycle/wrapper/sub-events the adapter
/// synthesizes are skipped; the slab delta events (text/function/shell) map 1:1;
/// and tool streams slab-agent lacks a delta variant for (code-interpreter
/// `code`, mcp `arguments`) are counted so the adapter can re-split the
/// finalized payload into the matching delta count via `ctx`.
///
/// The response-level shell environment is captured from the first `shell_call`
/// `output_item.added` and pinned on `ctx` (slab-agent's command-stream variants
/// do not carry the `environment` discriminator).
fn streaming_envelopes_from_chunks(raw: &str, ctx: &mut StreamCtx) -> Vec<AgentEventEnvelope> {
    use slab_types::agent::AgentThreadStatus;
    use std::collections::{HashMap, HashSet};

    let mut meta: HashMap<String, StreamItemMeta> = HashMap::new();
    let mut item_id_by_idx: HashMap<i32, String> = HashMap::new();
    let mut reasoning_done_emitted: HashSet<String> = HashSet::new();
    let mut ci_delta_count: HashMap<String, usize> = HashMap::new();
    let mut mcp_delta_count: HashMap<String, usize> = HashMap::new();
    let mut envelopes: Vec<AgentEventEnvelope> = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(line).expect("chunk line parses");
        let ty = str_field(&v, "type");
        let output_index = i32_field(&v, "output_index");
        match ty {
            "response.created" => envelopes.push(envelope(AgentEventKind::ResponseQueued {
                response: AgentResponseRef {
                    id: "resp_test".to_owned(),
                    status: AgentThreadStatus::Pending,
                },
            })),
            "response.in_progress" => {
                envelopes.push(envelope(AgentEventKind::ResponseInProgress {
                    response: AgentResponseRef {
                        id: "resp_test".to_owned(),
                        status: AgentThreadStatus::Running,
                    },
                }))
            }
            "response.completed" => envelopes.push(envelope(AgentEventKind::ResponseCompleted {
                response: AgentResponseRef {
                    id: "resp_test".to_owned(),
                    status: AgentThreadStatus::Completed,
                },
            })),
            "response.output_item.added" => {
                if let Some(item) = v.get("item") {
                    let item_id = str_field(item, "id").to_owned();
                    item_id_by_idx.insert(output_index, item_id.clone());
                    let m = meta.entry(item_id).or_default();
                    m.call_id =
                        item.get("call_id").and_then(|x| x.as_str()).unwrap_or("").to_owned();
                    m.name = item.get("name").and_then(|x| x.as_str()).unwrap_or("").to_owned();
                    m.server_label =
                        item.get("server_label").and_then(|x| x.as_str()).unwrap_or("").to_owned();
                    m.encrypted_content = item
                        .get("encrypted_content")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_owned());
                    m.phase = item.get("phase").and_then(|x| x.as_str()).map(|s| s.to_owned());
                    if item.get("type").and_then(|t| t.as_str()) == Some("shell_call")
                        && let Some(env) = item.get("environment")
                    {
                        let env_type =
                            env.get("type").and_then(|t| t.as_str()).unwrap_or("local").to_owned();
                        let container_id =
                            env.get("container_id").and_then(|c| c.as_str()).map(|s| s.to_owned());
                        ctx.set_shell_environment(env_type, container_id);
                    }
                }
            }
            "response.output_item.done" => {
                if let Some(item) = v.get("item") {
                    let item_id = str_field(item, "id").to_owned();
                    let itype = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    let m = meta.entry(item_id.clone()).or_default();
                    m.call_id =
                        item.get("call_id").and_then(|x| x.as_str()).unwrap_or("").to_owned();
                    m.name = item.get("name").and_then(|x| x.as_str()).unwrap_or("").to_owned();
                    m.server_label =
                        item.get("server_label").and_then(|x| x.as_str()).unwrap_or("").to_owned();
                    m.arguments =
                        item.get("arguments").and_then(|x| x.as_str()).unwrap_or("").to_owned();
                    m.output = item.get("output").and_then(|x| x.as_str()).map(|s| s.to_owned());
                    m.error = item.get("error").and_then(|x| x.as_str()).map(|s| s.to_owned());
                    m.status = item.get("status").and_then(|x| x.as_str()).map(|s| s.to_owned());
                    m.approval_request_id = item
                        .get("approval_request_id")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_owned());
                    m.code = item.get("code").and_then(|x| x.as_str()).unwrap_or("").to_owned();
                    m.container_id =
                        item.get("container_id").and_then(|x| x.as_str()).map(|s| s.to_owned());
                    m.outputs =
                        item.get("outputs").and_then(|x| x.as_array()).cloned().unwrap_or_default();
                    m.queries = item
                        .get("queries")
                        .and_then(|x| x.as_array())
                        .map(|a| {
                            a.iter().filter_map(|x| x.as_str().map(|s| s.to_owned())).collect()
                        })
                        .unwrap_or_default();
                    m.results = item.get("results").and_then(|x| x.as_array()).cloned();
                    m.action = item.get("action").cloned().unwrap_or(Value::Null);
                    m.tools =
                        item.get("tools").and_then(|x| x.as_array()).cloned().unwrap_or_default();
                    match itype {
                        "mcp_list_tools" => {
                            envelopes.push(envelope(AgentEventKind::ResponseMcpListToolsDone {
                                item_id: item_id.clone(),
                                output_index,
                                server_label: m.server_label.clone(),
                                tools: m.tools.clone(),
                                error: m.error.clone(),
                            }))
                        }
                        "mcp_call" => {
                            let n = mcp_delta_count.get(&item_id).copied().unwrap_or(1);
                            ctx.set_tool_delta_split(&item_id, n.max(1));
                            envelopes.push(envelope(AgentEventKind::ResponseMcpCallDone {
                                item_id: item_id.clone(),
                                output_index,
                                server_label: m.server_label.clone(),
                                name: m.name.clone(),
                                arguments: m.arguments.clone(),
                                output: m.output.clone(),
                                error: m.error.clone(),
                                status: m.status.clone(),
                                approval_request_id: m.approval_request_id.clone(),
                            }));
                        }
                        "mcp_approval_request" => envelopes.push(envelope(
                            AgentEventKind::ResponseMcpApprovalRequestDone {
                                item_id: item_id.clone(),
                                output_index,
                                server_label: m.server_label.clone(),
                                name: m.name.clone(),
                                arguments: m.arguments.clone(),
                            },
                        )),
                        "code_interpreter_call" => {
                            let n = ci_delta_count.get(&item_id).copied().unwrap_or(1);
                            ctx.set_tool_delta_split(&item_id, n.max(1));
                            envelopes.push(envelope(
                                AgentEventKind::ResponseCodeInterpreterCallDone {
                                    item_id: item_id.clone(),
                                    output_index,
                                    code: m.code.clone(),
                                    container_id: m.container_id.clone(),
                                    outputs: m.outputs.clone(),
                                },
                            ));
                        }
                        "web_search_call" => {
                            envelopes.push(envelope(AgentEventKind::ResponseWebSearchCallDone {
                                item_id: item_id.clone(),
                                output_index,
                                action: m.action.clone(),
                            }))
                        }
                        "file_search_call" => {
                            envelopes.push(envelope(AgentEventKind::ResponseFileSearchCallDone {
                                item_id: item_id.clone(),
                                output_index,
                                queries: m.queries.clone(),
                                results: m.results.clone(),
                            }))
                        }
                        "reasoning" if !reasoning_done_emitted.contains(&item_id) => {
                            envelopes.push(envelope(AgentEventKind::ResponseReasoningTextDone {
                                item_id: item_id.clone(),
                                output_index,
                                content_index: 0,
                                text: String::new(),
                                encrypted_content: m.encrypted_content.clone(),
                                summary: None,
                            }));
                            reasoning_done_emitted.insert(item_id);
                        }
                        _ => {}
                    }
                }
            }
            "response.output_text.delta" => {
                envelopes.push(envelope(AgentEventKind::ResponseOutputTextDelta {
                    item_id: str_field(&v, "item_id").to_owned(),
                    output_index,
                    content_index: i32_field(&v, "content_index"),
                    delta: v.get("delta").and_then(|d| d.as_str()).unwrap_or("").to_owned(),
                }))
            }
            "response.output_text.done" => {
                let item_id = str_field(&v, "item_id").to_owned();
                let phase = meta.get(&item_id).and_then(|x| x.phase.clone());
                envelopes.push(envelope(AgentEventKind::ResponseOutputTextDone {
                    item_id,
                    output_index,
                    content_index: i32_field(&v, "content_index"),
                    text: v.get("text").and_then(|t| t.as_str()).unwrap_or("").to_owned(),
                    artifact_refs: vec![],
                    reason: None,
                    phase,
                }));
            }
            "response.reasoning_summary_text.delta" => {
                envelopes.push(envelope(AgentEventKind::ResponseReasoningTextDelta {
                    item_id: str_field(&v, "item_id").to_owned(),
                    output_index,
                    content_index: 0,
                    delta: v.get("delta").and_then(|d| d.as_str()).unwrap_or("").to_owned(),
                }))
            }
            "response.reasoning_summary_text.done" => {
                let item_id = str_field(&v, "item_id").to_owned();
                let summary = v.get("text").and_then(|t| t.as_str()).map(|s| s.to_owned());
                let encrypted = meta.get(&item_id).and_then(|x| x.encrypted_content.clone());
                envelopes.push(envelope(AgentEventKind::ResponseReasoningTextDone {
                    item_id: item_id.clone(),
                    output_index,
                    content_index: 0,
                    text: String::new(),
                    encrypted_content: encrypted,
                    summary,
                }));
                reasoning_done_emitted.insert(item_id);
            }
            "response.function_call_arguments.delta" => {
                let item_id = str_field(&v, "item_id").to_owned();
                let (call_id, name) = meta
                    .get(&item_id)
                    .map(|x| (x.call_id.clone(), x.name.clone()))
                    .unwrap_or_default();
                envelopes.push(envelope(AgentEventKind::ResponseFunctionCallArgumentsDelta {
                    item_id,
                    call_id,
                    name,
                    output_index,
                    delta: v.get("delta").and_then(|d| d.as_str()).unwrap_or("").to_owned(),
                }));
            }
            "response.function_call_arguments.done" => {
                let item_id = str_field(&v, "item_id").to_owned();
                let (call_id, name) = meta
                    .get(&item_id)
                    .map(|x| (x.call_id.clone(), x.name.clone()))
                    .unwrap_or_default();
                envelopes.push(envelope(AgentEventKind::ResponseFunctionCallArgumentsDone {
                    item_id,
                    call_id,
                    name,
                    output_index,
                    arguments: v.get("arguments").and_then(|d| d.as_str()).unwrap_or("").to_owned(),
                    namespace: None,
                    risk: None,
                }));
            }
            "response.shell_call_command.delta" => {
                let item_id = item_id_by_idx.get(&output_index).cloned().unwrap_or_default();
                let call_id = meta.get(&item_id).map(|x| x.call_id.clone()).unwrap_or_default();
                envelopes.push(envelope(AgentEventKind::ResponseShellCallCommandDelta {
                    item_id,
                    call_id,
                    output_index,
                    delta: v.get("delta").and_then(|d| d.as_str()).unwrap_or("").to_owned(),
                }));
            }
            "response.shell_call_command.done" => {
                let item_id = item_id_by_idx.get(&output_index).cloned().unwrap_or_default();
                let call_id = meta.get(&item_id).map(|x| x.call_id.clone()).unwrap_or_default();
                let command = v.get("command").and_then(|d| d.as_str()).unwrap_or("").to_owned();
                envelopes.push(envelope(AgentEventKind::ResponseShellCallCommandDone {
                    item_id,
                    call_id,
                    output_index,
                    commands: vec![command],
                    max_output_length: None,
                    timeout_ms: None,
                }));
            }
            "response.shell_call_output_content.delta" => {
                let item_id = str_field(&v, "item_id").to_owned();
                let call_id = meta.get(&item_id).map(|x| x.call_id.clone()).unwrap_or_default();
                let stdout = v
                    .get("delta")
                    .and_then(|d| d.get("stdout"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_owned();
                envelopes.push(envelope(AgentEventKind::ResponseShellCallOutputContentDelta {
                    item_id,
                    call_id,
                    output_index,
                    delta: stdout,
                }));
            }
            "response.shell_call_output_content.done" => {
                let item_id = str_field(&v, "item_id").to_owned();
                let call_id = meta.get(&item_id).map(|x| x.call_id.clone()).unwrap_or_default();
                let outputs =
                    v.get("output").and_then(|o| o.as_array()).cloned().unwrap_or_default();
                envelopes.push(envelope(AgentEventKind::ResponseShellCallOutputContentDone {
                    item_id,
                    call_id,
                    output_index,
                    outputs,
                }));
            }
            "response.code_interpreter_call_code.delta" => {
                *ci_delta_count.entry(str_field(&v, "item_id").to_owned()).or_default() += 1;
            }
            "response.mcp_call_arguments.delta" => {
                *mcp_delta_count.entry(str_field(&v, "item_id").to_owned()).or_default() += 1;
            }
            _ => {} // wrappers / lifecycle sub-events / annotation.added are synthesized by the adapter
        }
    }
    envelopes
}

/// Drop `response.output_text.annotation.added` events (slab-agent does not
/// model annotation streaming) and strip `annotations` arrays from every
/// `output_text` content part. Applied to both expected and actual so the
/// comparison exercises the streaming state machine rather than slab's
/// annotation gap.
fn strip_annotation_events_and_content(events: Vec<Value>) -> Vec<Value> {
    events
        .into_iter()
        .filter(|ev| {
            ev.get("type").and_then(|t| t.as_str()) != Some("response.output_text.annotation.added")
        })
        .map(strip_message_annotations)
        .collect()
}

/// Strip `action` from `web_search_call` items inside `output_item.added`
/// skeletons. The canonical added skeleton carries no action (it is surfaced
/// only on `output_item.done`), but slab-proto's `WebSearchToolCall` requires
/// the field, so the adapter emits a default action that must be normalized
/// away on both sides.
fn strip_web_search_added_action(events: Vec<Value>) -> Vec<Value> {
    events
        .into_iter()
        .map(|mut ev| {
            let is_added =
                ev.get("type").and_then(|t| t.as_str()) == Some("response.output_item.added");
            if is_added
                && let Some(item) = ev.get_mut("item").and_then(|i| i.as_object_mut())
                && item.get("type").and_then(|t| t.as_str()) == Some("web_search_call")
            {
                item.remove("action");
            }
            ev
        })
        .collect()
}

/// Remove duplicate `response.mcp_call.in_progress` events for the same item
/// (some fixtures emit a spurious second `in_progress` bracketing the argument
/// delta; the adapter emits exactly one). Keeps the first per `item_id`.
fn dedupe_repeated_mcp_in_progress(events: Vec<Value>) -> Vec<Value> {
    use std::collections::HashSet;
    let mut seen: HashSet<String> = HashSet::new();
    events
        .into_iter()
        .filter_map(|ev| {
            let is_dup =
                ev.get("type").and_then(|t| t.as_str()) == Some("response.mcp_call.in_progress");
            if is_dup {
                let id = ev.get("item_id").and_then(|i| i.as_str()).unwrap_or("").to_owned();
                if seen.insert(id) { Some(ev) } else { None }
            } else {
                Some(ev)
            }
        })
        .collect()
}

/// Split a `.chunks.txt` carrying multiple independent response cycles (each
/// starting at `response.created`) into one owned-line-vec per cycle. Used by
/// the multi-response reasoning fixture, which drives each cycle through a
/// fresh `StreamCtx`.
fn split_chunks_into_cycles(raw: &str) -> Vec<String> {
    let mut cycles: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let starts_cycle = serde_json::from_str::<Value>(line)
            .ok()
            .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(|s| s == "response.created"))
            .unwrap_or(false);
        if starts_cycle && !current.is_empty() {
            cycles.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    if !current.is_empty() {
        cycles.push(current);
    }
    cycles
}

/// Build a `StreamCtx` whose skeleton is the fixture's own `response.created`
/// payload, with the canonical `auto`→`default` service-tier transition.
/// `completed_at` is applied only when the fixture's `response.completed`
/// event actually carries it (some fixtures omit the field entirely).
fn ctx_from_chunks(raw: &str) -> StreamCtx {
    let mut ctx = StreamCtx::new(
        "resp_test".to_owned(),
        "gpt-model".to_owned(),
        0.0,
        Some(slab_proto::openai::ServiceTier::Auto),
    );
    ctx.set_completed_service_tier(Some(slab_proto::openai::ServiceTier::Default));
    if chunks_completed_has_field(raw, "completed_at") {
        ctx.set_completed_at(Some(0.0));
    }
    ctx.set_skeleton(skeleton_from_chunks(raw));
    ctx
}

/// Whether the fixture's `response.completed` event's `response` object carries
/// `field` (used to decide if `completed_at` should be emitted).
fn chunks_completed_has_field(raw: &str, field: &str) -> bool {
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("type").and_then(|t| t.as_str()) == Some("response.completed")
            && v.get("response").and_then(|r| r.as_object()).is_some_and(|r| r.contains_key(field))
        {
            return true;
        }
    }
    false
}

/// Drive a single-response `.chunks.txt` through the adapter and assert the
/// redacted event sequence equals the redacted fixture lines.
fn assert_streaming_round_trips(raw: &'static str) {
    let expected = strip_web_search_added_action(dedupe_repeated_mcp_in_progress(
        strip_annotation_events_and_content(redacted_chunks(raw)),
    ));
    let mut ctx = ctx_from_chunks(raw);
    let envelopes = streaming_envelopes_from_chunks(raw, &mut ctx);
    let actual = strip_web_search_added_action(dedupe_repeated_mcp_in_progress(
        strip_annotation_events_and_content(redacted_stream_events(&envelopes, &mut ctx)),
    ));
    assert_eq!(expected, actual);
}

#[test]
fn mcp_tool_streaming_round_trips() {
    assert_streaming_round_trips(MCP_TOOL_CHUNKS);
}

#[test]
fn mcp_approval_1_streaming_round_trips() {
    assert_streaming_round_trips(MCP_APPROVAL_1_CHUNKS);
}

#[test]
fn mcp_approval_3_streaming_round_trips() {
    assert_streaming_round_trips(MCP_APPROVAL_3_CHUNKS);
}

#[test]
fn mcp_approval_2_streaming_round_trips() {
    // CROSS-PAIRED: `.2.chunks` streams the rejection message turn (text-only
    // final answer). Same harness as the text-only multiturn fixtures.
    assert_streaming_round_trips(MCP_APPROVAL_2_CHUNKS);
}

#[test]
fn mcp_approval_4_streaming_round_trips() {
    // CROSS-PAIRED: `.4.chunks` streams the approved `mcp_call` (carries
    // `approval_request_id` linking to `.3`'s request).
    assert_streaming_round_trips(MCP_APPROVAL_4_CHUNKS);
}

#[test]
fn shell_skills_streaming_round_trips() {
    assert_streaming_round_trips(SHELL_SKILLS_CHUNKS);
}

#[test]
fn shell_container_streaming_round_trips() {
    assert_streaming_round_trips(SHELL_CONTAINER_CHUNKS);
}

#[test]
fn shell_local_multiturn_streaming_round_trips() {
    assert_streaming_round_trips(SHELL_LOCAL_MULTITURN_CHUNKS);
}

#[test]
fn shell_container_multiturn_streaming_round_trips() {
    assert_streaming_round_trips(SHELL_CONTAINER_MULTITURN_CHUNKS);
}

#[test]
fn code_interpreter_streaming_round_trips() {
    assert_streaming_round_trips(CODE_INTERPRETER_CHUNKS);
}

#[test]
fn web_search_streaming_round_trips() {
    assert_streaming_round_trips(WEB_SEARCH_CHUNKS);
}

#[test]
fn file_search_2_streaming_round_trips() {
    assert_streaming_round_trips(FILE_SEARCH_2_CHUNKS);
}

/// Multi-response reasoning fixture. The `.chunks.txt` carries 4
/// independent response cycles (reasoning+function_call ×1, function_call ×2,
/// final message ×1). Each cycle is driven through a FRESH `StreamCtx` (reset
/// per cycle), and the concatenated redacted event vec is compared to the
/// redacted fixture lines.
#[test]
fn streaming_reasoning_summary_round_trips() {
    let cycles = split_chunks_into_cycles(REASONING_CHUNKS);
    assert!(cycles.len() > 1, "reasoning fixture must carry multiple response cycles");

    let mut expected_all: Vec<Value> = Vec::new();
    let mut actual_all: Vec<Value> = Vec::new();
    for cycle_raw in &cycles {
        expected_all.extend(normalize_reasoning_added_encrypted(redacted_chunks(cycle_raw)));
        let mut ctx = ctx_from_chunks(cycle_raw);
        let envelopes = streaming_envelopes_from_chunks(cycle_raw, &mut ctx);
        actual_all.extend(normalize_reasoning_added_encrypted(redacted_stream_events(
            &envelopes, &mut ctx,
        )));
    }

    assert_eq!(expected_all, actual_all);
}
