use crate::domain::models::ConversationMessage as DomainConversationMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChatPromptTemplate {
    Simple,
    ChatMl,
    Llama3,
    CommandR,
}

impl ChatPromptTemplate {
    fn from_name(value: Option<&str>) -> Self {
        match value.unwrap_or("simple").trim().to_ascii_lowercase().as_str() {
            "chatml" | "qwen" | "qwen3" | "deepseek" | "deepseek-r1" => Self::ChatMl,
            "llama3" | "llama-3" | "llama_3" => Self::Llama3,
            "command-r" | "command_r" | "commandr" => Self::CommandR,
            _ => Self::Simple,
        }
    }

    fn render(self, messages: &[DomainConversationMessage]) -> String {
        let (history, assistant_prefill) = split_prefill(messages);
        match self {
            Self::Simple => render_simple(history, assistant_prefill),
            Self::ChatMl => render_chatml(history, assistant_prefill),
            Self::Llama3 => render_llama3(history, assistant_prefill),
            Self::CommandR => render_command_r(history, assistant_prefill),
        }
    }
}

pub(super) fn build_prompt(
    messages: &[DomainConversationMessage],
    template_name: Option<&str>,
) -> String {
    ChatPromptTemplate::from_name(template_name).render(messages)
}

fn split_prefill(
    messages: &[DomainConversationMessage],
) -> (&[DomainConversationMessage], Option<&str>) {
    match messages.last() {
        Some(message) if message.role == "assistant" => {
            (&messages[..messages.len().saturating_sub(1)], Some(message.content.as_str()))
        }
        _ => (messages, None),
    }
}

fn render_simple(
    messages: &[DomainConversationMessage],
    assistant_prefill: Option<&str>,
) -> String {
    let mut parts: Vec<String> = messages
        .iter()
        .map(|message| format!("{}: {}", capitalize_role(&message.role), message.content))
        .collect();
    let mut assistant = String::from("Assistant:");
    if let Some(prefill) = assistant_prefill {
        if !prefill.is_empty() {
            assistant.push(' ');
            assistant.push_str(prefill);
        }
    }
    parts.push(assistant);
    parts.join("\n")
}

fn render_chatml(
    messages: &[DomainConversationMessage],
    assistant_prefill: Option<&str>,
) -> String {
    let mut prompt = String::new();
    for message in messages {
        prompt.push_str("<|im_start|>");
        prompt.push_str(chatml_role(&message.role));
        prompt.push('\n');
        prompt.push_str(&message.content);
        prompt.push_str("<|im_end|>\n");
    }
    prompt.push_str("<|im_start|>assistant\n");
    if let Some(prefill) = assistant_prefill {
        prompt.push_str(prefill);
    }
    prompt
}

fn render_llama3(
    messages: &[DomainConversationMessage],
    assistant_prefill: Option<&str>,
) -> String {
    let mut prompt = String::from("<|begin_of_text|>");
    for message in messages {
        prompt.push_str("<|start_header_id|>");
        prompt.push_str(llama3_role(&message.role));
        prompt.push_str("<|end_header_id|>\n\n");
        prompt.push_str(&message.content);
        prompt.push_str("<|eot_id|>");
    }
    prompt.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");
    if let Some(prefill) = assistant_prefill {
        prompt.push_str(prefill);
    }
    prompt
}

fn render_command_r(
    messages: &[DomainConversationMessage],
    assistant_prefill: Option<&str>,
) -> String {
    let mut prompt = String::new();
    for message in messages {
        prompt.push_str("<|START_OF_TURN_TOKEN|>");
        prompt.push_str(command_r_role(&message.role));
        prompt.push_str(&message.content);
        prompt.push_str("<|END_OF_TURN_TOKEN|>");
    }
    prompt.push_str("<|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>");
    if let Some(prefill) = assistant_prefill {
        prompt.push_str(prefill);
    }
    prompt
}

fn capitalize_role(role: &str) -> &str {
    match role {
        "user" => "User",
        "assistant" => "Assistant",
        "system" => "System",
        other => other,
    }
}

fn chatml_role(role: &str) -> &str {
    match role {
        "system" => "system",
        "assistant" => "assistant",
        _ => "user",
    }
}

fn llama3_role(role: &str) -> &str {
    match role {
        "system" => "system",
        "assistant" => "assistant",
        _ => "user",
    }
}

fn command_r_role(role: &str) -> &str {
    match role {
        "system" => "<|SYSTEM_TOKEN|>",
        "assistant" => "<|CHATBOT_TOKEN|>",
        _ => "<|USER_TOKEN|>",
    }
}
