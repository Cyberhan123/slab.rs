use slab_proto::slab::ipc::v1 as pb;

use super::{
    GgmlParakeetLoadRequest, GgmlParakeetTranscribeRequest, GgmlParakeetTranscribeResponse,
    ProtoConversionError, decode_ggml_parakeet_decode_options, decode_optional_path,
    encode_whisper_transcription,
};

pub(crate) fn decode_ggml_parakeet_load_request(
    request: &pb::GgmlParakeetLoadRequest,
) -> Result<GgmlParakeetLoadRequest, ProtoConversionError> {
    Ok(GgmlParakeetLoadRequest { model_path: decode_optional_path(request.model_path.as_ref()) })
}

pub(crate) fn decode_ggml_parakeet_transcribe_request(
    request: &pb::GgmlParakeetTranscribeRequest,
) -> Result<GgmlParakeetTranscribeRequest, ProtoConversionError> {
    Ok(GgmlParakeetTranscribeRequest {
        path: decode_optional_path(request.path.as_ref()),
        decode: request.decode.as_ref().map(decode_ggml_parakeet_decode_options),
    })
}

pub(crate) fn encode_ggml_parakeet_transcribe_response(
    response: &GgmlParakeetTranscribeResponse,
) -> pb::GgmlParakeetTranscribeResponse {
    pb::GgmlParakeetTranscribeResponse {
        transcription: Some(encode_whisper_transcription(&response.transcription)),
    }
}
