use serde_json::{Map, Value};
use slab_agent::error::AgentError;
use slab_agent::port::ParsedToolCall;
use slab_proto::openai::FunctionToolCall;
use uuid::Uuid;

const QWEN_TOOL_CALL_OPEN: &str = "<tool_call>";
const QWEN_TOOL_CALL_CLOSE: &str = "</tool_call>";

#[derive(Default)]
pub(crate) struct AgentStreamAssembler {
    content: String,
    reasoning: String,
    finish_reason: Option<String>,
    visibility: StreamVisibilityGate,
}

pub(crate) enum AgentStreamDelta {
    Text(String),
    Reasoning(String),
}

pub(crate) struct AgentStreamCompletion {
    pub(crate) content: String,
    pub(crate) content_already_streamed: bool,
    pub(crate) reasoning: String,
    pub(crate) unstreamed_text_delta: Option<String>,
    pub(crate) tool_calls: Vec<ParsedToolCall>,
    pub(crate) finish_reason: Option<String>,
}

impl AgentStreamAssembler {
    pub(crate) fn ingest_data(&mut self, data: &str) -> Result<Vec<AgentStreamDelta>, AgentError> {
        let Some(parsed) = parse_chat_stream_chunk(data)? else {
            return Ok(Vec::new());
        };

        let mut deltas = Vec::new();
        if let Some(reasoning_delta) = parsed.reasoning_delta {
            self.reasoning.push_str(&reasoning_delta);
            deltas.push(AgentStreamDelta::Reasoning(reasoning_delta));
        }
        if let Some(content_delta) = parsed.content_delta {
            self.content.push_str(&content_delta);
            if let Some(visible_delta) = self.visibility.visible_delta(&self.content) {
                deltas.push(AgentStreamDelta::Text(visible_delta));
            }
        }
        if parsed.finish_reason.is_some() {
            self.finish_reason = parsed.finish_reason;
        }

        Ok(deltas)
    }

    pub(crate) fn finish(mut self) -> AgentStreamCompletion {
        let parsed = parse_rendered_tool_call_output(&self.content);
        let should_hide_unparsed_buffer = parsed.tool_calls.is_empty()
            && unparsed_stream_buffer_should_remain_hidden(&self.content);
        let unstreamed_text_delta = if parsed.tool_calls.is_empty() && !should_hide_unparsed_buffer
        {
            self.visibility.flush(&self.content)
        } else {
            None
        };
        let content_already_streamed =
            self.visibility.has_emitted() || unstreamed_text_delta.is_some();

        AgentStreamCompletion {
            content: if should_hide_unparsed_buffer {
                self.visibility.emitted_prefix(&self.content)
            } else if parsed.tool_calls.is_empty() {
                self.content
            } else {
                parsed.content.unwrap_or_default()
            },
            content_already_streamed,
            reasoning: self.reasoning,
            unstreamed_text_delta,
            tool_calls: parsed.tool_calls,
            finish_reason: self.finish_reason,
        }
    }
}

#[derive(Default)]
pub(crate) struct RenderedToolCallOutput {
    pub(crate) content: Option<String>,
    pub(crate) tool_calls: Vec<ParsedToolCall>,
}

pub(crate) fn parse_rendered_tool_call_output(content: &str) -> RenderedToolCallOutput {
    if let Some(value) = parse_tool_json(content) {
        let tool_calls = parse_responses_tool_calls(&value);
        if !tool_calls.is_empty() {
            return RenderedToolCallOutput { content: None, tool_calls };
        }
    }

    parse_qwen_tool_call_output(content)
}

#[derive(Default)]
pub(crate) struct ParsedChatStreamChunk {
    pub(crate) content_delta: Option<String>,
    pub(crate) reasoning_delta: Option<String>,
    pub(crate) finish_reason: Option<String>,
}

pub(crate) fn parse_chat_stream_chunk(
    data: &str,
) -> Result<Option<ParsedChatStreamChunk>, AgentError> {
    let trimmed = data.trim();
    if trimmed.is_empty() || trimmed == "[DONE]" {
        return Ok(None);
    }

    let Ok(payload) = serde_json::from_str::<Value>(trimmed) else {
        return Ok(None);
    };

    if let Some(message) = stream_error_message(&payload) {
        return Err(AgentError::Llm(message));
    }

    Ok(Some(ParsedChatStreamChunk {
        content_delta: collect_text_delta(&payload, "content"),
        reasoning_delta: collect_text_delta(&payload, "reasoning_content"),
        finish_reason: stream_finish_reason(&payload),
    }))
}

fn stream_error_message(payload: &Value) -> Option<String> {
    let error = payload.get("error")?;
    error
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| error.as_str())
        .map(str::to_owned)
        .or_else(|| Some("LLM stream returned an error".to_owned()))
}

fn collect_text_delta(payload: &Value, field: &str) -> Option<String> {
    let text = payload
        .get("choices")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|choice| {
            choice.get("delta").and_then(|delta| delta.get(field)).and_then(Value::as_str)
        })
        .filter(|value| !value.is_empty())
        .collect::<String>();
    if text.is_empty() { None } else { Some(text) }
}

fn stream_finish_reason(payload: &Value) -> Option<String> {
    payload
        .get("choices")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|choice| choice.get("finish_reason").and_then(Value::as_str))
        .find(|value| !value.is_empty())
        .map(str::to_owned)
}

#[derive(Default)]
struct StreamVisibilityGate {
    emitted_len: usize,
}

impl StreamVisibilityGate {
    fn has_emitted(&self) -> bool {
        self.emitted_len > 0
    }

    fn emitted_prefix(&self, content: &str) -> String {
        content[..self.emitted_len].trim_end().to_owned()
    }

    fn visible_delta(&mut self, content: &str) -> Option<String> {
        let visible_end = self.visible_boundary(content);
        if visible_end <= self.emitted_len {
            return None;
        }

        let delta = content[self.emitted_len..visible_end].to_owned();
        self.emitted_len = visible_end;
        Some(delta)
    }

    fn flush(&mut self, content: &str) -> Option<String> {
        if self.emitted_len >= content.len() {
            return None;
        }

        let delta = content[self.emitted_len..].to_owned();
        self.emitted_len = content.len();
        if delta.is_empty() { None } else { Some(delta) }
    }

    fn visible_boundary(&self, content: &str) -> usize {
        if self.emitted_len == 0 && stream_prefix_needs_buffering(content) {
            return 0;
        }

        let rest = &content[self.emitted_len..];
        if let Some(index) = rest.find(QWEN_TOOL_CALL_OPEN) {
            return self.emitted_len + index;
        }

        content.len().saturating_sub(trailing_partial_marker_len(content, QWEN_TOOL_CALL_OPEN))
    }
}

fn stream_prefix_needs_buffering(buffer: &str) -> bool {
    let trimmed = buffer.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('{') || trimmed.starts_with('[') {
        return true;
    }

    let Some(rest) = trimmed.strip_prefix("```") else {
        return false;
    };
    let Some(newline) = rest.find('\n') else {
        return true;
    };
    let language = rest[..newline].trim();
    language.is_empty() || language.eq_ignore_ascii_case("json")
}

fn trailing_partial_marker_len(raw: &str, marker: &str) -> usize {
    let max = raw.len().min(marker.len().saturating_sub(1));
    (1..=max).rev().find(|len| raw.ends_with(&marker[..*len])).unwrap_or(0)
}

fn parse_tool_json(content: &str) -> Option<Value> {
    let trimmed = strip_json_fence(content.trim())?;
    serde_json::from_str::<Value>(trimmed).ok()
}

fn unparsed_stream_buffer_should_remain_hidden(content: &str) -> bool {
    let trimmed = content.trim_start();
    if trailing_partial_marker_len(content, QWEN_TOOL_CALL_OPEN) > 0 {
        return true;
    }
    if trimmed.contains(QWEN_TOOL_CALL_OPEN) {
        return true;
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return parse_tool_json(trimmed).is_none();
    }

    let Some(rest) = trimmed.strip_prefix("```") else {
        return false;
    };
    let Some(newline) = rest.find('\n') else {
        return true;
    };
    let language = rest[..newline].trim();
    (language.is_empty() || language.eq_ignore_ascii_case("json"))
        && parse_tool_json(trimmed).is_none()
}

fn strip_json_fence(content: &str) -> Option<&str> {
    let Some(rest) = content.strip_prefix("```") else {
        return Some(content);
    };
    let newline = rest.find('\n')?;
    let language = rest[..newline].trim();
    if !language.is_empty() && !language.eq_ignore_ascii_case("json") {
        return Some(content);
    }

    let body = rest[newline + 1..].trim();
    body.strip_suffix("```").map(str::trim)
}

fn parse_responses_tool_calls(value: &Value) -> Vec<ParsedToolCall> {
    if let Some(items) = value.get("output").and_then(Value::as_array) {
        return items.iter().filter_map(parse_responses_function_call).collect();
    }

    parse_responses_function_call(value).into_iter().collect()
}

fn parse_responses_function_call(value: &Value) -> Option<ParsedToolCall> {
    let call: FunctionToolCall = serde_json::from_value(value.clone()).ok()?;
    let name = call.name.trim().to_owned();
    if name.is_empty() {
        return None;
    }
    let id = if call.call_id.trim().is_empty() {
        call.id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string())
    } else {
        call.call_id
    };

    Some(ParsedToolCall { id, name, arguments: normalize_arguments(call.arguments) })
}

fn parse_qwen_tool_call_output(content: &str) -> RenderedToolCallOutput {
    let stripped = strip_reasoning_prefix(content.trim());
    let Some(tool_call_start) = stripped.find(QWEN_TOOL_CALL_OPEN) else {
        return RenderedToolCallOutput::default();
    };
    let visible_prefix = stripped[..tool_call_start].trim_end();
    let mut rest = stripped[tool_call_start..].trim_start();
    let mut calls = Vec::new();

    while let Some(after_open) = rest.strip_prefix(QWEN_TOOL_CALL_OPEN) {
        let Some(close_start) = after_open.find(QWEN_TOOL_CALL_CLOSE) else {
            return RenderedToolCallOutput::default();
        };
        let block = &after_open[..close_start];
        let Some(call) = parse_qwen_tool_call_block(block.trim()) else {
            return RenderedToolCallOutput::default();
        };
        calls.push(call);
        rest = after_open[close_start + QWEN_TOOL_CALL_CLOSE.len()..].trim_start();
    }

    if calls.is_empty() || !rest.trim().is_empty() {
        return RenderedToolCallOutput::default();
    }

    RenderedToolCallOutput {
        content: (!visible_prefix.is_empty()).then(|| visible_prefix.to_owned()),
        tool_calls: calls,
    }
}

fn strip_reasoning_prefix(content: &str) -> &str {
    let Some(close_start) = content.find("</think>") else {
        return content.trim();
    };
    let after_reasoning = content[close_start + "</think>".len()..].trim_start();
    if after_reasoning.contains(QWEN_TOOL_CALL_OPEN) || after_reasoning.starts_with('{') {
        after_reasoning
    } else {
        content.trim()
    }
}

fn parse_qwen_tool_call_block(block: &str) -> Option<ParsedToolCall> {
    if let Some(call) = parse_qwen_json_tool_call_block(block) {
        return Some(call);
    }

    let function_start = block.strip_prefix("<function=")?;
    let name_end = function_start.find('>')?;
    let name = function_start[..name_end].trim();
    if name.is_empty() {
        return None;
    }
    let function_body = function_start[name_end + 1..].trim();
    let function_body = function_body.strip_suffix("</function>")?.trim();
    let arguments = parse_qwen_parameters(function_body)?;

    Some(ParsedToolCall {
        id: Uuid::new_v4().to_string(),
        name: name.to_owned(),
        arguments: serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".to_owned()),
    })
}

fn parse_qwen_json_tool_call_block(block: &str) -> Option<ParsedToolCall> {
    let value = serde_json::from_str::<Value>(block).ok()?;
    let object = value.as_object()?;
    let name = object.get("name").and_then(Value::as_str)?.trim();
    if name.is_empty() {
        return None;
    }
    let arguments = object.get("arguments").cloned().unwrap_or_else(|| Value::Object(Map::new()));
    let arguments = match arguments {
        Value::String(text) => normalize_arguments(text),
        other => serde_json::to_string(&other).ok()?,
    };
    let id = object
        .get("call_id")
        .or_else(|| object.get("id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    Some(ParsedToolCall { id, name: name.to_owned(), arguments })
}

fn parse_qwen_parameters(mut input: &str) -> Option<Value> {
    let mut arguments = Map::new();
    input = input.trim();
    while !input.is_empty() {
        let parameter_start = input.strip_prefix("<parameter=")?;
        let name_end = parameter_start.find('>')?;
        let name = parameter_start[..name_end].trim();
        if name.is_empty() {
            return None;
        }
        let value_start = &parameter_start[name_end + 1..];
        let value_end = value_start.find("</parameter>")?;
        let raw_value = value_start[..value_end].trim();
        let value = serde_json::from_str::<Value>(raw_value)
            .unwrap_or_else(|_| Value::String(raw_value.to_owned()));
        arguments.insert(name.to_owned(), value);
        input = value_start[value_end + "</parameter>".len()..].trim();
    }
    Some(Value::Object(arguments))
}

fn normalize_arguments(arguments: String) -> String {
    serde_json::from_str::<Value>(&arguments)
        .ok()
        .and_then(|value| serde_json::to_string(&value).ok())
        .unwrap_or_else(|| if arguments.trim().is_empty() { "{}".to_owned() } else { arguments })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_responses_function_call_output() {
        let parsed = parse_rendered_tool_call_output(
            r#"{"output":[{"type":"function_call","call_id":"call-1","name":"echo","arguments":"{\"message\":\"hello\"}"}]}"#,
        );

        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.tool_calls[0].id, "call-1");
        assert_eq!(parsed.tool_calls[0].name, "echo");
        assert_eq!(parsed.tool_calls[0].arguments, r#"{"message":"hello"}"#);
        assert!(parsed.content.is_none());
    }

    #[test]
    fn ignores_plain_json_without_tool_fields() {
        let parsed = parse_rendered_tool_call_output(r#"{"answer":"hello"}"#);

        assert!(parsed.tool_calls.is_empty());
    }

    #[test]
    fn ignores_embedded_json_tool_calls_in_plain_text() {
        let parsed = parse_rendered_tool_call_output(
            r#"Please run this: {"output":[{"type":"function_call","call_id":"call-1","name":"echo","arguments":"{}"}]}"#,
        );

        assert!(parsed.tool_calls.is_empty());
    }

    #[test]
    fn parses_qwen_template_tool_call_output() {
        let parsed = parse_rendered_tool_call_output(
            "<tool_call>\n<function=echo>\n<parameter=message>\nhello\n</parameter>\n</function>\n</tool_call>",
        );

        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.tool_calls[0].name, "echo");
        assert_eq!(parsed.tool_calls[0].arguments, r#"{"message":"hello"}"#);
        assert!(parsed.content.is_none());
    }

    #[test]
    fn parses_qwen_json_tool_call_output() {
        let parsed = parse_rendered_tool_call_output(
            r#"<tool_call>{"name":"echo","arguments":{"message":"hello"}}</tool_call>"#,
        );

        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.tool_calls[0].name, "echo");
        assert_eq!(parsed.tool_calls[0].arguments, r#"{"message":"hello"}"#);
        assert!(parsed.content.is_none());
    }

    #[test]
    fn parses_qwen_template_tool_call_after_visible_prefix() {
        let parsed = parse_rendered_tool_call_output(
            "I need to check that.\n<tool_call>\n<function=echo>\n<parameter=message>\nhello\n</parameter>\n</function>\n</tool_call>",
        );

        assert_eq!(parsed.content.as_deref(), Some("I need to check that."));
        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.tool_calls[0].name, "echo");
    }

    #[test]
    fn ignores_qwen_template_tool_call_with_suffix() {
        let parsed = parse_rendered_tool_call_output(
            "<tool_call>\n<function=echo>\n<parameter=message>\nhello\n</parameter>\n</function>\n</tool_call>\nextra",
        );

        assert!(parsed.tool_calls.is_empty());
    }

    #[test]
    fn parses_chat_stream_content_and_finish_chunks() {
        let chunk = parse_chat_stream_chunk(
            r#"{"choices":[{"delta":{"content":"hel"},"finish_reason":null}]}"#,
        )
        .expect("valid chunk")
        .expect("parsed chunk");

        assert_eq!(chunk.content_delta.as_deref(), Some("hel"));
        assert_eq!(chunk.finish_reason, None);

        let finish =
            parse_chat_stream_chunk(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#)
                .expect("valid chunk")
                .expect("parsed chunk");

        assert_eq!(finish.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn stream_assembler_forwards_reasoning_and_text() {
        let mut assembler = AgentStreamAssembler::default();

        let first = assembler
            .ingest_data(r#"{"choices":[{"delta":{"reasoning_content":"plan "}}]}"#)
            .expect("reasoning chunk");
        let second = assembler
            .ingest_data(
                r#"{"choices":[{"delta":{"reasoning_content":"done","content":"answer"}}]}"#,
            )
            .expect("mixed chunk");
        let completion = assembler.finish();

        assert!(matches!(&first[0], AgentStreamDelta::Reasoning(delta) if delta == "plan "));
        assert!(matches!(&second[0], AgentStreamDelta::Reasoning(delta) if delta == "done"));
        assert!(matches!(&second[1], AgentStreamDelta::Text(delta) if delta == "answer"));
        assert_eq!(completion.reasoning, "plan done");
        assert_eq!(completion.content, "answer");
    }

    #[test]
    fn stream_assembler_holds_qwen_tool_call_until_complete() {
        let mut assembler = AgentStreamAssembler::default();

        assert!(
            assembler
                .ingest_data(r#"{"choices":[{"delta":{"content":"<tool"}}]}"#)
                .expect("prefix")
                .is_empty()
        );
        assert!(
            assembler
                .ingest_data(r#"{"choices":[{"delta":{"content":"_call>\n<function=echo>\n<parameter=message>\nhello\n</parameter>\n</function>\n</tool_call>"}}]}"#)
                .expect("complete tool call")
                .is_empty()
        );
        let completion = assembler.finish();

        assert_eq!(completion.tool_calls.len(), 1);
        assert_eq!(completion.tool_calls[0].name, "echo");
        assert_eq!(completion.tool_calls[0].arguments, r#"{"message":"hello"}"#);
        assert_eq!(completion.unstreamed_text_delta, None);
    }

    #[test]
    fn stream_assembler_holds_responses_json_until_finish() {
        let mut assembler = AgentStreamAssembler::default();

        assert!(
            assembler
                .ingest_data(r#"{"choices":[{"delta":{"content":"{\"output\":[{\"type\":\"function_call\","}}]}"#)
                .expect("json prefix")
                .is_empty()
        );
        assert!(
            assembler
                .ingest_data(r#"{"choices":[{"delta":{"content":"\"call_id\":\"call-1\",\"name\":\"echo\",\"arguments\":\"{\\\"message\\\":\\\"hello\\\"}\"}]}"}}]}"#)
                .expect("json suffix")
                .is_empty()
        );
        let completion = assembler.finish();

        assert_eq!(completion.tool_calls.len(), 1);
        assert_eq!(completion.tool_calls[0].id, "call-1");
        assert_eq!(completion.tool_calls[0].name, "echo");
        assert_eq!(completion.unstreamed_text_delta, None);
    }

    #[test]
    fn stream_assembler_flushes_held_non_tool_json_on_finish() {
        let mut assembler = AgentStreamAssembler::default();

        assert!(
            assembler
                .ingest_data(r#"{"choices":[{"delta":{"content":"{\"answer\":\"hello\"}"}}]}"#)
                .expect("json answer")
                .is_empty()
        );
        let completion = assembler.finish();

        assert!(completion.tool_calls.is_empty());
        assert_eq!(completion.unstreamed_text_delta.as_deref(), Some(r#"{"answer":"hello"}"#));
        assert_eq!(completion.content, r#"{"answer":"hello"}"#);
    }

    #[test]
    fn stream_assembler_keeps_incomplete_json_hidden_on_finish() {
        let mut assembler = AgentStreamAssembler::default();

        assert!(
            assembler
                .ingest_data(r#"{"choices":[{"delta":{"content":"{\"output\":[{\"type\":\"function_call\""}}]}"#)
                .expect("json prefix")
                .is_empty()
        );
        let completion = assembler.finish();

        assert!(completion.tool_calls.is_empty());
        assert_eq!(completion.unstreamed_text_delta, None);
        assert_eq!(completion.content, "");
    }

    #[test]
    fn stream_assembler_keeps_incomplete_json_fence_hidden_on_finish() {
        let mut assembler = AgentStreamAssembler::default();

        assert!(
            assembler
                .ingest_data(r#"{"choices":[{"delta":{"content":"```json\n{\"output\":[{\"type\":\"function_call\",\"name\":\"echo\"}]"}}]}"#)
                .expect("json fence")
                .is_empty()
        );
        let completion = assembler.finish();

        assert!(completion.tool_calls.is_empty());
        assert_eq!(completion.unstreamed_text_delta, None);
        assert_eq!(completion.content, "");
    }

    #[test]
    fn stream_assembler_keeps_incomplete_qwen_tool_call_hidden_on_finish() {
        let mut assembler = AgentStreamAssembler::default();

        let deltas = assembler
            .ingest_data(r#"{"choices":[{"delta":{"content":"I need to check. <tool"}}]}"#)
            .expect("tool prefix");
        assert!(
            matches!(&deltas[0], AgentStreamDelta::Text(delta) if delta == "I need to check. ")
        );
        let completion = assembler.finish();

        assert!(completion.tool_calls.is_empty());
        assert_eq!(completion.unstreamed_text_delta, None);
        assert_eq!(completion.content, "I need to check.");
    }

    #[test]
    fn stream_assembler_streams_plain_text_before_qwen_tool_call() {
        let mut assembler = AgentStreamAssembler::default();

        let deltas = assembler
            .ingest_data(r#"{"choices":[{"delta":{"content":"I need to check. <tool"}}]}"#)
            .expect("prefix");
        assert!(
            matches!(&deltas[0], AgentStreamDelta::Text(delta) if delta == "I need to check. ")
        );
        assert!(
            assembler
                .ingest_data(r#"{"choices":[{"delta":{"content":"_call>\n<function=echo>\n<parameter=message>\nhello\n</parameter>\n</function>\n</tool_call>"}}]}"#)
                .expect("tool call")
                .is_empty()
        );
        let completion = assembler.finish();

        assert_eq!(completion.tool_calls.len(), 1);
        assert_eq!(completion.content, "I need to check.");
    }

    #[test]
    fn stream_assembler_flushes_non_json_code_fences_after_language() {
        let mut assembler = AgentStreamAssembler::default();

        assert!(
            assembler
                .ingest_data(r#"{"choices":[{"delta":{"content":"```"}}]}"#)
                .expect("fence prefix")
                .is_empty()
        );
        let deltas = assembler
            .ingest_data(r#"{"choices":[{"delta":{"content":"rust\nfn main() {}"}}]}"#)
            .expect("rust fence");

        assert!(
            matches!(&deltas[0], AgentStreamDelta::Text(delta) if delta == "```rust\nfn main() {}")
        );
    }
}
