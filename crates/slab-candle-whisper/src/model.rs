use candle_core::{IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self, Config};

use crate::config::WhisperWeightSource;
use crate::error::CandleWhisperError;

pub(crate) enum WhisperModel {
    Normal(whisper::model::Whisper),
    Quantized(whisper::quantized_model::Whisper),
}

impl WhisperModel {
    pub(crate) fn load(
        source: WhisperWeightSource,
        model_path: &std::path::Path,
        config: Config,
        device: &candle_core::Device,
    ) -> Result<Self, CandleWhisperError> {
        match source {
            WhisperWeightSource::Safetensors => {
                let vb = unsafe {
                    VarBuilder::from_mmaped_safetensors(&[model_path], whisper::DTYPE, device)
                        .map_err(|error| {
                            CandleWhisperError::load_model(model_path.display(), error)
                        })?
                };
                whisper::model::Whisper::load(&vb, config)
                    .map(Self::Normal)
                    .map_err(|error| CandleWhisperError::load_model(model_path.display(), error))
            }
            WhisperWeightSource::QuantizedGguf => {
                let vb = candle_transformers::quantized_var_builder::VarBuilder::from_gguf(
                    model_path, device,
                )
                .map_err(|error| CandleWhisperError::load_model(model_path.display(), error))?;
                whisper::quantized_model::Whisper::load(&vb, config)
                    .map(Self::Quantized)
                    .map_err(|error| CandleWhisperError::load_model(model_path.display(), error))
            }
        }
    }

    pub(crate) fn config(&self) -> &Config {
        match self {
            Self::Normal(model) => &model.config,
            Self::Quantized(model) => &model.config,
        }
    }

    pub(crate) fn encoder_forward(&mut self, mel: &Tensor) -> candle_core::Result<Tensor> {
        let mel = mel.squeeze(0)?;
        match self {
            Self::Normal(model) => model.encoder.forward(&mel, true)?.unsqueeze(0),
            Self::Quantized(model) => model.encoder.forward(&mel, true)?.unsqueeze(0),
        }
    }

    pub(crate) fn decoder_forward(
        &mut self,
        tokens: &Tensor,
        audio_features: &Tensor,
        flush: bool,
    ) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(model) => model.decoder.forward(tokens, audio_features, flush),
            Self::Quantized(model) => model.decoder.forward(tokens, audio_features, flush),
        }
    }

    pub(crate) fn final_linear(&self, logits: &Tensor) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(model) => model.decoder.final_linear(logits),
            Self::Quantized(model) => model.decoder.final_linear(logits),
        }
    }

    pub(crate) fn last_token_logits(
        &self,
        decoder_output: &Tensor,
        token_count: usize,
    ) -> candle_core::Result<Tensor> {
        self.final_linear(&decoder_output.i((.., token_count - 1, ..))?)?.squeeze(0)
    }
}
