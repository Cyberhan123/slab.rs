use std::path::{Path, PathBuf};

use candle_core::{Device, Tensor};
use candle_transformers::models::whisper::{self, Config, audio};
use tokenizers::Tokenizer;

use super::config::{CandleWhisperLoadConfig, TranscriptionRequest, TranscriptionResponse};
use super::decoder::WhisperDecoder;
use super::error::CandleWhisperError;
use super::model::WhisperModel;
use crate::device::resolve_device;
use crate::runtime::CandleRuntimeEngine;

pub struct CandleWhisperEngine {
    model: Option<WhisperModel>,
    tokenizer: Option<Tokenizer>,
    config: Option<Config>,
    mel_filters: Option<Vec<f32>>,
    device: Option<Device>,
}

impl CandleWhisperEngine {
    pub fn new() -> Self {
        Self { model: None, tokenizer: None, config: None, mel_filters: None, device: None }
    }

    fn model_dir(model_path: &Path) -> &Path {
        if model_path.is_dir() {
            model_path
        } else {
            model_path.parent().unwrap_or_else(|| Path::new("."))
        }
    }

    fn tokenizer_path(model_path: &Path, tokenizer_path: Option<PathBuf>) -> PathBuf {
        tokenizer_path.unwrap_or_else(|| Self::model_dir(model_path).join("tokenizer.json"))
    }

    fn config_path(model_path: &Path, config_path: Option<PathBuf>) -> PathBuf {
        config_path.unwrap_or_else(|| Self::model_dir(model_path).join("config.json"))
    }

    fn mel_filters_path(model_path: &Path, mel_filters_path: Option<PathBuf>) -> PathBuf {
        mel_filters_path.unwrap_or_else(|| Self::model_dir(model_path).join("mel_filters.npz"))
    }

    fn load_config(path: &Path) -> Result<Config, CandleWhisperError> {
        let data =
            std::fs::read_to_string(path).map_err(|error| CandleWhisperError::LoadModel {
                model_path: path.display().to_string(),
                message: error.to_string(),
            })?;
        serde_json::from_str(&data).map_err(|error| CandleWhisperError::LoadModel {
            model_path: path.display().to_string(),
            message: error.to_string(),
        })
    }

    fn load_mel_filters(path: &Path, config: &Config) -> Result<Vec<f32>, CandleWhisperError> {
        let n_fft_half = whisper::N_FFT / 2 + 1;
        let n_mels = config.num_mel_bins;
        if !path.exists() {
            return Err(CandleWhisperError::InvalidAssetLayout {
                path: path.display().to_string(),
                message: "missing mel_filters.npz".to_owned(),
            });
        }
        candle_core::npy::NpzTensors::new(path)
            .and_then(|npz| {
                let key = format!("mel_{n_mels}");
                npz.get(&key)?.ok_or_else(|| {
                    candle_core::Error::Msg(format!("mel_filters.npz missing key '{key}'"))
                })
            })
            .and_then(|tensor| tensor.flatten_all()?.to_vec1::<f32>())
            .and_then(|filters| {
                let expected = n_mels * n_fft_half;
                if filters.len() == expected {
                    Ok(filters)
                } else {
                    Err(candle_core::Error::Msg(format!(
                        "mel filter length {}, expected {expected}",
                        filters.len()
                    )))
                }
            })
            .map_err(|error| CandleWhisperError::InvalidAssetLayout {
                path: path.display().to_string(),
                message: error.to_string(),
            })
    }
}

impl Default for CandleWhisperEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CandleRuntimeEngine for CandleWhisperEngine {
    type Error = CandleWhisperError;
    type InferenceRequest = TranscriptionRequest;
    type InferenceResponse = TranscriptionResponse;
    type LoadConfig = CandleWhisperLoadConfig;

    fn load_model(&mut self, config: Self::LoadConfig) -> Result<(), Self::Error> {
        let device = resolve_device(config.device)
            .map_err(|error| CandleWhisperError::load_model(config.model_path.display(), error))?;
        let config_path = Self::config_path(&config.model_path, config.config_path);
        let model_config = Self::load_config(&config_path)?;
        let tokenizer_path = Self::tokenizer_path(&config.model_path, config.tokenizer_path);
        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|error| {
            CandleWhisperError::LoadTokenizer {
                tokenizer_path: tokenizer_path.display().to_string(),
                message: error.to_string(),
            }
        })?;
        let mel_filters_path = Self::mel_filters_path(&config.model_path, config.mel_filters_path);
        let mel_filters = Self::load_mel_filters(&mel_filters_path, &model_config)?;
        let model = WhisperModel::load(
            config.weight_source,
            &config.model_path,
            model_config.clone(),
            &device,
        )?;

        self.model = Some(model);
        self.tokenizer = Some(tokenizer);
        self.config = Some(model_config);
        self.mel_filters = Some(mel_filters);
        self.device = Some(device);
        Ok(())
    }

    fn unload_model(&mut self) {
        self.model = None;
        self.tokenizer = None;
        self.config = None;
        self.mel_filters = None;
        self.device = None;
    }

    fn is_model_loaded(&self) -> bool {
        self.model.is_some()
            && self.tokenizer.is_some()
            && self.config.is_some()
            && self.device.is_some()
    }

    fn infer(
        &mut self,
        request: Self::InferenceRequest,
    ) -> Result<Self::InferenceResponse, Self::Error> {
        if request.samples.is_empty() {
            return Err(CandleWhisperError::InvalidAudio {
                message: "audio samples are empty".to_owned(),
            });
        }
        let config = self.config.clone().ok_or(CandleWhisperError::ModelNotLoaded)?;
        let mel_filters = self.mel_filters.clone().ok_or(CandleWhisperError::ModelNotLoaded)?;
        let tokenizer = self.tokenizer.as_ref().ok_or(CandleWhisperError::ModelNotLoaded)?.clone();
        let model = self.model.as_mut().ok_or(CandleWhisperError::ModelNotLoaded)?;
        let device = self.device.as_ref().ok_or(CandleWhisperError::ModelNotLoaded)?;
        let mel_data = audio::pcm_to_mel(&config, &request.samples, &mel_filters);
        let n_mels = config.num_mel_bins;
        let n_frames = mel_data.len() / n_mels;
        let mel = Tensor::from_vec(mel_data, (1usize, n_mels, n_frames), device)
            .map_err(|error| CandleWhisperError::inference(format!("mel tensor: {error}")))?;
        let decoder = WhisperDecoder::new(tokenizer, model, request.timestamps)?;
        let (text, detected_language, segments) = decoder.decode(model, &mel, &request)?;

        Ok(TranscriptionResponse { text, detected_language, segments })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_engine_is_unloaded() {
        assert!(!CandleWhisperEngine::new().is_model_loaded());
    }

    #[test]
    fn empty_audio_is_rejected_before_model_access() {
        let error = CandleWhisperEngine::new()
            .infer(TranscriptionRequest { samples: Vec::new(), ..TranscriptionRequest::default() })
            .expect_err("empty audio should be rejected");
        assert!(matches!(error, CandleWhisperError::InvalidAudio { .. }));
    }

    #[test]
    fn missing_mel_filters_are_rejected() {
        let config = Config {
            num_mel_bins: 80,
            max_source_positions: 1500,
            d_model: 384,
            encoder_attention_heads: 6,
            encoder_layers: 4,
            vocab_size: 51864,
            max_target_positions: 448,
            decoder_attention_heads: 6,
            decoder_layers: 4,
            suppress_tokens: Vec::new(),
        };
        let error = CandleWhisperEngine::load_mel_filters(Path::new("missing.npz"), &config)
            .expect_err("missing mel filters should fail");
        assert!(matches!(error, CandleWhisperError::InvalidAssetLayout { .. }));
    }
}
