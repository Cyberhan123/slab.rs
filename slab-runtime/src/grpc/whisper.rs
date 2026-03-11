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

        debug!(audio_path = %req.path, "whisper transcribe request received");

        // Capture the current span so it can be entered inside the
        // spawn_blocking closure used by the CpuStage; without this the
        // preprocess logs would lose the per-request request_id/backend fields.
        let preprocess_span = tracing::Span::current();
        let output = slab_core::api::backend(Backend::GGMLWhisper)
            .inference()
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
