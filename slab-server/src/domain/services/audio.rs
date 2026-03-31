use std::sync::Arc;

use slab_types::RuntimeBackendId;
use tracing::{debug, warn};

use crate::context::{SubmitOperation, WorkerState};
use crate::domain::models::{
    AcceptedOperation, AudioTranscriptionCommand, TranscribeDecodeOptions, TranscribeVadOptions,
};
use crate::error::ServerError;
use crate::infra::rpc::{self, pb};

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
    ) -> Result<AcceptedOperation, ServerError> {
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

        let transcribe_channel = self.state.grpc().transcribe_channel().ok_or_else(|| {
            ServerError::BackendNotReady("whisper gRPC endpoint is not configured".into())
        })?;

        let grpc_req = pb::TranscribeRequest { path: req.path.clone(), vad, decode };

        let model_auto_unload = Arc::clone(self.state.auto_unload());
        let transcribe_channel_for_spawn = transcribe_channel;
        let input_data = req.path.clone();
        let operation_id = self
            .state
            .submit_operation(
                SubmitOperation::running("ggml.whisper", None, Some(input_data)),
                move |operation| async move {
                    let operation_id = operation.id().to_owned();
                    let _usage_guard =
                        match model_auto_unload
                            .acquire_for_inference(RuntimeBackendId::GgmlWhisper)
                            .await
                        {
                            Ok(guard) => guard,
                            Err(error) => {
                                let msg = format!("whisper backend not ready: {error}");
                                if let Err(db_e) = operation.mark_failed(&msg).await {
                                    warn!(task_id = %operation_id, error = %db_e, "failed to update auto-reload failure");
                                }
                                return;
                            }
                        };

                    let rpc_result =
                        rpc::client::transcribe(transcribe_channel_for_spawn, grpc_req).await;
                    if operation.is_cancelled().await {
                        return;
                    }

                    match rpc_result {
                        Ok(text) => {
                            let payload = serde_json::json!({ "text": text }).to_string();
                            if let Err(error) = operation.mark_succeeded(&payload).await {
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
                },
            )
            .await?;

        Ok(AcceptedOperation { operation_id })
    }
}

fn build_vad_request(
    vad: Option<&TranscribeVadOptions>,
) -> Result<Option<pb::TranscribeVadOptions>, ServerError> {
    let Some(vad) = vad else {
        return Ok(None);
    };

    if !vad.enabled {
        return Ok(None);
    }

    let model_path = vad.model_path.as_deref().ok_or_else(|| {
        ServerError::BadRequest(
            "VAD is enabled but model_path is empty. Please select a VAD model.".into(),
        )
    })?;

    let has_custom_params = vad.threshold.is_some()
        || vad.min_speech_duration_ms.is_some()
        || vad.min_silence_duration_ms.is_some()
        || vad.max_speech_duration_s.is_some()
        || vad.speech_pad_ms.is_some()
        || vad.samples_overlap.is_some();

    let params = has_custom_params.then_some(pb::TranscribeVadParams {
        threshold: vad.threshold,
        min_speech_duration_ms: vad.min_speech_duration_ms,
        min_silence_duration_ms: vad.min_silence_duration_ms,
        max_speech_duration_s: vad.max_speech_duration_s,
        speech_pad_ms: vad.speech_pad_ms,
        samples_overlap: vad.samples_overlap,
    });

    Ok(Some(pb::TranscribeVadOptions { enabled: true, model_path: model_path.to_owned(), params }))
}

fn build_decode_request(
    decode: Option<&TranscribeDecodeOptions>,
) -> Result<Option<pb::TranscribeDecodeOptions>, ServerError> {
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

    Ok(Some(pb::TranscribeDecodeOptions {
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

#[cfg(test)]
mod test {}
