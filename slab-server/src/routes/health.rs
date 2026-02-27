//! Health / heartbeat endpoint.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use utoipa::OpenApi;
use tracing::{debug, warn};

use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(paths(get_health, get_diagnostics))]
pub struct HealthApi;

/// Register health-check routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(get_health))
        .route("/diagnostics", get(get_diagnostics))
}

/// Heartbeat endpoint.
///
/// Returns `{"status": "ok", "version": "..."}` with HTTP 200.
/// Load-balancers and monitoring systems should poll this endpoint.
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Server is healthy", body = Value)
    )
)]
pub async fn get_health() -> Json<Value> {
    Json(json!({
        "status":  "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// System diagnostics endpoint.
///
/// Returns detailed information about backend configuration and readiness.
/// Useful for troubleshooting issues with AI backends.
#[utoipa::path(
    get,
    path = "/diagnostics",
    tag = "health",
    responses(
        (status = 200, description = "Diagnostics information", body = Value)
    )
)]
pub async fn get_diagnostics(State(state): State<Arc<AppState>>) -> Json<Value> {
    let mut backends = json!({});

    // Check Whisper backend
    let whisper_ready = slab_core::api::is_backend_ready(slab_core::api::Backend::GGMLWhisper).await;
    backends["whisper"] = match &whisper_ready {
        Ok(true) => json!({
            "configured": state.config.whisper_lib_dir.is_some(),
            "library_path": state.config.whisper_lib_dir.as_ref().map(|p: &String| p.as_str()),
            "ready": true,
            "status": "ready"
        }),
        Ok(false) => json!({
            "configured": state.config.whisper_lib_dir.is_some(),
            "library_path": state.config.whisper_lib_dir.as_ref().map(|p: &String| p.as_str()),
            "ready": false,
            "status": "library_loaded_no_model",
            "message": "Whisper library is loaded but no model has been loaded. Load a model via the admin API to enable transcription."
        }),
        Err(e) => {
            warn!(error = %e, "Failed to check Whisper backend readiness for diagnostics");
            json!({
                "configured": state.config.whisper_lib_dir.is_some(),
                "library_path": state.config.whisper_lib_dir.as_ref().map(|p: &String| p.as_str()),
                "ready": false,
                "status": "error",
                "error": e.to_string()
            })
        }
    };

    // Check Llama backend
    let llama_configured = state.config.llama_lib_dir.is_some();
    backends["llama"] = json!({
        "configured": llama_configured,
        "library_path": state.config.llama_lib_dir.as_ref().map(|p: &String| p.as_str()),
        "status": if llama_configured { "configured" } else { "not_configured" }
    });

    // Check Diffusion backend
    let diffusion_ready = slab_core::api::is_backend_ready(slab_core::api::Backend::GGMLDiffusion).await;
    backends["diffusion"] = match &diffusion_ready {
        Ok(true) => json!({
            "configured": state.config.diffusion_lib_dir.is_some(),
            "library_path": state.config.diffusion_lib_dir.as_ref().map(|p: &String| p.as_str()),
            "ready": true,
            "status": "ready"
        }),
        Ok(false) => json!({
            "configured": state.config.diffusion_lib_dir.is_some(),
            "library_path": state.config.diffusion_lib_dir.as_ref().map(|p: &String| p.as_str()),
            "ready": false,
            "status": "library_loaded_no_model"
        }),
        Err(e) => {
            debug!(error = %e, "Failed to check Diffusion backend readiness for diagnostics");
            json!({
                "configured": state.config.diffusion_lib_dir.is_some(),
                "library_path": state.config.diffusion_lib_dir.as_ref().map(|p: &String| p.as_str()),
                "ready": false,
                "status": "error",
                "error": e.to_string()
            })
        }
    };

    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "environment": {
            "whisper_lib_dir": state.config.whisper_lib_dir.as_deref(),
            "llama_lib_dir": state.config.llama_lib_dir.as_deref(),
            "diffusion_lib_dir": state.config.diffusion_lib_dir.as_deref(),
            "bind_address": state.config.bind_address,
            "transport_mode": state.config.transport_mode,
            "log_level": state.config.log_level,
        },
        "backends": backends,
        "recommendations": generate_recommendations(&state, &whisper_ready, &diffusion_ready)
    }))
}

fn generate_recommendations(
    state: &AppState,
    whisper_ready: &Result<bool, slab_core::api::RuntimeError>,
    diffusion_ready: &Result<bool, slab_core::api::RuntimeError>,
) -> Value {
    let mut recommendations = Vec::new();

    // Whisper recommendations
    if state.config.whisper_lib_dir.is_none() {
        recommendations.push(json!({
            "backend": "whisper",
            "severity": "warning",
            "message": "SLAB_WHISPER_LIB_DIR is not set. Audio transcription will not be available.",
            "action": "Set the SLAB_WHISPER_LIB_DIR environment variable to the directory containing libwhisper.so"
        }));
    } else if let Ok(false) = whisper_ready {
        recommendations.push(json!({
            "backend": "whisper",
            "severity": "info",
            "message": "Whisper library is loaded but no model has been loaded.",
            "action": "Load a Whisper model via the admin API (POST /admin/v1/backends/ggml.whisper/model) to enable transcription"
        }));
    }

    // Llama recommendations
    if state.config.llama_lib_dir.is_none() {
        recommendations.push(json!({
            "backend": "llama",
            "severity": "info",
            "message": "SLAB_LLAMA_LIB_DIR is not set. Text generation will not be available.",
            "action": "Set the SLAB_LLAMA_LIB_DIR environment variable to enable Llama backend"
        }));
    }

    // Diffusion recommendations
    if state.config.diffusion_lib_dir.is_none() {
        recommendations.push(json!({
            "backend": "diffusion",
            "severity": "info",
            "message": "SLAB_DIFFUSION_LIB_DIR is not set. Image generation will not be available.",
            "action": "Set the SLAB_DIFFUSION_LIB_DIR environment variable to enable Stable Diffusion backend"
        }));
    }

    json!(recommendations)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn health_response_has_ok_status() {
        let Json(body) = get_health().await;
        assert_eq!(body["status"], "ok");
    }

    #[tokio::test]
    async fn health_response_has_version() {
        let Json(body) = get_health().await;
        assert!(!body["version"].as_str().unwrap_or("").is_empty());
    }
}
