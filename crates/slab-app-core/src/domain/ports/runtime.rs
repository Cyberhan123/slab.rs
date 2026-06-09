use std::collections::BTreeMap;

use async_trait::async_trait;
use futures::stream::BoxStream;
use serde_json::Value;
use slab_agent_tracing::AgentTraceContext;
use slab_types::{RuntimeBackendId, RuntimeBackendLoadSpec};

use crate::domain::models::TimedTextSegment;
use crate::error::AppCoreError;

pub type RuntimeJsonOptions = BTreeMap<String, Value>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeTextPromptTokensDetails {
    pub cached_tokens: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeTextGenerationUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub prompt_tokens_details: RuntimeTextPromptTokensDetails,
    pub estimated: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeTextGenerationRequest {
    pub backend_id: Option<RuntimeBackendId>,
    pub model: String,
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub min_p: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub repetition_penalty: Option<f32>,
    pub session_key: Option<String>,
    pub stream: bool,
    pub gbnf: Option<String>,
    pub stop_sequences: Vec<String>,
    pub agent_trace: Option<AgentTraceContext>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeTextGenerationResponse {
    pub text: String,
    pub finish_reason: Option<String>,
    pub tokens_used: Option<u32>,
    pub usage: Option<RuntimeTextGenerationUsage>,
    pub metadata: RuntimeJsonOptions,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeTextGenerationChunk {
    pub delta: String,
    pub done: bool,
    pub finish_reason: Option<String>,
    pub usage: Option<RuntimeTextGenerationUsage>,
    pub metadata: RuntimeJsonOptions,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeRawImageInput {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub channels: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeGeneratedImage {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub channels: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeGeneratedFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub channels: u8,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeDiffusionImageRequest {
    pub backend_id: Option<RuntimeBackendId>,
    pub model: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    pub init_image: Option<RuntimeRawImageInput>,
    pub count: Option<u32>,
    pub cfg_scale: Option<f32>,
    pub guidance: Option<f32>,
    pub steps: Option<i32>,
    pub seed: Option<i64>,
    pub sample_method: Option<String>,
    pub scheduler: Option<String>,
    pub clip_skip: Option<i32>,
    pub strength: Option<f32>,
    pub eta: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeDiffusionImageResult {
    pub images: Vec<RuntimeGeneratedImage>,
    pub metadata: RuntimeJsonOptions,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeDiffusionVideoRequest {
    pub model: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    pub init_image: Option<RuntimeRawImageInput>,
    pub video_frames: Option<i32>,
    pub fps: Option<f32>,
    pub cfg_scale: Option<f32>,
    pub guidance: Option<f32>,
    pub steps: Option<i32>,
    pub seed: Option<i64>,
    pub sample_method: Option<String>,
    pub scheduler: Option<String>,
    pub strength: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeDiffusionVideoResult {
    pub frames: Vec<RuntimeGeneratedFrame>,
    pub metadata: RuntimeJsonOptions,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeTranscriptionVadParams {
    pub threshold: Option<f32>,
    pub min_speech_duration_ms: Option<i32>,
    pub min_silence_duration_ms: Option<i32>,
    pub max_speech_duration_s: Option<f32>,
    pub speech_pad_ms: Option<i32>,
    pub samples_overlap: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeTranscriptionVadOptions {
    pub enabled: bool,
    pub model_path: Option<String>,
    pub params: Option<RuntimeTranscriptionVadParams>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeTranscriptionDecodeOptions {
    pub offset_ms: Option<i32>,
    pub duration_ms: Option<i32>,
    pub no_context: Option<bool>,
    pub no_timestamps: Option<bool>,
    pub token_timestamps: Option<bool>,
    pub split_on_word: Option<bool>,
    pub suppress_nst: Option<bool>,
    pub word_thold: Option<f32>,
    pub max_len: Option<i32>,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
    pub temperature_inc: Option<f32>,
    pub entropy_thold: Option<f32>,
    pub logprob_thold: Option<f32>,
    pub no_speech_thold: Option<f32>,
    pub tdrz_enable: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeTranscriptionRequest {
    pub backend_id: Option<RuntimeBackendId>,
    pub path: String,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub detect_language: Option<bool>,
    pub vad: Option<RuntimeTranscriptionVadOptions>,
    pub decode: Option<RuntimeTranscriptionDecodeOptions>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeTranscriptionResult {
    pub text: String,
    pub segments: Vec<TimedTextSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBackendStatus {
    pub backend: RuntimeBackendId,
    pub status: String,
    pub context_length: Option<u32>,
    pub training_context_length: Option<u32>,
}

/// Domain port for model runtime inference.
///
/// Implementations adapt business runtime commands to a concrete transport while
/// keeping protobuf, gRPC, and backend client details outside domain services.
#[async_trait]
pub trait RuntimeInferenceGateway: Send + Sync + std::fmt::Debug {
    fn backend_available(&self, backend_id: RuntimeBackendId) -> bool;

    async fn chat(
        &self,
        request: RuntimeTextGenerationRequest,
    ) -> Result<RuntimeTextGenerationResponse, AppCoreError>;

    async fn chat_stream(
        &self,
        request: RuntimeTextGenerationRequest,
    ) -> Result<BoxStream<'static, Result<RuntimeTextGenerationChunk, AppCoreError>>, AppCoreError>;

    async fn transcribe(
        &self,
        request: RuntimeTranscriptionRequest,
    ) -> Result<RuntimeTranscriptionResult, AppCoreError>;

    async fn generate_image(
        &self,
        request: RuntimeDiffusionImageRequest,
    ) -> Result<RuntimeDiffusionImageResult, AppCoreError>;

    async fn generate_video(
        &self,
        request: RuntimeDiffusionVideoRequest,
    ) -> Result<RuntimeDiffusionVideoResult, AppCoreError>;

    async fn load_model(
        &self,
        spec: &RuntimeBackendLoadSpec,
    ) -> Result<RuntimeBackendStatus, AppCoreError>;

    async fn unload_model(
        &self,
        backend_id: RuntimeBackendId,
    ) -> Result<RuntimeBackendStatus, AppCoreError>;
}
