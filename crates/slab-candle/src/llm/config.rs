use std::path::PathBuf;

use candle_transformers::generation::Sampling;
use serde::{Deserialize, Serialize};
use slab_types::RuntimeDevicePreference;

use super::error::CandleLlmError;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmWeightSource {
    #[default]
    QuantizedGguf,
    Safetensors,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmModelKind {
    #[default]
    Llama,
    Qwen2,
    Qwen2Moe,
    Qwen3,
    Qwen3Moe,
    Gemma,
    Gemma2,
    Gemma3,
    Glm4,
    Glm4New,
    DeepSeek2,
    Mamba,
    Mamba2,
    Phi,
}

impl std::fmt::Display for LlmModelKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Llama => "llama",
            Self::Qwen2 => "qwen2",
            Self::Qwen2Moe => "qwen2_moe",
            Self::Qwen3 => "qwen3",
            Self::Qwen3Moe => "qwen3_moe",
            Self::Gemma => "gemma",
            Self::Gemma2 => "gemma2",
            Self::Gemma3 => "gemma3",
            Self::Glm4 => "glm4",
            Self::Glm4New => "glm4_new",
            Self::DeepSeek2 => "deepseek2",
            Self::Mamba => "mamba",
            Self::Mamba2 => "mamba2",
            Self::Phi => "phi",
        })
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromptFormat {
    #[default]
    Raw,
    LlamaChat,
    MistralInstruct,
    Zephyr,
    OpenChat,
    DeepSeek,
    QwenChat,
    GemmaInstruct,
    PhiChat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SamplingConfig {
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub top_k: Option<usize>,
    #[serde(default = "default_repeat_penalty")]
    pub repeat_penalty: f32,
    #[serde(default = "default_repeat_last_n")]
    pub repeat_last_n: usize,
}

impl SamplingConfig {
    pub fn validate(&self) -> Result<(), CandleLlmError> {
        if let Some(temperature) = self.temperature
            && temperature < 0.0
        {
            return Err(CandleLlmError::InvalidSampling {
                message: "temperature must be >= 0".to_owned(),
            });
        }
        if let Some(top_p) = self.top_p
            && !(0.0..=1.0).contains(&top_p)
        {
            return Err(CandleLlmError::InvalidSampling {
                message: "top_p must be between 0 and 1".to_owned(),
            });
        }
        if let Some(top_k) = self.top_k
            && top_k == 0
        {
            return Err(CandleLlmError::InvalidSampling {
                message: "top_k must be greater than 0 when set".to_owned(),
            });
        }
        if self.repeat_penalty <= 0.0 {
            return Err(CandleLlmError::InvalidSampling {
                message: "repeat_penalty must be greater than 0".to_owned(),
            });
        }
        Ok(())
    }

    pub(crate) fn sampling(&self) -> Sampling {
        let temperature = self.temperature.unwrap_or(0.0);
        if temperature < 1e-7 {
            return Sampling::ArgMax;
        }
        match (self.top_k, self.top_p) {
            (None, None) => Sampling::All { temperature },
            (Some(k), None) => Sampling::TopK { k, temperature },
            (None, Some(p)) => Sampling::TopP { p, temperature },
            (Some(k), Some(p)) => Sampling::TopKThenTopP { k, p, temperature },
        }
    }
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            temperature: None,
            top_p: None,
            top_k: None,
            repeat_penalty: default_repeat_penalty(),
            repeat_last_n: default_repeat_last_n(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandleLlmLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<RuntimeDevicePreference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<PathBuf>,
    #[serde(default)]
    pub extra_weight_paths: Vec<PathBuf>,
    #[serde(default)]
    pub model_kind: LlmModelKind,
    #[serde(default)]
    pub weight_source: LlmWeightSource,
    #[serde(default)]
    pub prompt_format: PromptFormat,
    #[serde(default)]
    pub seed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextGenerationRequest {
    pub prompt: String,
    pub max_tokens: usize,
    #[serde(default)]
    pub sampling: SamplingConfig,
    #[serde(default)]
    pub stop_sequences: Vec<String>,
    #[serde(default)]
    pub ignore_eos: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextGenerationUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextGenerationResponse {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TextGenerationUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TextGenerationStreamChunk {
    Token(String),
    Done(TextGenerationResponse),
}

fn default_repeat_penalty() -> f32 {
    1.1
}

fn default_repeat_last_n() -> usize {
    64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sampling_is_argmax() {
        assert!(matches!(SamplingConfig::default().sampling(), Sampling::ArgMax));
    }

    #[test]
    fn invalid_top_p_is_rejected() {
        let config = SamplingConfig { top_p: Some(1.5), ..SamplingConfig::default() };
        assert!(matches!(config.validate(), Err(CandleLlmError::InvalidSampling { .. })));
    }
}
