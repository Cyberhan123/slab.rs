use slab_proto::slab::ipc::v1 as pb;

use super::{
    OnnxEmbeddingLoadRequest, OnnxEmbeddingRequest, OnnxEmbeddingResponse, OnnxTextLoadRequest,
    OnnxTextRequest, OnnxTextResponse, ProtoConversionError, decode_binary_payload,
    decode_optional_path, decode_optional_string_list, decode_raw_tensor, encode_raw_tensor,
};

pub(crate) fn decode_onnx_text_load_request(
    request: &pb::OnnxTextLoadRequest,
) -> Result<OnnxTextLoadRequest, ProtoConversionError> {
    Ok(OnnxTextLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        execution_providers: decode_optional_string_list(request.execution_providers.as_ref()),
        intra_op_num_threads: request.intra_op_num_threads,
        inter_op_num_threads: request.inter_op_num_threads,
    })
}

pub(crate) fn decode_onnx_text_request(
    request: &pb::OnnxTextRequest,
) -> Result<OnnxTextRequest, ProtoConversionError> {
    Ok(OnnxTextRequest { inputs: request.inputs.iter().map(decode_raw_tensor).collect() })
}

pub(crate) fn encode_onnx_text_response(response: &OnnxTextResponse) -> pb::OnnxTextResponse {
    pb::OnnxTextResponse { outputs: response.outputs.iter().map(encode_raw_tensor).collect() }
}

pub(crate) fn decode_onnx_embedding_load_request(
    request: &pb::OnnxEmbeddingLoadRequest,
) -> Result<OnnxEmbeddingLoadRequest, ProtoConversionError> {
    Ok(OnnxEmbeddingLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        execution_providers: decode_optional_string_list(request.execution_providers.as_ref()),
        intra_op_num_threads: request.intra_op_num_threads,
        inter_op_num_threads: request.inter_op_num_threads,
        input_tensor_name: request.input_tensor_name.clone(),
        output_tensor_name: request.output_tensor_name.clone(),
    })
}

pub(crate) fn decode_onnx_embedding_request(
    request: &pb::OnnxEmbeddingRequest,
) -> Result<OnnxEmbeddingRequest, ProtoConversionError> {
    Ok(OnnxEmbeddingRequest { image: request.image.as_ref().map(decode_binary_payload) })
}

pub(crate) fn encode_onnx_embedding_response(
    response: &OnnxEmbeddingResponse,
) -> pb::OnnxEmbeddingResponse {
    pb::OnnxEmbeddingResponse { output: response.output.as_ref().map(encode_raw_tensor) }
}
