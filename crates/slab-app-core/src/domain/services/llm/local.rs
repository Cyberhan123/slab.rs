//! Local LLM backend (slab-llama runtime): runtime exec + type conversion + trace payloads.
//!
//! Wraps slab-llama runtime non-streaming / streaming calls (including inference guard
//! acquisition) plus `RuntimeTextGeneration*` -> domain `TextGeneration*` conversion and
//! trace payloads. Prompt engineering (chat template / gbnf / reasoning controls) and chat
//! wire assembly remain in `domain::services::chat::local`; this module owns only the
//! provider-agnostic low-level runtime invocation, shared by chat and the future response
//! service.

use futures::stream::BoxStream;

use crate::context::ModelState;
use crate::domain::models::{
    TextGenerationChunk, TextGenerationResponse, TextGenerationUsage, TextPromptTokensDetails,
};
use crate::domain::ports::{
    RuntimeTextGenerationChunk, RuntimeTextGenerationRequest, RuntimeTextGenerationResponse,
    RuntimeTextGenerationUsage,
};
use crate::error::AppCoreError;
use crate::model_auto_unload::ModelUsageGuard;
use slab_types::RuntimeBackendId;

/// Non-streaming local inference: acquire inference guard -> `runtime.chat` -> raw
/// response. The guard is released when the call returns (subsequent text post-processing
/// does not invoke the runtime again).
pub(crate) async fn local_chat(
    state: &ModelState,
    backend_id: RuntimeBackendId,
    request: RuntimeTextGenerationRequest,
) -> Result<RuntimeTextGenerationResponse, AppCoreError> {
    let _guard = acquire_inference_guard(state, backend_id).await?;
    state.runtime().chat(request).await
}

/// Streaming local inference: acquire inference guard -> `runtime.chat_stream` ->
/// `(chunk stream, guard)`. The caller must keep the guard alive until the stream ends
/// (move it into the terminal stream's closure), otherwise the model may be auto-unloaded
/// while the stream is still being consumed.
pub(crate) async fn local_chat_stream(
    state: &ModelState,
    backend_id: RuntimeBackendId,
    request: RuntimeTextGenerationRequest,
) -> Result<
    (BoxStream<'static, Result<RuntimeTextGenerationChunk, AppCoreError>>, ModelUsageGuard),
    AppCoreError,
> {
    let guard = acquire_inference_guard(state, backend_id).await?;
    let stream = state.runtime().chat_stream(request).await?;
    Ok((stream, guard))
}

async fn acquire_inference_guard(
    state: &ModelState,
    backend_id: RuntimeBackendId,
) -> Result<ModelUsageGuard, AppCoreError> {
    state.auto_unload().acquire_for_inference(backend_id).await.map_err(|error| {
        AppCoreError::BackendNotReady(format!(
            "{} backend not ready: {error}",
            backend_id.canonical_id()
        ))
    })
}

pub(crate) fn text_response_from_runtime(
    response: RuntimeTextGenerationResponse,
) -> TextGenerationResponse {
    TextGenerationResponse {
        text: response.text,
        finish_reason: response.finish_reason,
        tokens_used: response.tokens_used,
        usage: response.usage.map(text_usage_from_runtime),
        metadata: response.metadata,
        tool_calls: Vec::new(),
    }
}

pub(crate) fn text_chunk_from_runtime(chunk: RuntimeTextGenerationChunk) -> TextGenerationChunk {
    TextGenerationChunk {
        delta: chunk.delta,
        done: chunk.done,
        finish_reason: chunk.finish_reason,
        usage: chunk.usage.map(text_usage_from_runtime),
        metadata: chunk.metadata,
    }
}

pub(crate) fn text_usage_from_runtime(usage: RuntimeTextGenerationUsage) -> TextGenerationUsage {
    TextGenerationUsage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        prompt_tokens_details: TextPromptTokensDetails {
            cached_tokens: usage.prompt_tokens_details.cached_tokens,
        },
        estimated: usage.estimated,
    }
}

pub(crate) fn runtime_request_payload(request: &RuntimeTextGenerationRequest) -> serde_json::Value {
    serde_json::json!({
        "model": request.model,
        "backend_id": request.backend_id.map(|backend| backend.canonical_id()),
        "prompt": request.prompt,
        "system_prompt": request.system_prompt,
        "max_tokens": request.max_tokens,
        "temperature": request.temperature,
        "top_p": request.top_p,
        "top_k": request.top_k,
        "min_p": request.min_p,
        "presence_penalty": request.presence_penalty,
        "repetition_penalty": request.repetition_penalty,
        "session_key": request.session_key,
        "stream": request.stream,
        "gbnf": request.gbnf,
        "stop_sequences": request.stop_sequences,
    })
}

pub(crate) fn runtime_response_payload(
    response: &RuntimeTextGenerationResponse,
) -> serde_json::Value {
    serde_json::json!({
        "text": response.text,
        "finish_reason": response.finish_reason,
        "tokens_used": response.tokens_used,
        "usage": response.usage.as_ref().map(runtime_usage_payload),
        "metadata": response.metadata,
    })
}

pub(crate) fn runtime_chunk_payload(chunk: &RuntimeTextGenerationChunk) -> serde_json::Value {
    serde_json::json!({
        "delta": chunk.delta,
        "done": chunk.done,
        "finish_reason": chunk.finish_reason,
        "usage": chunk.usage.as_ref().map(runtime_usage_payload),
        "metadata": chunk.metadata,
    })
}

pub(crate) fn runtime_usage_payload(usage: &RuntimeTextGenerationUsage) -> serde_json::Value {
    serde_json::json!({
        "prompt_tokens": usage.prompt_tokens,
        "completion_tokens": usage.completion_tokens,
        "total_tokens": usage.total_tokens,
        "prompt_tokens_details": {
            "cached_tokens": usage.prompt_tokens_details.cached_tokens,
        },
        "estimated": usage.estimated,
    })
}
