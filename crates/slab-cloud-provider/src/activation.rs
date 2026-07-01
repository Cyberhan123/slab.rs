//! Default cloud-model catalog per provider family.
//!
//! genai 0.6.5's `Client::all_model_names` performs a live `/models` web call (and several
//! vendors — notably Anthropic and Gemini — have no such endpoint), so it cannot be used as a
//! reliable, offline source at settings-apply time. Instead we ship a curated flagship catalog
//! per `ProviderFamily` so that configuring a provider immediately yields usable cloud models.
//! The catalog is plain data; `slab-app-core` owns the DB upsert.

use slab_config::{CloudProviderConfig, ProviderFamily};

/// A cloud model that should be activated for a configured provider.
///
/// Pure data — `slab-app-core` performs the actual DB upsert. Keeping this free of any
/// `slab-app-core` dependency avoids a cyclic crate graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudModelSpec {
    /// The model id used when calling the provider (e.g. `gpt-4o`, `claude-sonnet-4-5`).
    pub remote_model_id: String,
    /// Label shown in the model catalog UI.
    pub display_name: String,
}

/// Returns the curated default model list for `provider`'s family.
///
/// Empty for `OpenaiCompatible` (custom endpoints have no known model list) and for families
/// without a curated catalog — the user can still add models manually via `/v1/models`.
pub fn default_models_for_provider(provider: &CloudProviderConfig) -> Vec<CloudModelSpec> {
    catalog_for_family(provider.family)
        .iter()
        .map(|(remote_model_id, display_name)| CloudModelSpec {
            remote_model_id: (*remote_model_id).to_owned(),
            display_name: (*display_name).to_owned(),
        })
        .collect()
}

/// Curated `(remote_model_id, display_name)` pairs per family.
///
/// Model ids follow each vendor's native naming (genai-valid). Refresh this table when vendors
/// ship new flagship models; it intentionally stays a small, well-known set rather than an
/// exhaustive (and fast-stale) dump.
fn catalog_for_family(family: ProviderFamily) -> &'static [(&'static str, &'static str)] {
    match family {
        ProviderFamily::Openai => &[
            ("gpt-4.1", "GPT-4.1"),
            ("gpt-4.1-mini", "GPT-4.1 mini"),
            ("gpt-4.1-nano", "GPT-4.1 nano"),
            ("gpt-4o", "GPT-4o"),
            ("gpt-4o-mini", "GPT-4o mini"),
        ],
        ProviderFamily::OpenaiResp => &[
            ("gpt-4.1", "GPT-4.1"),
            ("gpt-4.1-mini", "GPT-4.1 mini"),
            ("gpt-4o", "GPT-4o"),
            ("o3", "o3"),
            ("o4-mini", "o4-mini"),
        ],
        ProviderFamily::Anthropic => &[
            ("claude-sonnet-4-5", "Claude Sonnet 4.5"),
            ("claude-opus-4-1", "Claude Opus 4.1"),
            ("claude-haiku-4-5", "Claude Haiku 4.5"),
            ("claude-3-7-sonnet", "Claude 3.7 Sonnet"),
            ("claude-3-5-sonnet", "Claude 3.5 Sonnet"),
            ("claude-3-5-haiku", "Claude 3.5 Haiku"),
        ],
        ProviderFamily::Gemini => &[
            ("gemini-2.5-pro", "Gemini 2.5 Pro"),
            ("gemini-2.5-flash", "Gemini 2.5 Flash"),
            ("gemini-2.0-flash", "Gemini 2.0 Flash"),
            ("gemini-2.0-flash-lite", "Gemini 2.0 Flash Lite"),
        ],
        ProviderFamily::Groq => &[
            ("llama-3.3-70b-versatile", "Llama 3.3 70B Versatile"),
            ("llama-3.1-8b-instant", "Llama 3.1 8B Instant"),
        ],
        ProviderFamily::DeepSeek => {
            &[("deepseek-chat", "DeepSeek Chat"), ("deepseek-reasoner", "DeepSeek Reasoner")]
        }
        ProviderFamily::Cohere => &[
            ("command-r-plus", "Command R+"),
            ("command-r", "Command R"),
            ("command-r7b", "Command R7B"),
        ],
        ProviderFamily::Xai => {
            &[("grok-4", "Grok 4"), ("grok-3", "Grok 3"), ("grok-3-mini", "Grok 3 mini")]
        }
        ProviderFamily::Moonshot => &[
            ("kimi-k2", "Kimi K2"),
            ("moonshot-v1-128k", "Moonshot v1 128K"),
            ("moonshot-v1-32k", "Moonshot v1 32K"),
        ],
        ProviderFamily::Zai => &[
            ("glm-4.6", "GLM-4.6"),
            ("glm-4.5", "GLM-4.5"),
            ("glm-4.5-air", "GLM-4.5 Air"),
            ("glm-4-flash", "GLM-4 Flash"),
        ],
        ProviderFamily::Aliyun => {
            &[("qwen-max", "Qwen Max"), ("qwen-plus", "Qwen Plus"), ("qwen-turbo", "Qwen Turbo")]
        }
        ProviderFamily::Baidu => &[
            ("ernie-4.0-8k-latest", "ERNIE 4.0"),
            ("ernie-3.5-8k", "ERNIE 3.5"),
            ("ernie-speed-128k", "ERNIE Speed 128K"),
        ],
        ProviderFamily::OpenRouter => &[
            ("openai/gpt-4o", "OpenAI · GPT-4o"),
            ("anthropic/claude-3.5-sonnet", "Anthropic · Claude 3.5 Sonnet"),
            ("google/gemini-2.0-flash", "Google · Gemini 2.0 Flash"),
        ],
        ProviderFamily::Together => &[
            ("meta-llama/Llama-3.3-70B-Instruct-Turbo", "Llama 3.3 70B Turbo"),
            ("meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo", "Llama 3.1 8B Turbo"),
        ],
        ProviderFamily::Fireworks => &[
            ("accounts/fireworks/models/llama-v3p1-70b-instruct", "Llama 3.1 70B"),
            ("accounts/fireworks/models/llama-v3p1-8b-instruct", "Llama 3.1 8B"),
        ],
        ProviderFamily::MiniMax => &[("MiniMax-M1", "MiniMax M1"), ("abab6.5s-chat", "ABAB 6.5s")],
        // No curated catalog: the user adds models manually via /v1/models.
        ProviderFamily::OpenaiCompatible
        | ProviderFamily::Aihubmix
        | ProviderFamily::Mimo
        | ProviderFamily::Nebius
        | ProviderFamily::BigModel
        | ProviderFamily::Ollama
        | ProviderFamily::OllamaCloud
        | ProviderFamily::Vertex
        | ProviderFamily::GithubCopilot
        | ProviderFamily::OpenCodeGo
        | ProviderFamily::BedrockApi => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_config::CloudProviderConfig;

    fn provider(id: &str, family: ProviderFamily) -> CloudProviderConfig {
        CloudProviderConfig {
            id: id.to_owned(),
            family,
            name: id.to_owned(),
            api_base: String::new(),
            api_key: None,
            api_key_env: None,
        }
    }

    #[test]
    fn openai_compatible_has_no_default_models() {
        let specs =
            default_models_for_provider(&provider("custom", ProviderFamily::OpenaiCompatible));
        assert!(specs.is_empty(), "custom OpenAI-compatible endpoints have no known model list");
    }

    #[test]
    fn anthropic_catalog_lists_claude_models() {
        let specs = default_models_for_provider(&provider("anthropic", ProviderFamily::Anthropic));
        assert!(!specs.is_empty(), "Anthropic should expose a curated Claude catalog");
        assert!(specs.iter().any(|spec| spec.remote_model_id.contains("claude")));
        assert!(specs.iter().all(|spec| !spec.display_name.is_empty()));
    }

    #[test]
    fn every_family_has_a_stable_catalog() {
        // Exhaustiveness guard: every family must resolve without panicking.
        for &value in ProviderFamily::all_str() {
            let family = serde_json::from_value::<ProviderFamily>(serde_json::Value::String(
                value.to_owned(),
            ))
            .expect("ProviderFamily::all_str values must deserialize");
            let _ = catalog_for_family(family);
        }
    }
}
