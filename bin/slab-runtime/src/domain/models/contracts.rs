use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GgmlLlamaLoadConfig {
    pub model_path: PathBuf,
    pub engine_workers: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
    /// Free GPU VRAM (bytes) snapshot; used to size an `auto` context
    /// (`context_length == None`). `None` when no VRAM signal is available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_vram_bytes: Option<u64>,
    #[serde(default)]
    pub flash_attn: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gbnf: Option<String>,
    /// Optional multimodal vision/audio projector (mmproj) GGUF path. When set,
    /// the engine loads an mtmd context bound to the text model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mmproj_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GgmlLlamaLoadMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training_context_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<String>,
}

/// Typed quantize request payload that flows through the driver to the engine.
/// `ftype` is the raw `llama_ftype` int (e.g. 15 = Q4_K_M, 36 = TQ1_0).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GgmlLlamaQuantizeInput {
    pub input_path: String,
    pub output_path: String,
    #[serde(default)]
    pub ftype: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nthread: Option<i32>,
    #[serde(default)]
    pub allow_requantize: bool,
    #[serde(default)]
    pub quantize_output_tensor: bool,
    #[serde(default)]
    pub only_copy: bool,
    #[serde(default)]
    pub pure: bool,
    #[serde(default)]
    pub keep_split: bool,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GgmlLlamaQuantizeOutput {
    #[serde(default)]
    pub layers_processed: u32,
    #[serde(default)]
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GgmlWhisperLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash_attn: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GgmlParakeetLoadConfig {
    pub model_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GgmlDiffusionLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffusion_model_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taesd_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_l_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_g_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t5xxl_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_vision_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_net_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash_attn: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offload_params_to_cpu: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_mmap: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_threads: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct TextPromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct TextStopMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_id: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_kind: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct TextGenerationMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<TextStopMetadata>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl TextGenerationMetadata {
    #[cfg_attr(not(feature = "ggml"), allow(dead_code))]
    pub(crate) fn is_empty(&self) -> bool {
        self.reasoning_content.is_none() && self.stop.is_none() && self.extra.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct TextGenerationUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
    #[serde(default)]
    pub prompt_tokens_details: TextPromptTokensDetails,
    #[serde(default)]
    pub estimated: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct TextGenerationOptions {
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub top_k: Option<i32>,
    #[serde(default)]
    pub min_p: Option<f32>,
    #[serde(default)]
    pub presence_penalty: Option<f32>,
    #[serde(default)]
    pub repetition_penalty: Option<f32>,
    #[serde(default)]
    pub session_key: Option<String>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub gbnf: Option<String>,
    #[serde(default)]
    pub ignore_eos: bool,
    #[serde(default)]
    pub logit_bias: Option<serde_json::Value>,
    #[serde(default)]
    pub stop_sequences: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_trace: Option<slab_agent_tracing::AgentTraceContext>,
    /// Encoded image bytes for a multimodal (mtmd) turn. Empty for text-only.
    #[serde(default)]
    pub image_parts: Vec<TextGenerationImagePart>,
}

/// An image input accompanying a multimodal text-generation turn.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct TextGenerationImagePart {
    #[serde(default)]
    pub data: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct TextGenerationResponse {
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TextGenerationUsage>,
    #[serde(default)]
    pub metadata: TextGenerationMetadata,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct TextGenerationStreamEvent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TextGenerationUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<TextGenerationMetadata>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct AudioTranscriptionVadParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_speech_duration_ms: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_silence_duration_ms: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_speech_duration_s: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speech_pad_ms: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub samples_overlap: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct AudioTranscriptionVadOptions {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<AudioTranscriptionVadParams>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct AudioTranscriptionDecodeOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_ms: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_context: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_timestamps: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_timestamps: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split_on_word: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_nst: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub word_thold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_len: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_inc: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entropy_thold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprob_thold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_speech_thold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tdrz_enable: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct AudioTranscriptionOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect_language: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vad: Option<AudioTranscriptionVadOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decode: Option<AudioTranscriptionDecodeOptions>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct AudioTranscriptionResponse {
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GeneratedImage {
    #[serde(default)]
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub channels: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct ImageGenerationRequest {
    #[serde(default)]
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_skip: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init_image: Option<GeneratedImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_steps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guidance_scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distilled_guidance: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(default)]
    pub batch_count: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ImageGenerationResponse {
    #[serde(default)]
    pub images: Vec<GeneratedImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OnnxTensor {
    pub name: String,
    pub shape: Vec<i64>,
    pub dtype: String,
    #[serde(default)]
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OnnxInferenceRequest {
    #[serde(default)]
    pub inputs: Vec<OnnxTensor>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OnnxInferenceResponse {
    #[serde(default)]
    pub outputs: Vec<OnnxTensor>,
}
