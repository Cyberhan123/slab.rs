use std::sync::Arc;

use slab_types::RuntimeBackendId;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::context::WorkerState;
use crate::domain::models::{
    AUDIO_TRANSCRIPTION_TASK_TYPE, AcceptedOperation, AudioTranscriptionCommand,
    AudioTranscriptionRequestData, AudioTranscriptionResultData, AudioTranscriptionTaskView,
    TaskResult, TaskStatus, TranscribeDecodeOptions, TranscribeVadOptions,
};
use crate::domain::ports::{
    RuntimeTranscriptionDecodeOptions, RuntimeTranscriptionRequest, RuntimeTranscriptionVadOptions,
    RuntimeTranscriptionVadParams,
};
use crate::domain::services::model;
use crate::domain::services::task::{
    parse_json_payload, parse_json_payload_optional, serialize_json_payload,
};
use crate::error::AppCoreError;
use crate::infra::db::{
    AudioTranscriptionTaskViewRecord, MediaTaskStore, NewAudioTranscriptionTaskRecord, TaskRecord,
};

const DEFAULT_AUDIO_BACKEND_ID: RuntimeBackendId = RuntimeBackendId::GgmlWhisper;

#[derive(Clone)]
pub struct AudioService {
    state: WorkerState,
}

impl AudioService {
    pub fn new(state: WorkerState) -> Self {
        Self { state }
    }

    pub async fn transcribe(
        &self,
        req: AudioTranscriptionCommand,
    ) -> Result<AcceptedOperation, AppCoreError> {
        let vad = build_vad_request(req.vad.as_ref())?;
        let decode = build_decode_request(req.decode.as_ref())?;
        let backend_id = model::resolve_worker_model_backend_or_default(
            &self.state,
            req.model_id.as_deref(),
            DEFAULT_AUDIO_BACKEND_ID,
        )
        .await?;
        ensure_audio_backend(backend_id)?;
        let vad_enabled = vad.is_some();
        let decode_configured = decode.is_some();
        debug!(
            file_path = %req.path,
            vad_enabled,
            decode_configured,
            "transcription request"
        );

        if !self.state.runtime().backend_available(backend_id) {
            return Err(AppCoreError::BackendNotReady(format!(
                "{} gRPC endpoint is not configured",
                backend_id.canonical_id()
            )));
        }

        let runtime_request = RuntimeTranscriptionRequest {
            backend_id: Some(backend_id),
            path: req.path.clone(),
            language: req.language.clone(),
            prompt: req.prompt.clone(),
            detect_language: req.detect_language,
            vad,
            decode,
        };

        let request_payload = AudioTranscriptionRequestData {
            model_id: req.model_id.clone(),
            source_path: req.path.clone(),
            language: req.language.clone(),
            prompt: req.prompt.clone(),
            detect_language: req.detect_language,
            vad: req.vad.clone(),
            decode: req.decode.clone(),
        };
        let request_data = serialize_json_payload(&request_payload)?;

        let operation_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        self.state
            .store()
            .insert_audio_transcription_operation(
                TaskRecord {
                    id: operation_id.clone(),
                    task_type: AUDIO_TRANSCRIPTION_TASK_TYPE.to_owned(),
                    status: TaskStatus::Running,
                    model_id: req.model_id.clone(),
                    input_data: Some(request_data.clone()),
                    result_data: None,
                    error_msg: None,
                    core_task_id: None,
                    created_at: now,
                    updated_at: now,
                },
                NewAudioTranscriptionTaskRecord {
                    task_id: operation_id.clone(),
                    backend_id: backend_id.canonical_id().to_owned(),
                    model_id: req.model_id.clone(),
                    source_path: req.path.clone(),
                    language: req.language.clone(),
                    prompt: req.prompt.clone(),
                    detect_language: req.detect_language,
                    vad_json: req.vad.as_ref().map(to_json_string),
                    decode_json: req.decode.as_ref().map(to_json_string),
                    request_data,
                    created_at: now,
                    updated_at: now,
                },
            )
            .await?;

        let model_auto_unload = Arc::clone(self.state.auto_unload());
        let store = Arc::clone(self.state.store());
        let worker_state = self.state.clone();
        self.state
            .clone()
            .spawn_existing_operation(operation_id.clone(), move |operation| async move {
                let operation_id = operation.id().to_owned();
                let _usage_guard = match model_auto_unload.acquire_for_inference(backend_id).await {
                    Ok(guard) => guard,
                    Err(error) => {
                        let msg = format!("{} backend not ready: {error}", backend_id.canonical_id());
                        if let Err(db_e) = operation.mark_failed(&msg).await {
                            warn!(task_id = %operation_id, error = %db_e, "failed to update auto-reload failure");
                        }
                        return;
                    }
                };

                let rpc_result = worker_state.runtime().transcribe(runtime_request).await;
                if operation.is_cancelled().await {
                    return;
                }

                match rpc_result {
                    Ok(response) => {
                        let text = response.text;
                        let segments = response.segments;
                        let task_payload = serde_json::to_string(&TaskResult {
                            image: None,
                            images: None,
                            video_path: None,
                            output_path: None,
                            text: Some(text.clone()),
                            segments: Some(segments.clone()),
                        })
                        .unwrap_or_default();
                        if let Err(error) = store
                            .update_audio_transcription_result(&operation_id, Some(&text))
                            .await
                        {
                            let message =
                                format!("failed to persist audio transcription metadata: {error}");
                            if let Err(db_e) = operation.mark_failed(&message).await {
                                warn!(task_id = %operation_id, error = %db_e, "failed to update audio transcription metadata failure");
                            }
                            return;
                        }
                        if let Err(error) = operation.mark_succeeded(&task_payload).await {
                            warn!(task_id = %operation_id, error = %error, "failed to update remote transcription result");
                        }
                    }
                    Err(error) => {
                        let message = error.to_string();
                        if let Err(db_e) = operation.mark_failed(&message).await {
                            warn!(task_id = %operation_id, error = %db_e, "failed to update remote transcription failure");
                        }
                    }
                }
            });

        Ok(AcceptedOperation { operation_id })
    }

    pub async fn list_transcription_tasks(
        &self,
    ) -> Result<Vec<AudioTranscriptionTaskView>, AppCoreError> {
        let rows = self.state.store().list_audio_transcription_tasks().await?;
        Ok(rows.into_iter().map(map_audio_view).collect())
    }

    pub async fn get_transcription_task(
        &self,
        task_id: &str,
    ) -> Result<AudioTranscriptionTaskView, AppCoreError> {
        let row =
            self.state.store().get_audio_transcription_task(task_id).await?.ok_or_else(|| {
                AppCoreError::NotFound(format!("audio transcription task {task_id} not found"))
            })?;
        Ok(map_audio_view(row))
    }
}

fn ensure_audio_backend(backend_id: RuntimeBackendId) -> Result<(), AppCoreError> {
    match backend_id {
        RuntimeBackendId::GgmlWhisper | RuntimeBackendId::CandleWhisper => Ok(()),
        other => Err(AppCoreError::BadRequest(format!(
            "backend '{}' does not support audio transcription",
            other.canonical_id()
        ))),
    }
}

fn build_vad_request(
    vad: Option<&TranscribeVadOptions>,
) -> Result<Option<RuntimeTranscriptionVadOptions>, AppCoreError> {
    let Some(vad) = vad else {
        return Ok(None);
    };

    if !vad.enabled {
        return Ok(None);
    }

    let model_path = vad.model_path.as_deref().ok_or_else(|| {
        AppCoreError::BadRequest(
            "VAD is enabled but model_path is empty. Please select a VAD model.".into(),
        )
    })?;

    let has_custom_params = vad.threshold.is_some()
        || vad.min_speech_duration_ms.is_some()
        || vad.min_silence_duration_ms.is_some()
        || vad.max_speech_duration_s.is_some()
        || vad.speech_pad_ms.is_some()
        || vad.samples_overlap.is_some();

    let params = has_custom_params.then_some(RuntimeTranscriptionVadParams {
        threshold: vad.threshold,
        min_speech_duration_ms: vad.min_speech_duration_ms,
        min_silence_duration_ms: vad.min_silence_duration_ms,
        max_speech_duration_s: vad.max_speech_duration_s,
        speech_pad_ms: vad.speech_pad_ms,
        samples_overlap: vad.samples_overlap,
    });

    Ok(Some(RuntimeTranscriptionVadOptions {
        enabled: true,
        model_path: Some(model_path.to_owned()),
        params,
    }))
}

fn build_decode_request(
    decode: Option<&TranscribeDecodeOptions>,
) -> Result<Option<RuntimeTranscriptionDecodeOptions>, AppCoreError> {
    let Some(decode) = decode else {
        return Ok(None);
    };

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

    Ok(Some(RuntimeTranscriptionDecodeOptions {
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

fn map_audio_view(row: AudioTranscriptionTaskViewRecord) -> AudioTranscriptionTaskView {
    AudioTranscriptionTaskView {
        task_id: row.task.task_id,
        task_type: AUDIO_TRANSCRIPTION_TASK_TYPE.to_owned(),
        status: row.state.status,
        progress: row.state.progress,
        error_msg: row.state.error_msg,
        backend_id: row.task.backend_id,
        model_id: row.task.model_id,
        source_path: row.task.source_path,
        language: row.task.language,
        prompt: row.task.prompt,
        detect_language: row.task.detect_language,
        vad_json: row.task.vad_json.as_deref().and_then(parse_json_payload_optional),
        decode_json: row.task.decode_json.as_deref().and_then(parse_json_payload_optional),
        transcript_text: row.task.transcript_text,
        segments: row.task.result_data.as_deref().and_then(parse_result_segments),
        request_data: parse_json_payload(&row.task.request_data),
        result_data: row.task.result_data.as_deref().map(parse_json_payload),
        created_at: row.state.task_created_at.to_rfc3339(),
        updated_at: row.state.task_updated_at.to_rfc3339(),
    }
}

fn parse_result_segments(raw: &str) -> Option<Vec<crate::domain::models::TimedTextSegment>> {
    Some(serde_json::from_str::<AudioTranscriptionResultData>(raw).ok()?.segments)
}

fn to_json_string<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned())
}

#[cfg(test)]
mod tests {
    use super::parse_result_segments;

    #[test]
    fn parses_segments_from_task_result_payload() {
        let segments = parse_result_segments(
            r#"{"text":"done","segments":[{"start_ms":0,"end_ms":120,"text":"hello"}]}"#,
        )
        .expect("segments should deserialize");

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text.as_deref(), Some("hello"));
    }
}
