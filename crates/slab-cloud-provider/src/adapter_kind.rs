//! Mapping from slab's [`ProviderFamily`] to genai's [`AdapterKind`].

use genai::adapter::AdapterKind;
use slab_config::ProviderFamily;

/// Map a configured provider family to the genai adapter that drives its native protocol.
///
/// `OpenaiCompatible` (the "Other / custom" family) and the OpenAI Chat Completions family both
/// map to [`AdapterKind::OpenAI`], which is genai's OpenAI-compatible adapter. Every other variant
/// maps 1:1 to the matching genai adapter.
///
/// Keep this in sync with [`ProviderFamily`] and genai's `AdapterKind`. The compiler enforces
/// exhaustiveness when either enum gains a variant.
pub fn family_to_adapter_kind(family: ProviderFamily) -> AdapterKind {
    match family {
        // OpenAI-compatible endpoints (custom or first-party) all use the Chat Completions adapter.
        ProviderFamily::OpenaiCompatible | ProviderFamily::Openai => AdapterKind::OpenAI,
        ProviderFamily::OpenaiResp => AdapterKind::OpenAIResp,
        ProviderFamily::Gemini => AdapterKind::Gemini,
        ProviderFamily::Anthropic => AdapterKind::Anthropic,
        ProviderFamily::Fireworks => AdapterKind::Fireworks,
        ProviderFamily::Together => AdapterKind::Together,
        ProviderFamily::Groq => AdapterKind::Groq,
        ProviderFamily::Aihubmix => AdapterKind::Aihubmix,
        ProviderFamily::Mimo => AdapterKind::Mimo,
        ProviderFamily::Moonshot => AdapterKind::Moonshot,
        ProviderFamily::Nebius => AdapterKind::Nebius,
        ProviderFamily::Xai => AdapterKind::Xai,
        ProviderFamily::DeepSeek => AdapterKind::DeepSeek,
        ProviderFamily::Zai => AdapterKind::Zai,
        ProviderFamily::BigModel => AdapterKind::BigModel,
        ProviderFamily::Aliyun => AdapterKind::Aliyun,
        ProviderFamily::Baidu => AdapterKind::Baidu,
        ProviderFamily::Cohere => AdapterKind::Cohere,
        ProviderFamily::Ollama => AdapterKind::Ollama,
        ProviderFamily::OllamaCloud => AdapterKind::OllamaCloud,
        ProviderFamily::Vertex => AdapterKind::Vertex,
        ProviderFamily::GithubCopilot => AdapterKind::GithubCopilot,
        ProviderFamily::OpenCodeGo => AdapterKind::OpenCodeGo,
        ProviderFamily::BedrockApi => AdapterKind::BedrockApi,
        ProviderFamily::OpenRouter => AdapterKind::OpenRouter,
        ProviderFamily::MiniMax => AdapterKind::MiniMax,
    }
}
