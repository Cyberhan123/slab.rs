use slab_proto::slab::ipc::v1 as pb;

use super::{
    GgmlWhisperLoadRequest, GgmlWhisperTranscribeRequest, GgmlWhisperTranscribeResponse,
    ProtoConversionError, decode_ggml_whisper_decode_options, decode_ggml_whisper_vad_options,
    decode_optional_path, encode_whisper_transcription,
};

pub(crate) fn decode_ggml_whisper_load_request(
    request: &pb::GgmlWhisperLoadRequest,
) -> Result<GgmlWhisperLoadRequest, ProtoConversionError> {
    Ok(GgmlWhisperLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        flash_attn: request.flash_attn,
    })
}

pub(crate) fn decode_ggml_whisper_transcribe_request(
    request: &pb::GgmlWhisperTranscribeRequest,
) -> Result<GgmlWhisperTranscribeRequest, ProtoConversionError> {
    Ok(GgmlWhisperTranscribeRequest {
        path: decode_optional_path(request.path.as_ref()),
        language: request.language.clone(),
        prompt: request.prompt.clone(),
        detect_language: request.detect_language,
        vad: request.vad.as_ref().map(decode_ggml_whisper_vad_options),
        decode: request.decode.as_ref().map(decode_ggml_whisper_decode_options),
    })
}

pub(crate) fn encode_ggml_whisper_transcribe_response(
    response: &GgmlWhisperTranscribeResponse,
) -> pb::GgmlWhisperTranscribeResponse {
    pb::GgmlWhisperTranscribeResponse {
        transcription: Some(encode_whisper_transcription(&response.transcription)),
    }
}
