use std::str::FromStr;

use strum::{Display, EnumIter, EnumString, IntoStaticStr};

use crate::api::v1::backend::schema::{BackendTypeQuery, DownloadLibRequest, ReloadLibRequest};

/// Canonical backend identifier understood by slab-runtime.
///
/// This mirrors the former `slab_core::api::Backend` enum which was removed
/// during the slab-core refactoring.  Backend identity is now a server-level
/// concern; the slab-core API uses capability-based `ModelSpec` instead.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Display,
    EnumString,
    EnumIter,
    IntoStaticStr,
    strum::AsRefStr,
)]
#[strum(serialize_all = "snake_case")]
pub enum BackendId {
    #[strum(serialize = "ggml.llama")]
    GGMLLlama,
    #[strum(serialize = "ggml.whisper")]
    GGMLWhisper,
    #[strum(serialize = "ggml.diffusion")]
    GGMLDiffusion,
    #[strum(serialize = "candle.llama")]
    CandleLlama,
    #[strum(serialize = "candle.whisper")]
    CandleWhisper,
    #[strum(serialize = "candle.diffusion")]
    CandleDiffusion,
    #[strum(serialize = "onnx")]
    Onnx,
}

#[derive(Debug, Clone)]
pub struct BackendStatusQuery {
    pub backend_id: String,
}

#[derive(Debug, Clone)]
pub struct DownloadBackendLibCommand {
    pub backend_id: String,
    pub target_dir: String,
}

#[derive(Debug, Clone)]
pub struct ReloadBackendLibCommand {
    pub backend_id: String,
    pub lib_path: String,
    pub model_path: String,
    pub num_workers: u32,
}

#[derive(Debug, Clone)]
pub struct BackendStatusView {
    pub backend: String,
    pub status: String,
}

impl From<BackendTypeQuery> for BackendStatusQuery {
    fn from(query: BackendTypeQuery) -> Self {
        Self {
            backend_id: query.backend_id,
        }
    }
}

impl From<DownloadLibRequest> for DownloadBackendLibCommand {
    fn from(request: DownloadLibRequest) -> Self {
        Self {
            backend_id: request.backend_id,
            target_dir: request.target_dir,
        }
    }
}

impl From<ReloadLibRequest> for ReloadBackendLibCommand {
    fn from(request: ReloadLibRequest) -> Self {
        Self {
            backend_id: request.backend_id,
            lib_path: request.lib_path,
            model_path: request.model_path,
            num_workers: request.num_workers,
        }
    }
}
