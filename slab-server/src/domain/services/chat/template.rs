use crate::domain::models::{ConversationMessage as DomainConversationMessage, UnifiedModel};

#[derive(Debug, Clone, Default)]
pub(super) struct PromptTemplateContext {
    explicit_template: Option<String>,
    model_id: Option<String>,
    display_name: Option<String>,
    repo_id: Option<String>,
    filename: Option<String>,
    local_path: Option<String>,
}

impl PromptTemplateContext {
    pub(super) fn from_model(model: &UnifiedModel) -> Self {
        Self {
            explicit_template: model.spec.chat_template.clone(),
            model_id: Some(model.id.clone()),
            display_name: Some(model.display_name.clone()),
            repo_id: model.spec.repo_id.clone(),
            filename: model.spec.filename.clone(),
            local_path: model.spec.local_path.clone(),
        }
    }

    fn hint_values(&self) -> impl Iterator<Item = &str> {
        [
            self.model_id.as_deref(),
            self.display_name.as_deref(),
            self.repo_id.as_deref(),
            self.filename.as_deref(),
            self.local_path.as_deref(),
        ]
        .into_iter()
        .flatten()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChatPromptTemplate {
    Simple,
    ChatMl,
    Llama3,
    CommandR,
}

impl ChatPromptTemplate {
    fn from_name(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "chatml" | "qwen" | "qwen3" | "deepseek" | "deepseek-r1" => Self::ChatMl,
            "llama3" | "llama-3" | "llama_3" | "meta-llama-3" => Self::Llama3,
            "command-r" | "command_r" | "commandr" | "cohere" => Self::CommandR,
            _ => Self::Simple,
        }
    }

    fn resolve(context: Option<&PromptTemplateContext>) -> Self {
        if let Some(template_name) = context.and_then(|ctx| ctx.explicit_template.as_deref()) {
            return Self::from_name(template_name);
        }

        let combined_hints = context
            .map(|ctx| {
                ctx.hint_values().map(str::to_ascii_lowercase).collect::<Vec<_>>().join("\n")
            })
            .unwrap_or_default();

        if combined_hints.contains("command-r")
            || combined_hints.contains("commandr")
            || combined_hints.contains("cohere")
        {
            Self::CommandR
        } else if combined_hints.contains("llama3")
            || combined_hints.contains("llama-3")
            || combined_hints.contains("llama_3")
            || combined_hints.contains("meta-llama-3")
        {
            Self::Llama3
        } else if combined_hints.contains("chatml")
            || combined_hints.contains("qwen")
            || combined_hints.contains("deepseek")
        {
            Self::ChatMl
        } else {
            Self::Simple
        }
    }

    fn render(self, messages: &[DomainConversationMessage]) -> String {
        let (history, assistant_prefill) = split_prefill(messages);
        match self {
            Self::Simple => render_simple(history, assistant_prefill.as_deref()),
            Self::ChatMl => render_chatml(history, assistant_prefill.as_deref()),
            Self::Llama3 => render_llama3(history, assistant_prefill.as_deref()),
            Self::CommandR => render_command_r(history, assistant_prefill.as_deref()),
        }
    }
}

pub(super) fn build_prompt(
    messages: &[DomainConversationMessage],
    context: Option<&PromptTemplateContext>,
) -> String {
    ChatPromptTemplate::resolve(context).render(messages)
}

fn split_prefill(
    messages: &[DomainConversationMessage],
) -> (&[DomainConversationMessage], Option<String>) {
    match messages.last() {
        Some(message) if message.role == "assistant" => {
            (&messages[..messages.len().saturating_sub(1)], Some(message.rendered_text()))
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
        .map(|message| format!("{}: {}", display_role(&message.role), message.rendered_text()))
        .collect();
    let mut assistant = String::from("Assistant:");
    if let Some(prefill) = assistant_prefill
        && !prefill.is_empty()
    {
        assistant.push(' ');
        assistant.push_str(prefill);
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
        prompt.push_str(template_role(&message.role));
        prompt.push('\n');
        prompt.push_str(&message.rendered_text());
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
        prompt.push_str(template_role(&message.role));
        prompt.push_str("<|end_header_id|>\n\n");
        prompt.push_str(&message.rendered_text());
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
        prompt.push_str(&message.rendered_text());
        prompt.push_str("<|END_OF_TURN_TOKEN|>");
    }
    prompt.push_str("<|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>");
    if let Some(prefill) = assistant_prefill {
        prompt.push_str(prefill);
    }
    prompt
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

fn template_role(role: &str) -> &str {
    match role {
        "system" | "developer" => "system",
        "assistant" => "assistant",
        _ => "user",
    }
}

fn command_r_role(role: &str) -> &str {
    match template_role(role) {
        "system" => "<|SYSTEM_TOKEN|>",
        "assistant" => "<|CHATBOT_TOKEN|>",
        _ => "<|USER_TOKEN|>",
    }
}
