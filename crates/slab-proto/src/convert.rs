use std::path::{Path, PathBuf};

use base64::Engine as _;
use image::GenericImageView;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slab_types::backend::RuntimeBackendId;
use slab_types::diffusion::{
    DiffusionImageBackend, DiffusionImageRequest, DiffusionImageResponse, DiffusionRequestCommon,
    DiffusionVideoBackend, DiffusionVideoRequest, DiffusionVideoResponse, GgmlDiffusionImageParams,
    GgmlDiffusionVideoParams,
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
                gbnf: non_empty_string(config.gbnf.as_deref()),
                flash_attn: Some(config.flash_attn),
            }))
        }
        RuntimeBackendLoadSpec::GgmlWhisper(config) => {
            Some(BackendParams::GgmlWhisper(pb::GgmlWhisperLoadParams {
                flash_attn: Some(config.flash_attn),
            }))
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
                flash_attn: Some(config.flash_attn),
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
                flash_attn: config.flash_attn.unwrap_or(true),
                chat_template: non_empty_string(config.chat_template.as_deref()),
                gbnf: non_empty_string(config.gbnf.as_deref()),
            }))
        }
        BackendParams::GgmlWhisper(config) => {
            Ok(RuntimeBackendLoadSpec::GgmlWhisper(GgmlWhisperLoadConfig {
                model_path,
                flash_attn: config.flash_attn.unwrap_or(true),
            }))
        }
        BackendParams::GgmlDiffusion(config) => {
            Ok(RuntimeBackendLoadSpec::GgmlDiffusion(Box::new(GgmlDiffusionLoadConfig {
                model_path,
                diffusion_model_path: non_empty_path(config.diffusion_model_path.as_deref()),
                vae_path: non_empty_path(config.vae_path.as_deref()),
                taesd_path: non_empty_path(config.taesd_path.as_deref()),
                clip_l_path: non_empty_path(config.clip_l_path.as_deref()),
                clip_g_path: non_empty_path(config.clip_g_path.as_deref()),
                t5xxl_path: non_empty_path(config.t5xxl_path.as_deref()),
                clip_vision_path: non_empty_path(config.clip_vision_path.as_deref()),
                control_net_path: non_empty_path(config.control_net_path.as_deref()),
                flash_attn: config.flash_attn.unwrap_or(true),
                vae_device: non_empty_string(config.vae_device.as_deref()),
                clip_device: non_empty_string(config.clip_device.as_deref()),
                offload_params_to_cpu: config.offload_params_to_cpu,
                enable_mmap: config.enable_mmap,
                n_threads: config.n_threads.filter(|value| *value != 0),
            })))
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

    pb::ChatRequest {
        prompt,
        model: model.into(),
        max_tokens: request.max_tokens.unwrap_or_default(),
        temperature: request.temperature.unwrap_or_default(),
        top_p: request.top_p.unwrap_or_default(),
        top_k: request.top_k,
        min_p: request.min_p,
        presence_penalty: request.presence_penalty,
        repetition_penalty: request.repetition_penalty,
        session_key: request.session_key.clone().unwrap_or_default(),
        gbnf: request.gbnf.clone().unwrap_or_default(),
        stop_sequences: request.stop_sequences.clone(),
    }
}

pub fn decode_chat_request(
    request: &pb::ChatRequest,
    stream: bool,
) -> Result<TextGenerationRequest, ProtoConversionError> {
    if request.prompt.trim().is_empty() {
        return Err(ProtoConversionError::EmptyField { field: "prompt" });
    }

    Ok(TextGenerationRequest {
        prompt: request.prompt.clone(),
        system_prompt: None,
        max_tokens: (request.max_tokens > 0).then_some(request.max_tokens),
        temperature: (request.temperature > 0.0).then_some(request.temperature),
        top_p: (request.top_p > 0.0).then_some(request.top_p),
        top_k: request.top_k,
        min_p: request.min_p,
        presence_penalty: request.presence_penalty,
        repetition_penalty: request.repetition_penalty,
        session_key: (!request.session_key.is_empty()).then_some(request.session_key.clone()),
        stream,
        gbnf: (!request.gbnf.is_empty()).then_some(request.gbnf.clone()),
        stop_sequences: request.stop_sequences.clone(),
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

pub fn decode_diffusion_image_request(
    request: &pb::ImageRequest,
) -> Result<DiffusionImageRequest, ProtoConversionError> {
    use pb::image_request::BackendParams;

    let common =
        request.common.as_ref().ok_or(ProtoConversionError::MissingField { field: "common" })?;
    let backend = request
        .backend_params
        .as_ref()
        .ok_or(ProtoConversionError::MissingField { field: "backend_params" })?;

    Ok(DiffusionImageRequest {
        common: decode_diffusion_request_common(common)?,
        backend: match backend {
            BackendParams::Ggml(params) => {
                ensure_optional_u32_at_least(params.n, 1, "backend_params.ggml.n")?;
                ensure_optional_i32_at_least(
                    params.sample_steps,
                    1,
                    "backend_params.ggml.sample_steps",
                )?;

                DiffusionImageBackend::Ggml(GgmlDiffusionImageParams {
                    count: params.n,
                    cfg_scale: params.cfg_scale,
                    guidance: params.guidance,
                    steps: params.sample_steps,
                    seed: params.seed,
                    sample_method: optional_non_empty_owned(params.sample_method.as_ref()),
                    scheduler: optional_non_empty_owned(params.scheduler.as_ref()),
                    clip_skip: params.clip_skip,
                    strength: params.strength,
                    eta: params.eta,
                })
            }
        },
    })
}

pub fn encode_diffusion_image_request(
    model: impl Into<String>,
    request: &DiffusionImageRequest,
) -> pb::ImageRequest {
    use pb::image_request::BackendParams;

    let backend_params = match &request.backend {
        DiffusionImageBackend::Ggml(params) => {
            Some(BackendParams::Ggml(pb::GgmlDiffusionImageParams {
                n: params.count,
                cfg_scale: params.cfg_scale,
                guidance: params.guidance,
                sample_steps: params.steps,
                seed: params.seed,
                sample_method: non_empty_string(params.sample_method.as_deref()),
                scheduler: non_empty_string(params.scheduler.as_deref()),
                clip_skip: params.clip_skip,
                strength: params.strength,
                eta: params.eta,
            }))
        }
    };

    pb::ImageRequest {
        model: model.into(),
        common: Some(encode_diffusion_request_common(&request.common)),
        backend_params,
    }
}

pub fn decode_diffusion_video_request(
    request: &pb::VideoRequest,
) -> Result<DiffusionVideoRequest, ProtoConversionError> {
    use pb::video_request::BackendParams;

    let common =
        request.common.as_ref().ok_or(ProtoConversionError::MissingField { field: "common" })?;
    let backend = request
        .backend_params
        .as_ref()
        .ok_or(ProtoConversionError::MissingField { field: "backend_params" })?;

    Ok(DiffusionVideoRequest {
        common: decode_diffusion_request_common(common)?,
        backend: match backend {
            BackendParams::Ggml(params) => {
                ensure_optional_i32_at_least(
                    params.video_frames,
                    1,
                    "backend_params.ggml.video_frames",
                )?;
                ensure_optional_i32_at_least(
                    params.sample_steps,
                    1,
                    "backend_params.ggml.sample_steps",
                )?;

                DiffusionVideoBackend::Ggml(GgmlDiffusionVideoParams {
                    video_frames: params.video_frames,
                    fps: params.fps,
                    cfg_scale: params.cfg_scale,
                    guidance: params.guidance,
                    steps: params.sample_steps,
                    seed: params.seed,
                    sample_method: optional_non_empty_owned(params.sample_method.as_ref()),
                    scheduler: optional_non_empty_owned(params.scheduler.as_ref()),
                    strength: params.strength,
                })
            }
        },
    })
}

pub fn encode_diffusion_video_request(
    model: impl Into<String>,
    request: &DiffusionVideoRequest,
) -> pb::VideoRequest {
    use pb::video_request::BackendParams;

    let backend_params = match &request.backend {
        DiffusionVideoBackend::Ggml(params) => {
            Some(BackendParams::Ggml(pb::GgmlDiffusionVideoParams {
                video_frames: params.video_frames,
                fps: params.fps,
                cfg_scale: params.cfg_scale,
                guidance: params.guidance,
                sample_steps: params.steps,
                seed: params.seed,
                sample_method: non_empty_string(params.sample_method.as_deref()),
                scheduler: non_empty_string(params.scheduler.as_deref()),
                strength: params.strength,
            }))
        }
    };

    pb::VideoRequest {
        model: model.into(),
        common: Some(encode_diffusion_request_common(&request.common)),
        backend_params,
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
                width: metadata.width,
                height: metadata.height,
                channels: metadata.channels,
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

fn decode_diffusion_request_common(
    common: &pb::DiffusionRequestCommon,
) -> Result<DiffusionRequestCommon, ProtoConversionError> {
    ensure_non_empty(&common.prompt, "common.prompt")?;
    ensure_u32_at_least(common.width, 1, "common.width")?;
    ensure_u32_at_least(common.height, 1, "common.height")?;

    Ok(DiffusionRequestCommon {
        prompt: common.prompt.clone(),
        negative_prompt: optional_non_empty_owned(common.negative_prompt.as_ref()),
        width: common.width,
        height: common.height,
        init_image: raw_image_input_from_proto(common.init_image.as_ref())?,
        options: Default::default(),
    })
}

fn encode_diffusion_request_common(common: &DiffusionRequestCommon) -> pb::DiffusionRequestCommon {
    pb::DiffusionRequestCommon {
        prompt: common.prompt.clone(),
        negative_prompt: non_empty_string(common.negative_prompt.as_deref()),
        width: common.width,
        height: common.height,
        init_image: raw_image_input_to_proto(common.init_image.as_ref()),
    }
}

fn raw_image_input_from_proto(
    init_image: Option<&pb::RawImageInput>,
) -> Result<Option<RawImageInput>, ProtoConversionError> {
    let Some(image) = init_image else {
        return Ok(None);
    };

    Ok(Some(RawImageInput {
        data: image.data.clone(),
        width: image.width,
        height: image.height,
        channels: u8::try_from(image.channels).map_err(|_| ProtoConversionError::OutOfRange {
            field: "common.init_image.channels",
        })?,
    }))
}

fn raw_image_input_to_proto(init_image: Option<&RawImageInput>) -> Option<pb::RawImageInput> {
    init_image.map(|image| pb::RawImageInput {
        data: image.data.clone(),
        width: image.width,
        height: image.height,
        channels: u32::from(image.channels),
    })
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

fn ensure_optional_u32_at_least(
    value: Option<u32>,
    minimum: u32,
    field: &'static str,
) -> Result<(), ProtoConversionError> {
    if let Some(value) = value {
        ensure_u32_at_least(value, minimum, field)?;
    }
    Ok(())
}

fn ensure_optional_i32_at_least(
    value: Option<i32>,
    minimum: i32,
    field: &'static str,
) -> Result<(), ProtoConversionError> {
    if let Some(value) = value {
        ensure_i32_at_least(value, minimum, field)?;
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

fn optional_non_empty_owned(value: Option<&String>) -> Option<String> {
    value.and_then(|value| non_empty_string(Some(value.as_str())))
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
        let spec = RuntimeBackendLoadSpec::GgmlDiffusion(Box::new(GgmlDiffusionLoadConfig {
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
        }));

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
            flash_attn: true,
            chat_template: Some(
                "{% for message in messages %}{{ message['content'] }}{% endfor %}".to_owned(),
            ),
            gbnf: Some("root ::= object".to_owned()),
        });

        let request = encode_model_load_request(&spec);
        let roundtrip = decode_model_load_request(&request).unwrap();

        assert_eq!(roundtrip, spec);
    }

    #[test]
    fn model_load_spec_round_trips_ggml_whisper_fields() {
        let spec = RuntimeBackendLoadSpec::GgmlWhisper(GgmlWhisperLoadConfig {
            model_path: PathBuf::from("C:/models/model.bin"),
            flash_attn: false,
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
    fn decode_model_load_request_treats_legacy_zero_sentinels_as_unset() {
        let ggml_llama_request = pb::ModelLoadRequest {
            common: Some(pb::ModelLoadCommon { model_path: "C:/models/model.gguf".to_owned() }),
            backend_params: Some(pb::model_load_request::BackendParams::GgmlLlama(
                pb::GgmlLlamaLoadParams {
                    num_workers: 1,
                    context_length: Some(0),
                    chat_template: None,
                    gbnf: None,
                    flash_attn: Some(true),
                },
            )),
        };
        let ggml_diffusion_request = pb::ModelLoadRequest {
            common: Some(pb::ModelLoadCommon { model_path: "C:/models/model.gguf".to_owned() }),
            backend_params: Some(pb::model_load_request::BackendParams::GgmlDiffusion(
                pb::GgmlDiffusionLoadParams {
                    diffusion_model_path: None,
                    vae_path: None,
                    taesd_path: None,
                    clip_l_path: None,
                    clip_g_path: None,
                    t5xxl_path: None,
                    clip_vision_path: None,
                    control_net_path: None,
                    flash_attn: Some(true),
                    vae_device: None,
                    clip_device: None,
                    offload_params_to_cpu: false,
                    enable_mmap: false,
                    n_threads: Some(0),
                },
            )),
        };
        let onnx_request = pb::ModelLoadRequest {
            common: Some(pb::ModelLoadCommon { model_path: "C:/models/model.onnx".to_owned() }),
            backend_params: Some(pb::model_load_request::BackendParams::Onnx(pb::OnnxLoadParams {
                execution_providers: vec!["CPU".to_owned()],
                intra_op_num_threads: Some(0),
                inter_op_num_threads: Some(0),
            })),
        };

        let ggml_llama = decode_model_load_request(&ggml_llama_request).unwrap();
        let ggml_diffusion = decode_model_load_request(&ggml_diffusion_request).unwrap();
        let onnx = decode_model_load_request(&onnx_request).unwrap();

        match ggml_llama {
            RuntimeBackendLoadSpec::GgmlLlama(config) => assert_eq!(config.context_length, None),
            other => panic!("expected ggml llama config, got {other:?}"),
        }

        match ggml_diffusion {
            RuntimeBackendLoadSpec::GgmlDiffusion(config) => assert_eq!(config.n_threads, None),
            other => panic!("expected ggml diffusion config, got {other:?}"),
        }

        match onnx {
            RuntimeBackendLoadSpec::Onnx(config) => {
                assert_eq!(config.intra_op_num_threads, None);
                assert_eq!(config.inter_op_num_threads, None);
            }
            other => panic!("expected onnx config, got {other:?}"),
        }
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
                    flash_attn: Some(false),
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
            common: DiffusionRequestCommon {
                prompt: "test".to_owned(),
                negative_prompt: Some("bad".to_owned()),
                width: 640,
                height: 480,
                init_image: Some(RawImageInput {
                    data: vec![1, 2, 3, 4, 5, 6],
                    width: 1,
                    height: 2,
                    channels: 3,
                }),
                options: Default::default(),
            },
            backend: DiffusionImageBackend::Ggml(GgmlDiffusionImageParams {
                count: Some(2),
                cfg_scale: Some(6.5),
                guidance: Some(3.0),
                steps: Some(30),
                seed: Some(7),
                sample_method: Some("euler".to_owned()),
                scheduler: Some("normal".to_owned()),
                clip_skip: Some(1),
                strength: Some(0.8),
                eta: Some(0.2),
            }),
        };

        let proto = encode_diffusion_image_request("demo-model", &request);
        let roundtrip = decode_diffusion_image_request(&proto).unwrap();

        assert_eq!(roundtrip, request);
    }

    #[test]
    fn diffusion_image_request_preserves_unset_clip_skip_as_none() {
        let request = DiffusionImageRequest {
            common: DiffusionRequestCommon {
                prompt: "test".to_owned(),
                width: 512,
                height: 512,
                ..Default::default()
            },
            backend: DiffusionImageBackend::Ggml(GgmlDiffusionImageParams {
                clip_skip: None,
                ..Default::default()
            }),
        };

        let proto = encode_diffusion_image_request("demo-model", &request);
        assert!(matches!(
            proto.backend_params.as_ref(),
            Some(pb::image_request::BackendParams::Ggml(params)) if params.clip_skip.is_none()
        ));

        let roundtrip = decode_diffusion_image_request(&proto).unwrap();
        assert_eq!(roundtrip.backend.as_ggml().clip_skip, None);
    }

    #[test]
    fn diffusion_video_request_round_trips_backend_params() {
        let request = DiffusionVideoRequest {
            common: DiffusionRequestCommon {
                prompt: "animate".to_owned(),
                negative_prompt: Some("artifact".to_owned()),
                width: 512,
                height: 512,
                init_image: Some(RawImageInput {
                    data: vec![7, 8, 9, 10, 11, 12],
                    width: 1,
                    height: 2,
                    channels: 3,
                }),
                options: Default::default(),
            },
            backend: DiffusionVideoBackend::Ggml(GgmlDiffusionVideoParams {
                video_frames: Some(24),
                fps: Some(12.0),
                cfg_scale: Some(7.5),
                guidance: Some(4.0),
                steps: Some(32),
                seed: Some(99),
                sample_method: Some("euler".to_owned()),
                scheduler: Some("normal".to_owned()),
                strength: Some(0.65),
            }),
        };

        let proto = encode_diffusion_video_request("demo-model", &request);
        let roundtrip = decode_diffusion_video_request(&proto).unwrap();

        assert_eq!(roundtrip, request);
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
    fn diffusion_image_decode_prefers_actual_image_metadata() {
        let png = make_png_bytes();
        let proto = pb::ImageResponse {
            images_json: serde_json::to_vec(&ProtoImagePayload {
                images: vec![ProtoImageEntry {
                    image: base64::engine::general_purpose::STANDARD.encode(&png),
                    width: 999,
                    height: 888,
                    channels: 4,
                }],
            })
            .unwrap(),
        };

        let roundtrip = decode_diffusion_image_response(&proto).unwrap();

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
