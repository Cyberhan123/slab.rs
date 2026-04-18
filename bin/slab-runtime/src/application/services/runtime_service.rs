use crate::domain::services::ExecutionHub;
use crate::infra::config::EnabledBackends;

use super::{
    CandleService, GgmlDiffusionService, GgmlLlamaService, GgmlWhisperService, OnnxService,
};

#[derive(Clone)]
pub struct RuntimeApplication {
    ggml_llama: GgmlLlamaService,
    ggml_whisper: GgmlWhisperService,
    ggml_diffusion: GgmlDiffusionService,
    candle: CandleService,
    onnx: OnnxService,
}

impl RuntimeApplication {
    pub fn new(execution: ExecutionHub, _enabled_backends: EnabledBackends) -> Self {
        Self {
            ggml_llama: GgmlLlamaService::new(execution.clone()),
            ggml_whisper: GgmlWhisperService::new(execution.clone()),
            ggml_diffusion: GgmlDiffusionService::new(execution.clone()),
            candle: CandleService::new(execution.clone()),
            onnx: OnnxService::new(execution),
        }
    }

    pub(crate) fn ggml_llama(&self) -> &GgmlLlamaService {
        &self.ggml_llama
    }

    pub(crate) fn ggml_whisper(&self) -> &GgmlWhisperService {
        &self.ggml_whisper
    }

    pub(crate) fn ggml_diffusion(&self) -> &GgmlDiffusionService {
        &self.ggml_diffusion
    }

    pub(crate) fn candle(&self) -> &CandleService {
        &self.candle
    }

    pub(crate) fn onnx(&self) -> &OnnxService {
        &self.onnx
    }
}
