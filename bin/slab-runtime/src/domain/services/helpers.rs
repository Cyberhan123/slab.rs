use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use bytemuck::cast_slice;
use image::{DynamicImage, GenericImageView, imageops::FilterType};
use serde_json::{Map, Value, json};
use slab_diffusion::Image as DiffusionImage;
use slab_runtime_core::backend::StreamChunk;
use slab_runtime_core::scheduler::CpuStage;
use slab_runtime_core::{CoreError, Payload};
use slab_types::{Capability, ModelFamily, ModelSource, ModelSpec};

use slab_proto::convert::dto;

pub(crate) fn invalid_model(field: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::InvalidModelSpec { message: format!("{field}: {}", message.into()) }
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

pub(crate) fn model_spec(
    family: ModelFamily,
    capability: Capability,
    model_path: PathBuf,
) -> ModelSpec {
    ModelSpec::new(family, capability, ModelSource::LocalPath { path: model_path })
}

pub(crate) fn decode_text_response(
    payload: Payload,
    task_kind: &'static str,
) -> Result<dto::LlamaChatResponse, CoreError> {
    match payload {
        Payload::Bytes(bytes) => Ok(dto::LlamaChatResponse {
            text: Some(String::from_utf8_lossy(&bytes).into_owned()),
            ..Default::default()
        }),
        Payload::Text(text) => {
            Ok(dto::LlamaChatResponse { text: Some(text.to_string()), ..Default::default() })
        }
        Payload::Json(value) => Ok(dto::LlamaChatResponse {
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
            reasoning_content: value
                .get("reasoning_content")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .or_else(|| {
                    value
                        .get("metadata")
                        .and_then(Value::as_object)
                        .and_then(|metadata| metadata.get("reasoning_content"))
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                }),
        }),
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
        StreamChunk::Json(value) => Ok(Some(dto::LlamaChatStreamChunk {
            delta: value.get("delta").and_then(Value::as_str).map(ToOwned::to_owned),
            done: value.get("done").and_then(Value::as_bool),
            finish_reason: value
                .get("finish_reason")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            usage: value.get("usage").map(decode_usage_value),
            reasoning_content: value
                .get("reasoning_content")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .or_else(|| {
                    value
                        .get("metadata")
                        .and_then(Value::as_object)
                        .and_then(|metadata| metadata.get("reasoning_content"))
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                }),
        })),
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

pub(crate) fn decode_utf8_payload(
    payload: Payload,
    task_kind: &'static str,
) -> Result<String, CoreError> {
    match payload {
        Payload::Bytes(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Payload::Text(text) => Ok(text.to_string()),
        Payload::Json(value) => {
            serde_json::to_string(&value).map_err(|error| CoreError::ResultDecodeFailed {
                task_kind: task_kind.to_owned(),
                message: format!("failed to serialize JSON payload: {error}"),
            })
        }
        other => Err(CoreError::ResultDecodeFailed {
            task_kind: task_kind.to_owned(),
            message: format!("unsupported payload for utf8 decode: {other:?}"),
        }),
    }
}

pub(crate) fn decode_images_payload(
    payload: Payload,
    task_kind: &'static str,
) -> Result<Vec<dto::RawImage>, CoreError> {
    let images: Vec<DiffusionImage> =
        payload.to_typed().map_err(|error| CoreError::ResultDecodeFailed {
            task_kind: task_kind.to_owned(),
            message: format!("invalid diffusion image payload: {error}"),
        })?;
    Ok(images.iter().map(diffusion_image_to_raw_image).collect())
}

pub(crate) fn diffusion_image_to_raw_image(image: &DiffusionImage) -> dto::RawImage {
    dto::RawImage {
        data: image.data.clone(),
        width: Some(image.width),
        height: Some(image.height),
        channels: Some(image.channel),
    }
}

pub(crate) fn raw_image_to_diffusion_image(
    image: &dto::RawImage,
    task_kind: &'static str,
) -> Result<DiffusionImage, CoreError> {
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
    if channels == 0 {
        return Err(CoreError::ResultDecodeFailed {
            task_kind: task_kind.to_owned(),
            message: "raw image channels must be >= 1".to_owned(),
        });
    }

    Ok(DiffusionImage { width, height, channel: channels, data: image.data.clone() })
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

pub(crate) fn onnx_tensors_to_json(tensors: &[dto::RawTensor]) -> Result<Value, CoreError> {
    let inputs = tensors
        .iter()
        .map(|tensor| {
            let name = required_string("onnx.inputs[].name", tensor.name.clone())?;
            let dtype = required_string("onnx.inputs[].dtype", tensor.dtype.clone())?;
            Ok((
                name,
                json!({
                    "shape": tensor.shape,
                    "dtype": dtype,
                    "data_b64": base64::engine::general_purpose::STANDARD.encode(&tensor.data),
                }),
            ))
        })
        .collect::<Result<Map<String, Value>, CoreError>>()?;

    Ok(json!({ "inputs": inputs }))
}

pub(crate) fn onnx_outputs_from_payload(
    payload: Payload,
) -> Result<Vec<dto::RawTensor>, CoreError> {
    let value = match payload {
        Payload::Json(value) => value,
        Payload::Bytes(bytes) => {
            serde_json::from_slice(&bytes).map_err(|error| CoreError::ResultDecodeFailed {
                task_kind: "onnx".to_owned(),
                message: format!("failed to parse ONNX JSON output: {error}"),
            })?
        }
        other => {
            return Err(CoreError::ResultDecodeFailed {
                task_kind: "onnx".to_owned(),
                message: format!("unsupported ONNX output payload: {other:?}"),
            });
        }
    };

    let outputs = value.get("outputs").and_then(Value::as_object).ok_or_else(|| {
        CoreError::ResultDecodeFailed {
            task_kind: "onnx".to_owned(),
            message: "ONNX output payload is missing `outputs`".to_owned(),
        }
    })?;

    outputs.iter().map(|(name, tensor)| decode_onnx_tensor(name, tensor)).collect()
}

pub(crate) fn onnx_named_output_from_payload(
    payload: Payload,
    output_name: &str,
) -> Result<dto::RawTensor, CoreError> {
    let outputs = onnx_outputs_from_payload(payload)?;
    outputs.into_iter().find(|tensor| tensor.name.as_deref() == Some(output_name)).ok_or_else(
        || CoreError::ResultDecodeFailed {
            task_kind: "onnx_embedding".to_owned(),
            message: format!("ONNX output tensor `{output_name}` not found"),
        },
    )
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

fn decode_onnx_tensor(name: &str, value: &Value) -> Result<dto::RawTensor, CoreError> {
    let shape = value
        .get("shape")
        .and_then(Value::as_array)
        .ok_or_else(|| CoreError::ResultDecodeFailed {
            task_kind: "onnx".to_owned(),
            message: format!("tensor `{name}` is missing `shape`"),
        })?
        .iter()
        .filter_map(Value::as_i64)
        .collect::<Vec<_>>();
    let dtype = value.get("dtype").and_then(Value::as_str).ok_or_else(|| {
        CoreError::ResultDecodeFailed {
            task_kind: "onnx".to_owned(),
            message: format!("tensor `{name}` is missing `dtype`"),
        }
    })?;
    let data_b64 = value.get("data_b64").and_then(Value::as_str).ok_or_else(|| {
        CoreError::ResultDecodeFailed {
            task_kind: "onnx".to_owned(),
            message: format!("tensor `{name}` is missing `data_b64`"),
        }
    })?;
    let data = base64::engine::general_purpose::STANDARD.decode(data_b64).map_err(|error| {
        CoreError::ResultDecodeFailed {
            task_kind: "onnx".to_owned(),
            message: format!("failed to decode tensor `{name}` bytes: {error}"),
        }
    })?;

    Ok(dto::RawTensor { name: Some(name.to_owned()), shape, dtype: Some(dtype.to_owned()), data })
}
