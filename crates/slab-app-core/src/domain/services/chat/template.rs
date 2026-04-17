use std::time::{SystemTime, UNIX_EPOCH};

use minijinja::{Environment, Error, ErrorKind, UndefinedBehavior, context};
use serde_json::{Map, Value};

use crate::domain::models::{
    ChatReasoningEffort, ConversationMessage as DomainConversationMessage,
};
use crate::error::AppCoreError;

pub(super) fn build_prompt(
    messages: &[DomainConversationMessage],
    chat_template_source: Option<&str>,
    reasoning_effort: Option<ChatReasoningEffort>,
) -> Result<String, AppCoreError> {
    match chat_template_source.map(str::trim).filter(|value| !value.is_empty()) {
        Some(source) => render_minijinja_template(
            source,
            messages,
            reasoning_effort.map(|value| !matches!(value, ChatReasoningEffort::None)),
        ),
        None => Ok(render_raw_chat(messages)),
    }
}

pub(super) fn default_stop_sequences(chat_template_source: Option<&str>) -> Vec<String> {
    let Some(source) = chat_template_source.map(str::trim).filter(|value| !value.is_empty()) else {
        return raw_chat_stop_sequences();
    };

    let mut stop_sequences = Vec::new();
    if source.contains("<|im_start|>") && source.contains("<|im_end|>") {
        push_unique(&mut stop_sequences, "<|im_end|>");
        push_unique(&mut stop_sequences, "<|endoftext|><|im_start|>");
        push_unique(&mut stop_sequences, "<|endoftext|>\n<|im_start|>");
    }
    if source.contains("<|eot_id|>") {
        push_unique(&mut stop_sequences, "<|eot_id|>");
    }
    if source.contains("</s>") {
        push_unique(&mut stop_sequences, "</s>");
    }

    stop_sequences
}

pub(super) fn trailing_stop_markers(chat_template_source: Option<&str>) -> Vec<String> {
    let Some(source) = chat_template_source.map(str::trim).filter(|value| !value.is_empty()) else {
        return Vec::new();
    };

    let mut markers = Vec::new();
    if source.contains("<|im_start|>") && source.contains("<|im_end|>") {
        push_unique(&mut markers, "<|endoftext|>");
    }

    markers
}

/// Returns `true` when the Jinja chat template natively references the
/// `enable_thinking` variable.  When this is the case the template itself
/// controls thinking behaviour (e.g. Qwen3, DeepSeek-R1) and external
/// reasoning-guidance system messages should be skipped to avoid confusing
/// the model.
pub(super) fn template_supports_thinking(chat_template_source: Option<&str>) -> bool {
    chat_template_source
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .is_some_and(|source| source.contains("enable_thinking"))
}

fn render_minijinja_template(
    source: &str,
    messages: &[DomainConversationMessage],
    enable_thinking: Option<bool>,
) -> Result<String, AppCoreError> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.add_function("raise_exception", |message: String| -> Result<String, Error> {
        Err(Error::new(ErrorKind::InvalidOperation, message))
    });
    env.add_function("strftime_now", |format: String| -> Result<String, Error> {
        render_strftime_now(&format)
            .ok_or_else(|| Error::new(ErrorKind::InvalidOperation, "unsupported time format"))
    });
    env.add_template("chat_template", source).map_err(|error| {
        AppCoreError::BadRequest(format!("configured chat_template failed to parse: {error}"))
    })?;

    let has_assistant_prefill = messages.last().is_some_and(|message| message.role == "assistant");
    let template_messages = normalize_template_messages(messages);
    let template = env.get_template("chat_template").map_err(|error| {
        AppCoreError::BadRequest(format!("configured chat_template failed to load: {error}"))
    })?;
    let eos_token = infer_eos_token(source);

    let render_result = match enable_thinking {
        Some(enable_thinking) => template.render(context! {
            messages => &template_messages,
            add_generation_prompt => !has_assistant_prefill,
            continue_final_message => has_assistant_prefill,
            bos_token => "",
            eos_token => eos_token,
            unk_token => "",
            pad_token => "",
            tools => Vec::<Value>::new(),
            documents => Vec::<Value>::new(),
            enable_thinking => enable_thinking,
        }),
        None => template.render(context! {
            messages => &template_messages,
            add_generation_prompt => !has_assistant_prefill,
            continue_final_message => has_assistant_prefill,
            bos_token => "",
            eos_token => eos_token,
            unk_token => "",
            pad_token => "",
            tools => Vec::<Value>::new(),
            documents => Vec::<Value>::new(),
        }),
    };

    render_result.map_err(|error| {
        AppCoreError::BadRequest(format!("configured chat_template failed to render: {error}"))
    })
}

const THINK_OPEN_MARKER: &str = "<think";
const THINK_CLOSE_TAG: &str = "</think>";

fn infer_eos_token(source: &str) -> &'static str {
    if source.contains("<|im_end|>") {
        "<|im_end|>"
    } else if source.contains("<|eot_id|>") {
        "<|eot_id|>"
    } else if source.contains("</s>") {
        "</s>"
    } else {
        ""
    }
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|item| item == value) {
        values.push(value.to_owned());
    }
}

fn raw_chat_stop_sequences() -> Vec<String> {
    // The raw fallback prompt renders `Role: ...` lines and ends with `Assistant:`.
    // Treat the next role marker as a stop boundary so echoed turns do not leak into output.
    let mut stop_sequences = Vec::new();
    for marker in ["\nSystem:", "\nUser:", "\nAssistant:", "\nTool:"] {
        push_unique(&mut stop_sequences, marker);
    }
    stop_sequences
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedAssistantContent {
    content: String,
    reasoning: Option<String>,
}

fn normalize_template_messages(messages: &[DomainConversationMessage]) -> Vec<Value> {
    messages.iter().map(normalize_template_message).collect()
}

fn normalize_template_message(message: &DomainConversationMessage) -> Value {
    let mut object = Map::new();
    let mut content = serde_json::to_value(&message.content)
        .unwrap_or_else(|_| Value::String(message.rendered_text()));

    if message.role == "assistant"
        && let Some(text) = content.as_str()
        && let Some(normalized) = normalize_assistant_content(text)
    {
        content = Value::String(normalized.content);
        if let Some(reasoning) = normalized.reasoning {
            object.insert("reasoning_content".to_owned(), Value::String(reasoning));
        }
    }

    object.insert("role".to_owned(), Value::String(message.role.clone()));
    object.insert("content".to_owned(), content);

    if let Some(name) = message.name.as_ref() {
        object.insert("name".to_owned(), Value::String(name.clone()));
    }
    if let Some(tool_call_id) = message.tool_call_id.as_ref() {
        object.insert("tool_call_id".to_owned(), Value::String(tool_call_id.clone()));
    }
    if !message.tool_calls.is_empty() {
        object.insert(
            "tool_calls".to_owned(),
            serde_json::to_value(&message.tool_calls).unwrap_or_else(|_| Value::Array(Vec::new())),
        );
    }

    Value::Object(object)
}

fn normalize_assistant_content(raw: &str) -> Option<NormalizedAssistantContent> {
    let open_start = raw.find(THINK_OPEN_MARKER)?;
    let after_open_marker = &raw[open_start..];
    let open_end_rel = after_open_marker.find('>')?;
    let reasoning_start = open_start + open_end_rel + 1;
    let close_rel = raw[reasoning_start..].find(THINK_CLOSE_TAG)?;
    let close_start = reasoning_start + close_rel;
    let close_end = close_start + THINK_CLOSE_TAG.len();

    let mut content = raw[..open_start].to_owned();
    content.push_str(&raw[close_end..]);

    let reasoning = raw[reasoning_start..close_start].trim();
    Some(NormalizedAssistantContent {
        content,
        reasoning: (!reasoning.is_empty()).then(|| reasoning.to_owned()),
    })
}

fn render_strftime_now(format: &str) -> Option<String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(now as i64, 0)?;
    Some(datetime.format(format).to_string())
}

fn render_raw_chat(messages: &[DomainConversationMessage]) -> String {
    let (history, assistant_prefill) = split_assistant_prefill(messages);
    let mut lines: Vec<String> = history
        .iter()
        .map(|message| format!("{}: {}", display_role(&message.role), message.rendered_text()))
        .collect();
    let mut assistant = String::from("Assistant:");
    if let Some(prefill) = assistant_prefill.as_deref()
        && !prefill.is_empty()
    {
        assistant.push(' ');
        assistant.push_str(prefill);
    }
    lines.push(assistant);
    lines.join("\n")
}

fn split_assistant_prefill(
    messages: &[DomainConversationMessage],
) -> (&[DomainConversationMessage], Option<String>) {
    match messages.last() {
        Some(message) if message.role == "assistant" => {
            (&messages[..messages.len().saturating_sub(1)], Some(message.rendered_text()))
        }
        _ => (messages, None),
    }
}

fn display_role(role: &str) -> &str {
    match role {
        "user" => "User",
        "assistant" => "Assistant",
        "system" | "developer" => "System",
        "tool" | "function" => "Tool",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::build_prompt;
    use crate::domain::models::{
        ChatReasoningEffort, ConversationMessage as DomainConversationMessage,
        ConversationMessageContent,
    };

    const QWEN35_TEMPLATE: &str =
        include_str!("../../../../../../models/llama/Qwen3.5-9B/configs/chat_template.jinja");

    fn message(role: &str, content: &str) -> DomainConversationMessage {
        DomainConversationMessage {
            role: role.to_owned(),
            content: ConversationMessageContent::Text(content.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    #[test]
    fn raw_chat_fallback_transcribes_messages_without_heuristics() {
        let rendered =
            build_prompt(&[message("system", "hi"), message("user", "hello")], None, None)
                .expect("raw fallback prompt");

        assert_eq!(rendered, "System: hi\nUser: hello\nAssistant:");
    }

    #[test]
    fn minijinja_template_renders_hf_style_messages() {
        let template = "{% for message in messages %}[{{ message.role }}] {{ message.content }}\n{% endfor %}{% if add_generation_prompt %}[assistant] {% endif %}";
        let rendered = build_prompt(
            &[message("system", "hi"), message("user", "hello")],
            Some(template),
            None,
        )
        .expect("template prompt");

        assert_eq!(rendered, "[system] hi\n[user] hello\n[assistant] ");
    }

    #[test]
    fn assistant_prefill_disables_generation_prompt_for_templates() {
        let template = "{% for message in messages %}[{{ message.role }}] {{ message.content }}\n{% endfor %}{% if add_generation_prompt %}[assistant]{% endif %}";
        let rendered = build_prompt(
            &[message("user", "hello"), message("assistant", "partial")],
            Some(template),
            None,
        )
        .expect("template prompt");

        assert_eq!(rendered, "[user] hello\n[assistant] partial\n");
    }

    #[test]
    fn minijinja_template_normalizes_stored_reasoning_blocks_for_history() {
        let template = "{% for message in messages %}{% if message.role == 'assistant' %}{% if message.reasoning_content is defined and message.reasoning_content is string %}<think>{{ message.reasoning_content }}</think>{{ message.content }}{% else %}{{ message.content }}{% endif %}{% endif %}{% endfor %}";
        let rendered = build_prompt(
            &[message(
                "assistant",
                "<think status=\"done\">\n\nstep by step\n\n</think>\n\nfinal answer",
            )],
            Some(template),
            None,
        )
        .expect("normalized template prompt");

        assert_eq!(rendered, "<think>step by step</think>\n\nfinal answer");
    }

    #[test]
    fn minijinja_template_omits_enable_thinking_when_unspecified() {
        let rendered = build_prompt(
            &[message("user", "hello")],
            Some("{% if enable_thinking is defined %}defined{% else %}undefined{% endif %}"),
            None,
        )
        .expect("template prompt");

        assert_eq!(rendered, "undefined");
    }

    #[test]
    fn minijinja_template_sets_enable_thinking_when_reasoning_is_disabled() {
        let rendered = build_prompt(
            &[message("user", "hello")],
            Some(
                "{% if add_generation_prompt %}{% if enable_thinking is defined and not enable_thinking %}<think></think>{% else %}<think>{% endif %}{% endif %}",
            ),
            Some(ChatReasoningEffort::None),
        )
        .expect("template prompt");

        assert_eq!(rendered, "<think></think>");
    }

    #[test]
    fn qwen35_template_keeps_auto_reasoning_prompt_open() {
        let rendered = build_prompt(&[message("user", "hello")], Some(QWEN35_TEMPLATE), None)
            .expect("qwen3.5 prompt");

        assert!(rendered.ends_with("<|im_start|>assistant\n"));
        assert!(!rendered.ends_with("<think>\n"));
    }

    #[test]
    fn qwen35_template_does_not_prefill_open_think_when_reasoning_is_enabled() {
        let rendered = build_prompt(
            &[message("user", "hello")],
            Some(QWEN35_TEMPLATE),
            Some(ChatReasoningEffort::Medium),
        )
        .expect("qwen3.5 prompt");

        assert!(rendered.ends_with("<|im_start|>assistant\n"));
        assert!(!rendered.ends_with("<think>\n"));
    }

    #[test]
    fn qwen35_template_closes_empty_think_block_when_reasoning_is_disabled() {
        let rendered = build_prompt(
            &[message("user", "hello")],
            Some(QWEN35_TEMPLATE),
            Some(ChatReasoningEffort::None),
        )
        .expect("qwen3.5 prompt");

        assert!(rendered.ends_with("<|im_start|>assistant\n<think>\n\n</think>\n\n"));
    }

    #[test]
    fn minijinja_template_infers_chatml_eos_token() {
        let rendered =
            build_prompt(&[message("user", "hello")], Some("{{ eos_token }}<|im_end|>"), None)
                .expect("template prompt");

        assert_eq!(rendered, "<|im_end|><|im_end|>");
    }

    #[test]
    fn default_stop_sequences_detect_chatml_boundaries() {
        let stop = super::default_stop_sequences(Some("<|im_start|>assistant\n<|im_end|>\n"));
        let trailing = super::trailing_stop_markers(Some("<|im_start|>assistant\n<|im_end|>\n"));

        assert!(stop.contains(&"<|im_end|>".to_owned()));
        assert!(stop.contains(&"<|endoftext|><|im_start|>".to_owned()));
        assert!(trailing.contains(&"<|endoftext|>".to_owned()));
    }

    #[test]
    fn default_stop_sequences_detect_raw_chat_boundaries() {
        let stop = super::default_stop_sequences(None);

        assert!(stop.contains(&"\nUser:".to_owned()));
        assert!(stop.contains(&"\nAssistant:".to_owned()));
        assert!(stop.contains(&"\nSystem:".to_owned()));
        assert!(stop.contains(&"\nTool:".to_owned()));
    }
}
