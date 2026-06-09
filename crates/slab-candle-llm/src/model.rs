use std::fs::File;
use std::path::{Path, PathBuf};

use candle_core::quantized::gguf_file;
use candle_core::{DType, Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::{
    deepseek2, gemma, gemma2, gemma3, glm4, glm4_new, llama, mamba, mamba2, phi, quantized_gemma3,
    quantized_glm4, quantized_llama, quantized_phi, quantized_qwen2, quantized_qwen3,
    quantized_qwen3_moe, qwen2, qwen2_moe, qwen3, qwen3_moe,
};

use crate::config::{CandleLlmLoadConfig, LlmModelKind, LlmWeightSource};
use crate::error::CandleLlmError;

pub(crate) enum LoadedLlmModel {
    QuantizedLlama(quantized_llama::ModelWeights),
    QuantizedQwen2(quantized_qwen2::ModelWeights),
    QuantizedQwen3(quantized_qwen3::ModelWeights),
    QuantizedQwen3Moe(quantized_qwen3_moe::GGUFQWenMoE),
    QuantizedGemma3(quantized_gemma3::ModelWeights),
    QuantizedGlm4(quantized_glm4::ModelWeights),
    QuantizedPhi(quantized_phi::ModelWeights),
    Llama(Box<LlamaLoadedModel>),
    Qwen2(qwen2::ModelForCausalLM),
    Qwen2Moe(qwen2_moe::Model),
    Qwen3(qwen3::ModelForCausalLM),
    Qwen3Moe(qwen3_moe::ModelForCausalLM),
    Gemma(gemma::Model),
    Gemma2(gemma2::Model),
    Gemma3(gemma3::Model),
    Glm4(glm4::Model),
    Glm4New(glm4_new::ModelForCausalLM),
    DeepSeek2(deepseek2::DeepSeekV2),
    Mamba(Box<MambaLoadedModel>),
    Mamba2(Box<Mamba2LoadedModel>),
    Phi(phi::Model),
}

pub(crate) struct LlamaLoadedModel {
    model: llama::Llama,
    cache: llama::Cache,
    config: llama::Config,
    dtype: DType,
    device: Device,
}

pub(crate) struct MambaLoadedModel {
    model: mamba::Model,
    state: mamba::State,
    config: mamba::Config,
    dtype: DType,
    device: Device,
}

pub(crate) struct Mamba2LoadedModel {
    model: mamba2::Model,
    state: mamba2::State,
    config: mamba2::Config,
    dtype: DType,
    device: Device,
}

impl LoadedLlmModel {
    pub(crate) fn load(
        config: &CandleLlmLoadConfig,
        device: &Device,
        dtype: DType,
    ) -> Result<Self, CandleLlmError> {
        match config.weight_source {
            LlmWeightSource::QuantizedGguf => Self::load_quantized(config, device),
            LlmWeightSource::Safetensors => Self::load_safetensors(config, device, dtype),
        }
    }

    pub(crate) fn forward(&mut self, input: &Tensor, offset: usize) -> candle_core::Result<Tensor> {
        match self {
            Self::QuantizedLlama(model) => model.forward(input, offset),
            Self::QuantizedQwen2(model) => model.forward(input, offset),
            Self::QuantizedQwen3(model) => model.forward(input, offset),
            Self::QuantizedQwen3Moe(model) => model.forward(input, offset),
            Self::QuantizedGemma3(model) => model.forward(input, offset),
            Self::QuantizedGlm4(model) => model.forward(input, offset),
            Self::QuantizedPhi(model) => model.forward(input, offset),
            Self::Llama(model) => model.model.forward(input, offset, &mut model.cache),
            Self::Qwen2(model) => model.forward(input, offset),
            Self::Qwen2Moe(model) => model.forward(input, offset),
            Self::Qwen3(model) => model.forward(input, offset),
            Self::Qwen3Moe(model) => model.forward(input, offset),
            Self::Gemma(model) => model.forward(input, offset),
            Self::Gemma2(model) => model.forward(input, offset),
            Self::Gemma3(model) => model.forward(input, offset),
            Self::Glm4(model) => model.forward(input),
            Self::Glm4New(model) => model.forward(input, offset),
            Self::DeepSeek2(model) => model.forward(input, offset),
            Self::Mamba(model) => model.forward(input),
            Self::Mamba2(model) => model.forward(input),
            Self::Phi(model) => model.forward(input),
        }
    }

    pub(crate) fn reset_cache(&mut self) {
        match self {
            Self::Llama(model) => {
                if let Ok(cache) =
                    llama::Cache::new(true, model.dtype, &model.config, &model.device)
                {
                    model.cache = cache;
                }
            }
            Self::Qwen2(model) => model.clear_kv_cache(),
            Self::Qwen2Moe(model) => model.clear_kv_cache(),
            Self::Qwen3(model) => model.clear_kv_cache(),
            Self::Qwen3Moe(model) => model.clear_kv_cache(),
            Self::Glm4(model) => model.reset_kv_cache(),
            Self::Glm4New(model) => model.clear_kv_cache(),
            Self::DeepSeek2(model) => model.clear_kv_cache(),
            Self::Mamba(model) => {
                if let Ok(state) = mamba::State::new(1, &model.config, model.dtype, &model.device) {
                    model.state = state;
                }
            }
            Self::Mamba2(model) => {
                if let Ok(state) = mamba2::State::new(1, &model.config, model.dtype, &model.device)
                {
                    model.state = state;
                }
            }
            Self::Phi(model) => model.clear_kv_cache(),
            Self::QuantizedLlama(_)
            | Self::QuantizedQwen2(_)
            | Self::QuantizedQwen3(_)
            | Self::QuantizedQwen3Moe(_)
            | Self::QuantizedGemma3(_)
            | Self::QuantizedGlm4(_)
            | Self::QuantizedPhi(_)
            | Self::Gemma(_)
            | Self::Gemma2(_)
            | Self::Gemma3(_) => {}
        }
    }

    fn load_quantized(
        config: &CandleLlmLoadConfig,
        device: &Device,
    ) -> Result<Self, CandleLlmError> {
        match config.model_kind {
            LlmModelKind::Qwen2Moe => {
                return Err(CandleLlmError::UnsupportedModel {
                    kind: config.model_kind.to_string(),
                    message:
                        "candle-transformers 0.10.2 does not expose a quantized Qwen2 MoE adapter"
                            .to_owned(),
                });
            }
            LlmModelKind::Gemma | LlmModelKind::Gemma2 => {
                return Err(CandleLlmError::UnsupportedModel {
                    kind: config.model_kind.to_string(),
                    message: "candle-transformers 0.10.2 only exposes a quantized Gemma 3 adapter"
                        .to_owned(),
                });
            }
            LlmModelKind::Glm4New => {
                return Err(CandleLlmError::UnsupportedModel {
                    kind: config.model_kind.to_string(),
                    message: "candle-transformers 0.10.2 only exposes a quantized GLM4 adapter"
                        .to_owned(),
                });
            }
            LlmModelKind::DeepSeek2 | LlmModelKind::Mamba | LlmModelKind::Mamba2 => {
                return Err(CandleLlmError::UnsupportedModel {
                    kind: config.model_kind.to_string(),
                    message: "no quantized GGUF adapter is exposed by candle-transformers 0.10.2 for this kind".to_owned(),
                });
            }
            LlmModelKind::Llama
            | LlmModelKind::Qwen2
            | LlmModelKind::Qwen3
            | LlmModelKind::Qwen3Moe
            | LlmModelKind::Gemma3
            | LlmModelKind::Glm4
            | LlmModelKind::Phi => {}
        }
        let mut file = File::open(&config.model_path)
            .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))?;
        match config.model_kind {
            LlmModelKind::Llama => {
                quantized_llama::ModelWeights::from_gguf(content, &mut file, device)
                    .map(Self::QuantizedLlama)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Qwen2 => {
                quantized_qwen2::ModelWeights::from_gguf(content, &mut file, device)
                    .map(Self::QuantizedQwen2)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Qwen3 => {
                quantized_qwen3::ModelWeights::from_gguf(content, &mut file, device)
                    .map(Self::QuantizedQwen3)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Qwen3Moe => {
                quantized_qwen3_moe::GGUFQWenMoE::from_gguf(content, &mut file, device, DType::F32)
                    .map(Self::QuantizedQwen3Moe)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Gemma3 => {
                quantized_gemma3::ModelWeights::from_gguf(content, &mut file, device)
                    .map(Self::QuantizedGemma3)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Glm4 => {
                quantized_glm4::ModelWeights::from_gguf(content, &mut file, device, DType::F32)
                    .map(Self::QuantizedGlm4)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Phi => quantized_phi::ModelWeights::from_gguf(content, &mut file, device)
                .map(Self::QuantizedPhi)
                .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error)),
            LlmModelKind::Qwen2Moe
            | LlmModelKind::Gemma
            | LlmModelKind::Gemma2
            | LlmModelKind::Glm4New
            | LlmModelKind::DeepSeek2
            | LlmModelKind::Mamba
            | LlmModelKind::Mamba2 => Err(CandleLlmError::UnsupportedModel {
                kind: config.model_kind.to_string(),
                message: "unsupported quantized model kind".to_owned(),
            }),
        }
    }

    fn load_safetensors(
        config: &CandleLlmLoadConfig,
        device: &Device,
        dtype: DType,
    ) -> Result<Self, CandleLlmError> {
        match config.model_kind {
            LlmModelKind::Qwen2
            | LlmModelKind::Qwen2Moe
            | LlmModelKind::Qwen3
            | LlmModelKind::Qwen3Moe
            | LlmModelKind::Gemma
            | LlmModelKind::Gemma2
            | LlmModelKind::Gemma3
            | LlmModelKind::Glm4
            | LlmModelKind::Glm4New
            | LlmModelKind::DeepSeek2
            | LlmModelKind::Llama
            | LlmModelKind::Mamba
            | LlmModelKind::Mamba2
            | LlmModelKind::Phi => {}
        }

        let config_path = resolve_config_path(&config.model_path, config.config_path.as_ref())?;
        let weights = resolve_weight_paths(&config.model_path, &config.extra_weight_paths)?;
        let vb_paths = weights.iter().map(PathBuf::as_path).collect::<Vec<_>>();
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(vb_paths.as_slice(), dtype, device)
                .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))?
        };

        match config.model_kind {
            LlmModelKind::Llama => {
                let cfg = read_json::<llama::LlamaConfig>(&config_path)?.into_config(false);
                let cache = llama::Cache::new(true, dtype, &cfg, device).map_err(|error| {
                    CandleLlmError::load_model(config.model_path.display(), error)
                })?;
                llama::Llama::load(vb, &cfg)
                    .map(|model| {
                        Self::Llama(Box::new(LlamaLoadedModel {
                            model,
                            cache,
                            config: cfg,
                            dtype,
                            device: device.clone(),
                        }))
                    })
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Qwen2 => {
                let cfg = read_json::<qwen2::Config>(&config_path)?;
                qwen2::ModelForCausalLM::new(&cfg, vb)
                    .map(Self::Qwen2)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Qwen2Moe => {
                let cfg = read_json::<qwen2_moe::Config>(&config_path)?;
                qwen2_moe::Model::new(&cfg, vb)
                    .map(Self::Qwen2Moe)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Qwen3 => {
                let cfg = read_json::<qwen3::Config>(&config_path)?;
                qwen3::ModelForCausalLM::new(&cfg, vb)
                    .map(Self::Qwen3)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Qwen3Moe => {
                let cfg = read_json::<qwen3_moe::Config>(&config_path)?;
                qwen3_moe::ModelForCausalLM::new(&cfg, vb)
                    .map(Self::Qwen3Moe)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Gemma => {
                let cfg = read_json::<gemma::Config>(&config_path)?;
                gemma::Model::new(false, &cfg, vb)
                    .map(Self::Gemma)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Gemma2 => {
                let cfg = read_json::<gemma2::Config>(&config_path)?;
                gemma2::Model::new(false, &cfg, vb)
                    .map(Self::Gemma2)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Gemma3 => {
                let cfg = read_json::<gemma3::Config>(&config_path)?;
                gemma3::Model::new(false, &cfg, vb)
                    .map(Self::Gemma3)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Glm4 => {
                let cfg = read_json::<glm4::Config>(&config_path)?;
                glm4::Model::new(&cfg, vb)
                    .map(Self::Glm4)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Glm4New => {
                let cfg = read_json::<glm4_new::Config>(&config_path)?;
                glm4_new::ModelForCausalLM::new(&cfg, vb)
                    .map(Self::Glm4New)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::DeepSeek2 => {
                let cfg = read_json::<deepseek2::DeepSeekV2Config>(&config_path)?;
                deepseek2::DeepSeekV2::new(&cfg, vb)
                    .map(Self::DeepSeek2)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Mamba => {
                let cfg = read_json::<mamba::Config>(&config_path)?;
                let state = mamba::State::new(1, &cfg, dtype, device).map_err(|error| {
                    CandleLlmError::load_model(config.model_path.display(), error)
                })?;
                mamba::Model::new(&cfg, vb)
                    .map(|model| {
                        Self::Mamba(Box::new(MambaLoadedModel {
                            model,
                            state,
                            config: cfg,
                            dtype,
                            device: device.clone(),
                        }))
                    })
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Mamba2 => {
                let cfg = read_json::<mamba2::Config>(&config_path)?;
                let state = mamba2::State::new(1, &cfg, dtype, device).map_err(|error| {
                    CandleLlmError::load_model(config.model_path.display(), error)
                })?;
                mamba2::Model::new(&cfg, vb)
                    .map(|model| {
                        Self::Mamba2(Box::new(Mamba2LoadedModel {
                            model,
                            state,
                            config: cfg,
                            dtype,
                            device: device.clone(),
                        }))
                    })
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
            LlmModelKind::Phi => {
                let cfg = read_json::<phi::Config>(&config_path)?;
                phi::Model::new(&cfg, vb)
                    .map(Self::Phi)
                    .map_err(|error| CandleLlmError::load_model(config.model_path.display(), error))
            }
        }
    }
}

impl MambaLoadedModel {
    fn forward(&mut self, input: &Tensor) -> candle_core::Result<Tensor> {
        let tokens = input.squeeze(0)?;
        let seq_len = tokens.dim(0)?;
        let mut logits = None;
        for index in 0..seq_len {
            let token = tokens.i(index)?.unsqueeze(0)?;
            logits = Some(self.model.forward(&token, &mut self.state)?);
        }
        logits
            .ok_or_else(|| candle_core::Error::Msg("mamba forward received no tokens".to_owned()))?
            .unsqueeze(0)
    }
}

impl Mamba2LoadedModel {
    fn forward(&mut self, input: &Tensor) -> candle_core::Result<Tensor> {
        let seq_len = input.dim(1)?;
        if seq_len > 1 {
            self.model.forward_prefill(input, &mut self.state, 64)?.narrow(1, seq_len - 1, 1)
        } else {
            self.model.forward(input, &mut self.state)?.unsqueeze(1)
        }
    }
}

fn resolve_config_path(
    model_path: &Path,
    explicit: Option<&PathBuf>,
) -> Result<PathBuf, CandleLlmError> {
    if let Some(path) = explicit {
        return Ok(path.clone());
    }
    let dir = if model_path.is_dir() {
        model_path
    } else {
        model_path.parent().unwrap_or_else(|| Path::new("."))
    };
    let path = dir.join("config.json");
    if path.exists() {
        Ok(path)
    } else {
        Err(CandleLlmError::InvalidAssetLayout {
            path: dir.display().to_string(),
            message: "missing config.json for safetensors model".to_owned(),
        })
    }
}

fn resolve_weight_paths(
    model_path: &Path,
    extra_weight_paths: &[PathBuf],
) -> Result<Vec<PathBuf>, CandleLlmError> {
    let mut paths = if model_path.is_dir() {
        std::fs::read_dir(model_path)
            .map_err(|error| CandleLlmError::InvalidAssetLayout {
                path: model_path.display().to_string(),
                message: error.to_string(),
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("safetensors"))
            .collect::<Vec<_>>()
    } else {
        vec![model_path.to_path_buf()]
    };
    paths.extend(extra_weight_paths.iter().cloned());
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        Err(CandleLlmError::InvalidAssetLayout {
            path: model_path.display().to_string(),
            message: "no safetensors weight files found".to_owned(),
        })
    } else {
        Ok(paths)
    }
}

fn read_json<T>(path: &Path) -> Result<T, CandleLlmError>
where
    T: serde::de::DeserializeOwned,
{
    let data = std::fs::read_to_string(path).map_err(|error| CandleLlmError::LoadModel {
        model_path: path.display().to_string(),
        message: error.to_string(),
    })?;
    serde_json::from_str(&data).map_err(|error| CandleLlmError::LoadModel {
        model_path: path.display().to_string(),
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_config_path_is_used() {
        let path = PathBuf::from("custom.json");
        let resolved = resolve_config_path(Path::new("model.safetensors"), Some(&path))
            .expect("explicit config should resolve");
        assert_eq!(resolved, path);
    }

    #[test]
    fn unsupported_quantized_combinations_do_not_touch_filesystem() {
        for model_kind in [
            LlmModelKind::Qwen2Moe,
            LlmModelKind::Gemma,
            LlmModelKind::Gemma2,
            LlmModelKind::Glm4New,
            LlmModelKind::DeepSeek2,
            LlmModelKind::Mamba,
            LlmModelKind::Mamba2,
        ] {
            let config = CandleLlmLoadConfig {
                model_path: PathBuf::from("missing.gguf"),
                tokenizer_path: None,
                config_path: None,
                extra_weight_paths: Vec::new(),
                model_kind,
                weight_source: LlmWeightSource::QuantizedGguf,
                prompt_format: crate::config::PromptFormat::Raw,
                seed: 0,
            };
            let error = match LoadedLlmModel::load(&config, &Device::Cpu, DType::F32) {
                Ok(_) => panic!("unsupported combination should fail before opening a model file"),
                Err(error) => error,
            };
            assert!(matches!(error, CandleLlmError::UnsupportedModel { .. }));
        }
    }
}
