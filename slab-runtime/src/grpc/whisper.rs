use bytemuck::cast_slice;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, instrument, warn};

use slab_core::api::Backend;
use slab_proto::slab::ipc::v1 as pb;

use super::{
    extract_request_id, load_model_for_backend, reload_library_for_backend, runtime_to_status,
    unload_model_for_backend, GrpcServiceImpl,
};

#[tonic::async_trait]
impl pb::whisper_service_server::WhisperService for GrpcServiceImpl {
    #[instrument(skip_all, fields(request_id, backend = "ggml.whisper"))]
    async fn transcribe(
        &self,
        request: Request<pb::TranscribeRequest>,
    ) -> Result<Response<pb::TranscribeResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        let req = request.into_inner();
        if req.path.is_empty() {
            warn!("transcribe rejected: audio file path is empty");
            return Err(Status::invalid_argument("audio file path is empty"));
        }

        let op_options = build_whisper_inference_options(&req)?;
        let vad_enabled = req.vad.as_ref().is_some_and(|v| v.enabled);
        let decode_configured = req.decode.is_some();

        debug!(
            audio_path = %req.path,
            vad_enabled,
            decode_configured,
            "whisper transcribe request received"
        );

        // Capture the current span so it can be entered inside the
        // spawn_blocking closure used by the CpuStage; without this the
        // preprocess logs would lose the per-request request_id/backend fields.
        let preprocess_span = tracing::Span::current();
        let output = slab_core::api::backend(Backend::GGMLWhisper)
            .inference()
            .options(slab_core::Payload::Json(op_options))
            .input(slab_core::Payload::Text(req.path.into()))
            .preprocess("ffmpeg.to_pcm_f32le", move |payload| {
                let _guard = preprocess_span.enter();
                convert_to_pcm_f32le(payload)
            })
            .run_wait()
            .await
            .map_err(|e| {
                error!(error = %e, "whisper inference failed");
                runtime_to_status(e)
            })?;

        let text = String::from_utf8_lossy(&output).into_owned();
        info!(output_len = text.len(), "whisper transcription completed");
        Ok(Response::new(pb::TranscribeResponse { text }))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.whisper"))]
    async fn load_model(
        &self,
        request: Request<pb::ModelLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("whisper load_model request received");
        let status = load_model_for_backend(Backend::GGMLWhisper, request.into_inner()).await?;
        Ok(Response::new(status))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.whisper"))]
    async fn unload_model(
        &self,
        request: Request<pb::ModelUnloadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("whisper unload_model request received");
        let status = unload_model_for_backend(Backend::GGMLWhisper).await?;
        Ok(Response::new(status))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.whisper"))]
    async fn reload_library(
        &self,
        request: Request<pb::ReloadLibraryRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("whisper reload_library request received");
        let status = reload_library_for_backend(Backend::GGMLWhisper, request.into_inner()).await?;
        Ok(Response::new(status))
    }
}

fn build_whisper_inference_options(
    req: &pb::TranscribeRequest,
) -> Result<serde_json::Value, Status> {
    let mut options = serde_json::Map::new();

    if let Some(vad) = req.vad.as_ref() {
        if vad.enabled {
            let model_path = vad.model_path.trim();
            if model_path.is_empty() {
                return Err(Status::invalid_argument(
                    "vad.model_path is required when VAD is enabled",
                ));
            }

            let mut vad_json = serde_json::Map::new();
            vad_json.insert(
                "model_path".to_owned(),
                serde_json::Value::String(model_path.to_owned()),
            );

            if let Some(params) = vad.params.as_ref() {
                if let Some(threshold) = params.threshold {
                    if !(0.0..=1.0).contains(&threshold) {
                        return Err(Status::invalid_argument(
                            "vad.threshold must be between 0.0 and 1.0",
                        ));
                    }
                    vad_json.insert("threshold".to_owned(), serde_json::json!(threshold));
                }

                for (name, value) in [
                    ("vad.min_speech_duration_ms", params.min_speech_duration_ms),
                    (
                        "vad.min_silence_duration_ms",
                        params.min_silence_duration_ms,
                    ),
                    ("vad.speech_pad_ms", params.speech_pad_ms),
                ] {
                    if let Some(v) = value {
                        if v < 0 {
                            return Err(Status::invalid_argument(format!("{name} must be >= 0")));
                        }
                        vad_json.insert(
                            name.trim_start_matches("vad.").to_owned(),
                            serde_json::json!(v),
                        );
                    }
                }

                if let Some(max_speech_duration_s) = params.max_speech_duration_s {
                    if max_speech_duration_s <= 0.0 {
                        return Err(Status::invalid_argument(
                            "vad.max_speech_duration_s must be > 0.0",
                        ));
                    }
                    vad_json.insert(
                        "max_speech_duration_s".to_owned(),
                        serde_json::json!(max_speech_duration_s),
                    );
                }

                if let Some(samples_overlap) = params.samples_overlap {
                    if samples_overlap < 0.0 {
                        return Err(Status::invalid_argument(
                            "vad.samples_overlap must be >= 0.0",
                        ));
                    }
                    vad_json.insert(
                        "samples_overlap".to_owned(),
                        serde_json::json!(samples_overlap),
                    );
                }
            }

            options.insert("vad".to_owned(), serde_json::Value::Object(vad_json));
        }
    }

    if let Some(decode) = req.decode.as_ref() {
        let mut decode_json = serde_json::Map::new();

        for (name, value) in [
            ("decode.offset_ms", decode.offset_ms),
            ("decode.duration_ms", decode.duration_ms),
            ("decode.max_len", decode.max_len),
            ("decode.max_tokens", decode.max_tokens),
        ] {
            if let Some(v) = value {
                if v < 0 {
                    return Err(Status::invalid_argument(format!("{name} must be >= 0")));
                }
                decode_json.insert(
                    name.trim_start_matches("decode.").to_owned(),
                    serde_json::json!(v),
                );
            }
        }

        if let Some(word_thold) = decode.word_thold {
            if !(0.0..=1.0).contains(&word_thold) {
                return Err(Status::invalid_argument(
                    "decode.word_thold must be between 0.0 and 1.0",
                ));
            }
            decode_json.insert("word_thold".to_owned(), serde_json::json!(word_thold));
        }

        for (name, value) in [
            ("decode.temperature", decode.temperature),
            ("decode.temperature_inc", decode.temperature_inc),
        ] {
            if let Some(v) = value {
                if v < 0.0 {
                    return Err(Status::invalid_argument(format!("{name} must be >= 0.0")));
                }
                decode_json.insert(
                    name.trim_start_matches("decode.").to_owned(),
                    serde_json::json!(v),
                );
            }
        }

        for (name, value) in [
            ("decode.no_context", decode.no_context),
            ("decode.no_timestamps", decode.no_timestamps),
            ("decode.token_timestamps", decode.token_timestamps),
            ("decode.split_on_word", decode.split_on_word),
            ("decode.suppress_nst", decode.suppress_nst),
            ("decode.tdrz_enable", decode.tdrz_enable),
        ] {
            if let Some(v) = value {
                decode_json.insert(
                    name.trim_start_matches("decode.").to_owned(),
                    serde_json::json!(v),
                );
            }
        }

        for (name, value) in [
            ("decode.entropy_thold", decode.entropy_thold),
            ("decode.logprob_thold", decode.logprob_thold),
            ("decode.no_speech_thold", decode.no_speech_thold),
        ] {
            if let Some(v) = value {
                decode_json.insert(
                    name.trim_start_matches("decode.").to_owned(),
                    serde_json::json!(v),
                );
            }
        }

        if !decode_json.is_empty() {
            options.insert("decode".to_owned(), serde_json::Value::Object(decode_json));
        }
    }

    Ok(serde_json::Value::Object(options))
}

fn convert_to_pcm_f32le(payload: slab_core::Payload) -> Result<slab_core::Payload, String> {
    let path = payload
        .to_str()
        .map_err(|e| format!("invalid payload for preprocess: {e}"))?;

    debug!(audio_path = %path, "running ffmpeg PCM conversion");

    let output = std::process::Command::new("ffmpeg")
        .arg("-i")
        .arg(path)
        .args([
            "-vn",
            "-f",
            "f32le",
            "-acodec",
            "pcm_f32le",
            "-ar",
            "16000",
            "-ac",
            "1",
            "-",
        ])
        .output()
        .map_err(|e| format!("ffmpeg start failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = format!(
            "ffmpeg failed with status {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        );
        warn!(audio_path = %path, "{}", msg);
        return Err(msg);
    }

    let pcm_bytes = output.stdout;
    if pcm_bytes.len() % std::mem::size_of::<f32>() != 0 {
        return Err(format!("PCM not aligned: {} bytes", pcm_bytes.len()));
    }

    let samples: Vec<f32> = cast_slice::<u8, f32>(&pcm_bytes).to_vec();
    debug!(samples = samples.len(), "ffmpeg PCM conversion succeeded");
    Ok(slab_core::Payload::F32(std::sync::Arc::from(
        samples.as_slice(),
    )))
}
