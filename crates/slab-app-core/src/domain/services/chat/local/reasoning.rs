use serde_json::Value;

use crate::domain::models::{
    ChatReasoningEffort, ChatVerbosity, ConversationMessage as DomainConversationMessage,
    ConversationMessageContent, JsonOptions, TextGenerationResponse,
};

const REASONING_CONTENT_METADATA_KEY: &str = "reasoning_content";
const THINK_OPEN_MARKER: &str = "<think";
const THINK_CLOSE_TAG: &str = "</think>";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedThinkingOutput {
    content: String,
    reasoning: String,
}

#[derive(Debug, Default)]
pub(super) struct ContentStopState {
    raw_content: String,
    emitted_len: usize,
    matched: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct StopEmission {
    pub(super) text: String,
    pub(super) matched: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct StreamDeltaRouting {
    pub(super) content: String,
    pub(super) reasoning: Option<String>,
}

fn trailing_partial_marker_len(raw: &str, marker: &str) -> usize {
    let max = raw.len().min(marker.len().saturating_sub(1));
    (1..=max).rev().find(|len| raw.ends_with(&marker[..*len])).unwrap_or(0)
}

fn normalize_thinking_content_prefix(prefix: &str) -> &str {
    if prefix.trim().is_empty() { "" } else { prefix }
}

fn parse_thinking_output(raw: &str, complete: bool) -> ParsedThinkingOutput {
    let Some(open_start) = raw.find(THINK_OPEN_MARKER) else {
        // No <think found - treat all text as content.
        return ParsedThinkingOutput { content: raw.to_owned(), reasoning: String::new() };
    };

    let content_prefix = normalize_thinking_content_prefix(&raw[..open_start]).to_owned();
    let after_open_marker = &raw[open_start..];
    let Some(open_end_rel) = after_open_marker.find('>') else {
        return ParsedThinkingOutput {
            content: if complete { raw.to_owned() } else { content_prefix },
            reasoning: String::new(),
        };
    };

    let reasoning_start = open_start + open_end_rel + 1;
    let after_open = &raw[reasoning_start..];
    if let Some(close_rel) = after_open.find(THINK_CLOSE_TAG) {
        let close_start = reasoning_start + close_rel;
        let close_end = close_start + THINK_CLOSE_TAG.len();
        let mut content = content_prefix;
        content.push_str(&raw[close_end..]);
        return ParsedThinkingOutput {
            content,
            reasoning: raw[reasoning_start..close_start].to_owned(),
        };
    }

    let stable_reasoning_end = if complete {
        raw.len()
    } else {
        raw.len().saturating_sub(trailing_partial_marker_len(raw, THINK_CLOSE_TAG))
    };
    ParsedThinkingOutput {
        content: content_prefix,
        reasoning: raw[reasoning_start..stable_reasoning_end].to_owned(),
    }
}

pub(super) fn reasoning_content_from_metadata(metadata: &JsonOptions) -> Option<&str> {
    metadata.get(REASONING_CONTENT_METADATA_KEY).and_then(Value::as_str)
}

fn trailing_partial_stop_len(raw: &str, stop: &[String]) -> usize {
    stop.iter()
        .filter(|value| value.len() > 1)
        .map(|value| {
            let max = raw.len().min(value.len().saturating_sub(1));
            (1..=max).rev().find(|len| raw.ends_with(&value[..*len])).unwrap_or(0)
        })
        .max()
        .unwrap_or(0)
}

fn stable_stop_boundary(raw: &str, stop: &[String], complete: bool) -> (usize, bool) {
    if let Some((index, _)) = stop
        .iter()
        .filter(|value| !value.is_empty())
        .filter_map(|value| raw.find(value).map(|index| (index, value)))
        .min_by_key(|(index, _)| *index)
    {
        return (index, true);
    }

    if complete {
        return (raw.len(), false);
    }

    let hold_len = trailing_partial_stop_len(raw, stop);
    (raw.len().saturating_sub(hold_len), false)
}

fn trailing_trim_len(raw: &str, trailing: &[String]) -> usize {
    trailing
        .iter()
        .filter(|value| !value.is_empty())
        .filter(|value| raw.ends_with(value.as_str()))
        .map(|value| value.len())
        .max()
        .unwrap_or(0)
}

pub(super) fn trim_trailing_stop_markers(raw: &str, trailing: &[String]) -> String {
    let trim_len = trailing_trim_len(raw, trailing);
    if trim_len == 0 {
        raw.to_owned()
    } else {
        raw[..raw.len().saturating_sub(trim_len)].to_owned()
    }
}

pub(super) fn merge_stop_sequences(primary: &[String], extra: &[String]) -> Vec<String> {
    let mut merged = Vec::new();
    for value in primary.iter().chain(extra.iter()) {
        if value.is_empty() || merged.iter().any(|existing| existing == value) {
            continue;
        }
        merged.push(value.clone());
    }
    merged
}

impl ContentStopState {
    pub(super) fn ingest(
        &mut self,
        delta: &str,
        stop: &[String],
        trailing: &[String],
    ) -> StopEmission {
        if self.matched || delta.is_empty() {
            return StopEmission::default();
        }

        self.raw_content.push_str(delta);
        self.emit(stop, trailing, false)
    }

    pub(super) fn finish(&mut self, stop: &[String], trailing: &[String]) -> StopEmission {
        self.emit(stop, trailing, true)
    }

    fn emit(&mut self, stop: &[String], trailing: &[String], complete: bool) -> StopEmission {
        if self.matched {
            return StopEmission::default();
        }

        let (mut visible_len, matched) = stable_stop_boundary(&self.raw_content, stop, complete);
        if complete && !matched {
            let trim_len = trailing_trim_len(&self.raw_content[..visible_len], trailing);
            visible_len = visible_len.saturating_sub(trim_len);
        }

        let text = if visible_len > self.emitted_len {
            self.raw_content[self.emitted_len..visible_len].to_owned()
        } else {
            String::new()
        };
        self.emitted_len = visible_len;
        if matched {
            self.matched = true;
        }

        StopEmission { text, matched }
    }
}

pub(super) fn reasoning_is_disabled(reasoning_effort: Option<ChatReasoningEffort>) -> bool {
    matches!(reasoning_effort, Some(ChatReasoningEffort::None))
}

fn remove_reasoning_content_metadata(metadata: &mut JsonOptions) -> Option<String> {
    match metadata.remove(REASONING_CONTENT_METADATA_KEY) {
        Some(Value::String(reasoning)) if !reasoning.trim().is_empty() => Some(reasoning),
        _ => None,
    }
}

pub(super) fn suppress_reasoning_output(response: &mut TextGenerationResponse) {
    let reasoning = remove_reasoning_content_metadata(&mut response.metadata);
    if response.text.trim().is_empty()
        && let Some(reasoning) = reasoning.map(|value| value.trim().to_owned())
    {
        response.text = reasoning;
    }
}

pub(super) fn route_stream_delta(
    content_delta: &str,
    reasoning_delta: Option<&str>,
    reasoning_disabled: bool,
) -> StreamDeltaRouting {
    if !reasoning_disabled {
        return StreamDeltaRouting {
            content: content_delta.to_owned(),
            reasoning: reasoning_delta.filter(|value| !value.is_empty()).map(str::to_owned),
        };
    }

    if !content_delta.is_empty() {
        return StreamDeltaRouting { content: content_delta.to_owned(), reasoning: None };
    }

    StreamDeltaRouting { content: reasoning_delta.unwrap_or_default().to_owned(), reasoning: None }
}

pub(super) fn attach_reasoning_metadata(response: &mut TextGenerationResponse) {
    if reasoning_content_from_metadata(&response.metadata).is_some() {
        return;
    }

    let parsed = parse_thinking_output(&response.text, true);
    let reasoning = parsed.reasoning.trim();
    if reasoning.is_empty() {
        return;
    }

    response.text = parsed.content;
    response
        .metadata
        .insert(REASONING_CONTENT_METADATA_KEY.into(), Value::String(reasoning.to_owned()));
}

pub(super) fn local_reasoning_guidance(
    reasoning_effort: Option<ChatReasoningEffort>,
    verbosity: Option<ChatVerbosity>,
) -> Option<String> {
    let mut lines = Vec::new();

    match reasoning_effort {
        Some(ChatReasoningEffort::None) => {
            lines.push(
                "Answer directly and do not emit <think>...</think> blocks or hidden reasoning."
                    .to_owned(),
            );
        }
        Some(ChatReasoningEffort::Minimal) => {
            lines.push(
                "If reasoning is necessary, keep it minimal and place it in a very short <think>...</think> block before the final answer."
                    .to_owned(),
            );
        }
        Some(ChatReasoningEffort::Low) => {
            lines.push(
                "If reasoning is useful, keep the <think>...</think> block brief and keep the final answer outside the block."
                    .to_owned(),
            );
        }
        Some(ChatReasoningEffort::Medium) => {
            lines.push(
                "If reasoning is useful, use a moderate <think>...</think> block and keep the final answer outside the block."
                    .to_owned(),
            );
        }
        Some(ChatReasoningEffort::High) => {
            lines.push(
                "You may use a detailed <think>...</think> block before the final answer, but keep the final answer outside the block."
                    .to_owned(),
            );
        }
        None => {}
    }

    match verbosity {
        Some(ChatVerbosity::Low) => {
            lines.push("Keep the final answer concise and compact.".to_owned());
        }
        Some(ChatVerbosity::Medium) => {
            lines.push("Keep the final answer moderately detailed.".to_owned());
        }
        Some(ChatVerbosity::High) => {
            lines.push("Keep the final answer detailed and thorough.".to_owned());
        }
        None => {}
    }

    if lines.is_empty() {
        None
    } else {
        Some(format!("Local response policy:\n{}", lines.join("\n")))
    }
}

pub(super) fn apply_local_reasoning_controls(
    messages: &[DomainConversationMessage],
    reasoning_effort: Option<ChatReasoningEffort>,
    verbosity: Option<ChatVerbosity>,
) -> Vec<DomainConversationMessage> {
    let Some(guidance) = local_reasoning_guidance(reasoning_effort, verbosity) else {
        return messages.to_vec();
    };

    let insert_at = messages
        .iter()
        .take_while(|message| matches!(message.role.as_str(), "system" | "developer"))
        .count();
    let mut guided_messages = messages.to_vec();
    guided_messages.insert(
        insert_at,
        DomainConversationMessage {
            role: "system".to_owned(),
            content: ConversationMessageContent::Text(guidance),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        },
    );
    guided_messages
}

pub(super) fn apply_local_reasoning_controls_to_prompt(
    prompt: &str,
    reasoning_effort: Option<ChatReasoningEffort>,
    verbosity: Option<ChatVerbosity>,
) -> String {
    match local_reasoning_guidance(reasoning_effort, verbosity) {
        Some(guidance) => format!("{guidance}\n\nPrompt:\n{prompt}"),
        None => prompt.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        ContentStopState, ParsedThinkingOutput, StopEmission, apply_local_reasoning_controls,
        apply_local_reasoning_controls_to_prompt, attach_reasoning_metadata,
        local_reasoning_guidance, parse_thinking_output, reasoning_content_from_metadata,
        route_stream_delta, suppress_reasoning_output, trim_trailing_stop_markers,
    };
    use crate::domain::models::{
        ChatReasoningEffort, ChatVerbosity, ConversationMessage as DomainConversationMessage,
        ConversationMessageContent, TextGenerationResponse,
    };

    #[test]
    fn parse_thinking_output_extracts_reasoning_block() {
        let parsed = parse_thinking_output("<think>step one</think>\n\nfinal answer", true);
        assert_eq!(
            parsed,
            ParsedThinkingOutput {
                content: "\n\nfinal answer".to_owned(),
                reasoning: "step one".to_owned(),
            }
        );
    }

    #[test]
    fn parse_thinking_output_no_think_tag_passes_through() {
        // Without <think>, all text is content - no hold-back.
        let parsed = parse_thinking_output("answer<th", false);
        assert_eq!(
            parsed,
            ParsedThinkingOutput { content: "answer<th".to_owned(), reasoning: String::new() }
        );
    }

    #[test]
    fn attach_reasoning_metadata_moves_reasoning_out_of_text() {
        let mut response = TextGenerationResponse {
            text: "<think>step by step</think>\n\nanswer".to_owned(),
            metadata: Default::default(),
            ..Default::default()
        };

        attach_reasoning_metadata(&mut response);

        assert_eq!(response.text, "\n\nanswer");
        assert_eq!(response.metadata.get("reasoning_content"), Some(&json!("step by step")));
    }

    #[test]
    fn attach_reasoning_metadata_ignores_whitespace_prefix_before_think() {
        let mut response = TextGenerationResponse {
            text: " <think>step by step</think>\n\nanswer".to_owned(),
            metadata: Default::default(),
            ..Default::default()
        };

        attach_reasoning_metadata(&mut response);

        assert_eq!(response.text, "\n\nanswer");
        assert_eq!(response.metadata.get("reasoning_content"), Some(&json!("step by step")));
    }

    #[test]
    fn attach_reasoning_metadata_keeps_runtime_reasoning_metadata() {
        let mut response = TextGenerationResponse {
            text: "answer".to_owned(),
            metadata: [("reasoning_content".to_owned(), json!("from runtime"))]
                .into_iter()
                .collect(),
            ..Default::default()
        };

        attach_reasoning_metadata(&mut response);

        assert_eq!(response.text, "answer");
        assert_eq!(reasoning_content_from_metadata(&response.metadata), Some("from runtime"));
    }

    #[test]
    fn suppress_reasoning_output_drops_reasoning_metadata_when_answer_exists() {
        let mut response = TextGenerationResponse {
            text: "answer".to_owned(),
            metadata: [("reasoning_content".to_owned(), json!("hidden chain"))]
                .into_iter()
                .collect(),
            ..Default::default()
        };

        suppress_reasoning_output(&mut response);

        assert_eq!(response.text, "answer");
        assert!(reasoning_content_from_metadata(&response.metadata).is_none());
    }

    #[test]
    fn suppress_reasoning_output_falls_back_to_reasoning_when_answer_is_empty() {
        let mut response = TextGenerationResponse {
            text: String::new(),
            metadata: [("reasoning_content".to_owned(), json!("answer hidden in reasoning"))]
                .into_iter()
                .collect(),
            ..Default::default()
        };

        suppress_reasoning_output(&mut response);

        assert_eq!(response.text, "answer hidden in reasoning");
        assert!(reasoning_content_from_metadata(&response.metadata).is_none());
    }

    #[test]
    fn local_reasoning_guidance_disables_think_blocks() {
        let guidance = local_reasoning_guidance(Some(ChatReasoningEffort::None), None)
            .expect("guidance should be produced");

        assert!(guidance.contains("do not emit <think>...</think>"));
    }

    #[test]
    fn route_stream_delta_suppresses_reasoning_when_disabled() {
        let routed = route_stream_delta("", Some("hidden answer"), true);

        assert_eq!(routed.content, "hidden answer");
        assert_eq!(routed.reasoning, None);
    }

    #[test]
    fn route_stream_delta_prefers_visible_content_when_disabled() {
        let routed = route_stream_delta("final answer", Some("hidden chain"), true);

        assert_eq!(routed.content, "final answer");
        assert_eq!(routed.reasoning, None);
    }

    #[test]
    fn apply_local_reasoning_controls_inserts_system_policy_after_existing_system_messages() {
        let messages = vec![
            DomainConversationMessage {
                role: "system".to_owned(),
                content: ConversationMessageContent::Text("existing".to_owned()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
            DomainConversationMessage {
                role: "user".to_owned(),
                content: ConversationMessageContent::Text("hello".to_owned()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
        ];

        let guided = apply_local_reasoning_controls(
            &messages,
            Some(ChatReasoningEffort::Low),
            Some(ChatVerbosity::High),
        );

        assert_eq!(guided.len(), 3);
        assert_eq!(guided[0].role, "system");
        assert_eq!(guided[1].role, "system");
        assert!(guided[1].rendered_text().contains("<think>...</think> block"));
        assert_eq!(guided[2].role, "user");
    }

    #[test]
    fn apply_local_reasoning_controls_to_prompt_prefixes_guidance() {
        let prompt = apply_local_reasoning_controls_to_prompt(
            "Solve 2+2",
            Some(ChatReasoningEffort::Minimal),
            Some(ChatVerbosity::Low),
        );

        assert!(prompt.starts_with("Local response policy:"));
        assert!(prompt.contains("Prompt:\nSolve 2+2"));
    }

    #[test]
    fn content_stop_state_trims_chatml_boundaries() {
        let stop = vec!["<|im_end|>".to_owned(), "<|endoftext|><|im_start|>".to_owned()];
        let trailing = vec!["<|endoftext|>".to_owned()];
        let mut state = ContentStopState::default();

        let first = state.ingest("hello<|endoftext|>", &stop, &trailing);
        let last = state.finish(&stop, &trailing);

        assert_eq!(first, StopEmission { text: "hello".to_owned(), matched: false });
        assert_eq!(last, StopEmission::default());
    }

    #[test]
    fn content_stop_state_stops_before_im_end_marker() {
        let stop = vec!["<|im_end|>".to_owned()];
        let trailing = Vec::new();
        let mut state = ContentStopState::default();

        let first = state.ingest("hello<|im", &stop, &trailing);
        let second = state.ingest("_end|>ignored", &stop, &trailing);

        assert_eq!(first, StopEmission { text: "hello".to_owned(), matched: false });
        assert_eq!(second, StopEmission { text: String::new(), matched: true });
        assert!(state.finish(&stop, &trailing).text.is_empty());
    }

    #[test]
    fn content_stop_state_stops_before_raw_chat_role_marker() {
        let stop = vec!["\nUser:".to_owned(), "\nAssistant:".to_owned()];
        let trailing = Vec::new();
        let mut state = ContentStopState::default();

        let first = state.ingest("hello\nUs", &stop, &trailing);
        let second = state.ingest("er: next turn", &stop, &trailing);

        assert_eq!(first, StopEmission { text: "hello".to_owned(), matched: false });
        assert_eq!(second, StopEmission { text: String::new(), matched: true });
        assert!(state.finish(&stop, &trailing).text.is_empty());
    }

    #[test]
    fn trim_trailing_stop_markers_removes_final_endoftext() {
        let trimmed =
            trim_trailing_stop_markers("answer<|endoftext|>", &["<|endoftext|>".to_owned()]);

        assert_eq!(trimmed, "answer");
    }
}
