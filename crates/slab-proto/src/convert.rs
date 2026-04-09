use std::path::{Path, PathBuf};

use base64::Engine as _;
use image::GenericImageView;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slab_types::backend::RuntimeBackendId;
use slab_types::chat::{
    ConversationContentPart, ConversationMessage, ConversationMessageContent, ConversationToolCall,
    ConversationToolFunction,
};
use slab_types::diffusion::{
    DiffusionImageRequest, DiffusionImageResponse, DiffusionVideoRequest, DiffusionVideoResponse,
};
use slab_types::inference::{
    ImageGenerationResponse, TextGenerationChunk, TextGenerationRequest, TextGenerationResponse,
    TextGenerationUsage, TextPromptTokensDetails,
};
use slab_types::media::{GeneratedFrame, GeneratedImage, RawImageInput};
use slab_types::runtime::RuntimeModelStatus;
use slab_types::{
    CandleDiffusionLoadConfig, CandleLlamaLoadConfig, CandleWhisperLoadConfig,
    GgmlDiffusionLoadConfig, GgmlLlamaLoadConfig, GgmlWhisperLoadConfig, OnnxLoadConfig,
    RuntimeBackendLoadSpec,
};
use thiserror::Error;

use crate::slab::ipc::v1 as pb;

const REASONING_CONTENT_METADATA_KEY: &str = "reasoning_content";

fn reasoning_content_from_metadata(metadata: &slab_types::inference::JsonOptions) -> String {
    metadata
        .get(REASONING_CONTENT_METADATA_KEY)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn insert_reasoning_content_metadata(
    metadata: &mut slab_types::inference::JsonOptions,
    reasoning_content: &str,
) {
    if reasoning_content.is_empty() {
        return;
    }
    metadata.insert(
        REASONING_CONTENT_METADATA_KEY.to_owned(),
        Value::String(reasoning_content.to_owned()),
    );
}

#[derive(Debug, Error)]
pub enum ProtoConversionError {
    #[error("{field} must not be empty")]
    EmptyField { field: &'static str },
    #[error("{field} is missing")]
    MissingField { field: &'static str },
    #[error("{field} must be at least {minimum}")]
    BelowMinimum { field: &'static str, minimum: i64 },
    #[error("{field} exceeds supported range")]
    OutOfRange { field: &'static str },
    #[error("unknown runtime backend id: {0}")]
    UnknownBackend(String),
    #[error("failed to parse {field} JSON: {source}")]
    Json {
        field: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to decode {field} as base64: {source}")]
    Base64 {
        field: &'static str,
        #[source]
        source: base64::DecodeError,
    },
    #[error("failed to decode {field} image bytes: {source}")]
    Image {
        field: &'static str,
        #[source]
        source: image::ImageError,
    },
}

pub fn encode_model_load_request(spec: &RuntimeBackendLoadSpec) -> pb::ModelLoadRequest {
    use pb::model_load_request::BackendParams;

    let common =
        pb::ModelLoadCommon { model_path: path_to_string(model_path_from_backend_load_spec(spec)) };

    let backend_params = match spec {
        RuntimeBackendLoadSpec::GgmlLlama(config) => {
            Some(BackendParams::GgmlLlama(pb::GgmlLlamaLoadParams {
                num_workers: usize_to_u32(config.num_workers),
                context_length: config.context_length.filter(|value| *value != 0),
                chat_template: non_empty_string(config.chat_template.as_deref()),
            }))
        }
        RuntimeBackendLoadSpec::GgmlWhisper(_) => {
            Some(BackendParams::GgmlWhisper(pb::GgmlWhisperLoadParams {}))
        }
        RuntimeBackendLoadSpec::GgmlDiffusion(config) => {
            Some(BackendParams::GgmlDiffusion(pb::GgmlDiffusionLoadParams {
                diffusion_model_path: opt_path_to_string(config.diffusion_model_path.as_deref()),
                vae_path: opt_path_to_string(config.vae_path.as_deref()),
                taesd_path: opt_path_to_string(config.taesd_path.as_deref()),
                clip_l_path: opt_path_to_string(config.clip_l_path.as_deref()),
                clip_g_path: opt_path_to_string(config.clip_g_path.as_deref()),
                t5xxl_path: opt_path_to_string(config.t5xxl_path.as_deref()),
                clip_vision_path: opt_path_to_string(config.clip_vision_path.as_deref()),
                control_net_path: opt_path_to_string(config.control_net_path.as_deref()),
                flash_attn: config.flash_attn,
                vae_device: non_empty_string(config.vae_device.as_deref()),
                clip_device: non_empty_string(config.clip_device.as_deref()),
                offload_params_to_cpu: config.offload_params_to_cpu,
                enable_mmap: config.enable_mmap,
                n_threads: config.n_threads.filter(|value| *value != 0),
            }))
        }
        RuntimeBackendLoadSpec::CandleLlama(config) => {
            Some(BackendParams::CandleLlama(pb::CandleLlamaLoadParams {
                tokenizer_path: opt_path_to_string(config.tokenizer_path.as_deref()),
                seed: config.seed,
            }))
        }
        RuntimeBackendLoadSpec::CandleWhisper(config) => {
            Some(BackendParams::CandleWhisper(pb::CandleWhisperLoadParams {
                tokenizer_path: opt_path_to_string(config.tokenizer_path.as_deref()),
            }))
        }
        RuntimeBackendLoadSpec::CandleDiffusion(config) => {
            Some(BackendParams::CandleDiffusion(pb::CandleDiffusionLoadParams {
                vae_path: opt_path_to_string(config.vae_path.as_deref()),
                sd_version: config.sd_version.clone(),
            }))
        }
        RuntimeBackendLoadSpec::Onnx(config) => Some(BackendParams::Onnx(pb::OnnxLoadParams {
            execution_providers: config.execution_providers.clone(),
            intra_op_num_threads: config.intra_op_num_threads.map(usize_to_u32),
            inter_op_num_threads: config.inter_op_num_threads.map(usize_to_u32),
        })),
    };

    pb::ModelLoadRequest { common: Some(common), backend_params }
}

pub fn decode_model_load_request(
    request: &pb::ModelLoadRequest,
) -> Result<RuntimeBackendLoadSpec, ProtoConversionError> {
    use pb::model_load_request::BackendParams;

    let model_path = model_path_from_model_load_request(request)?;
    let backend_params = request
        .backend_params
        .as_ref()
        .ok_or(ProtoConversionError::MissingField { field: "backend_params" })?;

    match backend_params {
        BackendParams::GgmlLlama(config) => {
            ensure_u32_at_least(config.num_workers, 1, "ggml_llama.num_workers")?;

            Ok(RuntimeBackendLoadSpec::GgmlLlama(GgmlLlamaLoadConfig {
                model_path,
                num_workers: u32_to_usize(config.num_workers, "ggml_llama.num_workers")?,
                context_length: config.context_length.filter(|value| *value != 0),
                chat_template: non_empty_string(config.chat_template.as_deref()),
            }))
        }
        BackendParams::GgmlWhisper(_) => {
            Ok(RuntimeBackendLoadSpec::GgmlWhisper(GgmlWhisperLoadConfig { model_path }))
        }
        BackendParams::GgmlDiffusion(config) => {
            Ok(RuntimeBackendLoadSpec::GgmlDiffusion(GgmlDiffusionLoadConfig {
                model_path,
                diffusion_model_path: non_empty_path(config.diffusion_model_path.as_deref()),
                vae_path: non_empty_path(config.vae_path.as_deref()),
                taesd_path: non_empty_path(config.taesd_path.as_deref()),
                clip_l_path: non_empty_path(config.clip_l_path.as_deref()),
                clip_g_path: non_empty_path(config.clip_g_path.as_deref()),
                t5xxl_path: non_empty_path(config.t5xxl_path.as_deref()),
                clip_vision_path: non_empty_path(config.clip_vision_path.as_deref()),
                control_net_path: non_empty_path(config.control_net_path.as_deref()),
                flash_attn: config.flash_attn,
                vae_device: non_empty_string(config.vae_device.as_deref()),
                clip_device: non_empty_string(config.clip_device.as_deref()),
                offload_params_to_cpu: config.offload_params_to_cpu,
                enable_mmap: config.enable_mmap,
                n_threads: config.n_threads.filter(|value| *value != 0),
            }))
        }
        BackendParams::CandleLlama(config) => {
            Ok(RuntimeBackendLoadSpec::CandleLlama(CandleLlamaLoadConfig {
                model_path,
                tokenizer_path: non_empty_path(config.tokenizer_path.as_deref()),
                seed: config.seed,
            }))
        }
        BackendParams::CandleWhisper(config) => {
            Ok(RuntimeBackendLoadSpec::CandleWhisper(CandleWhisperLoadConfig {
                model_path,
                tokenizer_path: non_empty_path(config.tokenizer_path.as_deref()),
            }))
        }
        BackendParams::CandleDiffusion(config) => {
            ensure_non_empty(&config.sd_version, "candle_diffusion.sd_version")?;

            Ok(RuntimeBackendLoadSpec::CandleDiffusion(CandleDiffusionLoadConfig {
                model_path,
                vae_path: non_empty_path(config.vae_path.as_deref()),
                sd_version: config.sd_version.clone(),
            }))
        }
        BackendParams::Onnx(config) => Ok(RuntimeBackendLoadSpec::Onnx(OnnxLoadConfig {
            model_path,
            execution_providers: config.execution_providers.clone(),
            intra_op_num_threads: config
                .intra_op_num_threads
                .filter(|value| *value != 0)
                .map(|value| u32_to_usize(value, "onnx.intra_op_num_threads"))
                .transpose()?,
            inter_op_num_threads: config
                .inter_op_num_threads
                .filter(|value| *value != 0)
                .map(|value| u32_to_usize(value, "onnx.inter_op_num_threads"))
                .transpose()?,
        })),
    }
}

pub fn encode_model_status_response(status: &RuntimeModelStatus) -> pb::ModelStatusResponse {
    pb::ModelStatusResponse { backend: status.backend.to_string(), status: status.status.clone() }
}

pub fn decode_model_status_response(
    response: &pb::ModelStatusResponse,
) -> Result<RuntimeModelStatus, ProtoConversionError> {
    let backend = response
        .backend
        .parse::<RuntimeBackendId>()
        .map_err(|_| ProtoConversionError::UnknownBackend(response.backend.clone()))?;

    Ok(RuntimeModelStatus { backend, status: response.status.clone() })
}

pub fn encode_chat_request(
    model: impl Into<String>,
    request: &TextGenerationRequest,
) -> pb::ChatRequest {
    // The prompt field always carries the pre-rendered fallback.  Preserve the
    // legacy behavior of prefixing `system_prompt` when present, even when
    // using structured messages for template application.
    let prompt = match request.system_prompt.as_deref() {
        Some(system_prompt) if !system_prompt.is_empty() => {
            format!("{system_prompt}\n\n{}", request.prompt)
        }
        _ => request.prompt.clone(),
    };

    let messages = request.chat_messages.iter().map(conversation_message_to_proto).collect();

    pb::ChatRequest {
        prompt,
        model: model.into(),
        max_tokens: request.max_tokens.unwrap_or_default(),
        temperature: request.temperature.unwrap_or_default(),
        top_p: request.top_p.unwrap_or_default(),
        session_key: request.session_key.clone().unwrap_or_default(),
        messages,
        apply_chat_template: request.apply_chat_template,
        grammar: request.grammar.clone().unwrap_or_default(),
        grammar_json: request.grammar_json,
        grammar_tool_call: request.grammar_tool_call,
    }
}

pub fn decode_chat_request(
    request: &pb::ChatRequest,
    stream: bool,
) -> Result<TextGenerationRequest, ProtoConversionError> {
    // Require a non-empty prompt, or (apply_chat_template && non-empty messages).
    let prompt_empty = request.prompt.trim().is_empty();
    let messages_empty = request.messages.is_empty();
    let allow_empty_prompt = request.apply_chat_template && !messages_empty;

    if prompt_empty && !allow_empty_prompt {
        return Err(ProtoConversionError::EmptyField { field: "prompt" });
    }

    let chat_messages: Vec<ConversationMessage> =
        request.messages.iter().map(conversation_message_from_proto).collect();

    Ok(TextGenerationRequest {
        prompt: request.prompt.clone(),
        system_prompt: None,
        chat_messages,
        apply_chat_template: request.apply_chat_template,
        max_tokens: (request.max_tokens > 0).then_some(request.max_tokens),
        temperature: (request.temperature > 0.0).then_some(request.temperature),
        top_p: (request.top_p > 0.0).then_some(request.top_p),
        session_key: (!request.session_key.is_empty()).then_some(request.session_key.clone()),
        stream,
        grammar: (!request.grammar.is_empty()).then_some(request.grammar.clone()),
        grammar_json: request.grammar_json,
        grammar_tool_call: request.grammar_tool_call,
        ..Default::default()
    })
}

pub fn encode_chat_response(response: &TextGenerationResponse) -> pb::ChatResponse {
    pb::ChatResponse {
        text: response.text.clone(),
        finish_reason: response.finish_reason.clone().unwrap_or_default(),
        tokens_used: response.tokens_used.unwrap_or_default(),
        usage: response.usage.as_ref().map(encode_usage),
        reasoning_content: reasoning_content_from_metadata(&response.metadata),
    }
}

pub fn decode_chat_response(response: &pb::ChatResponse) -> TextGenerationResponse {
    let mut metadata = slab_types::inference::JsonOptions::default();
    insert_reasoning_content_metadata(&mut metadata, &response.reasoning_content);

    TextGenerationResponse {
        text: response.text.clone(),
        finish_reason: (!response.finish_reason.is_empty())
            .then_some(response.finish_reason.clone()),
        tokens_used: (response.tokens_used > 0).then_some(response.tokens_used),
        usage: response.usage.as_ref().map(decode_usage),
        metadata,
    }
}

pub fn encode_chat_stream_chunk(chunk: &TextGenerationChunk) -> pb::ChatStreamChunk {
    pb::ChatStreamChunk {
        token: chunk.delta.clone(),
        error: String::new(),
        done: chunk.done,
        finish_reason: chunk.finish_reason.clone().unwrap_or_default(),
        usage: chunk.usage.as_ref().map(encode_usage),
        reasoning_content: reasoning_content_from_metadata(&chunk.metadata),
    }
}

pub fn decode_chat_stream_chunk(chunk: &pb::ChatStreamChunk) -> TextGenerationChunk {
    let mut metadata = slab_types::inference::JsonOptions::default();
    insert_reasoning_content_metadata(&mut metadata, &chunk.reasoning_content);

    TextGenerationChunk {
        delta: chunk.token.clone(),
        done: chunk.done,
        finish_reason: (!chunk.finish_reason.is_empty()).then_some(chunk.finish_reason.clone()),
        usage: chunk.usage.as_ref().map(decode_usage),
        metadata,
    }
}

fn encode_usage(usage: &TextGenerationUsage) -> pb::Usage {
    pb::Usage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        prompt_cached_tokens: usage.prompt_tokens_details.cached_tokens,
        estimated: usage.estimated,
    }
}

fn decode_usage(usage: &pb::Usage) -> TextGenerationUsage {
    TextGenerationUsage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        prompt_tokens_details: TextPromptTokensDetails {
            cached_tokens: usage.prompt_cached_tokens,
        },
        estimated: usage.estimated,
    }
}

fn conversation_message_to_proto(message: &ConversationMessage) -> pb::ChatMessage {
    let (content, content_parts) = match &message.content {
        ConversationMessageContent::Text(text) => (text.clone(), Vec::new()),
        ConversationMessageContent::Parts(parts) => (
            message.rendered_text(),
            parts.iter().map(conversation_content_part_to_proto).collect(),
        ),
    };

    pb::ChatMessage {
        role: message.role.clone(),
        content,
        content_parts,
        name: message.name.clone().unwrap_or_default(),
        tool_call_id: message.tool_call_id.clone().unwrap_or_default(),
        tool_calls: message.tool_calls.iter().map(conversation_tool_call_to_proto).collect(),
    }
}

fn conversation_message_from_proto(message: &pb::ChatMessage) -> ConversationMessage {
    let content = if !message.content_parts.is_empty() {
        ConversationMessageContent::Parts(
            message.content_parts.iter().map(conversation_content_part_from_proto).collect(),
        )
    } else {
        ConversationMessageContent::Text(message.content.clone())
    };

    ConversationMessage {
        role: message.role.clone(),
        content,
        name: (!message.name.is_empty()).then_some(message.name.clone()),
        tool_call_id: (!message.tool_call_id.is_empty()).then_some(message.tool_call_id.clone()),
        tool_calls: message.tool_calls.iter().map(conversation_tool_call_from_proto).collect(),
    }
}

fn conversation_content_part_to_proto(part: &ConversationContentPart) -> pb::ChatContentPart {
    use pb::chat_content_part::Part;

    let part = match part {
        ConversationContentPart::Text { text } => {
            Part::Text(pb::ChatTextPart { text: text.clone() })
        }
        ConversationContentPart::InputText { text } => {
            Part::InputText(pb::ChatTextPart { text: text.clone() })
        }
        ConversationContentPart::OutputText { text } => {
            Part::OutputText(pb::ChatTextPart { text: text.clone() })
        }
        ConversationContentPart::Image { image_url, mime_type, detail } => {
            Part::Image(pb::ChatImagePart {
                image_url: image_url.clone().unwrap_or_default(),
                mime_type: mime_type.clone().unwrap_or_default(),
                detail: detail.clone().unwrap_or_default(),
            })
        }
        ConversationContentPart::ToolResult { tool_call_id, value } => {
            Part::ToolResult(pb::ChatToolResultPart {
                tool_call_id: tool_call_id.clone().unwrap_or_default(),
                value_json: serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned()),
            })
        }
        ConversationContentPart::Json { value } => Part::Json(pb::ChatJsonPart {
            value_json: serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned()),
        }),
        ConversationContentPart::Refusal { text } => {
            Part::Refusal(pb::ChatTextPart { text: text.clone() })
        }
    };

    pb::ChatContentPart { part: Some(part) }
}

fn conversation_content_part_from_proto(part: &pb::ChatContentPart) -> ConversationContentPart {
    use pb::chat_content_part::Part;

    match part.part.as_ref() {
        Some(Part::Text(value)) => ConversationContentPart::Text { text: value.text.clone() },
        Some(Part::InputText(value)) => {
            ConversationContentPart::InputText { text: value.text.clone() }
        }
        Some(Part::OutputText(value)) => {
            ConversationContentPart::OutputText { text: value.text.clone() }
        }
        Some(Part::Image(value)) => ConversationContentPart::Image {
            image_url: (!value.image_url.is_empty()).then_some(value.image_url.clone()),
            mime_type: (!value.mime_type.is_empty()).then_some(value.mime_type.clone()),
            detail: (!value.detail.is_empty()).then_some(value.detail.clone()),
        },
        Some(Part::ToolResult(value)) => ConversationContentPart::ToolResult {
            tool_call_id: (!value.tool_call_id.is_empty()).then_some(value.tool_call_id.clone()),
            value: parse_json_or_null(&value.value_json),
        },
        Some(Part::Json(value)) => {
            ConversationContentPart::Json { value: parse_json_or_null(&value.value_json) }
        }
        Some(Part::Refusal(value)) => ConversationContentPart::Refusal { text: value.text.clone() },
        None => ConversationContentPart::Text { text: String::new() },
    }
}

fn conversation_tool_call_to_proto(tool_call: &ConversationToolCall) -> pb::ChatToolCall {
    pb::ChatToolCall {
        id: tool_call.id.clone().unwrap_or_default(),
        r#type: tool_call.r#type.clone(),
        function: Some(pb::ChatToolFunction {
            name: tool_call.function.name.clone(),
            arguments: tool_call.function.arguments.clone(),
        }),
    }
}

fn conversation_tool_call_from_proto(tool_call: &pb::ChatToolCall) -> ConversationToolCall {
    ConversationToolCall {
        id: (!tool_call.id.is_empty()).then_some(tool_call.id.clone()),
        r#type: if tool_call.r#type.is_empty() {
            "function".to_owned()
        } else {
            tool_call.r#type.clone()
        },
        function: conversation_tool_function_from_proto(tool_call.function.as_ref()),
    }
}

fn conversation_tool_function_from_proto(
    function: Option<&pb::ChatToolFunction>,
) -> ConversationToolFunction {
    let Some(function) = function else {
        return ConversationToolFunction { name: String::new(), arguments: String::new() };
    };

    ConversationToolFunction { name: function.name.clone(), arguments: function.arguments.clone() }
}

fn parse_json_or_null(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or(serde_json::Value::Null)
}

pub fn decode_diffusion_image_request(
    request: &pb::ImageRequest,
) -> Result<DiffusionImageRequest, ProtoConversionError> {
    ensure_non_empty(&request.prompt, "prompt")?;
    ensure_i32_at_least(request.sample_steps, 0, "sample_steps")?;

    Ok(DiffusionImageRequest {
        prompt: request.prompt.clone(),
        negative_prompt: (!request.negative_prompt.is_empty())
            .then_some(request.negative_prompt.clone()),
        count: request.n.max(1),
        width: request.width.max(1),
        height: request.height.max(1),
        cfg_scale: Some(request.cfg_scale),
        guidance: Some(request.guidance),
        steps: Some(request.sample_steps.max(1)),
        seed: Some(request.seed),
        sample_method: (!request.sample_method.is_empty()).then_some(request.sample_method.clone()),
        scheduler: (!request.scheduler.is_empty()).then_some(request.scheduler.clone()),
        clip_skip: (request.clip_skip > 0).then_some(request.clip_skip),
        strength: Some(request.strength),
        eta: Some(request.eta),
        init_image: raw_image_input_from_proto_parts(
            &request.init_image_data,
            request.init_image_width,
            request.init_image_height,
            request.init_image_channels,
        ),
        options: Default::default(),
    })
}

pub fn encode_diffusion_image_request(
    model: impl Into<String>,
    request: &DiffusionImageRequest,
) -> pb::ImageRequest {
    let (init_image_data, init_image_width, init_image_height, init_image_channels) =
        raw_image_input_to_proto_parts(request.init_image.as_ref());

    pb::ImageRequest {
        model: model.into(),
        prompt: request.prompt.clone(),
        negative_prompt: request.negative_prompt.clone().unwrap_or_default(),
        n: request.count.max(1),
        width: request.width,
        height: request.height,
        cfg_scale: request.cfg_scale.unwrap_or(7.0),
        guidance: request.guidance.unwrap_or(3.5),
        sample_steps: request.steps.unwrap_or(20),
        seed: request.seed.unwrap_or(42),
        sample_method: request.sample_method.clone().unwrap_or_default(),
        scheduler: request.scheduler.clone().unwrap_or_default(),
        clip_skip: request.clip_skip.unwrap_or(-1),
        strength: request.strength.unwrap_or(0.75),
        eta: request.eta.unwrap_or_default(),
        init_image_data,
        init_image_width,
        init_image_height,
        init_image_channels,
    }
}

pub fn decode_diffusion_video_request(
    request: &pb::VideoRequest,
) -> Result<DiffusionVideoRequest, ProtoConversionError> {
    ensure_non_empty(&request.prompt, "prompt")?;
    ensure_i32_at_least(request.sample_steps, 0, "sample_steps")?;

    Ok(DiffusionVideoRequest {
        prompt: request.prompt.clone(),
        negative_prompt: (!request.negative_prompt.is_empty())
            .then_some(request.negative_prompt.clone()),
        width: request.width.max(1),
        height: request.height.max(1),
        video_frames: request.video_frames.max(1),
        fps: request.fps,
        cfg_scale: Some(request.cfg_scale),
        guidance: Some(request.guidance),
        steps: Some(request.sample_steps.max(1)),
        seed: Some(request.seed),
        sample_method: (!request.sample_method.is_empty()).then_some(request.sample_method.clone()),
        scheduler: (!request.scheduler.is_empty()).then_some(request.scheduler.clone()),
        strength: Some(request.strength),
        init_image: raw_image_input_from_proto_parts(
            &request.init_image_data,
            request.init_image_width,
            request.init_image_height,
            request.init_image_channels,
        ),
        options: Default::default(),
    })
}

pub fn encode_diffusion_video_request(
    model: impl Into<String>,
    request: &DiffusionVideoRequest,
) -> pb::VideoRequest {
    let (init_image_data, init_image_width, init_image_height, init_image_channels) =
        raw_image_input_to_proto_parts(request.init_image.as_ref());

    pb::VideoRequest {
        model: model.into(),
        prompt: request.prompt.clone(),
        negative_prompt: request.negative_prompt.clone().unwrap_or_default(),
        width: request.width,
        height: request.height,
        cfg_scale: request.cfg_scale.unwrap_or(7.0),
        guidance: request.guidance.unwrap_or(3.5),
        sample_steps: request.steps.unwrap_or(20),
        seed: request.seed.unwrap_or(42),
        sample_method: request.sample_method.clone().unwrap_or_default(),
        scheduler: request.scheduler.clone().unwrap_or_default(),
        video_frames: request.video_frames,
        fps: request.fps,
        strength: request.strength.unwrap_or(0.75),
        init_image_data,
        init_image_width,
        init_image_height,
        init_image_channels,
    }
}

pub fn diffusion_image_response_from_generated(
    response: &ImageGenerationResponse,
) -> Result<DiffusionImageResponse, ProtoConversionError> {
    let images = response
        .images
        .iter()
        .map(|image_bytes| {
            let metadata = image_metadata_from_bytes(image_bytes)?;

            Ok(GeneratedImage {
                bytes: image_bytes.clone(),
                width: metadata.width,
                height: metadata.height,
                channels: metadata.channels,
            })
        })
        .collect::<Result<Vec<_>, ProtoConversionError>>()?;

    Ok(DiffusionImageResponse { images, metadata: response.metadata.clone() })
}

pub fn encode_generated_image_response(
    response: &ImageGenerationResponse,
) -> Result<pb::ImageResponse, ProtoConversionError> {
    let response = diffusion_image_response_from_generated(response)?;
    encode_diffusion_image_response(&response)
}

pub fn encode_diffusion_image_response(
    response: &DiffusionImageResponse,
) -> Result<pb::ImageResponse, ProtoConversionError> {
    let images = response
        .images
        .iter()
        .map(|image| {
            let metadata = if image.width == 0 || image.height == 0 || image.channels == 0 {
                Some(image_metadata_from_bytes(&image.bytes)?)
            } else {
                None
            };

            Ok(ProtoImageEntry {
                image: base64::engine::general_purpose::STANDARD.encode(&image.bytes),
                width: image.width.max(metadata.map_or(0, |value| value.width)),
                height: image.height.max(metadata.map_or(0, |value| value.height)),
                channels: max_channels(image.channels, metadata.map_or(0, |value| value.channels)),
            })
        })
        .collect::<Result<Vec<_>, ProtoConversionError>>()?;

    let images_json = serde_json::to_vec(&ProtoImagePayload { images })
        .map_err(|source| ProtoConversionError::Json { field: "image_response", source })?;

    Ok(pb::ImageResponse { images_json })
}

pub fn decode_diffusion_image_response(
    response: &pb::ImageResponse,
) -> Result<DiffusionImageResponse, ProtoConversionError> {
    let payload: ProtoImagePayload = serde_json::from_slice(&response.images_json)
        .map_err(|source| ProtoConversionError::Json { field: "images_json", source })?;

    let images = payload
        .images
        .into_iter()
        .map(|image| {
            let bytes = base64::engine::general_purpose::STANDARD.decode(image.image).map_err(
                |source| ProtoConversionError::Base64 {
                    field: "images_json.images[].image",
                    source,
                },
            )?;

            let metadata = image_metadata_from_bytes(&bytes)?;
            Ok(GeneratedImage {
                bytes,
                width: image.width.max(metadata.width),
                height: image.height.max(metadata.height),
                channels: max_channels(image.channels, metadata.channels),
            })
        })
        .collect::<Result<Vec<_>, ProtoConversionError>>()?;

    Ok(DiffusionImageResponse { images, metadata: Default::default() })
}

pub fn diffusion_video_response_from_generated(
    response: &ImageGenerationResponse,
) -> Result<DiffusionVideoResponse, ProtoConversionError> {
    let frames = response
        .images
        .iter()
        .map(|image_bytes| {
            let decoded = image::load_from_memory(image_bytes).map_err(|source| {
                ProtoConversionError::Image { field: "generated_frame", source }
            })?;
            let (width, height) = decoded.dimensions();

            let (data, channels) = if decoded.color().channel_count() == 4 {
                (decoded.to_rgba8().into_raw(), 4u8)
            } else {
                (decoded.to_rgb8().into_raw(), 3u8)
            };

            Ok(GeneratedFrame { data, width, height, channels })
        })
        .collect::<Result<Vec<_>, ProtoConversionError>>()?;

    Ok(DiffusionVideoResponse { frames, metadata: response.metadata.clone() })
}

pub fn encode_generated_video_response(
    response: &ImageGenerationResponse,
) -> Result<pb::VideoResponse, ProtoConversionError> {
    let response = diffusion_video_response_from_generated(response)?;
    encode_diffusion_video_response(&response)
}

pub fn encode_diffusion_video_response(
    response: &DiffusionVideoResponse,
) -> Result<pb::VideoResponse, ProtoConversionError> {
    let frames = response
        .frames
        .iter()
        .map(|frame| {
            Ok(ProtoFrameEntry {
                b64: base64::engine::general_purpose::STANDARD.encode(&frame.data),
                width: frame.width,
                height: frame.height,
                channels: frame.channels.max(1),
            })
        })
        .collect::<Result<Vec<_>, ProtoConversionError>>()?;

    let frames_json = serde_json::to_vec(&frames)
        .map_err(|source| ProtoConversionError::Json { field: "frames_json", source })?;

    Ok(pb::VideoResponse { frames_json })
}

pub fn decode_diffusion_video_response(
    response: &pb::VideoResponse,
) -> Result<DiffusionVideoResponse, ProtoConversionError> {
    let frames: Vec<ProtoFrameEntry> = serde_json::from_slice(&response.frames_json)
        .map_err(|source| ProtoConversionError::Json { field: "frames_json", source })?;

    let frames = frames
        .into_iter()
        .map(|frame| {
            let data =
                base64::engine::general_purpose::STANDARD.decode(frame.b64).map_err(|source| {
                    ProtoConversionError::Base64 { field: "frames_json[].b64", source }
                })?;

            Ok(GeneratedFrame {
                data,
                width: frame.width,
                height: frame.height,
                channels: frame.channels.max(1),
            })
        })
        .collect::<Result<Vec<_>, ProtoConversionError>>()?;

    Ok(DiffusionVideoResponse { frames, metadata: Default::default() })
}

fn raw_image_input_from_proto_parts(
    data: &[u8],
    width: u32,
    height: u32,
    channels: u32,
) -> Option<RawImageInput> {
    if data.is_empty() {
        return None;
    }

    Some(RawImageInput {
        data: data.to_vec(),
        width,
        height,
        channels: normalize_channels(channels),
    })
}

fn raw_image_input_to_proto_parts(init_image: Option<&RawImageInput>) -> (Vec<u8>, u32, u32, u32) {
    match init_image {
        Some(image) => {
            (image.data.clone(), image.width, image.height, u32::from(image.channels.max(1)))
        }
        None => (Vec::new(), 0, 0, 3),
    }
}

fn image_metadata_from_bytes(bytes: &[u8]) -> Result<ImageMetadata, ProtoConversionError> {
    let decoded = image::load_from_memory(bytes)
        .map_err(|source| ProtoConversionError::Image { field: "generated_image", source })?;
    let (width, height) = decoded.dimensions();

    Ok(ImageMetadata { width, height, channels: decoded.color().channel_count() })
}

fn ensure_non_empty(value: &str, field: &'static str) -> Result<(), ProtoConversionError> {
    if value.trim().is_empty() {
        return Err(ProtoConversionError::EmptyField { field });
    }
    Ok(())
}

fn ensure_u32_at_least(
    value: u32,
    minimum: u32,
    field: &'static str,
) -> Result<(), ProtoConversionError> {
    if value < minimum {
        return Err(ProtoConversionError::BelowMinimum { field, minimum: i64::from(minimum) });
    }
    Ok(())
}

fn ensure_i32_at_least(
    value: i32,
    minimum: i32,
    field: &'static str,
) -> Result<(), ProtoConversionError> {
    if value < minimum {
        return Err(ProtoConversionError::BelowMinimum { field, minimum: i64::from(minimum) });
    }
    Ok(())
}

fn model_path_from_model_load_request(
    request: &pb::ModelLoadRequest,
) -> Result<PathBuf, ProtoConversionError> {
    let common =
        request.common.as_ref().ok_or(ProtoConversionError::MissingField { field: "common" })?;
    ensure_non_empty(&common.model_path, "common.model_path")?;
    Ok(PathBuf::from(&common.model_path))
}

fn model_path_from_backend_load_spec(spec: &RuntimeBackendLoadSpec) -> &Path {
    match spec {
        RuntimeBackendLoadSpec::GgmlLlama(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::GgmlWhisper(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::GgmlDiffusion(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::CandleLlama(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::CandleWhisper(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::CandleDiffusion(config) => config.model_path.as_path(),
        RuntimeBackendLoadSpec::Onnx(config) => config.model_path.as_path(),
    }
}

fn non_empty_string(value: Option<&str>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty()).map(ToOwned::to_owned)
}

fn non_empty_path(value: Option<&str>) -> Option<PathBuf> {
    value.filter(|value| !value.trim().is_empty()).map(PathBuf::from)
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn opt_path_to_string(path: Option<&Path>) -> Option<String> {
    path.map(path_to_string)
}

fn usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn u32_to_usize(value: u32, field: &'static str) -> Result<usize, ProtoConversionError> {
    usize::try_from(value).map_err(|_| ProtoConversionError::OutOfRange { field })
}

fn normalize_channels(channels: u32) -> u8 {
    channels.clamp(1, u8::MAX as u32) as u8
}

fn max_channels(lhs: u8, rhs: u8) -> u8 {
    lhs.max(rhs).max(1)
}

#[derive(Debug, Clone, Copy)]
struct ImageMetadata {
    width: u32,
    height: u32,
    channels: u8,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProtoImagePayload {
    #[serde(default)]
    images: Vec<ProtoImageEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProtoImageEntry {
    image: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    channels: u8,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProtoFrameEntry {
    b64: String,
    width: u32,
    height: u32,
    channels: u8,
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, RgbImage};
    use std::io::Cursor;

    #[test]
    fn model_load_spec_round_trips_diffusion_backend_fields() {
        let spec = RuntimeBackendLoadSpec::GgmlDiffusion(GgmlDiffusionLoadConfig {
            model_path: PathBuf::from("C:/models/model.gguf"),
            diffusion_model_path: Some(PathBuf::from("C:/models/diffusion.safetensors")),
            vae_path: Some(PathBuf::from("C:/models/vae.safetensors")),
            taesd_path: None,
            clip_l_path: None,
            clip_g_path: Some(PathBuf::from("C:/models/clip-g.safetensors")),
            t5xxl_path: None,
            clip_vision_path: Some(PathBuf::from("C:/models/clip-vision.safetensors")),
            control_net_path: Some(PathBuf::from("C:/models/controlnet.safetensors")),
            flash_attn: true,
            vae_device: Some(String::from("cpu")),
            clip_device: None,
            offload_params_to_cpu: true,
            enable_mmap: true,
            n_threads: Some(8),
        });

        let request = encode_model_load_request(&spec);
        let roundtrip = decode_model_load_request(&request).unwrap();

        assert_eq!(roundtrip, spec);
    }

    #[test]
    fn model_load_spec_round_trips_ggml_llama_fields() {
        let spec = RuntimeBackendLoadSpec::GgmlLlama(GgmlLlamaLoadConfig {
            model_path: PathBuf::from("C:/models/model.gguf"),
            num_workers: 2,
            context_length: Some(8192),
            chat_template: Some(
                "{% for message in messages %}{{ message['content'] }}{% endfor %}".to_owned(),
            ),
        });

        let request = encode_model_load_request(&spec);
        let roundtrip = decode_model_load_request(&request).unwrap();

        assert_eq!(roundtrip, spec);
    }

    #[test]
    fn model_load_spec_round_trips_onnx_fields() {
        let spec = RuntimeBackendLoadSpec::Onnx(OnnxLoadConfig {
            model_path: PathBuf::from("C:/models/encoder.onnx"),
            execution_providers: vec!["CPU".to_owned(), "CUDA".to_owned()],
            intra_op_num_threads: Some(4),
            inter_op_num_threads: Some(2),
        });

        let request = encode_model_load_request(&spec);
        let roundtrip = decode_model_load_request(&request).unwrap();

        assert_eq!(roundtrip, spec);
    }

    #[test]
    fn model_load_request_rejects_missing_common_model_path() {
        let request = pb::ModelLoadRequest {
            common: Some(pb::ModelLoadCommon { model_path: String::new() }),
            backend_params: Some(pb::model_load_request::BackendParams::GgmlDiffusion(
                pb::GgmlDiffusionLoadParams {
                    diffusion_model_path: Some(
                        PathBuf::from("C:/models/diffusion.safetensors")
                            .to_string_lossy()
                            .into_owned(),
                    ),
                    vae_path: Some(
                        PathBuf::from("C:/models/vae.safetensors").to_string_lossy().into_owned(),
                    ),
                    taesd_path: None,
                    clip_l_path: None,
                    clip_g_path: None,
                    t5xxl_path: None,
                    clip_vision_path: None,
                    control_net_path: None,
                    flash_attn: false,
                    vae_device: None,
                    clip_device: None,
                    offload_params_to_cpu: false,
                    enable_mmap: false,
                    n_threads: None,
                },
            )),
        };

        let error = decode_model_load_request(&request).unwrap_err();

        assert!(matches!(error, ProtoConversionError::EmptyField { field: "common.model_path" }));
    }

    #[test]
    fn diffusion_image_request_round_trips_init_image() {
        let request = DiffusionImageRequest {
            prompt: "test".to_owned(),
            negative_prompt: Some("bad".to_owned()),
            count: 2,
            width: 640,
            height: 480,
            cfg_scale: Some(6.5),
            guidance: Some(3.0),
            steps: Some(30),
            seed: Some(7),
            sample_method: Some("euler".to_owned()),
            scheduler: Some("normal".to_owned()),
            clip_skip: Some(1),
            strength: Some(0.8),
            eta: Some(0.2),
            init_image: Some(RawImageInput {
                data: vec![1, 2, 3, 4, 5, 6],
                width: 1,
                height: 2,
                channels: 3,
            }),
            options: Default::default(),
        };

        let proto = encode_diffusion_image_request("demo-model", &request);
        let roundtrip = decode_diffusion_image_request(&proto).unwrap();

        assert_eq!(roundtrip, request);
    }

    #[test]
    fn diffusion_image_request_preserves_unset_clip_skip_as_none() {
        let request = DiffusionImageRequest {
            prompt: "test".to_owned(),
            width: 512,
            height: 512,
            clip_skip: None,
            ..Default::default()
        };

        let proto = encode_diffusion_image_request("demo-model", &request);
        assert_eq!(proto.clip_skip, -1);

        let roundtrip = decode_diffusion_image_request(&proto).unwrap();
        assert_eq!(roundtrip.clip_skip, None);
    }

    #[test]
    fn generated_images_round_trip_through_proto_payload() {
        let png = make_png_bytes();
        let response =
            ImageGenerationResponse { images: vec![png.clone()], metadata: Default::default() };

        let proto = encode_generated_image_response(&response).unwrap();
        let roundtrip = decode_diffusion_image_response(&proto).unwrap();

        assert_eq!(roundtrip.images.len(), 1);
        assert_eq!(roundtrip.images[0].bytes, png);
        assert_eq!(roundtrip.images[0].width, 2);
        assert_eq!(roundtrip.images[0].height, 1);
        assert_eq!(roundtrip.images[0].channels, 3);
    }

    #[test]
    fn generated_frames_round_trip_through_proto_payload() {
        let response = ImageGenerationResponse {
            images: vec![make_png_bytes()],
            metadata: Default::default(),
        };

        let proto = encode_generated_video_response(&response).unwrap();
        let roundtrip = decode_diffusion_video_response(&proto).unwrap();

        assert_eq!(roundtrip.frames.len(), 1);
        assert_eq!(roundtrip.frames[0].width, 2);
        assert_eq!(roundtrip.frames[0].height, 1);
        assert_eq!(roundtrip.frames[0].channels, 3);
        assert_eq!(roundtrip.frames[0].data.len(), 6);
    }

    fn make_png_bytes() -> Vec<u8> {
        let image = RgbImage::from_raw(2, 1, vec![255, 0, 0, 0, 255, 0]).unwrap();
        let dynamic = DynamicImage::ImageRgb8(image);
        let mut cursor = Cursor::new(Vec::new());
        dynamic.write_to(&mut cursor, ImageFormat::Png).unwrap();
        cursor.into_inner()
    }
}
