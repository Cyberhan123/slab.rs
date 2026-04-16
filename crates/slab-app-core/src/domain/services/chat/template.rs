use std::time::{SystemTime, UNIX_EPOCH};

use minijinja::{Environment, Error, ErrorKind, UndefinedBehavior, context};
use serde_json::Value;

use crate::domain::models::ConversationMessage as DomainConversationMessage;
use crate::error::AppCoreError;

pub(super) fn build_prompt(
    messages: &[DomainConversationMessage],
    chat_template_source: Option<&str>,
) -> Result<String, AppCoreError> {
    match chat_template_source.map(str::trim).filter(|value| !value.is_empty()) {
        Some(source) => render_minijinja_template(source, messages),
        None => Ok(render_raw_chat(messages)),
    }
}

fn render_minijinja_template(
    source: &str,
    messages: &[DomainConversationMessage],
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
    let template = env.get_template("chat_template").map_err(|error| {
        AppCoreError::BadRequest(format!("configured chat_template failed to load: {error}"))
    })?;

    template
        .render(context! {
            messages => messages,
            add_generation_prompt => !has_assistant_prefill,
            continue_final_message => has_assistant_prefill,
            bos_token => "",
            eos_token => "",
            unk_token => "",
            pad_token => "",
            tools => Vec::<Value>::new(),
            documents => Vec::<Value>::new(),
        })
        .map_err(|error| {
            AppCoreError::BadRequest(format!("configured chat_template failed to render: {error}"))
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
        ConversationMessage as DomainConversationMessage, ConversationMessageContent,
    };

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
        let rendered = build_prompt(&[message("system", "hi"), message("user", "hello")], None)
            .expect("raw fallback prompt");

        assert_eq!(rendered, "System: hi\nUser: hello\nAssistant:");
    }

    #[test]
    fn minijinja_template_renders_hf_style_messages() {
        let template = "{% for message in messages %}[{{ message.role }}] {{ message.content }}\n{% endfor %}{% if add_generation_prompt %}[assistant] {% endif %}";
        let rendered =
            build_prompt(&[message("system", "hi"), message("user", "hello")], Some(template))
                .expect("template prompt");

        assert_eq!(rendered, "[system] hi\n[user] hello\n[assistant] ");
    }

    #[test]
    fn assistant_prefill_disables_generation_prompt_for_templates() {
        let template = "{% for message in messages %}[{{ message.role }}] {{ message.content }}\n{% endfor %}{% if add_generation_prompt %}[assistant]{% endif %}";
        let rendered = build_prompt(
            &[message("user", "hello"), message("assistant", "partial")],
            Some(template),
        )
        .expect("template prompt");

        assert_eq!(rendered, "[user] hello\n[assistant] partial\n");
    }
}
