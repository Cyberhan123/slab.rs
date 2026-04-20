use slab_proto::slab::ipc::v1 as pb;

use super::{
    CandleChatRequest, CandleLlamaLoadRequest, CandleWhisperLoadRequest,
    CandleWhisperTranscribeRequest, CandleWhisperTranscribeResponse, LlamaChatResponse,
    LlamaChatStreamChunk, ProtoConversionError, decode_optional_path, encode_chat_metadata,
    encode_usage, encode_whisper_transcription,
};

pub(crate) fn decode_candle_llama_load_request(
    request: &pb::CandleLlamaLoadRequest,
) -> Result<CandleLlamaLoadRequest, ProtoConversionError> {
    Ok(CandleLlamaLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        tokenizer_path: decode_optional_path(request.tokenizer_path.as_ref()),
        seed: request.seed,
    })
}

pub(crate) fn decode_candle_chat_request(
    request: &pb::CandleChatRequest,
) -> Result<CandleChatRequest, ProtoConversionError> {
    Ok(CandleChatRequest {
        prompt: request.prompt.clone(),
        max_tokens: request.max_tokens,
        session_key: request.session_key.clone(),
    })
}

pub(crate) fn encode_candle_chat_response(response: &LlamaChatResponse) -> pb::CandleChatResponse {
    pb::CandleChatResponse {
        text: response.text.clone(),
        finish_reason: response.finish_reason.clone(),
        tokens_used: response.tokens_used,
        usage: response.usage.as_ref().map(encode_usage),
        reasoning_content: response.reasoning_content.clone(),
        metadata: response.metadata.as_ref().map(encode_chat_metadata),
    }
}

pub(crate) fn encode_candle_chat_stream_chunk(
    chunk: &LlamaChatStreamChunk,
) -> pb::CandleChatStreamChunk {
    pb::CandleChatStreamChunk {
        delta: chunk.delta.clone(),
        done: chunk.done,
        finish_reason: chunk.finish_reason.clone(),
        usage: chunk.usage.as_ref().map(encode_usage),
        reasoning_content: chunk.reasoning_content.clone(),
        metadata: chunk.metadata.as_ref().map(encode_chat_metadata),
    }
}

pub(crate) fn decode_candle_whisper_load_request(
    request: &pb::CandleWhisperLoadRequest,
) -> Result<CandleWhisperLoadRequest, ProtoConversionError> {
    Ok(CandleWhisperLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        tokenizer_path: decode_optional_path(request.tokenizer_path.as_ref()),
    })
}

pub(crate) fn decode_candle_whisper_transcribe_request(
    request: &pb::CandleWhisperTranscribeRequest,
) -> Result<CandleWhisperTranscribeRequest, ProtoConversionError> {
    Ok(CandleWhisperTranscribeRequest { path: decode_optional_path(request.path.as_ref()) })
}

pub(crate) fn encode_candle_whisper_transcribe_response(
    response: &CandleWhisperTranscribeResponse,
) -> pb::CandleWhisperTranscribeResponse {
    pb::CandleWhisperTranscribeResponse {
        transcription: Some(encode_whisper_transcription(&response.transcription)),
    }
}
