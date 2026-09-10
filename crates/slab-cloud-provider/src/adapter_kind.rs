//! Mapping from slab's [`ProviderFamily`] to genai's [`AdapterKind`].

use genai::adapter::AdapterKind;
use slab_config::{ApiStyle, ProviderFamily};

/// Map a configured provider family to the genai adapter that drives its native protocol.
///
/// NOTE: this family-only mapping is the Chat-Completions default for OpenAI-lineage families and
/// remains the correct choice for family-driven paths that are style-agnostic (model listing:
/// genai's `OpenAIResp` adapter delegates `all_model_names` to the OpenAI `GET /models` call).
/// Chat transport routing must go through [`resolve_adapter_kind`], which honors the provider's
/// [`ApiStyle`].
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_styles_win_within_the_openai_lineage() {
        for family in
            [ProviderFamily::Openai, ProviderFamily::OpenaiResp, ProviderFamily::OpenaiCompatible]
        {
            assert_eq!(
                resolve_adapter_kind(family, ApiStyle::Responses),
                AdapterKindResolution::Kind(AdapterKind::OpenAIResp)
            );
            assert_eq!(
                resolve_adapter_kind(family, ApiStyle::ChatCompletions),
                AdapterKindResolution::Kind(AdapterKind::OpenAI)
            );
        }
    }

    #[test]
    fn auto_routes_first_party_openai_to_responses_and_custom_to_probe() {
        // First-party OpenAI always supports /responses — no probe needed.
        assert_eq!(
            resolve_adapter_kind(ProviderFamily::Openai, ApiStyle::Auto),
            AdapterKindResolution::Kind(AdapterKind::OpenAIResp)
        );
        assert_eq!(
            resolve_adapter_kind(ProviderFamily::OpenaiResp, ApiStyle::Auto),
            AdapterKindResolution::Kind(AdapterKind::OpenAIResp)
        );
        // Custom endpoints are probed at first use.
        assert_eq!(
            resolve_adapter_kind(ProviderFamily::OpenaiCompatible, ApiStyle::Auto),
            AdapterKindResolution::ProbeEndpoint
        );
    }

    #[test]
    fn non_openai_families_ignore_api_style() {
        // The style knob is OpenAI-lineage only; native protocol wins even
        // when an explicit (mismatched) style is configured.
        for style in [ApiStyle::Auto, ApiStyle::Responses, ApiStyle::ChatCompletions] {
            assert_eq!(
                resolve_adapter_kind(ProviderFamily::Gemini, style),
                AdapterKindResolution::Kind(AdapterKind::Gemini)
            );
            assert_eq!(
                resolve_adapter_kind(ProviderFamily::Anthropic, style),
                AdapterKindResolution::Kind(AdapterKind::Anthropic)
            );
            assert_eq!(
                resolve_adapter_kind(ProviderFamily::DeepSeek, style),
                AdapterKindResolution::Kind(AdapterKind::DeepSeek)
            );
        }
    }
}

/// Chat-transport routing decision for a provider, combining family and [`ApiStyle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKindResolution {
    /// Route chat traffic through this adapter.
    Kind(AdapterKind),
    /// `auto` style on a custom OpenAI-compatible endpoint: the caller must probe the endpoint
    /// for `POST {api_base}/responses` support (see [`crate::responses_probe`]) and route to
    /// [`AdapterKind::OpenAIResp`] when supported, falling back to [`AdapterKind::OpenAI`]
    /// (Chat Completions) otherwise. The fallback must be silent — third-party support for the
    /// Responses API is uneven, and chat/completions is the universal denominator.
    ProbeEndpoint,
}

/// Resolve the chat adapter for a provider from its family plus API-style preference.
///
/// - Explicit `responses` / `chat_completions` wins within the OpenAI lineage
///   (`openai`, `openai_resp`, `openai_compatible`) and is ignored elsewhere.
/// - `auto` (the default): first-party `openai` and `openai_resp` route straight to the Responses
///   API (the official endpoint always supports it); custom `openai_compatible` endpoints return
///   [`AdapterKindResolution::ProbeEndpoint`]; every other family maps 1:1.
pub fn resolve_adapter_kind(family: ProviderFamily, api_style: ApiStyle) -> AdapterKindResolution {
    let openai_lineage = matches!(
        family,
        ProviderFamily::Openai | ProviderFamily::OpenaiResp | ProviderFamily::OpenaiCompatible
    );
    match (api_style, openai_lineage) {
        (ApiStyle::Responses, true) => AdapterKindResolution::Kind(AdapterKind::OpenAIResp),
        (ApiStyle::ChatCompletions, true) => AdapterKindResolution::Kind(AdapterKind::OpenAI),
        // Explicit styles on non-OpenAI families are ignored: the family's native protocol wins.
        _ => match family {
            ProviderFamily::Openai | ProviderFamily::OpenaiResp => {
                AdapterKindResolution::Kind(AdapterKind::OpenAIResp)
            }
            ProviderFamily::OpenaiCompatible => AdapterKindResolution::ProbeEndpoint,
            other => AdapterKindResolution::Kind(family_to_adapter_kind(other)),
        },
    }
}
