use crate::domain::models::EnabledBackends;
use crate::domain::runtime::CoreError;
use crate::domain::services::ExecutionHub;

use super::{
    CandleDiffusionService, CandleTransformersService, GgmlDiffusionService, GgmlLlamaService,
    GgmlWhisperService, OnnxService, RuntimeApplicationError,
};

#[derive(Clone, Copy, Debug, Default)]
struct RuntimeServiceAvailability {
    ggml_llama: bool,
    ggml_whisper: bool,
    ggml_diffusion: bool,
    candle_llama: bool,
    candle_whisper: bool,
    candle_diffusion: bool,
    onnx_text: bool,
    onnx_embedding: bool,
}

impl RuntimeServiceAvailability {
    fn from_enabled_backends(backends: &EnabledBackends) -> Self {
        Self {
            ggml_llama: backends.contains("ggml.llama"),
            ggml_whisper: backends.contains("ggml.whisper"),
            ggml_diffusion: backends.contains("ggml.diffusion"),
            candle_llama: backends.contains("candle.llama"),
            candle_whisper: backends.contains("candle.whisper"),
            candle_diffusion: backends.contains("candle.diffusion"),
            onnx_text: backends.contains("onnx.text"),
            onnx_embedding: backends.contains("onnx.embedding"),
        }
    }
}

#[derive(Clone)]
pub struct RuntimeApplication {
    availability: RuntimeServiceAvailability,
    ggml_llama: GgmlLlamaService,
    ggml_whisper: GgmlWhisperService,
    ggml_diffusion: GgmlDiffusionService,
    candle_transformers: CandleTransformersService,
    candle_diffusion: CandleDiffusionService,
    onnx: OnnxService,
}

impl RuntimeApplication {
    pub fn new(execution: ExecutionHub) -> Self {
        let availability =
            RuntimeServiceAvailability::from_enabled_backends(execution.enabled_backends());
        Self {
            availability,
            ggml_llama: GgmlLlamaService::new(execution.clone()),
            ggml_whisper: GgmlWhisperService::new(execution.clone()),
            ggml_diffusion: GgmlDiffusionService::new(execution.clone()),
            candle_transformers: CandleTransformersService::new(execution.clone()),
            candle_diffusion: CandleDiffusionService::new(execution.clone()),
            onnx: OnnxService::new(execution),
        }
    }

    fn require_backend(
        &self,
        enabled: bool,
        backend: &'static str,
    ) -> Result<(), RuntimeApplicationError> {
        if enabled {
            return Ok(());
        }

        Err(RuntimeApplicationError::Runtime(CoreError::BackendDisabled {
            backend: backend.to_owned(),
        }))
    }

    pub(crate) fn ggml_llama(&self) -> Result<&GgmlLlamaService, RuntimeApplicationError> {
        self.require_backend(self.availability.ggml_llama, "ggml.llama")?;
        Ok(&self.ggml_llama)
    }

    pub(crate) fn ggml_whisper(&self) -> Result<&GgmlWhisperService, RuntimeApplicationError> {
        self.require_backend(self.availability.ggml_whisper, "ggml.whisper")?;
        Ok(&self.ggml_whisper)
    }

    pub(crate) fn ggml_diffusion(&self) -> Result<&GgmlDiffusionService, RuntimeApplicationError> {
        self.require_backend(self.availability.ggml_diffusion, "ggml.diffusion")?;
        Ok(&self.ggml_diffusion)
    }

    pub(crate) fn candle_llama(
        &self,
    ) -> Result<&CandleTransformersService, RuntimeApplicationError> {
        self.require_backend(self.availability.candle_llama, "candle.llama")?;
        Ok(&self.candle_transformers)
    }

    pub(crate) fn candle_whisper(
        &self,
    ) -> Result<&CandleTransformersService, RuntimeApplicationError> {
        self.require_backend(self.availability.candle_whisper, "candle.whisper")?;
        Ok(&self.candle_transformers)
    }

    pub(crate) fn candle_diffusion(
        &self,
    ) -> Result<&CandleDiffusionService, RuntimeApplicationError> {
        self.require_backend(self.availability.candle_diffusion, "candle.diffusion")?;
        Ok(&self.candle_diffusion)
    }

    pub(crate) fn onnx_text(&self) -> Result<&OnnxService, RuntimeApplicationError> {
        self.require_backend(self.availability.onnx_text, "onnx.text")?;
        Ok(&self.onnx)
    }

    pub(crate) fn onnx_embedding(&self) -> Result<&OnnxService, RuntimeApplicationError> {
        self.require_backend(self.availability.onnx_embedding, "onnx.embedding")?;
        Ok(&self.onnx)
    }
}
