use slab_proto::slab::ipc::v1 as pb;

use super::{
    GgmlLlamaChatRequest, GgmlLlamaLoadRequest, GgmlLlamaQuantizeRequest, GgmlLlamaQuantizeResult,
    LlamaChatResponse, LlamaChatStreamChunk, ProtoConversionError, decode_optional_path,
    decode_optional_string_list, encode_chat_metadata, encode_usage,
};

pub(crate) fn decode_ggml_llama_load_request(
    request: &pb::GgmlLlamaLoadRequest,
) -> Result<GgmlLlamaLoadRequest, ProtoConversionError> {
    Ok(GgmlLlamaLoadRequest {
        model_path: decode_optional_path(request.model_path.as_ref()),
        num_workers: request.num_workers,
        context_length: request.context_length,
        chat_template: request.chat_template.clone(),
        gbnf: request.gbnf.clone(),
        flash_attn: request.flash_attn,
    })
}

pub(crate) fn decode_ggml_llama_chat_request(
    request: &pb::GgmlLlamaChatRequest,
) -> Result<GgmlLlamaChatRequest, ProtoConversionError> {
    Ok(GgmlLlamaChatRequest {
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
        agent_trace: request
            .agent_trace_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|_| ProtoConversionError)?,
    })
}

pub(crate) fn encode_ggml_llama_chat_response(
    response: &LlamaChatResponse,
) -> pb::GgmlLlamaChatResponse {
    pb::GgmlLlamaChatResponse {
        text: response.text.clone(),
        finish_reason: response.finish_reason.clone(),
        tokens_used: response.tokens_used,
        usage: response.usage.as_ref().map(encode_usage),
        reasoning_content: response.reasoning_content.clone(),
        metadata: response.metadata.as_ref().map(encode_chat_metadata),
    }
}

pub(crate) fn encode_ggml_llama_chat_stream_chunk(
    chunk: &LlamaChatStreamChunk,
) -> pb::GgmlLlamaChatStreamChunk {
    pb::GgmlLlamaChatStreamChunk {
        delta: chunk.delta.clone(),
        done: chunk.done,
        finish_reason: chunk.finish_reason.clone(),
        usage: chunk.usage.as_ref().map(encode_usage),
        reasoning_content: chunk.reasoning_content.clone(),
        metadata: chunk.metadata.as_ref().map(encode_chat_metadata),
    }
}

pub(crate) fn decode_ggml_llama_quantize_request(
    request: &pb::GgmlLlamaQuantizeRequest,
) -> Result<GgmlLlamaQuantizeRequest, ProtoConversionError> {
    Ok(GgmlLlamaQuantizeRequest {
        input_path: request.input_path.clone().ok_or(ProtoConversionError)?,
        output_path: request.output_path.clone().ok_or(ProtoConversionError)?,
        ftype: request.ftype.ok_or(ProtoConversionError)?,
        nthread: request.nthread,
        allow_requantize: request.allow_requantize,
        quantize_output_tensor: request.quantize_output_tensor,
        only_copy: request.only_copy,
        pure: request.pure,
        keep_split: request.keep_split,
        dry_run: request.dry_run,
    })
}

pub(crate) fn encode_ggml_llama_quantize_response(
    result: &GgmlLlamaQuantizeResult,
) -> pb::GgmlLlamaQuantizeResponse {
    pb::GgmlLlamaQuantizeResponse {
        layers_processed: Some(result.layers_processed),
        output_path: Some(result.output_path.clone()),
    }
}
