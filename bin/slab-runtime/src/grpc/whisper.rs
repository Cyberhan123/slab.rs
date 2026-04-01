use std::path::PathBuf;
use std::sync::Arc;

use bytemuck::cast_slice;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, instrument, warn};

use slab_runtime_core::api::AudioTranscriptionRequest;
use slab_proto::slab::ipc::v1 as pb;
use slab_types::{WhisperDecodeOptions, WhisperVadOptions, WhisperVadParams};

use super::{BackendKind, GrpcServiceImpl, extract_request_id, runtime_to_status};

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
        if req.path.trim().is_empty() {
            warn!("transcribe rejected: audio file path is empty");
            return Err(Status::invalid_argument("audio file path is empty"));
        }

        let (vad, decode) =
            build_whisper_inference_options(&req).map_err(Status::invalid_argument)?;
        let vad_enabled = vad.as_ref().is_some_and(|value| value.enabled);
        let decode_configured = decode.is_some();

        debug!(
            audio_path = %req.path,
            vad_enabled,
            decode_configured,
            "whisper transcribe request received"
        );

        let path = req.path.clone();
        let pcm_samples = tokio::task::spawn_blocking(move || convert_file_to_pcm_f32le(&path))
            .await
            .map_err(|error| Status::internal(format!("ffmpeg worker failed: {error}")))?
            .map_err(Status::internal)?;

        let pipeline = self.pipeline_for_backend(BackendKind::Whisper).await?;
        let response = pipeline
            .run_audio_transcription(AudioTranscriptionRequest {
                audio_path: PathBuf::from(req.path),
                pcm_samples: Some(pcm_samples),
                language: None,
                prompt: None,
                vad,
                decode,
                options: Default::default(),
            })
            .await
            .map_err(|error| {
                error!(error = %error, "whisper transcription failed");
                runtime_to_status(error)
            })?;

        info!(output_len = response.text.len(), "whisper transcription completed");
        Ok(Response::new(pb::TranscribeResponse { text: response.text }))
    }

    #[instrument(skip_all, fields(request_id, backend = "ggml.whisper"))]
    async fn load_model(
        &self,
        request: Request<pb::ModelLoadRequest>,
    ) -> Result<Response<pb::ModelStatusResponse>, Status> {
        let request_id = extract_request_id(request.metadata());
        tracing::Span::current().record("request_id", &request_id);

        debug!("whisper load_model request received");
        let status =
            self.load_model_for_backend(BackendKind::Whisper, request.into_inner()).await?;
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
        let _ = request.into_inner();
        let status = self.unload_model_for_backend(BackendKind::Whisper).await?;
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
        let status =
            self.reload_library_for_backend(BackendKind::Whisper, request.into_inner()).await?;
        Ok(Response::new(status))
    }
}

fn build_whisper_inference_options(
    req: &pb::TranscribeRequest,
) -> Result<(Option<WhisperVadOptions>, Option<WhisperDecodeOptions>), String> {
    let vad = if let Some(vad) = req.vad.as_ref() {
        if !vad.enabled {
            None
        } else {
            let model_path = vad.model_path.trim();
            if model_path.is_empty() {
                return Err("vad.model_path is required when VAD is enabled".to_owned());
            }

            let params = if let Some(params) = vad.params.as_ref() {
                if let Some(threshold) = params.threshold
                    && !(0.0..=1.0).contains(&threshold)
                {
                    return Err("vad.threshold must be between 0.0 and 1.0".to_owned());
                }

                for (name, value) in [
                    ("vad.min_speech_duration_ms", params.min_speech_duration_ms),
                    ("vad.min_silence_duration_ms", params.min_silence_duration_ms),
                    ("vad.speech_pad_ms", params.speech_pad_ms),
                ] {
                    if let Some(value) = value
                        && value < 0
                    {
                        return Err(format!("{name} must be >= 0"));
                    }
                }

                if let Some(max_speech_duration_s) = params.max_speech_duration_s
                    && max_speech_duration_s <= 0.0
                {
                    return Err("vad.max_speech_duration_s must be > 0.0".to_owned());
                }

                if let Some(samples_overlap) = params.samples_overlap
                    && samples_overlap < 0.0
                {
                    return Err("vad.samples_overlap must be >= 0.0".to_owned());
                }

                Some(WhisperVadParams {
                    threshold: params.threshold,
                    min_speech_duration_ms: params.min_speech_duration_ms,
                    min_silence_duration_ms: params.min_silence_duration_ms,
                    max_speech_duration_s: params.max_speech_duration_s,
                    speech_pad_ms: params.speech_pad_ms,
                    samples_overlap: params.samples_overlap,
                })
            } else {
                None
            };

            Some(WhisperVadOptions {
                enabled: true,
                model_path: Some(PathBuf::from(model_path)),
                params,
            })
        }
    } else {
        None
    };

    let decode = if let Some(decode) = req.decode.as_ref() {
        for (name, value) in [
            ("decode.offset_ms", decode.offset_ms),
            ("decode.duration_ms", decode.duration_ms),
            ("decode.max_len", decode.max_len),
            ("decode.max_tokens", decode.max_tokens),
        ] {
            if let Some(value) = value
                && value < 0
            {
                return Err(format!("{name} must be >= 0"));
            }
        }

        if let Some(word_thold) = decode.word_thold
            && !(0.0..=1.0).contains(&word_thold)
        {
            return Err("decode.word_thold must be between 0.0 and 1.0".to_owned());
        }

        for (name, value) in [
            ("decode.temperature", decode.temperature),
            ("decode.temperature_inc", decode.temperature_inc),
        ] {
            if let Some(value) = value
                && value < 0.0
            {
                return Err(format!("{name} must be >= 0.0"));
            }
        }

        Some(WhisperDecodeOptions {
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
        })
    } else {
        None
    };

    Ok((vad, decode))
}

fn convert_file_to_pcm_f32le(path: &str) -> Result<Arc<[f32]>, String> {
    debug!(audio_path = %path, "running ffmpeg PCM conversion");

    let output = std::process::Command::new("ffmpeg")
        .arg("-i")
        .arg(path)
        .args(["-vn", "-f", "f32le", "-acodec", "pcm_f32le", "-ar", "16000", "-ac", "1", "-"])
        .output()
        .map_err(|error| format!("ffmpeg start failed: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = format!(
            "ffmpeg failed with status {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        );
        warn!(audio_path = %path, "{message}");
        return Err(message);
    }

    let pcm_bytes = output.stdout;
    if pcm_bytes.len() % std::mem::size_of::<f32>() != 0 {
        return Err(format!("PCM not aligned: {} bytes", pcm_bytes.len()));
    }

    let samples: Vec<f32> = cast_slice::<u8, f32>(&pcm_bytes).to_vec();
    debug!(samples = samples.len(), "ffmpeg PCM conversion succeeded");
    Ok(Arc::from(samples))
}
