use std::path::{Path, PathBuf};
use std::sync::Arc;

use bytemuck::cast_slice;
use image::{DynamicImage, GenericImageView, imageops::FilterType};
use serde_json::Value;
use slab_runtime_core::Payload;
use slab_runtime_core::backend::StreamChunk;

use crate::application::dtos as dto;
use crate::domain::models::{
    GeneratedImage, OnnxInferenceRequest, OnnxTensor, TextGenerationMetadata,
    TextGenerationResponse, TextGenerationStreamEvent,
};
use crate::domain::runtime::{CoreError, CpuStage};

pub(crate) fn invalid_model(field: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::InvalidRequestPayload { message: format!("{field}: {}", message.into()) }
}

pub(crate) fn required_path(
    field: &'static str,
    value: Option<PathBuf>,
) -> Result<PathBuf, CoreError> {
    let Some(path) = value else {
        return Err(invalid_model(field, "missing required path"));
    };
    if path.as_os_str().is_empty() {
        return Err(invalid_model(field, "path must not be empty"));
    }
    Ok(path)
}

pub(crate) fn required_string(
    field: &'static str,
    value: Option<String>,
) -> Result<String, CoreError> {
    let Some(value) = value else {
        return Err(invalid_model(field, "missing required string"));
    };
    Ok(value)
}

fn dto_chat_metadata_from_contract(metadata: &TextGenerationMetadata) -> Option<dto::ChatMetadata> {
    let extra_json =
        if metadata.extra.is_empty() { None } else { serde_json::to_vec(&metadata.extra).ok() };
    dto::optional_chat_metadata(dto::ChatMetadata {
        reasoning_content: metadata.reasoning_content.clone(),
        stop: metadata.stop.as_ref().map(|stop| dto::ChatStopMetadata {
            token_id: stop.token_id,
            token_text: stop.token_text.clone(),
            token_kind: stop.token_kind.clone(),
        }),
        extra_json,
    })
}

fn dto_chat_metadata_from_json(value: &Value) -> Option<dto::ChatMetadata> {
    let metadata = value.get("metadata").and_then(Value::as_object);
    let reasoning_content =
        value.get("reasoning_content").and_then(Value::as_str).map(ToOwned::to_owned).or_else(
            || {
                metadata
                    .and_then(|metadata| metadata.get("reasoning_content"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            },
        );
    let stop =
        metadata.and_then(|metadata| metadata.get("stop")).and_then(Value::as_object).map(|stop| {
            dto::ChatStopMetadata {
                token_id: stop
                    .get("token_id")
                    .and_then(Value::as_i64)
                    .and_then(|value| i32::try_from(value).ok()),
                token_text: stop.get("token_text").and_then(Value::as_str).map(ToOwned::to_owned),
                token_kind: stop.get("token_kind").and_then(Value::as_str).map(ToOwned::to_owned),
            }
        });
    let extra_json = metadata
        .and_then(|metadata| metadata.get("extra"))
        .and_then(|extra| serde_json::to_vec(extra).ok());

    dto::optional_chat_metadata(dto::ChatMetadata { reasoning_content, stop, extra_json })
}

pub(crate) fn decode_text_response(
    payload: Payload,
    task_kind: &'static str,
) -> Result<dto::LlamaChatResponse, CoreError> {
    match payload {
        typed_payload @ Payload::Typed(_) => {
            let response: TextGenerationResponse =
                typed_payload.to_typed().map_err(|error| CoreError::ResultDecodeFailed {
                    task_kind: task_kind.to_owned(),
                    message: format!("invalid typed text response: {error}"),
                })?;
            let metadata = dto_chat_metadata_from_contract(&response.metadata);
            Ok(dto::LlamaChatResponse {
                text: Some(response.text),
                finish_reason: response.finish_reason,
                tokens_used: response.usage.as_ref().map(|usage| usage.completion_tokens),
                usage: response.usage.as_ref().map(decode_usage_contract),
                reasoning_content: metadata
                    .as_ref()
                    .and_then(|metadata| metadata.reasoning_content.clone()),
                metadata,
            })
        }
        Payload::Bytes(bytes) => Ok(dto::LlamaChatResponse {
            text: Some(String::from_utf8_lossy(&bytes).into_owned()),
            ..Default::default()
        }),
        Payload::Text(text) => {
            Ok(dto::LlamaChatResponse { text: Some(text.to_string()), ..Default::default() })
        }
        Payload::Json(value) => {
            let metadata = dto_chat_metadata_from_json(&value);
            Ok(dto::LlamaChatResponse {
                text: value.get("text").and_then(Value::as_str).map(ToOwned::to_owned),
                finish_reason: value
                    .get("finish_reason")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                tokens_used: value
                    .get("tokens_used")
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok()),
                usage: value.get("usage").map(decode_usage_value),
                reasoning_content: metadata
                    .as_ref()
                    .and_then(|metadata| metadata.reasoning_content.clone()),
                metadata,
            })
        }
        other => Err(CoreError::ResultDecodeFailed {
            task_kind: task_kind.to_owned(),
            message: format!("unsupported payload for text response: {other:?}"),
        }),
    }
}

pub(crate) fn decode_text_stream_chunk(
    chunk: StreamChunk,
    task_kind: &'static str,
) -> Result<Option<dto::LlamaChatStreamChunk>, CoreError> {
    match chunk {
        StreamChunk::Token(delta) => Ok(Some(dto::LlamaChatStreamChunk {
            delta: Some(delta),
            done: Some(false),
            ..Default::default()
        })),
        StreamChunk::Json(value) => {
            if let Ok(event) = serde_json::from_value::<TextGenerationStreamEvent>(value.clone()) {
                let metadata = event.metadata.as_ref().and_then(dto_chat_metadata_from_contract);
                return Ok(Some(dto::LlamaChatStreamChunk {
                    delta: event.delta,
                    done: event.done,
                    finish_reason: event.finish_reason,
                    usage: event.usage.as_ref().map(decode_usage_contract),
                    reasoning_content: metadata
                        .as_ref()
                        .and_then(|metadata| metadata.reasoning_content.clone()),
                    metadata,
                }));
            }

            let metadata = dto_chat_metadata_from_json(&value);
            Ok(Some(dto::LlamaChatStreamChunk {
                delta: value.get("delta").and_then(Value::as_str).map(ToOwned::to_owned),
                done: value.get("done").and_then(Value::as_bool),
                finish_reason: value
                    .get("finish_reason")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                usage: value.get("usage").map(decode_usage_value),
                reasoning_content: metadata
                    .as_ref()
                    .and_then(|metadata| metadata.reasoning_content.clone()),
                metadata,
            }))
        }
        StreamChunk::Done => Ok(None),
        StreamChunk::Error(message) => {
            Err(CoreError::ResultDecodeFailed { task_kind: task_kind.to_owned(), message })
        }
        StreamChunk::Image(_) => Err(CoreError::ResultDecodeFailed {
            task_kind: task_kind.to_owned(),
            message: "unexpected image chunk on text stream".to_owned(),
        }),
    }
}

pub(crate) fn raw_image_to_generated_image(
    image: &dto::RawImage,
    task_kind: &'static str,
) -> Result<GeneratedImage, CoreError> {
    let width = image.width.ok_or_else(|| CoreError::ResultDecodeFailed {
        task_kind: task_kind.to_owned(),
        message: "raw image width is required".to_owned(),
    })?;
    let height = image.height.ok_or_else(|| CoreError::ResultDecodeFailed {
        task_kind: task_kind.to_owned(),
        message: "raw image height is required".to_owned(),
    })?;
    let channels = image.channels.ok_or_else(|| CoreError::ResultDecodeFailed {
        task_kind: task_kind.to_owned(),
        message: "raw image channels are required".to_owned(),
    })?;

    Ok(GeneratedImage { data: image.data.clone(), width, height, channels })
}

pub(crate) fn contract_image_to_raw_image(image: &GeneratedImage) -> dto::RawImage {
    dto::RawImage {
        data: image.data.clone(),
        width: Some(image.width),
        height: Some(image.height),
        channels: Some(image.channels),
    }
}

pub(crate) fn audio_decode_stage(path: PathBuf) -> CpuStage {
    CpuStage::new("audio.decode.pcm", move |_| decode_audio_path(&path).map(Payload::F32))
}

pub(crate) fn whisper_transcription_from_raw(
    raw_text: String,
    language: Option<String>,
) -> dto::WhisperTranscription {
    dto::WhisperTranscription {
        raw_text: Some(raw_text.clone()),
        language,
        segments: raw_text.lines().filter_map(parse_whisper_segment_line).collect(),
    }
}

pub(crate) fn onnx_tensors_to_request(
    tensors: &[dto::RawTensor],
) -> Result<OnnxInferenceRequest, CoreError> {
    let inputs = tensors
        .iter()
        .map(|tensor| {
            Ok(OnnxTensor {
                name: required_string("onnx.inputs[].name", tensor.name.clone())?,
                shape: tensor.shape.clone(),
                dtype: required_string("onnx.inputs[].dtype", tensor.dtype.clone())?,
                data: tensor.data.clone(),
            })
        })
        .collect::<Result<Vec<_>, CoreError>>()?;

    Ok(OnnxInferenceRequest { inputs })
}

pub(crate) fn embedding_image_to_tensor(
    image_bytes: &[u8],
    input_name: &str,
) -> Result<dto::RawTensor, CoreError> {
    let image =
        image::load_from_memory(image_bytes).map_err(|error| CoreError::ResultDecodeFailed {
            task_kind: "onnx_embedding".to_owned(),
            message: format!("failed to decode embedding image: {error}"),
        })?;
    let image: DynamicImage = image.resize_exact(224, 224, FilterType::Lanczos3);

    let mut data = Vec::with_capacity(3 * 224 * 224);
    for channel in 0..3usize {
        for y in 0..224 {
            for x in 0..224 {
                let pixel = image.get_pixel(x as u32, y as u32);
                data.push(pixel.0[channel] as f32 / 255.0);
            }
        }
    }

    let raw_bytes: Vec<u8> = data.iter().flat_map(|value| value.to_le_bytes()).collect();
    Ok(dto::RawTensor {
        name: Some(input_name.to_owned()),
        shape: vec![1, 3, 224, 224],
        dtype: Some("float32".to_owned()),
        data: raw_bytes,
    })
}

pub(crate) fn embedding_image_to_contract_tensor(
    image_bytes: &[u8],
    input_name: &str,
) -> Result<OnnxTensor, CoreError> {
    let tensor = embedding_image_to_tensor(image_bytes, input_name)?;
    raw_tensor_to_contract_tensor(&tensor)
}

fn decode_usage_value(value: &Value) -> dto::Usage {
    dto::Usage {
        prompt_tokens: value
            .get("prompt_tokens")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        completion_tokens: value
            .get("completion_tokens")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        total_tokens: value
            .get("total_tokens")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        prompt_cached_tokens: value
            .get("prompt_tokens_details")
            .and_then(Value::as_object)
            .and_then(|details| details.get("cached_tokens"))
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        estimated: value.get("estimated").and_then(Value::as_bool),
    }
}

fn decode_usage_contract(value: &crate::domain::models::TextGenerationUsage) -> dto::Usage {
    dto::Usage {
        prompt_tokens: Some(value.prompt_tokens),
        completion_tokens: Some(value.completion_tokens),
        total_tokens: Some(value.total_tokens),
        prompt_cached_tokens: Some(value.prompt_tokens_details.cached_tokens),
        estimated: Some(value.estimated),
    }
}

pub(crate) fn raw_tensor_to_contract_tensor(
    tensor: &dto::RawTensor,
) -> Result<OnnxTensor, CoreError> {
    Ok(OnnxTensor {
        name: required_string("onnx.inputs[].name", tensor.name.clone())?,
        shape: tensor.shape.clone(),
        dtype: required_string("onnx.inputs[].dtype", tensor.dtype.clone())?,
        data: tensor.data.clone(),
    })
}

pub(crate) fn contract_tensor_to_raw_tensor(tensor: OnnxTensor) -> dto::RawTensor {
    dto::RawTensor {
        name: Some(tensor.name),
        shape: tensor.shape,
        dtype: Some(tensor.dtype),
        data: tensor.data,
    }
}

fn parse_whisper_segment_line(line: &str) -> Option<dto::WhisperSegment> {
    let (timespan, text) = line.split_once(": ")?;
    let (start_ms, end_ms) = timespan.split_once(" --> ")?;
    Some(dto::WhisperSegment {
        start_ms: start_ms.parse::<u64>().ok(),
        end_ms: end_ms.parse::<u64>().ok(),
        text: Some(text.to_owned()),
    })
}

fn decode_audio_path(path: &Path) -> Result<Arc<[f32]>, CoreError> {
    let ffmpeg_bin = ffmpeg_sidecar::paths::ffmpeg_path();
    let output = std::process::Command::new(&ffmpeg_bin)
        .arg("-i")
        .arg(path)
        .args(["-vn", "-f", "f32le", "-acodec", "pcm_f32le", "-ar", "16000", "-ac", "1", "-"])
        .output()
        .map_err(|error| CoreError::EngineIo(error.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CoreError::EngineIo(format!(
            "ffmpeg failed with status {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        )));
    }

    if output.stdout.len() % std::mem::size_of::<f32>() != 0 {
        return Err(CoreError::EngineIo(format!(
            "PCM output length {} is not aligned to f32",
            output.stdout.len()
        )));
    }

    let samples: Vec<f32> = cast_slice::<u8, f32>(&output.stdout).to_vec();
    Ok(Arc::from(samples))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::{
        TextGenerationMetadata, TextGenerationResponse, TextGenerationStreamEvent,
        TextGenerationUsage, TextPromptTokensDetails,
    };

    #[test]
    fn decode_text_response_prefers_typed_contract() {
        let payload = Payload::typed(TextGenerationResponse {
            text: "hello".to_owned(),
            finish_reason: Some("stop".to_owned()),
            usage: Some(TextGenerationUsage {
                prompt_tokens: 2,
                completion_tokens: 3,
                total_tokens: 5,
                prompt_tokens_details: TextPromptTokensDetails { cached_tokens: 1 },
                estimated: false,
            }),
            metadata: TextGenerationMetadata {
                reasoning_content: Some("thinking".to_owned()),
                ..Default::default()
            },
        });

        let response = decode_text_response(payload, "ggml_llama").expect("typed response decodes");

        assert_eq!(response.text.as_deref(), Some("hello"));
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
        assert_eq!(response.tokens_used, Some(3));
        assert_eq!(response.usage.as_ref().and_then(|usage| usage.prompt_cached_tokens), Some(1));
        assert_eq!(response.reasoning_content.as_deref(), Some("thinking"));
    }

    #[test]
    fn decode_text_stream_chunk_reads_contract_event() {
        let chunk = StreamChunk::Json(
            serde_json::to_value(TextGenerationStreamEvent {
                delta: Some("he".to_owned()),
                done: Some(true),
                finish_reason: Some("stop".to_owned()),
                usage: Some(TextGenerationUsage {
                    prompt_tokens: 2,
                    completion_tokens: 3,
                    total_tokens: 5,
                    prompt_tokens_details: TextPromptTokensDetails { cached_tokens: 1 },
                    estimated: false,
                }),
                metadata: Some(TextGenerationMetadata {
                    reasoning_content: Some("chain".to_owned()),
                    ..Default::default()
                }),
            })
            .expect("event serializes"),
        );

        let decoded = decode_text_stream_chunk(chunk, "ggml_llama")
            .expect("stream event decodes")
            .expect("stream chunk should be present");

        assert_eq!(decoded.delta.as_deref(), Some("he"));
        assert_eq!(decoded.done, Some(true));
        assert_eq!(decoded.finish_reason.as_deref(), Some("stop"));
        assert_eq!(decoded.usage.as_ref().and_then(|usage| usage.prompt_cached_tokens), Some(1));
        assert_eq!(decoded.reasoning_content.as_deref(), Some("chain"));
    }
}
