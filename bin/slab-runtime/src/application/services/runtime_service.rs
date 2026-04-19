use crate::domain::services::ExecutionHub;
use crate::infra::config::EnabledBackends;

use super::{
    CandleDiffusionService, CandleTransformersService, GgmlDiffusionService, GgmlLlamaService,
    GgmlWhisperService, OnnxService,
};

#[derive(Clone)]
pub struct RuntimeApplication {
    ggml_llama: GgmlLlamaService,
    ggml_whisper: GgmlWhisperService,
    ggml_diffusion: GgmlDiffusionService,
    candle_transformers: CandleTransformersService,
    candle_diffusion: CandleDiffusionService,
    onnx: OnnxService,
}

impl RuntimeApplication {
    pub fn new(execution: ExecutionHub, _enabled_backends: EnabledBackends) -> Self {
        Self {
            ggml_llama: GgmlLlamaService::new(execution.clone()),
            ggml_whisper: GgmlWhisperService::new(execution.clone()),
            ggml_diffusion: GgmlDiffusionService::new(execution.clone()),
            candle_transformers: CandleTransformersService::new(execution.clone()),
            candle_diffusion: CandleDiffusionService::new(execution.clone()),
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

    pub(crate) fn candle_transformers(&self) -> &CandleTransformersService {
        &self.candle_transformers
    }

    pub(crate) fn candle_diffusion(&self) -> &CandleDiffusionService {
        &self.candle_diffusion
    }

    pub(crate) fn onnx(&self) -> &OnnxService {
        &self.onnx
    }
}
