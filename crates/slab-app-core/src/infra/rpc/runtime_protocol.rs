use std::io::Cursor;

use image::{DynamicImage, ImageFormat};

use crate::domain::models::TimedTextSegment;
use crate::domain::ports::{
    RuntimeBackendStatus, RuntimeDiffusionImageRequest, RuntimeDiffusionImageResult,
    RuntimeDiffusionVideoRequest, RuntimeDiffusionVideoResult, RuntimeGeneratedFrame,
    RuntimeGeneratedImage, RuntimeJsonOptions, RuntimeRawImageInput, RuntimeTextGenerationChunk,
    RuntimeTextGenerationRequest, RuntimeTextGenerationResponse, RuntimeTextGenerationUsage,
    RuntimeTextPromptTokensDetails, RuntimeTranscriptionResult,
};
use slab_types::RuntimeBackendId;

use super::codec::RpcCodecError;
use super::pb;

const REASONING_CONTENT_METADATA_KEY: &str = "reasoning_content";
const STOP_METADATA_KEY: &str = "stop";
const TOKEN_ID_METADATA_KEY: &str = "token_id";
const TOKEN_TEXT_METADATA_KEY: &str = "token_text";
const TOKEN_KIND_METADATA_KEY: &str = "token_kind";

pub fn encode_chat_request(request: &RuntimeTextGenerationRequest) -> pb::GgmlLlamaChatRequest {
    let prompt = merged_prompt(request);

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
        agent_trace_json: request
            .agent_trace
            .as_ref()
            .and_then(|context| serde_json::to_string(context).ok()),
    }
}

pub fn encode_candle_chat_request(request: &RuntimeTextGenerationRequest) -> pb::CandleChatRequest {
    pb::CandleChatRequest {
        prompt: Some(merged_prompt(request)),
        max_tokens: request.max_tokens,
        session_key: request.session_key.clone(),
    }
}

pub fn decode_chat_response(response: &pb::GgmlLlamaChatResponse) -> RuntimeTextGenerationResponse {
    let metadata =
        decode_chat_metadata(response.metadata.as_ref(), response.reasoning_content.as_deref());

    RuntimeTextGenerationResponse {
        text: response.text.clone().unwrap_or_default(),
        finish_reason: response.finish_reason.clone(),
        tokens_used: response.tokens_used,
        usage: response.usage.as_ref().map(decode_usage),
        metadata,
    }
}

pub fn decode_candle_chat_response(
    response: &pb::CandleChatResponse,
) -> RuntimeTextGenerationResponse {
    let metadata =
        decode_chat_metadata(response.metadata.as_ref(), response.reasoning_content.as_deref());

    RuntimeTextGenerationResponse {
        text: response.text.clone().unwrap_or_default(),
        finish_reason: response.finish_reason.clone(),
        tokens_used: response.tokens_used,
        usage: response.usage.as_ref().map(decode_usage),
        metadata,
    }
}

pub fn decode_chat_stream_chunk(
    chunk: &pb::GgmlLlamaChatStreamChunk,
) -> RuntimeTextGenerationChunk {
    let metadata =
        decode_chat_metadata(chunk.metadata.as_ref(), chunk.reasoning_content.as_deref());

    RuntimeTextGenerationChunk {
        delta: chunk.delta.clone().unwrap_or_default(),
        done: chunk.done.unwrap_or_default(),
        finish_reason: chunk.finish_reason.clone(),
        usage: chunk.usage.as_ref().map(decode_usage),
        metadata,
    }
}

pub fn decode_candle_chat_stream_chunk(
    chunk: &pb::CandleChatStreamChunk,
) -> RuntimeTextGenerationChunk {
    let metadata =
        decode_chat_metadata(chunk.metadata.as_ref(), chunk.reasoning_content.as_deref());

    RuntimeTextGenerationChunk {
        delta: chunk.delta.clone().unwrap_or_default(),
        done: chunk.done.unwrap_or_default(),
        finish_reason: chunk.finish_reason.clone(),
        usage: chunk.usage.as_ref().map(decode_usage),
        metadata,
    }
}

pub fn encode_diffusion_image_request(
    request: &RuntimeDiffusionImageRequest,
) -> pb::GgmlDiffusionGenerateImageRequest {
    pb::GgmlDiffusionGenerateImageRequest {
        prompt: Some(request.prompt.clone()),
        negative_prompt: non_empty_string(request.negative_prompt.as_deref()),
        width: Some(request.width),
        height: Some(request.height),
        init_image: request.init_image.as_ref().map(raw_image_input_to_proto),
        count: request.count,
        cfg_scale: request.cfg_scale,
        guidance: request.guidance,
        sample_steps: request.steps,
        seed: request.seed,
        sample_method: non_empty_string(request.sample_method.as_deref()),
        scheduler: non_empty_string(request.scheduler.as_deref()),
        clip_skip: request.clip_skip,
        strength: request.strength,
        eta: request.eta,
    }
}

pub fn encode_candle_diffusion_image_request(
    request: &RuntimeDiffusionImageRequest,
) -> pb::CandleDiffusionGenerateImageRequest {
    pb::CandleDiffusionGenerateImageRequest {
        prompt: Some(request.prompt.clone()),
        negative_prompt: non_empty_string(request.negative_prompt.as_deref()),
        width: Some(request.width),
        height: Some(request.height),
        batch_count: request.count,
        sample_steps: request.steps,
        guidance_scale: request.guidance.or(request.cfg_scale),
        seed: request.seed,
    }
}

pub fn encode_diffusion_video_request(
    request: &RuntimeDiffusionVideoRequest,
) -> pb::GgmlDiffusionGenerateVideoRequest {
    pb::GgmlDiffusionGenerateVideoRequest {
        prompt: Some(request.prompt.clone()),
        negative_prompt: non_empty_string(request.negative_prompt.as_deref()),
        width: Some(request.width),
        height: Some(request.height),
        init_image: request.init_image.as_ref().map(raw_image_input_to_proto),
        video_frames: request.video_frames.and_then(|value| u32::try_from(value).ok()),
        fps: request.fps,
        cfg_scale: request.cfg_scale,
        guidance: request.guidance,
        sample_steps: request.steps,
        seed: request.seed,
        sample_method: non_empty_string(request.sample_method.as_deref()),
        scheduler: non_empty_string(request.scheduler.as_deref()),
        strength: request.strength,
    }
}

pub fn decode_diffusion_image_response(
    response: &pb::GgmlDiffusionGenerateImageResponse,
) -> Result<RuntimeDiffusionImageResult, RpcCodecError> {
    let images = response
        .images
        .iter()
        .map(|image| {
            Ok(RuntimeGeneratedImage {
                bytes: raw_image_to_png_bytes(image)?,
                width: required_u32(image.width, "images[].width")?,
                height: required_u32(image.height, "images[].height")?,
                channels: required_u8(image.channels, "images[].channels")?,
            })
        })
        .collect::<Result<Vec<_>, RpcCodecError>>()?;

    Ok(RuntimeDiffusionImageResult { images, metadata: RuntimeJsonOptions::default() })
}

pub fn decode_candle_diffusion_image_response(
    response: &pb::CandleDiffusionGenerateImageResponse,
) -> Result<RuntimeDiffusionImageResult, RpcCodecError> {
    let images = response
        .images
        .iter()
        .map(|image| {
            Ok(RuntimeGeneratedImage {
                bytes: raw_image_to_png_bytes(image)?,
                width: required_u32(image.width, "images[].width")?,
                height: required_u32(image.height, "images[].height")?,
                channels: required_u8(image.channels, "images[].channels")?,
            })
        })
        .collect::<Result<Vec<_>, RpcCodecError>>()?;

    Ok(RuntimeDiffusionImageResult { images, metadata: RuntimeJsonOptions::default() })
}

pub fn decode_diffusion_video_response(
    response: &pb::GgmlDiffusionGenerateVideoResponse,
) -> Result<RuntimeDiffusionVideoResult, RpcCodecError> {
    let frames = response
        .frames
        .iter()
        .map(|frame| {
            Ok(RuntimeGeneratedFrame {
                data: frame.data.clone(),
                width: required_u32(frame.width, "frames[].width")?,
                height: required_u32(frame.height, "frames[].height")?,
                channels: required_u8(frame.channels, "frames[].channels")?,
            })
        })
        .collect::<Result<Vec<_>, RpcCodecError>>()?;

    Ok(RuntimeDiffusionVideoResult { frames, metadata: RuntimeJsonOptions::default() })
}

pub fn decode_whisper_transcription_response(
    response: &pb::GgmlWhisperTranscribeResponse,
) -> RuntimeTranscriptionResult {
    RuntimeTranscriptionResult {
        text: response
            .transcription
            .as_ref()
            .and_then(|transcription| transcription.raw_text.clone())
            .unwrap_or_default(),
        segments: response
            .transcription
            .as_ref()
            .map(|transcription| {
                transcription
                    .segments
                    .iter()
                    .map(|segment| TimedTextSegment {
                        start_ms: segment.start_ms,
                        end_ms: segment.end_ms,
                        text: segment.text.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

pub fn decode_candle_whisper_transcription_response(
    response: &pb::CandleWhisperTranscribeResponse,
) -> RuntimeTranscriptionResult {
    RuntimeTranscriptionResult {
        text: response
            .transcription
            .as_ref()
            .and_then(|transcription| transcription.raw_text.clone())
            .unwrap_or_default(),
        segments: response
            .transcription
            .as_ref()
            .map(|transcription| {
                transcription
                    .segments
                    .iter()
                    .map(|segment| TimedTextSegment {
                        start_ms: segment.start_ms,
                        end_ms: segment.end_ms,
                        text: segment.text.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

pub fn decode_model_status_response(
    response: &pb::ModelStatusResponse,
) -> Result<RuntimeBackendStatus, RpcCodecError> {
    let backend = response.backend.parse::<RuntimeBackendId>().map_err(|error| {
        RpcCodecError::InvalidField { field: "backend", message: error.to_string() }
    })?;

    Ok(RuntimeBackendStatus {
        backend,
        status: response.status.clone(),
        context_length: response.context_length,
        training_context_length: response.training_context_length,
    })
}

fn decode_usage(usage: &pb::Usage) -> RuntimeTextGenerationUsage {
    RuntimeTextGenerationUsage {
        prompt_tokens: usage.prompt_tokens.unwrap_or_default(),
        completion_tokens: usage.completion_tokens.unwrap_or_default(),
        total_tokens: usage.total_tokens.unwrap_or_default(),
        prompt_tokens_details: RuntimeTextPromptTokensDetails {
            cached_tokens: usage.prompt_cached_tokens.unwrap_or_default(),
        },
        estimated: usage.estimated.unwrap_or_default(),
    }
}

fn merged_prompt(request: &RuntimeTextGenerationRequest) -> String {
    match request.system_prompt.as_deref() {
        Some(system_prompt) if !system_prompt.is_empty() => {
            format!("{system_prompt}\n\n{}", request.prompt)
        }
        _ => request.prompt.clone(),
    }
}

fn decode_chat_metadata(
    metadata: Option<&pb::ChatMetadata>,
    legacy_reasoning_content: Option<&str>,
) -> RuntimeJsonOptions {
    let mut decoded = RuntimeJsonOptions::default();

    if let Some(metadata) = metadata {
        insert_extra_json_metadata(&mut decoded, metadata.extra_json.as_deref());
        insert_stop_metadata(&mut decoded, metadata.stop.as_ref());
        insert_reasoning_content_metadata(&mut decoded, metadata.reasoning_content.as_deref());
    }

    if !decoded.contains_key(REASONING_CONTENT_METADATA_KEY) {
        insert_reasoning_content_metadata(&mut decoded, legacy_reasoning_content);
    }

    decoded
}

fn insert_extra_json_metadata(metadata: &mut RuntimeJsonOptions, extra_json: Option<&[u8]>) {
    let Some(extra_json) = extra_json else {
        return;
    };
    if extra_json.is_empty() {
        return;
    }
    let Ok(serde_json::Value::Object(extra)) =
        serde_json::from_slice::<serde_json::Value>(extra_json)
    else {
        return;
    };
    metadata.extend(extra);
}

fn insert_stop_metadata(metadata: &mut RuntimeJsonOptions, stop: Option<&pb::ChatStopMetadata>) {
    let Some(stop) = stop else {
        return;
    };
    let mut stop_metadata = serde_json::Map::new();
    if let Some(token_id) = stop.token_id {
        stop_metadata.insert(TOKEN_ID_METADATA_KEY.into(), serde_json::Value::from(token_id));
    }
    if let Some(token_text) = stop.token_text.as_ref().filter(|value| !value.is_empty()) {
        stop_metadata
            .insert(TOKEN_TEXT_METADATA_KEY.into(), serde_json::Value::String(token_text.clone()));
    }
    if let Some(token_kind) = stop.token_kind.as_ref().filter(|value| !value.is_empty()) {
        stop_metadata
            .insert(TOKEN_KIND_METADATA_KEY.into(), serde_json::Value::String(token_kind.clone()));
    }
    if !stop_metadata.is_empty() {
        metadata.insert(STOP_METADATA_KEY.into(), serde_json::Value::Object(stop_metadata));
    }
}

fn insert_reasoning_content_metadata(
    metadata: &mut RuntimeJsonOptions,
    reasoning_content: Option<&str>,
) {
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

fn raw_image_input_to_proto(input: &RuntimeRawImageInput) -> pb::RawImage {
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
        1 => image::ImageBuffer::<image::Luma<u8>, _>::from_raw(width, height, image.data.clone())
            .map(DynamicImage::ImageLuma8),
        2 => image::ImageBuffer::<image::LumaA<u8>, _>::from_raw(width, height, image.data.clone())
            .map(DynamicImage::ImageLumaA8),
        3 => image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(width, height, image.data.clone())
            .map(DynamicImage::ImageRgb8),
        4 => image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, image.data.clone())
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
    u8::try_from(value)
        .map_err(|error| RpcCodecError::InvalidField { field, message: error.to_string() })
}

fn non_empty_string(value: Option<&str>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty()).map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn decode_chat_response_prefers_structured_metadata() {
        let response = pb::GgmlLlamaChatResponse {
            text: Some("answer".to_owned()),
            reasoning_content: Some("legacy".to_owned()),
            metadata: Some(pb::ChatMetadata {
                reasoning_content: Some("from metadata".to_owned()),
                stop: Some(pb::ChatStopMetadata {
                    token_id: Some(42),
                    token_text: Some("</s>".to_owned()),
                    token_kind: Some("eos".to_owned()),
                }),
                extra_json: Some(
                    serde_json::to_vec(&json!({ "provider": "ggml" }))
                        .expect("extra JSON should serialize"),
                ),
            }),
            ..Default::default()
        };

        let decoded = decode_chat_response(&response);

        assert_eq!(decoded.metadata.get("reasoning_content"), Some(&json!("from metadata")));
        assert_eq!(decoded.metadata.get("provider"), Some(&json!("ggml")));
        assert_eq!(
            decoded.metadata.get("stop").and_then(|stop| stop.get("token_id")),
            Some(&json!(42))
        );
        assert_eq!(
            decoded.metadata.get("stop").and_then(|stop| stop.get("token_text")),
            Some(&json!("</s>"))
        );
    }

    #[test]
    fn decode_chat_stream_chunk_falls_back_to_legacy_reasoning() {
        let chunk = pb::GgmlLlamaChatStreamChunk {
            reasoning_content: Some("legacy reasoning".to_owned()),
            ..Default::default()
        };

        let decoded = decode_chat_stream_chunk(&chunk);

        assert_eq!(decoded.metadata.get("reasoning_content"), Some(&json!("legacy reasoning")));
    }
}
