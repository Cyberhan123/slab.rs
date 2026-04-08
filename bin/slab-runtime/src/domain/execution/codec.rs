use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use base64::Engine as _;
use image::{DynamicImage, GenericImageView, imageops::FilterType};
use serde_json::Value;
use slab_diffusion::{
    ContextParams as DiffusionContextParams, GuidanceParams as DiffusionGuidanceParams,
    Image as DiffusionImage, ImgParams as DiffusionImgParams,
    SampleMethod as DiffusionSampleMethod, SampleParams as DiffusionSampleParams,
    Scheduler as DiffusionScheduler, SlgParams,
};
use slab_llama::{
    ChatMessage as LlamaChatMessage,
    runtime::{LlamaInferenceParams, LlamaLoadConfig, resolve_grammar as resolve_llama_grammar},
};
use slab_whisper::{
    ContextParams as WhisperContextParams, FullParams as WhisperFullParams,
    SamplingStrategy as WhisperSamplingStrategy, WhisperVadParams as CanonicalWhisperVadParams,
};
use slab_types::{
    AudioTranscriptionOpOptions, AudioTranscriptionRequest, AudioTranscriptionResponse,
    CandleDiffusionLoadConfig, CandleLlamaLoadConfig, CandleWhisperLoadConfig,
    DiffusionImageRequest, ImageEmbeddingRequest, ImageEmbeddingResponse, ImageGenerationRequest,
    ImageGenerationResponse, ModelFamily, ModelSpec, OnnxLoadConfig, TextGenerationChunk,
    TextGenerationOpOptions, TextGenerationRequest, TextGenerationResponse,
};
use slab_runtime_core::backend::StreamChunk;
use slab_runtime_core::{CoreError, Payload};

use super::backend::ResolvedBackend;

pub(crate) fn encode_load_payload(
    spec: &ModelSpec,
    resolved: &ResolvedBackend,
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
    Ok(Payload::typed(LlamaLoadConfig {
        model_path: primary_model_path_buf(spec)?,
        num_workers: usize_option(spec, "num_workers").unwrap_or(1),
        context_length: optional_nonzero_u32_option(spec, "context_length")?,
        chat_template: optional_nonempty_string_option(spec, "chat_template"),
    }))
}

fn encode_ggml_llama_text_generation_options(request: &TextGenerationRequest) -> Payload {
    Payload::typed(LlamaInferenceParams {
        max_tokens: request.max_tokens.and_then(|value| usize::try_from(value).ok()).unwrap_or(256),
        session_key: request.session_key.clone(),
        apply_chat_template: request.apply_chat_template,
        chat_messages: extract_llama_chat_messages(&request.chat_messages),
        grammar: resolve_llama_grammar(
            request.grammar.as_deref(),
            request.grammar_json,
            request.grammar_tool_call,
        ),
    })
}

fn extract_llama_chat_messages(
    messages: &[slab_types::chat::ConversationMessage],
) -> Vec<LlamaChatMessage> {
    messages
        .iter()
        .filter(|message| !message.role.trim().is_empty() && message.has_meaningful_content())
        .map(|message| LlamaChatMessage {
            role: normalize_llama_chat_role(&message.role).to_owned(),
            content: message.rendered_text(),
        })
        .collect()
}

fn normalize_llama_chat_role(role: &str) -> &'static str {
    match role {
        "system" | "developer" => "system",
        "assistant" => "assistant",
        _ => "user",
    }
}

fn encode_ggml_whisper_load_payload(spec: &ModelSpec) -> Result<Payload, CoreError> {
    Ok(Payload::typed(build_ggml_whisper_context_params(spec)?))
}

fn encode_ggml_diffusion_load_payload(spec: &ModelSpec) -> Result<Payload, CoreError> {
    Ok(Payload::typed(DiffusionContextParams {
        model_path: Some(primary_model_path_buf(spec)?),
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
        flash_attn: bool_option(spec, "flash_attn"),
        vae_device: optional_nonempty_string_option(spec, "vae_device"),
        clip_device: optional_nonempty_string_option(spec, "clip_device"),
        offload_params_to_cpu: bool_option(spec, "offload_params_to_cpu"),
        enable_mmap: bool_option(spec, "enable_mmap"),
        n_threads: optional_nonzero_i32_option(spec, "n_threads"),
        ..Default::default()
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
    resolved: &ResolvedBackend,
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

    let options = if resolved.driver_id == "ggml.llama" {
        encode_ggml_llama_text_generation_options(request)
    } else {
        Payload::typed(TextGenerationOpOptions {
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            session_key: request.session_key.clone(),
            stream: request.stream,
            apply_chat_template: request.apply_chat_template,
            chat_messages: request.chat_messages.clone(),
            grammar: request.grammar.clone(),
            grammar_json: request.grammar_json,
            grammar_tool_call: request.grammar_tool_call,
        })
    };

    Ok((input, options))
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
            metadata: value
                .get("metadata")
                .cloned()
                .and_then(|metadata| serde_json::from_value(metadata).ok())
                .unwrap_or_default(),
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
        StreamChunk::Json(value) => Ok(Some(TextGenerationChunk {
            delta: value
                .get("delta")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_default(),
            done: value.get("done").and_then(Value::as_bool).unwrap_or(false),
            finish_reason: value
                .get("finish_reason")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            usage: value.get("usage").cloned().and_then(|usage| serde_json::from_value(usage).ok()),
            metadata: value
                .get("metadata")
                .cloned()
                .and_then(|metadata| serde_json::from_value(metadata).ok())
                .unwrap_or_default(),
        })),
        StreamChunk::Error(message) => {
            Err(CoreError::ResultDecodeFailed { task_kind: "text_generation".to_owned(), message })
        }
        StreamChunk::Image(_) => Err(CoreError::ResultDecodeFailed {
            task_kind: "text_generation".to_owned(),
            message: "unexpected image chunk on text stream".to_owned(),
        }),
    }
}

pub(crate) fn encode_audio_transcription_options(
    request: &AudioTranscriptionRequest,
    resolved: &ResolvedBackend,
) -> Result<Payload, CoreError> {
    match resolved.driver_id.as_str() {
        "ggml.whisper" => Ok(Payload::typed(build_ggml_whisper_full_params(request)?)),
        _ => Ok(Payload::typed(AudioTranscriptionOpOptions {
            language: request.language.clone(),
            prompt: request.prompt.clone(),
            vad: request.vad.clone(),
            decode: request.decode.clone(),
        })),
    }
}

pub(crate) fn build_ggml_whisper_context_params(
    spec: &ModelSpec,
) -> Result<WhisperContextParams, CoreError> {
    Ok(WhisperContextParams {
        model_path: Some(primary_model_path_buf(spec)?),
        use_gpu: bool_option(spec, "use_gpu"),
        flash_attn: bool_option(spec, "flash_attn"),
        gpu_device: i32_option(spec, "gpu_device"),
        ..Default::default()
    })
}

pub(crate) fn build_ggml_whisper_full_params(
    request: &AudioTranscriptionRequest,
) -> Result<WhisperFullParams, CoreError> {
    build_ggml_whisper_full_params_from_legacy(
        request.language.clone(),
        request.prompt.clone(),
        request.vad.clone(),
        request.decode.clone(),
    )
}

pub(crate) fn build_ggml_whisper_full_params_from_legacy(
    language: Option<String>,
    prompt: Option<String>,
    vad: Option<slab_types::WhisperVadOptions>,
    decode: Option<slab_types::WhisperDecodeOptions>,
) -> Result<WhisperFullParams, CoreError> {
    let mut params = WhisperFullParams {
        strategy: WhisperSamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 },
        language: language.filter(|value| !value.trim().is_empty()),
        initial_prompt: prompt.filter(|value| !value.trim().is_empty()),
        ..Default::default()
    };

    if let Some(vad) = vad
        && vad.enabled
    {
        let model_path = vad.model_path.ok_or_else(|| {
            diffusion_param_error(
                "audio_transcription",
                "invalid whisper inference options: vad.model_path is missing",
            )
        })?;
        if model_path.as_os_str().is_empty() {
            return Err(diffusion_param_error(
                "audio_transcription",
                "invalid whisper inference options: vad.model_path is empty",
            ));
        }

        params.vad = Some(true);
        params.vad_model_path = Some(model_path);

        if let Some(vad_params) = vad.params {
            if let Some(threshold) = vad_params.threshold
                && !(0.0..=1.0).contains(&threshold)
            {
                return Err(diffusion_param_error(
                    "audio_transcription",
                    "invalid whisper inference options: vad.threshold must be between 0.0 and 1.0",
                ));
            }

            for (name, value) in [
                ("vad.min_speech_duration_ms", vad_params.min_speech_duration_ms),
                ("vad.min_silence_duration_ms", vad_params.min_silence_duration_ms),
                ("vad.speech_pad_ms", vad_params.speech_pad_ms),
            ] {
                if value.is_some_and(|value| value < 0) {
                    return Err(diffusion_param_error(
                        "audio_transcription",
                        format!("invalid whisper inference options: {name} must be >= 0"),
                    ));
                }
            }

            if let Some(max_speech_duration_s) = vad_params.max_speech_duration_s
                && max_speech_duration_s <= 0.0
            {
                return Err(diffusion_param_error(
                    "audio_transcription",
                    "invalid whisper inference options: vad.max_speech_duration_s must be > 0.0",
                ));
            }

            if let Some(samples_overlap) = vad_params.samples_overlap
                && samples_overlap < 0.0
            {
                return Err(diffusion_param_error(
                    "audio_transcription",
                    "invalid whisper inference options: vad.samples_overlap must be >= 0.0",
                ));
            }

            params.vad_params = Some(CanonicalWhisperVadParams {
                threshold: vad_params.threshold,
                min_speech_duration_ms: vad_params.min_speech_duration_ms,
                min_silence_duration_ms: vad_params.min_silence_duration_ms,
                max_speech_duration_s: vad_params.max_speech_duration_s,
                speech_pad_ms: vad_params.speech_pad_ms,
                samples_overlap: vad_params.samples_overlap,
            });
        }
    }

    if let Some(decode) = decode {
        for (name, value) in [
            ("decode.offset_ms", decode.offset_ms),
            ("decode.duration_ms", decode.duration_ms),
            ("decode.max_len", decode.max_len),
            ("decode.max_tokens", decode.max_tokens),
        ] {
            if value.is_some_and(|value| value < 0) {
                return Err(diffusion_param_error(
                    "audio_transcription",
                    format!("invalid whisper inference options: {name} must be >= 0"),
                ));
            }
        }

        if let Some(word_thold) = decode.word_thold
            && !(0.0..=1.0).contains(&word_thold)
        {
            return Err(diffusion_param_error(
                "audio_transcription",
                "invalid whisper inference options: decode.word_thold must be between 0.0 and 1.0",
            ));
        }

        for (name, value) in [
            ("decode.temperature", decode.temperature),
            ("decode.temperature_inc", decode.temperature_inc),
        ] {
            if value.is_some_and(|value| value < 0.0) {
                return Err(diffusion_param_error(
                    "audio_transcription",
                    format!("invalid whisper inference options: {name} must be >= 0.0"),
                ));
            }
        }

        params.offset_ms = decode.offset_ms;
        params.duration_ms = decode.duration_ms;
        params.no_context = decode.no_context;
        params.no_timestamps = decode.no_timestamps;
        params.token_timestamps = decode.token_timestamps;
        params.split_on_word = decode.split_on_word;
        params.suppress_nst = decode.suppress_nst;
        params.thold_pt = decode.word_thold;
        params.max_len = decode.max_len;
        params.max_tokens = decode.max_tokens;
        params.temperature = decode.temperature;
        params.temperature_inc = decode.temperature_inc;
        params.entropy_thold = decode.entropy_thold;
        params.logprob_thold = decode.logprob_thold;
        params.no_speech_thold = decode.no_speech_thold;
        params.tdrz_enable = decode.tdrz_enable;
    }

    Ok(params)
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
    _resolved: &ResolvedBackend,
) -> Result<(Payload, Payload), CoreError> {
    let input = build_diffusion_img_params(request)?;
    Ok((Payload::typed(input), Payload::None))
}

pub(crate) fn decode_image_generation_response(
    payload: Payload,
) -> Result<ImageGenerationResponse, CoreError> {
    let response: Vec<DiffusionImage> =
        payload.to_typed().map_err(|error| CoreError::ResultDecodeFailed {
            task_kind: "image_generation".to_owned(),
            message: format!("invalid typed diffusion image result: {error}"),
        })?;

    Ok(ImageGenerationResponse {
        images: response
            .iter()
            .map(diffusion_image_to_png_bytes)
            .collect::<Result<Vec<_>, CoreError>>()?,
        metadata: Default::default(),
    })
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

fn raw_image_input_to_diffusion_image(
    image: &slab_types::RawImageInput,
) -> Result<DiffusionImage, CoreError> {
    if image.channels == 0 {
        return Err(diffusion_param_error(
            "image_generation",
            "raw image input channels must be >= 1",
        ));
    }

    Ok(DiffusionImage {
        width: image.width,
        height: image.height,
        channel: u32::from(image.channels),
        data: image.data.clone(),
    })
}

fn diffusion_image_to_png_bytes(image: &DiffusionImage) -> Result<Vec<u8>, CoreError> {
    let dynamic = match image.channel {
        3 => image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(
            image.width,
            image.height,
            image.data.clone(),
        )
        .map(DynamicImage::ImageRgb8),
        4 => image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
            image.width,
            image.height,
            image.data.clone(),
        )
        .map(DynamicImage::ImageRgba8),
        other => {
            return Err(CoreError::ResultDecodeFailed {
                task_kind: "image_generation".to_owned(),
                message: format!("unsupported diffusion image channel count: {other}"),
            });
        }
    }
    .ok_or_else(|| CoreError::ResultDecodeFailed {
        task_kind: "image_generation".to_owned(),
        message: format!(
            "invalid raw diffusion image buffer for {}x{}x{}",
            image.width, image.height, image.channel
        ),
    })?;

    let mut png_bytes = Vec::new();
    dynamic.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).map_err(
        |error| CoreError::ResultDecodeFailed {
            task_kind: "image_generation".to_owned(),
            message: format!("failed to encode diffusion image as PNG: {error}"),
        },
    )?;
    Ok(png_bytes)
}

fn diffusion_param_error(task_kind: &str, message: impl Into<String>) -> CoreError {
    CoreError::ResultDecodeFailed { task_kind: task_kind.to_owned(), message: message.into() }
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

pub(crate) fn diffusion_image_request_to_params(
    request: &DiffusionImageRequest,
) -> Result<DiffusionImgParams, CoreError> {
    let sample_method = request
        .sample_method
        .as_deref()
        .map(DiffusionSampleMethod::from_str)
        .transpose()
        .map_err(|message| diffusion_param_error("image_generation", message))?;
    let scheduler = request
        .scheduler
        .as_deref()
        .map(DiffusionScheduler::from_str)
        .transpose()
        .map_err(|message| diffusion_param_error("image_generation", message))?;

    if request.count < 1 {
        return Err(diffusion_param_error("image_generation", "count must be >= 1"));
    }
    if request.width < 1 {
        return Err(diffusion_param_error("image_generation", "width must be >= 1"));
    }
    if request.height < 1 {
        return Err(diffusion_param_error("image_generation", "height must be >= 1"));
    }
    if let Some(steps) = request.steps
        && steps < 1
    {
        return Err(diffusion_param_error("image_generation", "steps must be >= 1"));
    }

    let mut sample_params = DiffusionSampleParams::default();
    if request.cfg_scale.is_some() || request.guidance.is_some() {
        let cfg_scale = request.cfg_scale.or(request.guidance).unwrap_or_default();
        let distilled_guidance = request.guidance.or(request.cfg_scale).unwrap_or_default();
        sample_params.guidance = Some(DiffusionGuidanceParams {
            txt_cfg: cfg_scale,
            img_cfg: cfg_scale,
            distilled_guidance,
            slg: SlgParams::default(),
        });
    }
    sample_params.scheduler = scheduler;
    sample_params.sample_method = sample_method;
    sample_params.sample_steps = request.steps;
    sample_params.eta = request.eta;

    Ok(DiffusionImgParams {
        prompt: Some(request.prompt.clone()),
        negative_prompt: request.negative_prompt.clone(),
        clip_skip: request.clip_skip,
        init_image: request
            .init_image
            .as_ref()
            .map(raw_image_input_to_diffusion_image)
            .transpose()?,
        width: Some(request.width),
        height: Some(request.height),
        sample_params: (sample_params != DiffusionSampleParams::default()).then_some(sample_params),
        strength: request.strength,
        seed: request.seed,
        batch_count: Some(request.count),
        ..Default::default()
    })
}

fn build_diffusion_img_params(
    request: &ImageGenerationRequest,
) -> Result<DiffusionImgParams, CoreError> {
    diffusion_image_request_to_params(&DiffusionImageRequest {
        prompt: request.prompt.clone(),
        negative_prompt: request.negative_prompt.clone(),
        count: request.count,
        width: request.width,
        height: request.height,
        cfg_scale: request.cfg_scale,
        guidance: request.guidance,
        steps: request.steps,
        seed: request.seed,
        sample_method: request.sample_method.clone(),
        scheduler: request.scheduler.clone(),
        clip_skip: request.clip_skip,
        strength: request.strength,
        eta: request.eta,
        init_image: request.init_image.clone(),
        options: request.options.clone(),
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
