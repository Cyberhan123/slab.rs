use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use image::{imageops::FilterType, DynamicImage, GenericImageView};
use serde_json::{Map, Value};

use crate::base::error::CoreError;
use crate::base::types::{Payload, StreamChunk};
use crate::inference::{
    AudioTranscriptionRequest, AudioTranscriptionResponse, ImageEmbeddingRequest,
    ImageEmbeddingResponse, ImageGenerationRequest, ImageGenerationResponse, JsonOptions,
    TextGenerationChunk, TextGenerationRequest, TextGenerationResponse,
};
use crate::internal::dispatch::ResolvedDriver;
use crate::model::{ModelFamily, ModelSpec};
use slab_types::{
    CandleDiffusionLoadConfig, CandleLlamaLoadConfig, CandleWhisperLoadConfig,
    GgmlDiffusionLoadConfig, GgmlLlamaLoadConfig, GgmlWhisperLoadConfig, OnnxLoadConfig,
};

pub(crate) fn encode_load_payload(
    spec: &ModelSpec,
    resolved: &ResolvedDriver,
) -> Result<Payload, CoreError> {
    match resolved.driver_id.as_str() {
        "ggml.llama" => encode_ggml_llama_load_payload(spec),
        "ggml.whisper" => encode_ggml_whisper_load_payload(spec),
        "ggml.diffusion" => encode_ggml_diffusion_load_payload(spec),
        "candle.llama" => encode_candle_llama_load_payload(spec),
        "candle.whisper" => encode_candle_whisper_load_payload(spec),
        "candle.diffusion" => encode_candle_diffusion_load_payload(spec),
        "onnx.text" | "onnx.embedding" => encode_onnx_load_payload(spec),
        other => Err(CoreError::DriverNotRegistered { driver_id: other.to_owned() }),
    }
}

fn encode_ggml_llama_load_payload(spec: &ModelSpec) -> Result<Payload, CoreError> {
    Ok(Payload::typed(GgmlLlamaLoadConfig {
        model_path: primary_model_path_buf(spec)?,
        num_workers: usize_option(spec, "num_workers").unwrap_or(1),
        context_length: optional_nonzero_u32_option(spec, "context_length")?,
    }))
}

fn encode_ggml_whisper_load_payload(spec: &ModelSpec) -> Result<Payload, CoreError> {
    Ok(Payload::typed(GgmlWhisperLoadConfig { model_path: primary_model_path_buf(spec)? }))
}

fn encode_ggml_diffusion_load_payload(spec: &ModelSpec) -> Result<Payload, CoreError> {
    Ok(Payload::typed(GgmlDiffusionLoadConfig {
        model_path: primary_model_path_buf(spec)?,
        diffusion_model_path: artifact_or_option_path(
            spec,
            "diffusion_model",
            "diffusion_model_path",
        ),
        vae_path: artifact_or_option_path(spec, "vae", "vae_path"),
        taesd_path: artifact_or_option_path(spec, "taesd", "taesd_path"),
        clip_l_path: artifact_or_option_path(spec, "clip_l", "clip_l_path"),
        clip_g_path: artifact_or_option_path(spec, "clip_g", "clip_g_path"),
        t5xxl_path: artifact_or_option_path(spec, "t5xxl", "t5xxl_path"),
        clip_vision_path: artifact_or_option_path(spec, "clip_vision", "clip_vision_path"),
        control_net_path: artifact_or_option_path(spec, "control_net", "control_net_path"),
        flash_attn: bool_option(spec, "flash_attn").unwrap_or(false),
        vae_device: optional_nonempty_string_option(spec, "vae_device"),
        clip_device: optional_nonempty_string_option(spec, "clip_device"),
        offload_params_to_cpu: bool_option(spec, "offload_params_to_cpu").unwrap_or(false),
        enable_mmap: bool_option(spec, "enable_mmap").unwrap_or(false),
        n_threads: optional_nonzero_i32_option(spec, "n_threads"),
    }))
}

fn encode_candle_llama_load_payload(spec: &ModelSpec) -> Result<Payload, CoreError> {
    Ok(Payload::typed(CandleLlamaLoadConfig {
        model_path: primary_model_path_buf(spec)?,
        tokenizer_path: artifact_or_option_path(spec, "tokenizer", "tokenizer_path"),
        seed: u64_option(spec, "seed").unwrap_or(0),
    }))
}

fn encode_candle_whisper_load_payload(spec: &ModelSpec) -> Result<Payload, CoreError> {
    Ok(Payload::typed(CandleWhisperLoadConfig {
        model_path: primary_model_path_buf(spec)?,
        tokenizer_path: artifact_or_option_path(spec, "tokenizer", "tokenizer_path"),
    }))
}

fn encode_candle_diffusion_load_payload(spec: &ModelSpec) -> Result<Payload, CoreError> {
    Ok(Payload::typed(CandleDiffusionLoadConfig {
        model_path: primary_model_path_buf(spec)?,
        vae_path: artifact_or_option_path(spec, "vae", "vae_path"),
        sd_version: string_option(spec, "sd_version")
            .or_else(|| spec.metadata.get("sd_version").cloned())
            .unwrap_or_else(|| "v2-1".to_owned()),
    }))
}

fn encode_onnx_load_payload(spec: &ModelSpec) -> Result<Payload, CoreError> {
    Ok(Payload::typed(OnnxLoadConfig {
        model_path: primary_model_path_buf(spec)?,
        execution_providers: execution_providers(spec),
        intra_op_num_threads: optional_nonzero_usize_option(spec, "intra_op_num_threads"),
        inter_op_num_threads: optional_nonzero_usize_option(spec, "inter_op_num_threads"),
    }))
}

pub(crate) fn encode_text_generation_request(
    request: &TextGenerationRequest,
    resolved: &ResolvedDriver,
) -> Result<(Payload, Payload), CoreError> {
    let input = if matches!(resolved.family, ModelFamily::Onnx) {
        let value: Value = serde_json::from_str(&request.prompt).map_err(|error| {
            CoreError::ResultDecodeFailed {
                task_kind: "text_generation".to_owned(),
                message: format!("ONNX text generation prompt must be JSON tensor input: {error}"),
            }
        })?;
        Payload::json(value)
    } else {
        Payload::text(match &request.system_prompt {
            Some(system_prompt) if !system_prompt.is_empty() => {
                format!("{system_prompt}\n\n{}", request.prompt)
            }
            _ => request.prompt.clone(),
        })
    };

    let mut options = map_from_json_options(&request.options);
    insert_option(&mut options, "max_tokens", request.max_tokens);
    insert_option(&mut options, "temperature", request.temperature);
    insert_option(&mut options, "top_p", request.top_p);
    insert_option(&mut options, "session_key", request.session_key.clone());
    insert_option(&mut options, "stream", request.stream);

    // Pass structured chat messages and the template flag to the backend so
    // it can apply the model's embedded chat template instead of relying on
    // the server-side pre-rendered prompt.
    if request.apply_chat_template && !request.chat_messages.is_empty() {
        insert_option(&mut options, "apply_chat_template", true);
        options.insert(
            "chat_messages".to_owned(),
            serde_json::to_value(&request.chat_messages)
                .unwrap_or_else(|_| Value::Array(Vec::new())),
        );
    }

    // Transport grammar constraint fields to the backend.
    insert_option(&mut options, "grammar", request.grammar.clone());
    if request.grammar_json {
        insert_option(&mut options, "grammar_json", true);
    }
    if request.grammar_tool_call {
        insert_option(&mut options, "grammar_tool_call", true);
    }

    Ok((input, Payload::Json(Value::Object(options))))
}

pub(crate) fn decode_text_generation_response(
    payload: Payload,
) -> Result<TextGenerationResponse, CoreError> {
    match payload {
        Payload::Bytes(bytes) => Ok(TextGenerationResponse {
            text: String::from_utf8_lossy(&bytes).into_owned(),
            finish_reason: None,
            tokens_used: None,
            usage: None,
            metadata: BTreeMap::new(),
        }),
        Payload::Text(text) => Ok(TextGenerationResponse {
            text: text.to_string(),
            finish_reason: None,
            tokens_used: None,
            usage: None,
            metadata: BTreeMap::new(),
        }),
        Payload::Json(value) => Ok(TextGenerationResponse {
            text: value
                .get("text")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| serde_json::to_string(&value).unwrap_or_default()),
            finish_reason: value
                .get("finish_reason")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            tokens_used: value
                .get("tokens_used")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok()),
            usage: value.get("usage").cloned().and_then(|usage| serde_json::from_value(usage).ok()),
            metadata: BTreeMap::new(),
        }),
        other => Err(CoreError::ResultDecodeFailed {
            task_kind: "text_generation".to_owned(),
            message: format!("unsupported payload for text response: {other:?}"),
        }),
    }
}

pub(crate) fn decode_text_generation_chunk(
    chunk: StreamChunk,
) -> Result<Option<TextGenerationChunk>, CoreError> {
    match chunk {
        StreamChunk::Token(delta) => Ok(Some(TextGenerationChunk {
            delta,
            done: false,
            finish_reason: None,
            usage: None,
            metadata: BTreeMap::new(),
        })),
        StreamChunk::Done => Ok(None),
        StreamChunk::Error(message) => {
            Err(CoreError::ResultDecodeFailed { task_kind: "text_generation".to_owned(), message })
        }
        StreamChunk::Image(_) => Err(CoreError::ResultDecodeFailed {
            task_kind: "text_generation".to_owned(),
            message: "unexpected image chunk on text stream".to_owned(),
        }),
    }
}

pub(crate) fn encode_audio_transcription_options(request: &AudioTranscriptionRequest) -> Payload {
    let mut options = map_from_json_options(&request.options);
    insert_option(&mut options, "language", request.language.clone());
    insert_option(&mut options, "prompt", request.prompt.clone());
    Payload::Json(Value::Object(options))
}

pub(crate) fn decode_audio_transcription_response(
    payload: Payload,
    fallback_language: Option<String>,
) -> Result<AudioTranscriptionResponse, CoreError> {
    match payload {
        Payload::Bytes(bytes) => Ok(AudioTranscriptionResponse {
            text: String::from_utf8_lossy(&bytes).into_owned(),
            language: fallback_language,
            metadata: BTreeMap::new(),
        }),
        Payload::Text(text) => Ok(AudioTranscriptionResponse {
            text: text.to_string(),
            language: fallback_language,
            metadata: BTreeMap::new(),
        }),
        Payload::Json(value) => Ok(AudioTranscriptionResponse {
            text: value.get("text").and_then(Value::as_str).unwrap_or_default().to_owned(),
            language: value
                .get("language")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .or(fallback_language),
            metadata: BTreeMap::new(),
        }),
        other => Err(CoreError::ResultDecodeFailed {
            task_kind: "audio_transcription".to_owned(),
            message: format!("unsupported payload for audio response: {other:?}"),
        }),
    }
}

pub(crate) fn encode_image_generation_request(
    request: &ImageGenerationRequest,
    resolved: &ResolvedDriver,
) -> Result<(Payload, Payload), CoreError> {
    let mut object = map_from_json_options(&request.options);
    insert_option(&mut object, "prompt", request.prompt.clone());
    insert_option(
        &mut object,
        "negative_prompt",
        request.negative_prompt.clone().unwrap_or_default(),
    );
    insert_option(&mut object, "width", request.width);
    insert_option(&mut object, "height", request.height);
    insert_option(&mut object, "sample_steps", request.steps);
    insert_option(&mut object, "seed", request.seed.unwrap_or(-1));

    match resolved.driver_id.as_str() {
        "candle.diffusion" => {
            if !object.contains_key("cfg_scale") {
                insert_option(&mut object, "cfg_scale", request.guidance.map(f64::from));
            }
        }
        _ => {
            if !object.contains_key("cfg_scale") {
                insert_option(&mut object, "cfg_scale", request.guidance);
            }
            if !object.contains_key("guidance") {
                insert_option(&mut object, "guidance", request.guidance);
            }
        }
    }

    Ok((Payload::Json(Value::Object(object)), Payload::None))
}

pub(crate) fn decode_image_generation_response(
    payload: Payload,
) -> Result<ImageGenerationResponse, CoreError> {
    let images = match payload {
        Payload::Json(value) => decode_image_generation_json(&value)?,
        Payload::Bytes(bytes) => {
            if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                decode_image_generation_json(&value)?
            } else {
                vec![bytes.as_ref().to_vec()]
            }
        }
        other => {
            return Err(CoreError::ResultDecodeFailed {
                task_kind: "image_generation".to_owned(),
                message: format!("unsupported payload for image response: {other:?}"),
            });
        }
    };

    Ok(ImageGenerationResponse { images, metadata: BTreeMap::new() })
}

pub(crate) fn encode_image_embedding_request(
    request: &ImageEmbeddingRequest,
    input_tensor_name: &str,
) -> Result<(Payload, Payload), CoreError> {
    let tensor_json = encode_image_tensor(&request.image, input_tensor_name)?;
    Ok((Payload::Json(tensor_json), Payload::None))
}

pub(crate) fn decode_image_embedding_response(
    payload: Payload,
    output_tensor_name: &str,
) -> Result<ImageEmbeddingResponse, CoreError> {
    let value = match payload {
        Payload::Json(value) => value,
        Payload::Bytes(bytes) => {
            serde_json::from_slice(&bytes).map_err(|error| CoreError::ResultDecodeFailed {
                task_kind: "image_embedding".to_owned(),
                message: format!("failed to parse image embedding JSON response: {error}"),
            })?
        }
        other => {
            return Err(CoreError::ResultDecodeFailed {
                task_kind: "image_embedding".to_owned(),
                message: format!("unsupported payload for image embedding response: {other:?}"),
            });
        }
    };

    Ok(ImageEmbeddingResponse {
        embedding: decode_embedding_tensor(&value, output_tensor_name)?,
        metadata: BTreeMap::new(),
    })
}

pub(crate) fn image_embedding_input_name(spec: &ModelSpec) -> String {
    string_option(spec, "input_tensor_name")
        .or_else(|| spec.metadata.get("input_tensor_name").cloned())
        .unwrap_or_else(|| "input".to_owned())
}

pub(crate) fn image_embedding_output_name(spec: &ModelSpec) -> String {
    string_option(spec, "output_tensor_name")
        .or_else(|| spec.metadata.get("output_tensor_name").cloned())
        .unwrap_or_else(|| "output".to_owned())
}

fn primary_model_path_buf(spec: &ModelSpec) -> Result<PathBuf, CoreError> {
    spec.source.primary_path().map(Path::to_path_buf).ok_or_else(|| {
        CoreError::SourceResolveFailed {
            message: "model source does not expose a primary artifact".to_owned(),
        }
    })
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn artifact_or_option(spec: &ModelSpec, artifact: &str, option: &str) -> Option<String> {
    spec.source.artifact(artifact).map(path_to_string).or_else(|| string_option(spec, option))
}

fn artifact_or_option_path(spec: &ModelSpec, artifact: &str, option: &str) -> Option<PathBuf> {
    artifact_or_option(spec, artifact, option)
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

fn execution_providers(spec: &ModelSpec) -> Vec<String> {
    match spec.load_options.get("execution_providers") {
        Some(Value::Array(values)) => {
            values.iter().filter_map(Value::as_str).map(ToOwned::to_owned).collect()
        }
        Some(Value::String(value)) => value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        _ => vec!["CPU".to_owned()],
    }
}

fn map_from_json_options(options: &JsonOptions) -> Map<String, Value> {
    options.iter().map(|(key, value)| (key.clone(), value.clone())).collect()
}

fn insert_option<T>(map: &mut Map<String, Value>, key: &str, value: T)
where
    T: serde::Serialize,
{
    if let Ok(value) = serde_json::to_value(value) {
        if !value.is_null() {
            map.insert(key.to_owned(), value);
        }
    }
}

fn string_option(spec: &ModelSpec, key: &str) -> Option<String> {
    spec.load_options.get(key).and_then(|value| match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    })
}

fn optional_nonempty_string_option(spec: &ModelSpec, key: &str) -> Option<String> {
    string_option(spec, key).filter(|value| !value.trim().is_empty())
}

fn bool_option(spec: &ModelSpec, key: &str) -> Option<bool> {
    spec.load_options.get(key).and_then(Value::as_bool).or_else(|| {
        spec.load_options.get(key).and_then(Value::as_str).and_then(|value| value.parse().ok())
    })
}

fn u64_option(spec: &ModelSpec, key: &str) -> Option<u64> {
    spec.load_options.get(key).and_then(Value::as_u64).or_else(|| {
        spec.load_options.get(key).and_then(Value::as_str).and_then(|value| value.parse().ok())
    })
}

fn usize_option(spec: &ModelSpec, key: &str) -> Option<usize> {
    u64_option(spec, key).and_then(|value| usize::try_from(value).ok())
}

fn optional_nonzero_usize_option(spec: &ModelSpec, key: &str) -> Option<usize> {
    usize_option(spec, key).filter(|value| *value != 0)
}

fn i32_option(spec: &ModelSpec, key: &str) -> Option<i32> {
    spec.load_options
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .or_else(|| {
            spec.load_options.get(key).and_then(Value::as_str).and_then(|value| value.parse().ok())
        })
}

fn optional_nonzero_i32_option(spec: &ModelSpec, key: &str) -> Option<i32> {
    i32_option(spec, key).filter(|value| *value != 0)
}

fn optional_nonzero_u32_option(spec: &ModelSpec, key: &str) -> Result<Option<u32>, CoreError> {
    match u64_option(spec, key) {
        Some(0) | None => Ok(None),
        Some(value) => u32::try_from(value).map(Some).map_err(|_| CoreError::InvalidModelSpec {
            message: format!("load option `{key}` exceeds u32 range: {value}"),
        }),
    }
}

fn decode_image_generation_json(value: &Value) -> Result<Vec<Vec<u8>>, CoreError> {
    if let Some(images) = value.get("images").and_then(Value::as_array) {
        return images
            .iter()
            .map(|entry| match entry {
                Value::Object(object) => object
                    .get("image")
                    .and_then(Value::as_str)
                    .ok_or_else(|| CoreError::ResultDecodeFailed {
                        task_kind: "image_generation".to_owned(),
                        message: "image entry missing image field".to_owned(),
                    })
                    .and_then(|image| {
                        base64::engine::general_purpose::STANDARD.decode(image).map_err(|error| {
                            CoreError::ResultDecodeFailed {
                                task_kind: "image_generation".to_owned(),
                                message: format!("failed to decode image payload: {error}"),
                            }
                        })
                    }),
                _ => Err(CoreError::ResultDecodeFailed {
                    task_kind: "image_generation".to_owned(),
                    message: "unsupported image entry shape".to_owned(),
                }),
            })
            .collect();
    }

    Err(CoreError::ResultDecodeFailed {
        task_kind: "image_generation".to_owned(),
        message: "image generation response did not contain images".to_owned(),
    })
}

fn encode_image_tensor(image_bytes: &[u8], input_name: &str) -> Result<Value, CoreError> {
    let img =
        image::load_from_memory(image_bytes).map_err(|error| CoreError::ResultDecodeFailed {
            task_kind: "image_embedding".to_owned(),
            message: format!("image decode failed: {error}"),
        })?;

    let img: DynamicImage = img.resize_exact(224, 224, FilterType::Lanczos3);

    let mut data = Vec::with_capacity(3 * 224 * 224);
    for channel in 0..3usize {
        for y in 0..224 {
            for x in 0..224 {
                let pixel = img.get_pixel(x as u32, y as u32);
                data.push(pixel.0[channel] as f32 / 255.0);
            }
        }
    }

    let raw_bytes: Vec<u8> = data.iter().flat_map(|value| value.to_le_bytes()).collect();
    let data_b64 = base64::engine::general_purpose::STANDARD.encode(raw_bytes);

    Ok(serde_json::json!({
        "inputs": {
            input_name: {
                "shape": [1i64, 3i64, 224i64, 224i64],
                "dtype": "float32",
                "data_b64": data_b64,
            }
        }
    }))
}

fn decode_embedding_tensor(value: &Value, output_name: &str) -> Result<Vec<f32>, CoreError> {
    let tensor = value
        .get("outputs")
        .and_then(|outputs| outputs.get(output_name))
        .or_else(|| value.get(output_name))
        .ok_or_else(|| CoreError::ResultDecodeFailed {
            task_kind: "image_embedding".to_owned(),
            message: format!("output tensor '{output_name}' not found"),
        })?;

    let data_b64 = tensor.get("data_b64").and_then(Value::as_str).ok_or_else(|| {
        CoreError::ResultDecodeFailed {
            task_kind: "image_embedding".to_owned(),
            message: "embedding tensor missing data_b64".to_owned(),
        }
    })?;

    let raw = base64::engine::general_purpose::STANDARD.decode(data_b64).map_err(|error| {
        CoreError::ResultDecodeFailed {
            task_kind: "image_embedding".to_owned(),
            message: format!("failed to decode embedding bytes: {error}"),
        }
    })?;

    if raw.len() % 4 != 0 {
        return Err(CoreError::ResultDecodeFailed {
            task_kind: "image_embedding".to_owned(),
            message: format!("embedding tensor byte length {} is not divisible by 4", raw.len()),
        });
    }

    Ok(raw
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModelSource;
    use serde_json::json;
    use slab_types::chat::{ConversationMessage, ConversationMessageContent};
    use slab_types::runtime::Capability;
    use slab_types::{
        CandleDiffusionLoadConfig, CandleLlamaLoadConfig, CandleWhisperLoadConfig,
        GgmlDiffusionLoadConfig, GgmlLlamaLoadConfig, GgmlWhisperLoadConfig, OnnxLoadConfig,
    };
    use std::path::PathBuf;

    fn make_driver(
        driver_id: &str,
        backend_id: &str,
        family: ModelFamily,
        capability: Capability,
    ) -> ResolvedDriver {
        use crate::internal::dispatch::{DriverLoadStyle, ResolvedDriver};
        ResolvedDriver {
            driver_id: driver_id.to_owned(),
            backend_id: backend_id.to_owned(),
            family,
            capability,
            supports_streaming: true,
            load_style: DriverLoadStyle::DynamicLibraryThenModel,
        }
    }

    fn make_llama_driver() -> ResolvedDriver {
        make_driver("ggml.llama", "llama", ModelFamily::Llama, Capability::TextGeneration)
    }

    fn make_spec(family: ModelFamily, capability: Capability, model_path: &str) -> ModelSpec {
        ModelSpec::new(
            family,
            capability,
            ModelSource::LocalPath { path: PathBuf::from(model_path) },
        )
    }

    #[test]
    fn encode_load_payload_uses_typed_payload_for_ggml_llama() {
        let spec = make_spec(ModelFamily::Llama, Capability::TextGeneration, "model.gguf")
            .with_load_option("num_workers", 3)
            .with_load_option("context_length", 2048);

        let payload =
            encode_load_payload(&spec, &make_llama_driver()).expect("encode should succeed");
        let config = payload
            .to_typed::<GgmlLlamaLoadConfig>()
            .expect("ggml.llama payload should decode as typed config");

        assert_eq!(config.model_path, PathBuf::from("model.gguf"));
        assert_eq!(config.num_workers, 3);
        assert_eq!(config.context_length, Some(2048));
    }

    #[test]
    fn encode_load_payload_uses_typed_payload_for_ggml_whisper() {
        let spec = make_spec(ModelFamily::Whisper, Capability::AudioTranscription, "model.bin");
        let driver = make_driver(
            "ggml.whisper",
            "whisper",
            ModelFamily::Whisper,
            Capability::AudioTranscription,
        );

        let payload = encode_load_payload(&spec, &driver).expect("encode should succeed");
        let config = payload
            .to_typed::<GgmlWhisperLoadConfig>()
            .expect("ggml.whisper payload should decode as typed config");

        assert_eq!(config.model_path, PathBuf::from("model.bin"));
    }

    #[test]
    fn encode_load_payload_uses_typed_payload_for_all_model_load_backends() {
        let ggml_diffusion = encode_load_payload(
            &make_spec(ModelFamily::Diffusion, Capability::ImageGeneration, "model.gguf")
                .with_load_option("n_threads", 6)
                .with_load_option("vae_device", "cpu"),
            &make_driver(
                "ggml.diffusion",
                "diffusion",
                ModelFamily::Diffusion,
                Capability::ImageGeneration,
            ),
        )
        .expect("ggml.diffusion encode should succeed");
        let ggml_diffusion = ggml_diffusion
            .to_typed::<GgmlDiffusionLoadConfig>()
            .expect("ggml.diffusion payload should decode as typed config");
        assert_eq!(ggml_diffusion.model_path, PathBuf::from("model.gguf"));
        assert_eq!(ggml_diffusion.n_threads, Some(6));
        assert_eq!(ggml_diffusion.vae_device.as_deref(), Some("cpu"));

        let candle_llama = encode_load_payload(
            &make_spec(ModelFamily::Llama, Capability::TextGeneration, "model.gguf")
                .with_load_option("seed", 42)
                .with_load_option("tokenizer_path", "tokenizer.json"),
            &make_driver(
                "candle.llama",
                "candle.llama",
                ModelFamily::Llama,
                Capability::TextGeneration,
            ),
        )
        .expect("candle.llama encode should succeed");
        let candle_llama = candle_llama
            .to_typed::<CandleLlamaLoadConfig>()
            .expect("candle.llama payload should decode as typed config");
        assert_eq!(candle_llama.model_path, PathBuf::from("model.gguf"));
        assert_eq!(candle_llama.tokenizer_path, Some(PathBuf::from("tokenizer.json")));
        assert_eq!(candle_llama.seed, 42);

        let candle_whisper = encode_load_payload(
            &make_spec(ModelFamily::Whisper, Capability::AudioTranscription, "model.safetensors")
                .with_load_option("tokenizer_path", "tokenizer.json"),
            &make_driver(
                "candle.whisper",
                "candle.whisper",
                ModelFamily::Whisper,
                Capability::AudioTranscription,
            ),
        )
        .expect("candle.whisper encode should succeed");
        let candle_whisper = candle_whisper
            .to_typed::<CandleWhisperLoadConfig>()
            .expect("candle.whisper payload should decode as typed config");
        assert_eq!(candle_whisper.model_path, PathBuf::from("model.safetensors"));
        assert_eq!(candle_whisper.tokenizer_path, Some(PathBuf::from("tokenizer.json")));

        let candle_diffusion = encode_load_payload(
            &make_spec(ModelFamily::Diffusion, Capability::ImageGeneration, "model.safetensors")
                .with_load_option("sd_version", "v1-5")
                .with_load_option("vae_path", "vae.safetensors"),
            &make_driver(
                "candle.diffusion",
                "candle.diffusion",
                ModelFamily::Diffusion,
                Capability::ImageGeneration,
            ),
        )
        .expect("candle.diffusion encode should succeed");
        let candle_diffusion = candle_diffusion
            .to_typed::<CandleDiffusionLoadConfig>()
            .expect("candle.diffusion payload should decode as typed config");
        assert_eq!(candle_diffusion.model_path, PathBuf::from("model.safetensors"));
        assert_eq!(candle_diffusion.vae_path, Some(PathBuf::from("vae.safetensors")));
        assert_eq!(candle_diffusion.sd_version, "v1-5");

        let onnx = encode_load_payload(
            &make_spec(ModelFamily::Onnx, Capability::TextGeneration, "model.onnx")
                .with_load_option("execution_providers", json!(["CUDA", "CPU"]))
                .with_load_option("intra_op_num_threads", 8),
            &make_driver("onnx.text", "onnx.text", ModelFamily::Onnx, Capability::TextGeneration),
        )
        .expect("onnx.text encode should succeed");
        let onnx = onnx
            .to_typed::<OnnxLoadConfig>()
            .expect("onnx.text payload should decode as typed config");
        assert_eq!(onnx.model_path, PathBuf::from("model.onnx"));
        assert_eq!(onnx.execution_providers, vec!["CUDA".to_owned(), "CPU".to_owned()]);
        assert_eq!(onnx.intra_op_num_threads, Some(8));
        assert_eq!(onnx.inter_op_num_threads, None);
    }

    #[test]
    fn encode_text_generation_request_includes_chat_messages_when_flag_set() {
        let request = TextGenerationRequest {
            prompt: "fallback".to_owned(),
            chat_messages: vec![ConversationMessage {
                role: "user".to_owned(),
                content: ConversationMessageContent::Text("hello".to_owned()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            apply_chat_template: true,
            ..Default::default()
        };
        let driver = make_llama_driver();
        let (_input, opts_payload) =
            encode_text_generation_request(&request, &driver).expect("encode should succeed");

        let opts = match opts_payload {
            Payload::Json(Value::Object(m)) => m,
            other => panic!("expected JSON object options, got {other:?}"),
        };

        assert_eq!(
            opts.get("apply_chat_template").and_then(|v| v.as_bool()),
            Some(true),
            "options should include apply_chat_template=true"
        );
        let messages = opts.get("chat_messages").expect("options should include chat_messages");
        let arr = messages.as_array().expect("chat_messages should be an array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["role"], "user");
        assert_eq!(arr[0]["content"], "hello");
    }

    #[test]
    fn encode_text_generation_request_omits_chat_fields_when_flag_false() {
        let request = TextGenerationRequest {
            prompt: "just a prompt".to_owned(),
            chat_messages: vec![ConversationMessage {
                role: "user".to_owned(),
                content: ConversationMessageContent::Text("hi".to_owned()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            apply_chat_template: false,
            ..Default::default()
        };
        let driver = make_llama_driver();
        let (_input, opts_payload) =
            encode_text_generation_request(&request, &driver).expect("encode should succeed");

        let opts = match opts_payload {
            Payload::Json(Value::Object(m)) => m,
            other => panic!("expected JSON object options, got {other:?}"),
        };

        assert!(
            opts.get("apply_chat_template").is_none(),
            "apply_chat_template should be absent when flag is false"
        );
        assert!(
            opts.get("chat_messages").is_none(),
            "chat_messages should be absent when flag is false"
        );
    }

    #[test]
    fn encode_text_generation_request_omits_chat_fields_when_messages_empty() {
        let request = TextGenerationRequest {
            prompt: "just a prompt".to_owned(),
            chat_messages: vec![],
            apply_chat_template: true,
            ..Default::default()
        };
        let driver = make_llama_driver();
        let (_input, opts_payload) =
            encode_text_generation_request(&request, &driver).expect("encode should succeed");

        let opts = match opts_payload {
            Payload::Json(Value::Object(m)) => m,
            other => panic!("expected JSON object options, got {other:?}"),
        };

        assert!(
            opts.get("apply_chat_template").is_none(),
            "apply_chat_template should be absent when messages list is empty"
        );
        assert!(
            opts.get("chat_messages").is_none(),
            "chat_messages should be absent when messages list is empty"
        );
    }

    // ── grammar encoding ──────────────────────────────────────────────────────

    #[test]
    fn encode_text_generation_request_includes_raw_grammar() {
        let gbnf = "root ::= \"hello\"";
        let request = TextGenerationRequest {
            prompt: "hi".to_owned(),
            grammar: Some(gbnf.to_owned()),
            ..Default::default()
        };
        let driver = make_llama_driver();
        let (_input, opts_payload) =
            encode_text_generation_request(&request, &driver).expect("encode should succeed");
        let opts = match opts_payload {
            Payload::Json(Value::Object(m)) => m,
            other => panic!("expected JSON object options, got {other:?}"),
        };
        assert_eq!(
            opts.get("grammar").and_then(|v| v.as_str()),
            Some(gbnf),
            "raw grammar string should be present in options"
        );
        assert!(opts.get("grammar_json").is_none(), "grammar_json should be absent");
        assert!(opts.get("grammar_tool_call").is_none(), "grammar_tool_call should be absent");
    }

    #[test]
    fn encode_text_generation_request_includes_grammar_json_flag() {
        let request = TextGenerationRequest {
            prompt: "hi".to_owned(),
            grammar_json: true,
            ..Default::default()
        };
        let driver = make_llama_driver();
        let (_input, opts_payload) =
            encode_text_generation_request(&request, &driver).expect("encode should succeed");
        let opts = match opts_payload {
            Payload::Json(Value::Object(m)) => m,
            other => panic!("expected JSON object options, got {other:?}"),
        };
        assert_eq!(
            opts.get("grammar_json").and_then(|v| v.as_bool()),
            Some(true),
            "grammar_json flag should be present in options"
        );
    }

    #[test]
    fn encode_text_generation_request_includes_grammar_tool_call_flag() {
        let request = TextGenerationRequest {
            prompt: "hi".to_owned(),
            grammar_tool_call: true,
            ..Default::default()
        };
        let driver = make_llama_driver();
        let (_input, opts_payload) =
            encode_text_generation_request(&request, &driver).expect("encode should succeed");
        let opts = match opts_payload {
            Payload::Json(Value::Object(m)) => m,
            other => panic!("expected JSON object options, got {other:?}"),
        };
        assert_eq!(
            opts.get("grammar_tool_call").and_then(|v| v.as_bool()),
            Some(true),
            "grammar_tool_call flag should be present in options"
        );
    }

    #[test]
    fn encode_text_generation_request_grammar_flags_absent_when_not_set() {
        let request = TextGenerationRequest { prompt: "hi".to_owned(), ..Default::default() };
        let driver = make_llama_driver();
        let (_input, opts_payload) =
            encode_text_generation_request(&request, &driver).expect("encode should succeed");
        let opts = match opts_payload {
            Payload::Json(Value::Object(m)) => m,
            other => panic!("expected JSON object options, got {other:?}"),
        };
        assert!(opts.get("grammar").is_none(), "grammar should be absent when not set");
        assert!(opts.get("grammar_json").is_none(), "grammar_json should be absent when false");
        assert!(
            opts.get("grammar_tool_call").is_none(),
            "grammar_tool_call should be absent when false"
        );
    }

    #[test]
    fn decode_image_generation_response_accepts_unified_image_entries() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"mock-image");

        let response = decode_image_generation_response(Payload::Json(json!({
            "images": [{ "image": encoded }]
        })))
        .expect("image response should decode");

        assert_eq!(response.images, vec![b"mock-image".to_vec()]);
    }

    #[test]
    fn decode_image_generation_response_rejects_legacy_image_entries() {
        let error = decode_image_generation_response(Payload::Json(json!({
            "images": ["legacy-image"]
        })))
        .expect_err("legacy image entry shape should be rejected");

        match error {
            CoreError::ResultDecodeFailed { task_kind, message } => {
                assert_eq!(task_kind, "image_generation");
                assert_eq!(message, "unsupported image entry shape");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
