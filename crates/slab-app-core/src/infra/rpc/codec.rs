use std::io::Cursor;
use std::path::Path;

use image::{DynamicImage, ImageFormat};
use slab_types::diffusion::{
    DiffusionImageBackend, DiffusionImageRequest, DiffusionImageResponse, DiffusionVideoBackend,
    DiffusionVideoResponse, DiffusionVideoRequest,
};
use slab_types::inference::{
    JsonOptions, TextGenerationChunk, TextGenerationRequest, TextGenerationResponse,
    TextGenerationUsage, TextPromptTokensDetails,
};
use slab_types::media::{GeneratedFrame, GeneratedImage, RawImageInput};
use slab_types::{RuntimeBackendId, RuntimeBackendLoadSpec, RuntimeModelStatus};
use thiserror::Error;

use super::pb;

const REASONING_CONTENT_METADATA_KEY: &str = "reasoning_content";

#[derive(Debug, Error)]
pub enum RpcCodecError {
    #[error("missing required field `{field}`")]
    MissingField { field: &'static str },
    #[error("invalid field `{field}`: {message}")]
    InvalidField { field: &'static str, message: String },
    #[error("failed to encode raw image as PNG: {0}")]
    ImageEncode(#[from] image::ImageError),
}

#[derive(Debug, Clone)]
pub enum ModelLoadRpcRequest {
    GgmlLlama(pb::GgmlLlamaLoadRequest),
    GgmlWhisper(pb::GgmlWhisperLoadRequest),
    GgmlDiffusion(pb::GgmlDiffusionLoadRequest),
    CandleLlama(pb::CandleLlamaLoadRequest),
    CandleWhisper(pb::CandleWhisperLoadRequest),
    CandleDiffusion(pb::CandleDiffusionLoadRequest),
    OnnxText(pb::OnnxTextLoadRequest),
}

impl ModelLoadRpcRequest {
    pub fn backend_id(&self) -> RuntimeBackendId {
        match self {
            Self::GgmlLlama(_) => RuntimeBackendId::GgmlLlama,
            Self::GgmlWhisper(_) => RuntimeBackendId::GgmlWhisper,
            Self::GgmlDiffusion(_) => RuntimeBackendId::GgmlDiffusion,
            Self::CandleLlama(_) => RuntimeBackendId::CandleLlama,
            Self::CandleWhisper(_) => RuntimeBackendId::CandleWhisper,
            Self::CandleDiffusion(_) => RuntimeBackendId::CandleDiffusion,
            Self::OnnxText(_) => RuntimeBackendId::Onnx,
        }
    }

    pub fn model_path(&self) -> Option<&str> {
        match self {
            Self::GgmlLlama(request) => request.model_path.as_deref(),
            Self::GgmlWhisper(request) => request.model_path.as_deref(),
            Self::GgmlDiffusion(request) => request.model_path.as_deref(),
            Self::CandleLlama(request) => request.model_path.as_deref(),
            Self::CandleWhisper(request) => request.model_path.as_deref(),
            Self::CandleDiffusion(request) => request.model_path.as_deref(),
            Self::OnnxText(request) => request.model_path.as_deref(),
        }
    }
}

pub fn encode_model_load_request(spec: &RuntimeBackendLoadSpec) -> ModelLoadRpcRequest {
    match spec {
        RuntimeBackendLoadSpec::GgmlLlama(config) => {
            ModelLoadRpcRequest::GgmlLlama(pb::GgmlLlamaLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                num_workers: Some(usize_to_u32(config.num_workers)),
                context_length: config.context_length.filter(|value| *value != 0),
                chat_template: non_empty_string(config.chat_template.as_deref()),
                gbnf: non_empty_string(config.gbnf.as_deref()),
                flash_attn: Some(config.flash_attn),
            })
        }
        RuntimeBackendLoadSpec::GgmlWhisper(config) => {
            ModelLoadRpcRequest::GgmlWhisper(pb::GgmlWhisperLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                flash_attn: Some(config.flash_attn),
            })
        }
        RuntimeBackendLoadSpec::GgmlDiffusion(config) => {
            ModelLoadRpcRequest::GgmlDiffusion(pb::GgmlDiffusionLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                diffusion_model_path: opt_path_to_string(config.diffusion_model_path.as_deref()),
                vae_path: opt_path_to_string(config.vae_path.as_deref()),
                taesd_path: opt_path_to_string(config.taesd_path.as_deref()),
                clip_l_path: opt_path_to_string(config.clip_l_path.as_deref()),
                clip_g_path: opt_path_to_string(config.clip_g_path.as_deref()),
                t5xxl_path: opt_path_to_string(config.t5xxl_path.as_deref()),
                clip_vision_path: opt_path_to_string(config.clip_vision_path.as_deref()),
                control_net_path: opt_path_to_string(config.control_net_path.as_deref()),
                flash_attn: Some(config.flash_attn),
                vae_device: non_empty_string(config.vae_device.as_deref()),
                clip_device: non_empty_string(config.clip_device.as_deref()),
                offload_params_to_cpu: Some(config.offload_params_to_cpu),
                enable_mmap: Some(config.enable_mmap),
                n_threads: config.n_threads.filter(|value| *value != 0),
            })
        }
        RuntimeBackendLoadSpec::CandleLlama(config) => {
            ModelLoadRpcRequest::CandleLlama(pb::CandleLlamaLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                tokenizer_path: opt_path_to_string(config.tokenizer_path.as_deref()),
                seed: Some(config.seed),
            })
        }
        RuntimeBackendLoadSpec::CandleWhisper(config) => {
            ModelLoadRpcRequest::CandleWhisper(pb::CandleWhisperLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                tokenizer_path: opt_path_to_string(config.tokenizer_path.as_deref()),
            })
        }
        RuntimeBackendLoadSpec::CandleDiffusion(config) => {
            ModelLoadRpcRequest::CandleDiffusion(pb::CandleDiffusionLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                vae_path: opt_path_to_string(config.vae_path.as_deref()),
                sd_version: non_empty_string(Some(&config.sd_version)),
            })
        }
        RuntimeBackendLoadSpec::Onnx(config) => {
            ModelLoadRpcRequest::OnnxText(pb::OnnxTextLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                execution_providers: Some(pb::StringList {
                    values: config.execution_providers.clone(),
                }),
                intra_op_num_threads: config.intra_op_num_threads.map(usize_to_u32),
                inter_op_num_threads: config.inter_op_num_threads.map(usize_to_u32),
            })
        }
    }
}

pub fn decode_model_status_response(
    response: &pb::ModelStatusResponse,
) -> Result<RuntimeModelStatus, RpcCodecError> {
    let backend =
        response.backend.parse::<RuntimeBackendId>().map_err(|error| RpcCodecError::InvalidField {
            field: "backend",
            message: error.to_string(),
        })?;

    Ok(RuntimeModelStatus { backend, status: response.status.clone() })
}

pub fn encode_chat_request(
    _model: impl Into<String>,
    request: &TextGenerationRequest,
) -> pb::GgmlLlamaChatRequest {
    let prompt = match request.system_prompt.as_deref() {
        Some(system_prompt) if !system_prompt.is_empty() => {
            format!("{system_prompt}\n\n{}", request.prompt)
        }
        _ => request.prompt.clone(),
    };

    pb::GgmlLlamaChatRequest {
        prompt: Some(prompt),
        max_tokens: request.max_tokens,
        temperature: request.temperature,
        top_p: request.top_p,
        top_k: request.top_k,
        min_p: request.min_p,
        presence_penalty: request.presence_penalty,
        repetition_penalty: request.repetition_penalty,
        session_key: request.session_key.clone(),
        gbnf: request.gbnf.clone(),
        stop_sequences: Some(pb::StringList { values: request.stop_sequences.clone() }),
        ignore_eos: None,
        logit_bias_json: None,
    }
}

pub fn decode_chat_response(response: &pb::GgmlLlamaChatResponse) -> TextGenerationResponse {
    let mut metadata = JsonOptions::default();
    insert_reasoning_content_metadata(&mut metadata, response.reasoning_content.as_deref());

    TextGenerationResponse {
        text: response.text.clone().unwrap_or_default(),
        finish_reason: response.finish_reason.clone(),
        tokens_used: response.tokens_used,
        usage: response.usage.as_ref().map(decode_usage),
        metadata,
    }
}

pub fn decode_chat_stream_chunk(chunk: &pb::GgmlLlamaChatStreamChunk) -> TextGenerationChunk {
    let mut metadata = JsonOptions::default();
    insert_reasoning_content_metadata(&mut metadata, chunk.reasoning_content.as_deref());

    TextGenerationChunk {
        delta: chunk.delta.clone().unwrap_or_default(),
        done: chunk.done.unwrap_or_default(),
        finish_reason: chunk.finish_reason.clone(),
        usage: chunk.usage.as_ref().map(decode_usage),
        metadata,
    }
}

pub fn encode_diffusion_image_request(
    _model: impl Into<String>,
    request: &DiffusionImageRequest,
) -> pb::GgmlDiffusionGenerateImageRequest {
    let params = request.backend.as_ggml();

    pb::GgmlDiffusionGenerateImageRequest {
        prompt: Some(request.common.prompt.clone()),
        negative_prompt: non_empty_string(request.common.negative_prompt.as_deref()),
        width: Some(request.common.width),
        height: Some(request.common.height),
        init_image: request.common.init_image.as_ref().map(raw_image_input_to_proto),
        count: params.count,
        cfg_scale: params.cfg_scale,
        guidance: params.guidance,
        sample_steps: params.steps,
        seed: params.seed,
        sample_method: non_empty_string(params.sample_method.as_deref()),
        scheduler: non_empty_string(params.scheduler.as_deref()),
        clip_skip: params.clip_skip,
        strength: params.strength,
        eta: params.eta,
    }
}

pub fn encode_diffusion_video_request(
    _model: impl Into<String>,
    request: &DiffusionVideoRequest,
) -> pb::GgmlDiffusionGenerateVideoRequest {
    let params = request.backend.as_ggml();

    pb::GgmlDiffusionGenerateVideoRequest {
        prompt: Some(request.common.prompt.clone()),
        negative_prompt: non_empty_string(request.common.negative_prompt.as_deref()),
        width: Some(request.common.width),
        height: Some(request.common.height),
        init_image: request.common.init_image.as_ref().map(raw_image_input_to_proto),
        video_frames: params.video_frames.and_then(|value| u32::try_from(value).ok()),
        fps: params.fps,
        cfg_scale: params.cfg_scale,
        guidance: params.guidance,
        sample_steps: params.steps,
        seed: params.seed,
        sample_method: non_empty_string(params.sample_method.as_deref()),
        scheduler: non_empty_string(params.scheduler.as_deref()),
        strength: params.strength,
    }
}

pub fn decode_diffusion_image_response(
    response: &pb::GgmlDiffusionGenerateImageResponse,
) -> Result<DiffusionImageResponse, RpcCodecError> {
    let images = response
        .images
        .iter()
        .map(|image| {
            Ok(GeneratedImage {
                bytes: raw_image_to_png_bytes(image)?,
                width: required_u32(image.width, "images[].width")?,
                height: required_u32(image.height, "images[].height")?,
                channels: required_u8(image.channels, "images[].channels")?,
            })
        })
        .collect::<Result<Vec<_>, RpcCodecError>>()?;

    Ok(DiffusionImageResponse { images, metadata: JsonOptions::default() })
}

pub fn decode_diffusion_video_response(
    response: &pb::GgmlDiffusionGenerateVideoResponse,
) -> Result<DiffusionVideoResponse, RpcCodecError> {
    let frames = response
        .frames
        .iter()
        .map(|frame| {
            Ok(GeneratedFrame {
                data: frame.data.clone(),
                width: required_u32(frame.width, "frames[].width")?,
                height: required_u32(frame.height, "frames[].height")?,
                channels: required_u8(frame.channels, "frames[].channels")?,
            })
        })
        .collect::<Result<Vec<_>, RpcCodecError>>()?;

    Ok(DiffusionVideoResponse { frames, metadata: JsonOptions::default() })
}

pub fn decode_whisper_transcription_text(response: &pb::GgmlWhisperTranscribeResponse) -> String {
    response
        .transcription
        .as_ref()
        .and_then(|transcription| transcription.raw_text.clone())
        .unwrap_or_default()
}

fn decode_usage(usage: &pb::Usage) -> TextGenerationUsage {
    TextGenerationUsage {
        prompt_tokens: usage.prompt_tokens.unwrap_or_default(),
        completion_tokens: usage.completion_tokens.unwrap_or_default(),
        total_tokens: usage.total_tokens.unwrap_or_default(),
        prompt_tokens_details: TextPromptTokensDetails {
            cached_tokens: usage.prompt_cached_tokens.unwrap_or_default(),
        },
        estimated: usage.estimated.unwrap_or_default(),
    }
}

fn insert_reasoning_content_metadata(metadata: &mut JsonOptions, reasoning_content: Option<&str>) {
    let Some(reasoning_content) = reasoning_content else {
        return;
    };
    if reasoning_content.is_empty() {
        return;
    }
    metadata.insert(
        REASONING_CONTENT_METADATA_KEY.to_owned(),
        serde_json::Value::String(reasoning_content.to_owned()),
    );
}

fn raw_image_input_to_proto(input: &RawImageInput) -> pb::RawImage {
    pb::RawImage {
        data: input.data.clone(),
        width: Some(input.width),
        height: Some(input.height),
        channels: Some(u32::from(input.channels)),
    }
}

fn raw_image_to_png_bytes(image: &pb::RawImage) -> Result<Vec<u8>, RpcCodecError> {
    let width = required_u32(image.width, "raw_image.width")?;
    let height = required_u32(image.height, "raw_image.height")?;
    let channels = required_u8(image.channels, "raw_image.channels")?;

    let dynamic = match channels {
        1 => image::ImageBuffer::<image::Luma<u8>, _>::from_raw(
            width,
            height,
            image.data.clone(),
        )
        .map(DynamicImage::ImageLuma8),
        2 => image::ImageBuffer::<image::LumaA<u8>, _>::from_raw(
            width,
            height,
            image.data.clone(),
        )
        .map(DynamicImage::ImageLumaA8),
        3 => image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(
            width,
            height,
            image.data.clone(),
        )
        .map(DynamicImage::ImageRgb8),
        4 => image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
            width,
            height,
            image.data.clone(),
        )
        .map(DynamicImage::ImageRgba8),
        other => {
            return Err(RpcCodecError::InvalidField {
                field: "raw_image.channels",
                message: format!("unsupported channel count: {other}"),
            });
        }
    }
    .ok_or_else(|| RpcCodecError::InvalidField {
        field: "raw_image.data",
        message: format!(
            "pixel buffer length {} does not match {width}x{height}x{channels}",
            image.data.len()
        ),
    })?;

    let mut cursor = Cursor::new(Vec::new());
    dynamic.write_to(&mut cursor, ImageFormat::Png)?;
    Ok(cursor.into_inner())
}

fn required_u32(value: Option<u32>, field: &'static str) -> Result<u32, RpcCodecError> {
    let value = value.ok_or(RpcCodecError::MissingField { field })?;
    if value == 0 {
        return Err(RpcCodecError::InvalidField {
            field,
            message: "must be greater than zero".to_owned(),
        });
    }
    Ok(value)
}

fn required_u8(value: Option<u32>, field: &'static str) -> Result<u8, RpcCodecError> {
    let value = required_u32(value, field)?;
    u8::try_from(value).map_err(|error| RpcCodecError::InvalidField {
        field,
        message: error.to_string(),
    })
}

fn non_empty_string(value: Option<&str>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty()).map(ToOwned::to_owned)
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

#[allow(dead_code)]
fn _assert_diffusion_backend_shapes(
    image_backend: &DiffusionImageBackend,
    video_backend: &DiffusionVideoBackend,
) {
    let _ = (image_backend.as_ggml(), video_backend.as_ggml());
}
