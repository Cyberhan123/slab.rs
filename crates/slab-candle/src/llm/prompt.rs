use super::config::{LlmModelKind, PromptFormat};

pub(crate) fn apply_prompt_format(
    prompt: &str,
    requested: PromptFormat,
    kind: LlmModelKind,
) -> String {
    let format =
        if requested == PromptFormat::Raw { default_prompt_format(kind) } else { requested };

    match format {
        PromptFormat::Raw => prompt.to_owned(),
        PromptFormat::LlamaChat => format!("[INST] {prompt} [/INST]"),
        PromptFormat::MistralInstruct => format!("[INST] {prompt} [/INST]"),
        PromptFormat::Zephyr => {
            format!("<|user|>\n{prompt}</s>\n<|assistant|>\n")
        }
        PromptFormat::OpenChat => {
            format!("GPT4 Correct User: {prompt}<|end_of_turn|>GPT4 Correct Assistant:")
        }
        PromptFormat::DeepSeek => format!("<｜User｜>{prompt}<｜Assistant｜>"),
        PromptFormat::QwenChat => {
            format!("<|im_start|>user\n{prompt}<|im_end|>\n<|im_start|>assistant\n")
        }
        PromptFormat::GemmaInstruct => {
            format!("<start_of_turn>user\n{prompt}<end_of_turn>\n<start_of_turn>model\n")
        }
        PromptFormat::PhiChat => format!("<|user|>\n{prompt}<|end|>\n<|assistant|>\n"),
    }
}

pub(crate) fn default_prompt_format(kind: LlmModelKind) -> PromptFormat {
    match kind {
        LlmModelKind::Qwen2
        | LlmModelKind::Qwen2Moe
        | LlmModelKind::Qwen3
        | LlmModelKind::Qwen3Moe => PromptFormat::QwenChat,
        LlmModelKind::Gemma | LlmModelKind::Gemma2 | LlmModelKind::Gemma3 => {
            PromptFormat::GemmaInstruct
        }
        LlmModelKind::DeepSeek2 => PromptFormat::DeepSeek,
        LlmModelKind::Phi => PromptFormat::PhiChat,
        LlmModelKind::Llama
        | LlmModelKind::Glm4
        | LlmModelKind::Glm4New
        | LlmModelKind::Mamba
        | LlmModelKind::Mamba2 => PromptFormat::Raw,
    }
}

pub(crate) fn eos_candidates(kind: LlmModelKind) -> &'static [&'static str] {
    match kind {
        LlmModelKind::Qwen2
        | LlmModelKind::Qwen2Moe
        | LlmModelKind::Qwen3
        | LlmModelKind::Qwen3Moe => &["<|im_end|>", "<|endoftext|>", "</s>"],
        LlmModelKind::Gemma | LlmModelKind::Gemma2 | LlmModelKind::Gemma3 => {
            &["<end_of_turn>", "<eos>", "</s>"]
        }
        LlmModelKind::DeepSeek2 => &["<｜end▁of▁sentence｜>", "<｜EOT｜>", "</s>"],
        LlmModelKind::Phi => &["<|end|>", "<|endoftext|>", "</s>"],
        LlmModelKind::Glm4 | LlmModelKind::Glm4New => &["<|endoftext|>", "<|user|>", "</s>"],
        LlmModelKind::Llama | LlmModelKind::Mamba | LlmModelKind::Mamba2 => {
            &["</s>", "<|end_of_text|>", "<|endoftext|>"]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qwen_prompt_gets_chat_markers() {
        let prompt = apply_prompt_format("hello", PromptFormat::Raw, LlmModelKind::Qwen3);
        assert!(prompt.contains("<|im_start|>user"));
        assert!(prompt.ends_with("<|im_start|>assistant\n"));
    }

    #[test]
    fn deepseek_eos_prefers_family_token() {
        assert_eq!(eos_candidates(LlmModelKind::DeepSeek2).first(), Some(&"<｜end▁of▁sentence｜>"));
    }
}
