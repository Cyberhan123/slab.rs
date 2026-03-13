//! Audio transcription routes.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use tracing::{debug, warn};
use utoipa::OpenApi;
use uuid::Uuid;

use crate::entities::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::grpc;
use crate::schemas::v1::audio::{CompletionRequest, TranscribeDecodeRequest, TranscribeVadRequest};
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(paths(transcribe))]
pub struct AudioApi;

/// Register audio routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/audio/transcriptions", post(transcribe))
}

fn build_vad_request(
    vad: Option<&TranscribeVadRequest>,
) -> Result<Option<grpc::pb::TranscribeVadOptions>, ServerError> {
    let Some(vad) = vad else {
        return Ok(None);
    };

    if !vad.enabled {
        return Ok(None);
    }

    let model_path = vad
        .model_path
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            ServerError::BadRequest(
                "VAD is enabled but model_path is empty. Please select a VAD model.".into(),
            )
        })?;

    if let Some(threshold) = vad.threshold {
        if !(0.0..=1.0).contains(&threshold) {
            return Err(ServerError::BadRequest(
                "vad.threshold must be between 0.0 and 1.0".into(),
            ));
        }
    }

    for (name, value) in [
        ("vad.min_speech_duration_ms", vad.min_speech_duration_ms),
        ("vad.min_silence_duration_ms", vad.min_silence_duration_ms),
        ("vad.speech_pad_ms", vad.speech_pad_ms),
    ] {
        if value.is_some_and(|v| v < 0) {
            return Err(ServerError::BadRequest(format!("{name} must be >= 0")));
        }
    }

    if let Some(max_speech_duration_s) = vad.max_speech_duration_s {
        if max_speech_duration_s <= 0.0 {
            return Err(ServerError::BadRequest(
                "vad.max_speech_duration_s must be > 0.0".into(),
            ));
        }
    }

    if let Some(samples_overlap) = vad.samples_overlap {
        if samples_overlap < 0.0 {
            return Err(ServerError::BadRequest(
                "vad.samples_overlap must be >= 0.0".into(),
            ));
        }
    }

    let has_custom_params = vad.threshold.is_some()
        || vad.min_speech_duration_ms.is_some()
        || vad.min_silence_duration_ms.is_some()
        || vad.max_speech_duration_s.is_some()
        || vad.speech_pad_ms.is_some()
        || vad.samples_overlap.is_some();

    let params = has_custom_params.then_some(grpc::pb::TranscribeVadParams {
        threshold: vad.threshold,
        min_speech_duration_ms: vad.min_speech_duration_ms,
        min_silence_duration_ms: vad.min_silence_duration_ms,
        max_speech_duration_s: vad.max_speech_duration_s,
        speech_pad_ms: vad.speech_pad_ms,
        samples_overlap: vad.samples_overlap,
    });

    Ok(Some(grpc::pb::TranscribeVadOptions {
        enabled: true,
        model_path: model_path.to_owned(),
        params,
    }))
}

fn build_decode_request(
    decode: Option<&TranscribeDecodeRequest>,
) -> Result<Option<grpc::pb::TranscribeDecodeOptions>, ServerError> {
    let Some(decode) = decode else {
        return Ok(None);
    };

    for (name, value) in [
        ("decode.offset_ms", decode.offset_ms),
        ("decode.duration_ms", decode.duration_ms),
        ("decode.max_len", decode.max_len),
        ("decode.max_tokens", decode.max_tokens),
    ] {
        if value.is_some_and(|v| v < 0) {
            return Err(ServerError::BadRequest(format!("{name} must be >= 0")));
        }
    }

    if let Some(word_thold) = decode.word_thold {
        if !(0.0..=1.0).contains(&word_thold) {
            return Err(ServerError::BadRequest(
                "decode.word_thold must be between 0.0 and 1.0".into(),
            ));
        }
    }

    for (name, value) in [
        ("decode.temperature", decode.temperature),
        ("decode.temperature_inc", decode.temperature_inc),
    ] {
        if value.is_some_and(|v| v < 0.0) {
            return Err(ServerError::BadRequest(format!("{name} must be >= 0.0")));
        }
    }

    let has_values = decode.offset_ms.is_some()
        || decode.duration_ms.is_some()
        || decode.no_context.is_some()
        || decode.no_timestamps.is_some()
        || decode.token_timestamps.is_some()
        || decode.split_on_word.is_some()
        || decode.suppress_nst.is_some()
        || decode.word_thold.is_some()
        || decode.max_len.is_some()
        || decode.max_tokens.is_some()
        || decode.temperature.is_some()
        || decode.temperature_inc.is_some()
        || decode.entropy_thold.is_some()
        || decode.logprob_thold.is_some()
        || decode.no_speech_thold.is_some()
        || decode.tdrz_enable.is_some();

    if !has_values {
        return Ok(None);
    }

    Ok(Some(grpc::pb::TranscribeDecodeOptions {
        offset_ms: decode.offset_ms,
        duration_ms: decode.duration_ms,
        no_context: decode.no_context,
        no_timestamps: decode.no_timestamps,
        token_timestamps: decode.token_timestamps,
        split_on_word: decode.split_on_word,
        suppress_nst: decode.suppress_nst,
        word_thold: decode.word_thold,
        max_len: decode.max_len,
        max_tokens: decode.max_tokens,
        temperature: decode.temperature,
        temperature_inc: decode.temperature_inc,
        entropy_thold: decode.entropy_thold,
        logprob_thold: decode.logprob_thold,
        no_speech_thold: decode.no_speech_thold,
        tdrz_enable: decode.tdrz_enable,
    }))
}

/// Speech-to-text transcription (`POST /v1/audio/transcriptions`).
#[utoipa::path(
    post,
    path = "/v1/audio/transcriptions",
    tag = "audio",
    request_body(content = CompletionRequest, description = "Audio transcription request"),
    responses(
        (status = 202, description = "Task accepted", body = serde_json::Value),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn transcribe(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CompletionRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let vad = build_vad_request(req.vad.as_ref())?;
    let decode = build_decode_request(req.decode.as_ref())?;
    let vad_enabled = vad.is_some();
    let decode_configured = decode.is_some();
    debug!(
        file_path = %req.path,
        vad_enabled,
        decode_configured,
        "transcription request"
    );

    if req.path.is_empty() {
        return Err(ServerError::BadRequest("audio file path is empty".into()));
    }

    let transcribe_channel = state.grpc.transcribe_channel().ok_or_else(|| {
        ServerError::BackendNotReady("whisper gRPC endpoint is not configured".into())
    })?;

    let task_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: "ggml.whisper".into(),
            status: "running".into(),
            model_id: None,
            input_data: Some(req.path.clone()),
            result_data: None,
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;

    let grpc_req = grpc::pb::TranscribeRequest {
        path: req.path.clone(),
        vad,
        decode,
    };

    let store = Arc::clone(&state.store);
    let task_manager = Arc::clone(&state.task_manager);
    let model_auto_unload = Arc::clone(&state.model_auto_unload);
    let task_id_for_spawn = task_id.clone();
    let transcribe_channel_for_spawn = transcribe_channel;
    let join = tokio::spawn(async move {
        let _usage_guard = match model_auto_unload
            .acquire_for_inference("ggml.whisper")
            .await
        {
            Ok(guard) => guard,
            Err(error) => {
                store
                    .update_task_status(
                        &task_id_for_spawn,
                        "failed",
                        None,
                        Some(&format!("whisper backend not ready: {error}")),
                    )
                    .await
                    .unwrap_or_else(|db_e| {
                        warn!(task_id = %task_id_for_spawn, error = %db_e, "failed to update auto-reload failure")
                    });
                task_manager.remove(&task_id_for_spawn);
                return;
            }
        };
        let rpc_result = grpc::client::transcribe(transcribe_channel_for_spawn, grpc_req).await;
        if let Ok(Some(record)) = store.get_task(&task_id_for_spawn).await {
            if record.status == "cancelled" {
                task_manager.remove(&task_id_for_spawn);
                return;
            }
        }

        match rpc_result {
            Ok(text) => {
                let payload = serde_json::json!({ "text": text }).to_string();
                store
                    .update_task_status(&task_id_for_spawn, "succeeded", Some(&payload), None)
                    .await
                    .unwrap_or_else(|e| {
                        warn!(task_id = %task_id_for_spawn, error = %e, "failed to update remote transcription result")
                    });
            }
            Err(e) => {
                let msg = e.to_string();
                store
                    .update_task_status(&task_id_for_spawn, "failed", None, Some(&msg))
                    .await
                    .unwrap_or_else(|db_e| {
                        warn!(task_id = %task_id_for_spawn, error = %db_e, "failed to update remote transcription failure")
                    });
            }
        }

        task_manager.remove(&task_id_for_spawn);
    });
    state
        .task_manager
        .insert(task_id.clone(), join.abort_handle());

    Ok(Json(serde_json::json!({ "task_id": task_id })))
}

#[cfg(test)]
mod test {}
