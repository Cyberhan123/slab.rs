use std::path::PathBuf;

use thiserror::Error;

use crate::slab::ipc::v1 as pb;

pub mod dto {
    use std::path::PathBuf;

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct ModelStatus {
        pub backend: String,
        pub status: String,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct BinaryPayload {
        pub data: Vec<u8>,
        pub mime_type: Option<String>,
        pub file_name: Option<String>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Usage {
        pub prompt_tokens: Option<u32>,
        pub completion_tokens: Option<u32>,
        pub total_tokens: Option<u32>,
        pub prompt_cached_tokens: Option<u32>,
        pub estimated: Option<bool>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct RawImage {
        pub data: Vec<u8>,
        pub width: Option<u32>,
        pub height: Option<u32>,
        pub channels: Option<u32>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct RawTensor {
        pub name: Option<String>,
        pub shape: Vec<i64>,
        pub dtype: Option<String>,
        pub data: Vec<u8>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct WhisperSegment {
        pub start_ms: Option<u64>,
        pub end_ms: Option<u64>,
        pub text: Option<String>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct WhisperTranscription {
        pub raw_text: Option<String>,
        pub language: Option<String>,
        pub segments: Vec<WhisperSegment>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct LlamaChatResponse {
        pub text: Option<String>,
        pub finish_reason: Option<String>,
        pub tokens_used: Option<u32>,
        pub usage: Option<Usage>,
        pub reasoning_content: Option<String>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct LlamaChatStreamChunk {
        pub delta: Option<String>,
        pub done: Option<bool>,
        pub finish_reason: Option<String>,
        pub usage: Option<Usage>,
        pub reasoning_content: Option<String>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct GgmlLlamaLoadRequest {
        pub model_path: Option<PathBuf>,
        pub num_workers: Option<u32>,
        pub context_length: Option<u32>,
        pub chat_template: Option<String>,
        pub gbnf: Option<String>,
        pub flash_attn: Option<bool>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct GgmlLlamaChatRequest {
        pub prompt: Option<String>,
        pub max_tokens: Option<u32>,
        pub temperature: Option<f32>,
        pub top_p: Option<f32>,
        pub top_k: Option<i32>,
        pub min_p: Option<f32>,
        pub presence_penalty: Option<f32>,
        pub repetition_penalty: Option<f32>,
        pub session_key: Option<String>,
        pub gbnf: Option<String>,
        pub stop_sequences: Option<Vec<String>>,
        pub ignore_eos: Option<bool>,
        pub logit_bias_json: Option<Vec<u8>>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct GgmlWhisperVadParams {
        pub threshold: Option<f32>,
        pub min_speech_duration_ms: Option<i32>,
        pub min_silence_duration_ms: Option<i32>,
        pub max_speech_duration_s: Option<f32>,
        pub speech_pad_ms: Option<i32>,
        pub samples_overlap: Option<f32>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct GgmlWhisperVadOptions {
        pub enabled: Option<bool>,
        pub model_path: Option<PathBuf>,
        pub params: Option<GgmlWhisperVadParams>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct GgmlWhisperDecodeOptions {
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

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct GgmlWhisperLoadRequest {
        pub model_path: Option<PathBuf>,
        pub flash_attn: Option<bool>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct GgmlWhisperTranscribeRequest {
        pub path: Option<PathBuf>,
        pub language: Option<String>,
        pub prompt: Option<String>,
        pub detect_language: Option<bool>,
        pub vad: Option<GgmlWhisperVadOptions>,
        pub decode: Option<GgmlWhisperDecodeOptions>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct GgmlWhisperTranscribeResponse {
        pub transcription: WhisperTranscription,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct GgmlDiffusionLoadRequest {
        pub model_path: Option<PathBuf>,
        pub diffusion_model_path: Option<PathBuf>,
        pub vae_path: Option<PathBuf>,
        pub taesd_path: Option<PathBuf>,
        pub clip_l_path: Option<PathBuf>,
        pub clip_g_path: Option<PathBuf>,
        pub t5xxl_path: Option<PathBuf>,
        pub clip_vision_path: Option<PathBuf>,
        pub control_net_path: Option<PathBuf>,
        pub flash_attn: Option<bool>,
        pub vae_device: Option<String>,
        pub clip_device: Option<String>,
        pub offload_params_to_cpu: Option<bool>,
        pub enable_mmap: Option<bool>,
        pub n_threads: Option<i32>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct GgmlDiffusionGenerateImageRequest {
        pub prompt: Option<String>,
        pub negative_prompt: Option<String>,
        pub width: Option<u32>,
        pub height: Option<u32>,
        pub init_image: Option<RawImage>,
        pub count: Option<u32>,
        pub cfg_scale: Option<f32>,
        pub guidance: Option<f32>,
        pub sample_steps: Option<i32>,
        pub seed: Option<i64>,
        pub sample_method: Option<String>,
        pub scheduler: Option<String>,
        pub clip_skip: Option<i32>,
        pub strength: Option<f32>,
        pub eta: Option<f32>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct GgmlDiffusionGenerateImageResponse {
        pub images: Vec<RawImage>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct GgmlDiffusionGenerateVideoRequest {
        pub prompt: Option<String>,
        pub negative_prompt: Option<String>,
        pub width: Option<u32>,
        pub height: Option<u32>,
        pub init_image: Option<RawImage>,
        pub video_frames: Option<u32>,
        pub fps: Option<f32>,
        pub cfg_scale: Option<f32>,
        pub guidance: Option<f32>,
        pub sample_steps: Option<i32>,
        pub seed: Option<i64>,
        pub sample_method: Option<String>,
        pub scheduler: Option<String>,
        pub strength: Option<f32>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct GgmlDiffusionGenerateVideoResponse {
        pub frames: Vec<RawImage>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct CandleLlamaLoadRequest {
        pub model_path: Option<PathBuf>,
        pub tokenizer_path: Option<PathBuf>,
        pub seed: Option<u64>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct CandleChatRequest {
        pub prompt: Option<String>,
        pub max_tokens: Option<u32>,
        pub session_key: Option<String>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct CandleWhisperLoadRequest {
        pub model_path: Option<PathBuf>,
        pub tokenizer_path: Option<PathBuf>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct CandleWhisperTranscribeRequest {
        pub path: Option<PathBuf>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct CandleWhisperTranscribeResponse {
        pub transcription: WhisperTranscription,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct CandleDiffusionLoadRequest {
        pub model_path: Option<PathBuf>,
        pub vae_path: Option<PathBuf>,
        pub sd_version: Option<String>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct CandleDiffusionGenerateImageRequest {
        pub prompt: Option<String>,
        pub negative_prompt: Option<String>,
        pub width: Option<u32>,
        pub height: Option<u32>,
        pub batch_count: Option<u32>,
        pub sample_steps: Option<i32>,
        pub guidance_scale: Option<f32>,
        pub seed: Option<i64>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct CandleDiffusionGenerateImageResponse {
        pub images: Vec<RawImage>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct OnnxTextLoadRequest {
        pub model_path: Option<PathBuf>,
        pub execution_providers: Option<Vec<String>>,
        pub intra_op_num_threads: Option<u32>,
        pub inter_op_num_threads: Option<u32>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct OnnxTextRequest {
        pub inputs: Vec<RawTensor>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct OnnxTextResponse {
        pub outputs: Vec<RawTensor>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct OnnxEmbeddingLoadRequest {
        pub model_path: Option<PathBuf>,
        pub execution_providers: Option<Vec<String>>,
        pub intra_op_num_threads: Option<u32>,
        pub inter_op_num_threads: Option<u32>,
        pub input_tensor_name: Option<String>,
        pub output_tensor_name: Option<String>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct OnnxEmbeddingRequest {
        pub image: Option<BinaryPayload>,
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct OnnxEmbeddingResponse {
        pub output: Option<RawTensor>,
    }
}

#[derive(Debug, Error)]
pub enum ProtoConversionError {
    #[error("missing required field `{field}`")]
    MissingField { field: &'static str },
    #[error("invalid field `{field}`: {message}")]
    InvalidField { field: &'static str, message: String },
}

pub fn decode_ggml_llama_load_request(
    request: &pb::GgmlLlamaLoadRequest,
) -> Result<dto::GgmlLlamaLoadRequest, ProtoConversionError> {
    Ok(dto::GgmlLlamaLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        num_workers: request.num_workers,
        context_length: request.context_length,
        chat_template: request.chat_template.clone(),
        gbnf: request.gbnf.clone(),
        flash_attn: request.flash_attn,
    })
}

pub fn decode_ggml_llama_chat_request(
    request: &pb::GgmlLlamaChatRequest,
) -> Result<dto::GgmlLlamaChatRequest, ProtoConversionError> {
    Ok(dto::GgmlLlamaChatRequest {
        prompt: request.prompt.clone(),
        max_tokens: request.max_tokens,
        temperature: request.temperature,
        top_p: request.top_p,
        top_k: request.top_k,
        min_p: request.min_p,
        presence_penalty: request.presence_penalty,
        repetition_penalty: request.repetition_penalty,
        session_key: request.session_key.clone(),
        gbnf: request.gbnf.clone(),
        stop_sequences: decode_optional_string_list(request.stop_sequences.as_ref()),
        ignore_eos: request.ignore_eos,
        logit_bias_json: request.logit_bias_json.clone(),
    })
}

pub fn encode_ggml_llama_chat_response(
    response: &dto::LlamaChatResponse,
) -> pb::GgmlLlamaChatResponse {
    pb::GgmlLlamaChatResponse {
        text: response.text.clone(),
        finish_reason: response.finish_reason.clone(),
        tokens_used: response.tokens_used,
        usage: response.usage.as_ref().map(encode_usage),
        reasoning_content: response.reasoning_content.clone(),
    }
}

pub fn encode_ggml_llama_chat_stream_chunk(
    chunk: &dto::LlamaChatStreamChunk,
) -> pb::GgmlLlamaChatStreamChunk {
    pb::GgmlLlamaChatStreamChunk {
        delta: chunk.delta.clone(),
        done: chunk.done,
        finish_reason: chunk.finish_reason.clone(),
        usage: chunk.usage.as_ref().map(encode_usage),
        reasoning_content: chunk.reasoning_content.clone(),
    }
}

pub fn decode_ggml_whisper_load_request(
    request: &pb::GgmlWhisperLoadRequest,
) -> Result<dto::GgmlWhisperLoadRequest, ProtoConversionError> {
    Ok(dto::GgmlWhisperLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        flash_attn: request.flash_attn,
    })
}

pub fn decode_ggml_whisper_transcribe_request(
    request: &pb::GgmlWhisperTranscribeRequest,
) -> Result<dto::GgmlWhisperTranscribeRequest, ProtoConversionError> {
    Ok(dto::GgmlWhisperTranscribeRequest {
        path: decode_optional_path(request.path.as_ref()),
        language: request.language.clone(),
        prompt: request.prompt.clone(),
        detect_language: request.detect_language,
        vad: request.vad.as_ref().map(decode_ggml_whisper_vad_options),
        decode: request.decode.as_ref().map(decode_ggml_whisper_decode_options),
    })
}

pub fn encode_ggml_whisper_transcribe_response(
    response: &dto::GgmlWhisperTranscribeResponse,
) -> pb::GgmlWhisperTranscribeResponse {
    pb::GgmlWhisperTranscribeResponse {
        transcription: Some(encode_whisper_transcription(&response.transcription)),
    }
}

pub fn decode_ggml_diffusion_load_request(
    request: &pb::GgmlDiffusionLoadRequest,
) -> Result<dto::GgmlDiffusionLoadRequest, ProtoConversionError> {
    Ok(dto::GgmlDiffusionLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        diffusion_model_path: decode_optional_path(request.diffusion_model_path.as_ref()),
        vae_path: decode_optional_path(request.vae_path.as_ref()),
        taesd_path: decode_optional_path(request.taesd_path.as_ref()),
        clip_l_path: decode_optional_path(request.clip_l_path.as_ref()),
        clip_g_path: decode_optional_path(request.clip_g_path.as_ref()),
        t5xxl_path: decode_optional_path(request.t5xxl_path.as_ref()),
        clip_vision_path: decode_optional_path(request.clip_vision_path.as_ref()),
        control_net_path: decode_optional_path(request.control_net_path.as_ref()),
        flash_attn: request.flash_attn,
        vae_device: request.vae_device.clone(),
        clip_device: request.clip_device.clone(),
        offload_params_to_cpu: request.offload_params_to_cpu,
        enable_mmap: request.enable_mmap,
        n_threads: request.n_threads,
    })
}

pub fn decode_ggml_diffusion_generate_image_request(
    request: &pb::GgmlDiffusionGenerateImageRequest,
) -> Result<dto::GgmlDiffusionGenerateImageRequest, ProtoConversionError> {
    Ok(dto::GgmlDiffusionGenerateImageRequest {
        prompt: request.prompt.clone(),
        negative_prompt: request.negative_prompt.clone(),
        width: request.width,
        height: request.height,
        init_image: request.init_image.as_ref().map(decode_raw_image),
        count: request.count,
        cfg_scale: request.cfg_scale,
        guidance: request.guidance,
        sample_steps: request.sample_steps,
        seed: request.seed,
        sample_method: request.sample_method.clone(),
        scheduler: request.scheduler.clone(),
        clip_skip: request.clip_skip,
        strength: request.strength,
        eta: request.eta,
    })
}

pub fn encode_ggml_diffusion_generate_image_response(
    response: &dto::GgmlDiffusionGenerateImageResponse,
) -> pb::GgmlDiffusionGenerateImageResponse {
    pb::GgmlDiffusionGenerateImageResponse {
        images: response.images.iter().map(encode_raw_image).collect(),
    }
}

pub fn decode_ggml_diffusion_generate_video_request(
    request: &pb::GgmlDiffusionGenerateVideoRequest,
) -> Result<dto::GgmlDiffusionGenerateVideoRequest, ProtoConversionError> {
    Ok(dto::GgmlDiffusionGenerateVideoRequest {
        prompt: request.prompt.clone(),
        negative_prompt: request.negative_prompt.clone(),
        width: request.width,
        height: request.height,
        init_image: request.init_image.as_ref().map(decode_raw_image),
        video_frames: request.video_frames,
        fps: request.fps,
        cfg_scale: request.cfg_scale,
        guidance: request.guidance,
        sample_steps: request.sample_steps,
        seed: request.seed,
        sample_method: request.sample_method.clone(),
        scheduler: request.scheduler.clone(),
        strength: request.strength,
    })
}

pub fn encode_ggml_diffusion_generate_video_response(
    response: &dto::GgmlDiffusionGenerateVideoResponse,
) -> pb::GgmlDiffusionGenerateVideoResponse {
    pb::GgmlDiffusionGenerateVideoResponse {
        frames: response.frames.iter().map(encode_raw_image).collect(),
    }
}

pub fn decode_candle_llama_load_request(
    request: &pb::CandleLlamaLoadRequest,
) -> Result<dto::CandleLlamaLoadRequest, ProtoConversionError> {
    Ok(dto::CandleLlamaLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        tokenizer_path: decode_optional_path(request.tokenizer_path.as_ref()),
        seed: request.seed,
    })
}

pub fn decode_candle_chat_request(
    request: &pb::CandleChatRequest,
) -> Result<dto::CandleChatRequest, ProtoConversionError> {
    Ok(dto::CandleChatRequest {
        prompt: request.prompt.clone(),
        max_tokens: request.max_tokens,
        session_key: request.session_key.clone(),
    })
}

pub fn encode_candle_chat_response(response: &dto::LlamaChatResponse) -> pb::CandleChatResponse {
    pb::CandleChatResponse {
        text: response.text.clone(),
        finish_reason: response.finish_reason.clone(),
        tokens_used: response.tokens_used,
        usage: response.usage.as_ref().map(encode_usage),
        reasoning_content: response.reasoning_content.clone(),
    }
}

pub fn encode_candle_chat_stream_chunk(
    chunk: &dto::LlamaChatStreamChunk,
) -> pb::CandleChatStreamChunk {
    pb::CandleChatStreamChunk {
        delta: chunk.delta.clone(),
        done: chunk.done,
        finish_reason: chunk.finish_reason.clone(),
        usage: chunk.usage.as_ref().map(encode_usage),
        reasoning_content: chunk.reasoning_content.clone(),
    }
}

pub fn decode_candle_whisper_load_request(
    request: &pb::CandleWhisperLoadRequest,
) -> Result<dto::CandleWhisperLoadRequest, ProtoConversionError> {
    Ok(dto::CandleWhisperLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        tokenizer_path: decode_optional_path(request.tokenizer_path.as_ref()),
    })
}

pub fn decode_candle_whisper_transcribe_request(
    request: &pb::CandleWhisperTranscribeRequest,
) -> Result<dto::CandleWhisperTranscribeRequest, ProtoConversionError> {
    Ok(dto::CandleWhisperTranscribeRequest { path: decode_optional_path(request.path.as_ref()) })
}

pub fn encode_candle_whisper_transcribe_response(
    response: &dto::CandleWhisperTranscribeResponse,
) -> pb::CandleWhisperTranscribeResponse {
    pb::CandleWhisperTranscribeResponse {
        transcription: Some(encode_whisper_transcription(&response.transcription)),
    }
}

pub fn decode_candle_diffusion_load_request(
    request: &pb::CandleDiffusionLoadRequest,
) -> Result<dto::CandleDiffusionLoadRequest, ProtoConversionError> {
    Ok(dto::CandleDiffusionLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        vae_path: decode_optional_path(request.vae_path.as_ref()),
        sd_version: request.sd_version.clone(),
    })
}

pub fn decode_candle_diffusion_generate_image_request(
    request: &pb::CandleDiffusionGenerateImageRequest,
) -> Result<dto::CandleDiffusionGenerateImageRequest, ProtoConversionError> {
    Ok(dto::CandleDiffusionGenerateImageRequest {
        prompt: request.prompt.clone(),
        negative_prompt: request.negative_prompt.clone(),
        width: request.width,
        height: request.height,
        batch_count: request.batch_count,
        sample_steps: request.sample_steps,
        guidance_scale: request.guidance_scale,
        seed: request.seed,
    })
}

pub fn encode_candle_diffusion_generate_image_response(
    response: &dto::CandleDiffusionGenerateImageResponse,
) -> pb::CandleDiffusionGenerateImageResponse {
    pb::CandleDiffusionGenerateImageResponse {
        images: response.images.iter().map(encode_raw_image).collect(),
    }
}

pub fn decode_onnx_text_load_request(
    request: &pb::OnnxTextLoadRequest,
) -> Result<dto::OnnxTextLoadRequest, ProtoConversionError> {
    Ok(dto::OnnxTextLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        execution_providers: decode_optional_string_list(request.execution_providers.as_ref()),
        intra_op_num_threads: request.intra_op_num_threads,
        inter_op_num_threads: request.inter_op_num_threads,
    })
}

pub fn decode_onnx_text_request(
    request: &pb::OnnxTextRequest,
) -> Result<dto::OnnxTextRequest, ProtoConversionError> {
    Ok(dto::OnnxTextRequest { inputs: request.inputs.iter().map(decode_raw_tensor).collect() })
}

pub fn encode_onnx_text_response(response: &dto::OnnxTextResponse) -> pb::OnnxTextResponse {
    pb::OnnxTextResponse { outputs: response.outputs.iter().map(encode_raw_tensor).collect() }
}

pub fn decode_onnx_embedding_load_request(
    request: &pb::OnnxEmbeddingLoadRequest,
) -> Result<dto::OnnxEmbeddingLoadRequest, ProtoConversionError> {
    Ok(dto::OnnxEmbeddingLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        execution_providers: decode_optional_string_list(request.execution_providers.as_ref()),
        intra_op_num_threads: request.intra_op_num_threads,
        inter_op_num_threads: request.inter_op_num_threads,
        input_tensor_name: request.input_tensor_name.clone(),
        output_tensor_name: request.output_tensor_name.clone(),
    })
}

pub fn decode_onnx_embedding_request(
    request: &pb::OnnxEmbeddingRequest,
) -> Result<dto::OnnxEmbeddingRequest, ProtoConversionError> {
    Ok(dto::OnnxEmbeddingRequest { image: request.image.as_ref().map(decode_binary_payload) })
}

pub fn encode_onnx_embedding_response(
    response: &dto::OnnxEmbeddingResponse,
) -> pb::OnnxEmbeddingResponse {
    pb::OnnxEmbeddingResponse { output: response.output.as_ref().map(encode_raw_tensor) }
}

pub fn encode_model_status_response(status: &dto::ModelStatus) -> pb::ModelStatusResponse {
    pb::ModelStatusResponse { backend: status.backend.clone(), status: status.status.clone() }
}

fn decode_optional_path(value: Option<&String>) -> Option<PathBuf> {
    value.map(PathBuf::from)
}

fn decode_optional_string_list(value: Option<&pb::StringList>) -> Option<Vec<String>> {
    value.map(|list| list.values.clone())
}

fn encode_usage(usage: &dto::Usage) -> pb::Usage {
    pb::Usage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        prompt_cached_tokens: usage.prompt_cached_tokens,
        estimated: usage.estimated,
    }
}

fn decode_raw_image(image: &pb::RawImage) -> dto::RawImage {
    dto::RawImage {
        data: image.data.clone(),
        width: image.width,
        height: image.height,
        channels: image.channels,
    }
}

fn encode_raw_image(image: &dto::RawImage) -> pb::RawImage {
    pb::RawImage {
        data: image.data.clone(),
        width: image.width,
        height: image.height,
        channels: image.channels,
    }
}

fn decode_raw_tensor(tensor: &pb::RawTensor) -> dto::RawTensor {
    dto::RawTensor {
        name: tensor.name.clone(),
        shape: tensor.shape.clone(),
        dtype: tensor.dtype.clone(),
        data: tensor.data.clone(),
    }
}

fn encode_raw_tensor(tensor: &dto::RawTensor) -> pb::RawTensor {
    pb::RawTensor {
        name: tensor.name.clone(),
        shape: tensor.shape.clone(),
        dtype: tensor.dtype.clone(),
        data: tensor.data.clone(),
    }
}

fn decode_binary_payload(payload: &pb::BinaryPayload) -> dto::BinaryPayload {
    dto::BinaryPayload {
        data: payload.data.clone(),
        mime_type: payload.mime_type.clone(),
        file_name: payload.file_name.clone(),
    }
}

fn encode_whisper_transcription(
    transcription: &dto::WhisperTranscription,
) -> pb::WhisperTranscription {
    pb::WhisperTranscription {
        raw_text: transcription.raw_text.clone(),
        language: transcription.language.clone(),
        segments: transcription.segments.iter().map(encode_whisper_segment).collect(),
    }
}

fn encode_whisper_segment(segment: &dto::WhisperSegment) -> pb::WhisperSegment {
    pb::WhisperSegment {
        start_ms: segment.start_ms,
        end_ms: segment.end_ms,
        text: segment.text.clone(),
    }
}

fn decode_ggml_whisper_vad_options(
    value: &pb::GgmlWhisperVadOptions,
) -> dto::GgmlWhisperVadOptions {
    dto::GgmlWhisperVadOptions {
        enabled: value.enabled,
        model_path: decode_optional_path(value.model_path.as_ref()),
        params: value.params.as_ref().map(decode_ggml_whisper_vad_params),
    }
}

fn decode_ggml_whisper_vad_params(value: &pb::GgmlWhisperVadParams) -> dto::GgmlWhisperVadParams {
    dto::GgmlWhisperVadParams {
        threshold: value.threshold,
        min_speech_duration_ms: value.min_speech_duration_ms,
        min_silence_duration_ms: value.min_silence_duration_ms,
        max_speech_duration_s: value.max_speech_duration_s,
        speech_pad_ms: value.speech_pad_ms,
        samples_overlap: value.samples_overlap,
    }
}

fn decode_ggml_whisper_decode_options(
    value: &pb::GgmlWhisperDecodeOptions,
) -> dto::GgmlWhisperDecodeOptions {
    dto::GgmlWhisperDecodeOptions {
        offset_ms: value.offset_ms,
        duration_ms: value.duration_ms,
        no_context: value.no_context,
        no_timestamps: value.no_timestamps,
        token_timestamps: value.token_timestamps,
        split_on_word: value.split_on_word,
        suppress_nst: value.suppress_nst,
        word_thold: value.word_thold,
        max_len: value.max_len,
        max_tokens: value.max_tokens,
        temperature: value.temperature,
        temperature_inc: value.temperature_inc,
        entropy_thold: value.entropy_thold,
        logprob_thold: value.logprob_thold,
        no_speech_thold: value.no_speech_thold,
        tdrz_enable: value.tdrz_enable,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_ggml_llama_chat_request, decode_onnx_embedding_request, dto};
    use crate::slab::ipc::v1 as pb;

    #[test]
    fn ggml_llama_request_preserves_zero_false_and_empty_values() {
        let decoded = decode_ggml_llama_chat_request(&pb::GgmlLlamaChatRequest {
            prompt: Some(String::new()),
            max_tokens: Some(0),
            temperature: Some(0.0),
            top_p: Some(0.0),
            top_k: Some(0),
            min_p: Some(0.0),
            presence_penalty: Some(0.0),
            repetition_penalty: Some(0.0),
            session_key: Some(String::new()),
            gbnf: Some(String::new()),
            stop_sequences: Some(pb::StringList { values: Vec::new() }),
            ignore_eos: Some(false),
            logit_bias_json: Some(Vec::new()),
        })
        .expect("decode should succeed");

        assert_eq!(decoded.prompt, Some(String::new()));
        assert_eq!(decoded.max_tokens, Some(0));
        assert_eq!(decoded.temperature, Some(0.0));
        assert_eq!(decoded.top_p, Some(0.0));
        assert_eq!(decoded.top_k, Some(0));
        assert_eq!(decoded.min_p, Some(0.0));
        assert_eq!(decoded.ignore_eos, Some(false));
        assert_eq!(decoded.stop_sequences, Some(Vec::new()));
        assert_eq!(decoded.logit_bias_json, Some(Vec::new()));
    }

    #[test]
    fn onnx_embedding_request_preserves_empty_binary_payload() {
        let decoded = decode_onnx_embedding_request(&pb::OnnxEmbeddingRequest {
            image: Some(pb::BinaryPayload {
                data: Vec::new(),
                mime_type: Some(String::new()),
                file_name: Some(String::new()),
            }),
        })
        .expect("decode should succeed");

        assert_eq!(
            decoded,
            dto::OnnxEmbeddingRequest {
                image: Some(dto::BinaryPayload {
                    data: Vec::new(),
                    mime_type: Some(String::new()),
                    file_name: Some(String::new()),
                }),
            }
        );
    }

    #[test]
    fn model_status_encode_is_lossless_for_strings() {
        let encoded = super::encode_model_status_response(&dto::ModelStatus {
            backend: "onnx.text".to_owned(),
            status: "loaded".to_owned(),
        });

        assert_eq!(encoded.backend, "onnx.text");
        assert_eq!(encoded.status, "loaded");
    }
}
