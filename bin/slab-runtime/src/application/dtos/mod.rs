use std::path::PathBuf;

use thiserror::Error;

use slab_proto::slab::ipc::v1 as pb;

mod candle_diffusion;
mod candle_transformers;
mod ggml_diffusion;
mod ggml_llama;
mod ggml_whisper;
mod onnx;

pub(crate) use candle_diffusion::{
    decode_candle_diffusion_generate_image_request, decode_candle_diffusion_load_request,
    encode_candle_diffusion_generate_image_response,
};
pub(crate) use candle_transformers::{
    decode_candle_chat_request, decode_candle_llama_load_request,
    decode_candle_whisper_load_request, decode_candle_whisper_transcribe_request,
    encode_candle_chat_response, encode_candle_chat_stream_chunk,
    encode_candle_whisper_transcribe_response,
};
pub(crate) use ggml_diffusion::{
    decode_ggml_diffusion_generate_image_request, decode_ggml_diffusion_generate_video_request,
    decode_ggml_diffusion_load_request, encode_ggml_diffusion_generate_image_response,
    encode_ggml_diffusion_generate_video_response,
};
pub(crate) use ggml_llama::{
    decode_ggml_llama_chat_request, decode_ggml_llama_load_request,
    encode_ggml_llama_chat_response, encode_ggml_llama_chat_stream_chunk,
};
pub(crate) use ggml_whisper::{
    decode_ggml_whisper_load_request, decode_ggml_whisper_transcribe_request,
    encode_ggml_whisper_transcribe_response,
};
pub(crate) use onnx::{
    decode_onnx_embedding_load_request, decode_onnx_embedding_request,
    decode_onnx_text_load_request, decode_onnx_text_request, encode_onnx_embedding_response,
    encode_onnx_text_response,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ModelStatus {
    pub backend: String,
    pub status: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BinaryPayload {
    pub data: Vec<u8>,
    pub mime_type: Option<String>,
    pub file_name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Usage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub prompt_cached_tokens: Option<u32>,
    pub estimated: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RawImage {
    pub data: Vec<u8>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub channels: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RawTensor {
    pub name: Option<String>,
    pub shape: Vec<i64>,
    pub dtype: Option<String>,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WhisperSegment {
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub text: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WhisperTranscription {
    pub raw_text: Option<String>,
    pub language: Option<String>,
    pub segments: Vec<WhisperSegment>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LlamaChatResponse {
    pub text: Option<String>,
    pub finish_reason: Option<String>,
    pub tokens_used: Option<u32>,
    pub usage: Option<Usage>,
    pub reasoning_content: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LlamaChatStreamChunk {
    pub delta: Option<String>,
    pub done: Option<bool>,
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
    pub reasoning_content: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GgmlLlamaLoadRequest {
    pub model_path: Option<PathBuf>,
    pub num_workers: Option<u32>,
    pub context_length: Option<u32>,
    pub chat_template: Option<String>,
    pub gbnf: Option<String>,
    pub flash_attn: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GgmlLlamaChatRequest {
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
pub(crate) struct GgmlWhisperVadParams {
    pub threshold: Option<f32>,
    pub min_speech_duration_ms: Option<i32>,
    pub min_silence_duration_ms: Option<i32>,
    pub max_speech_duration_s: Option<f32>,
    pub speech_pad_ms: Option<i32>,
    pub samples_overlap: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GgmlWhisperVadOptions {
    pub enabled: Option<bool>,
    pub model_path: Option<PathBuf>,
    pub params: Option<GgmlWhisperVadParams>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GgmlWhisperDecodeOptions {
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
pub(crate) struct GgmlWhisperLoadRequest {
    pub model_path: Option<PathBuf>,
    pub flash_attn: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GgmlWhisperTranscribeRequest {
    pub path: Option<PathBuf>,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub detect_language: Option<bool>,
    pub vad: Option<GgmlWhisperVadOptions>,
    pub decode: Option<GgmlWhisperDecodeOptions>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GgmlWhisperTranscribeResponse {
    pub transcription: WhisperTranscription,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GgmlDiffusionLoadRequest {
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
pub(crate) struct GgmlDiffusionGenerateImageRequest {
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
pub(crate) struct GgmlDiffusionGenerateImageResponse {
    pub images: Vec<RawImage>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GgmlDiffusionGenerateVideoRequest {
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
pub(crate) struct GgmlDiffusionGenerateVideoResponse {
    pub frames: Vec<RawImage>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CandleLlamaLoadRequest {
    pub model_path: Option<PathBuf>,
    pub tokenizer_path: Option<PathBuf>,
    pub seed: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CandleChatRequest {
    pub prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub session_key: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CandleWhisperLoadRequest {
    pub model_path: Option<PathBuf>,
    pub tokenizer_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CandleWhisperTranscribeRequest {
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CandleWhisperTranscribeResponse {
    pub transcription: WhisperTranscription,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CandleDiffusionLoadRequest {
    pub model_path: Option<PathBuf>,
    pub vae_path: Option<PathBuf>,
    pub sd_version: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CandleDiffusionGenerateImageRequest {
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
pub(crate) struct CandleDiffusionGenerateImageResponse {
    pub images: Vec<RawImage>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OnnxTextLoadRequest {
    pub model_path: Option<PathBuf>,
    pub execution_providers: Option<Vec<String>>,
    pub intra_op_num_threads: Option<u32>,
    pub inter_op_num_threads: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OnnxTextRequest {
    pub inputs: Vec<RawTensor>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OnnxTextResponse {
    pub outputs: Vec<RawTensor>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OnnxEmbeddingLoadRequest {
    pub model_path: Option<PathBuf>,
    pub execution_providers: Option<Vec<String>>,
    pub intra_op_num_threads: Option<u32>,
    pub inter_op_num_threads: Option<u32>,
    pub input_tensor_name: Option<String>,
    pub output_tensor_name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OnnxEmbeddingRequest {
    pub image: Option<BinaryPayload>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OnnxEmbeddingResponse {
    pub output: Option<RawTensor>,
}

#[derive(Debug, Error)]
#[error("protobuf conversion failed")]
pub(crate) struct ProtoConversionError;

pub(crate) fn encode_model_status_response(status: &ModelStatus) -> pb::ModelStatusResponse {
    pb::ModelStatusResponse { backend: status.backend.clone(), status: status.status.clone() }
}

fn decode_optional_path(value: Option<&String>) -> Option<PathBuf> {
    value.map(PathBuf::from)
}

fn decode_optional_string_list(value: Option<&pb::StringList>) -> Option<Vec<String>> {
    value.map(|list| list.values.clone())
}

fn encode_usage(usage: &Usage) -> pb::Usage {
    pb::Usage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        prompt_cached_tokens: usage.prompt_cached_tokens,
        estimated: usage.estimated,
    }
}

fn decode_raw_image(image: &pb::RawImage) -> RawImage {
    RawImage {
        data: image.data.clone(),
        width: image.width,
        height: image.height,
        channels: image.channels,
    }
}

fn encode_raw_image(image: &RawImage) -> pb::RawImage {
    pb::RawImage {
        data: image.data.clone(),
        width: image.width,
        height: image.height,
        channels: image.channels,
    }
}

fn decode_raw_tensor(tensor: &pb::RawTensor) -> RawTensor {
    RawTensor {
        name: tensor.name.clone(),
        shape: tensor.shape.clone(),
        dtype: tensor.dtype.clone(),
        data: tensor.data.clone(),
    }
}

fn encode_raw_tensor(tensor: &RawTensor) -> pb::RawTensor {
    pb::RawTensor {
        name: tensor.name.clone(),
        shape: tensor.shape.clone(),
        dtype: tensor.dtype.clone(),
        data: tensor.data.clone(),
    }
}

fn decode_binary_payload(payload: &pb::BinaryPayload) -> BinaryPayload {
    BinaryPayload {
        data: payload.data.clone(),
        mime_type: payload.mime_type.clone(),
        file_name: payload.file_name.clone(),
    }
}

fn encode_whisper_transcription(transcription: &WhisperTranscription) -> pb::WhisperTranscription {
    pb::WhisperTranscription {
        raw_text: transcription.raw_text.clone(),
        language: transcription.language.clone(),
        segments: transcription.segments.iter().map(encode_whisper_segment).collect(),
    }
}

fn encode_whisper_segment(segment: &WhisperSegment) -> pb::WhisperSegment {
    pb::WhisperSegment {
        start_ms: segment.start_ms,
        end_ms: segment.end_ms,
        text: segment.text.clone(),
    }
}

fn decode_ggml_whisper_vad_options(value: &pb::GgmlWhisperVadOptions) -> GgmlWhisperVadOptions {
    GgmlWhisperVadOptions {
        enabled: value.enabled,
        model_path: decode_optional_path(value.model_path.as_ref()),
        params: value.params.as_ref().map(decode_ggml_whisper_vad_params),
    }
}

fn decode_ggml_whisper_vad_params(value: &pb::GgmlWhisperVadParams) -> GgmlWhisperVadParams {
    GgmlWhisperVadParams {
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
) -> GgmlWhisperDecodeOptions {
    GgmlWhisperDecodeOptions {
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
    use super::{
        BinaryPayload, ModelStatus, OnnxEmbeddingRequest, decode_ggml_llama_chat_request,
        decode_onnx_embedding_request, encode_model_status_response,
    };
    use slab_proto::slab::ipc::v1 as pb;

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
            OnnxEmbeddingRequest {
                image: Some(BinaryPayload {
                    data: Vec::new(),
                    mime_type: Some(String::new()),
                    file_name: Some(String::new()),
                }),
            }
        );
    }

    #[test]
    fn model_status_encode_is_lossless_for_strings() {
        let encoded = encode_model_status_response(&ModelStatus {
            backend: "onnx.text".to_owned(),
            status: "loaded".to_owned(),
        });

        assert_eq!(encoded.backend, "onnx.text");
        assert_eq!(encoded.status, "loaded");
    }
}
