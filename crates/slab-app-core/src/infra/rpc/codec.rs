use std::path::Path;

use slab_types::{RuntimeBackendId, RuntimeBackendLoadSpec};
use thiserror::Error;

use super::pb;

#[derive(Debug, Error)]
pub enum RpcCodecError {
    #[error("missing required field `{field}`")]
    MissingField { field: &'static str },
    #[error("invalid field `{field}`: {message}")]
    InvalidField { field: &'static str, message: String },
    #[error("failed to encode raw image as PNG: {0}")]
    ImageEncode(#[from] image::ImageError),
}

#[derive(Debug, Clone)]
pub enum ModelLoadRpcRequest {
    GgmlLlama(pb::GgmlLlamaLoadRequest),
    GgmlWhisper(pb::GgmlWhisperLoadRequest),
    GgmlDiffusion(pb::GgmlDiffusionLoadRequest),
    CandleLlama(pb::CandleLlamaLoadRequest),
    CandleWhisper(pb::CandleWhisperLoadRequest),
    CandleDiffusion(pb::CandleDiffusionLoadRequest),
    OnnxText(pb::OnnxTextLoadRequest),
}

impl ModelLoadRpcRequest {
    pub fn backend_id(&self) -> RuntimeBackendId {
        match self {
            Self::GgmlLlama(_) => RuntimeBackendId::GgmlLlama,
            Self::GgmlWhisper(_) => RuntimeBackendId::GgmlWhisper,
            Self::GgmlDiffusion(_) => RuntimeBackendId::GgmlDiffusion,
            Self::CandleLlama(_) => RuntimeBackendId::CandleLlama,
            Self::CandleWhisper(_) => RuntimeBackendId::CandleWhisper,
            Self::CandleDiffusion(_) => RuntimeBackendId::CandleDiffusion,
            Self::OnnxText(_) => RuntimeBackendId::Onnx,
        }
    }

    pub fn model_path(&self) -> Option<&str> {
        match self {
            Self::GgmlLlama(request) => request.model_path.as_deref(),
            Self::GgmlWhisper(request) => request.model_path.as_deref(),
            Self::GgmlDiffusion(request) => request.model_path.as_deref(),
            Self::CandleLlama(request) => request.model_path.as_deref(),
            Self::CandleWhisper(request) => request.model_path.as_deref(),
            Self::CandleDiffusion(request) => request.model_path.as_deref(),
            Self::OnnxText(request) => request.model_path.as_deref(),
        }
    }
}

pub fn encode_model_load_request(spec: &RuntimeBackendLoadSpec) -> ModelLoadRpcRequest {
    match spec {
        RuntimeBackendLoadSpec::GgmlLlama(config) => {
            ModelLoadRpcRequest::GgmlLlama(pb::GgmlLlamaLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                num_workers: Some(usize_to_u32(config.num_workers)),
                context_length: config.context_length.filter(|value| *value != 0),
                chat_template: non_empty_string(config.chat_template.as_deref()),
                gbnf: non_empty_string(config.gbnf.as_deref()),
                flash_attn: Some(config.flash_attn),
            })
        }
        RuntimeBackendLoadSpec::GgmlWhisper(config) => {
            ModelLoadRpcRequest::GgmlWhisper(pb::GgmlWhisperLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                flash_attn: Some(config.flash_attn),
            })
        }
        RuntimeBackendLoadSpec::GgmlDiffusion(config) => {
            ModelLoadRpcRequest::GgmlDiffusion(pb::GgmlDiffusionLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                diffusion_model_path: opt_path_to_string(config.diffusion_model_path.as_deref()),
                vae_path: opt_path_to_string(config.vae_path.as_deref()),
                taesd_path: opt_path_to_string(config.taesd_path.as_deref()),
                clip_l_path: opt_path_to_string(config.clip_l_path.as_deref()),
                clip_g_path: opt_path_to_string(config.clip_g_path.as_deref()),
                t5xxl_path: opt_path_to_string(config.t5xxl_path.as_deref()),
                clip_vision_path: opt_path_to_string(config.clip_vision_path.as_deref()),
                control_net_path: opt_path_to_string(config.control_net_path.as_deref()),
                flash_attn: Some(config.flash_attn),
                vae_device: non_empty_string(config.vae_device.as_deref()),
                clip_device: non_empty_string(config.clip_device.as_deref()),
                offload_params_to_cpu: Some(config.offload_params_to_cpu),
                enable_mmap: Some(config.enable_mmap),
                n_threads: config.n_threads.filter(|value| *value != 0),
            })
        }
        RuntimeBackendLoadSpec::CandleLlama(config) => {
            ModelLoadRpcRequest::CandleLlama(pb::CandleLlamaLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                tokenizer_path: opt_path_to_string(config.tokenizer_path.as_deref()),
                seed: Some(config.seed),
                device: config.device.map(String::from),
            })
        }
        RuntimeBackendLoadSpec::CandleWhisper(config) => {
            ModelLoadRpcRequest::CandleWhisper(pb::CandleWhisperLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                tokenizer_path: opt_path_to_string(config.tokenizer_path.as_deref()),
                device: config.device.map(String::from),
            })
        }
        RuntimeBackendLoadSpec::CandleDiffusion(config) => {
            ModelLoadRpcRequest::CandleDiffusion(pb::CandleDiffusionLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                vae_path: opt_path_to_string(config.vae_path.as_deref()),
                sd_version: non_empty_string(Some(&config.sd_version)),
                device: config.device.map(String::from),
            })
        }
        RuntimeBackendLoadSpec::Onnx(config) => {
            ModelLoadRpcRequest::OnnxText(pb::OnnxTextLoadRequest {
                model_path: Some(path_to_string(&config.model_path)),
                execution_providers: Some(pb::StringList {
                    values: config.execution_providers.clone(),
                }),
                intra_op_num_threads: config.intra_op_num_threads.map(usize_to_u32),
                inter_op_num_threads: config.inter_op_num_threads.map(usize_to_u32),
            })
        }
    }
}

fn non_empty_string(value: Option<&str>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty()).map(ToOwned::to_owned)
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn opt_path_to_string(path: Option<&Path>) -> Option<String> {
    path.map(path_to_string)
}

fn usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use slab_types::{
        CandleDiffusionLoadConfig, CandleLlamaLoadConfig, CandleWhisperLoadConfig,
        RuntimeBackendLoadSpec, RuntimeDevicePreference,
    };

    use super::{ModelLoadRpcRequest, encode_model_load_request};

    #[test]
    fn encodes_candle_load_device_preferences() {
        let llama = encode_model_load_request(&RuntimeBackendLoadSpec::CandleLlama(
            CandleLlamaLoadConfig {
                model_path: PathBuf::from("llama.gguf"),
                tokenizer_path: None,
                device: Some(RuntimeDevicePreference::Cuda { ordinal: 0 }),
                seed: 42,
            },
        ));
        let whisper = encode_model_load_request(&RuntimeBackendLoadSpec::CandleWhisper(
            CandleWhisperLoadConfig {
                model_path: PathBuf::from("whisper.bin"),
                tokenizer_path: None,
                device: Some(RuntimeDevicePreference::Metal { ordinal: 1 }),
            },
        ));
        let diffusion = encode_model_load_request(&RuntimeBackendLoadSpec::CandleDiffusion(
            CandleDiffusionLoadConfig {
                model_path: PathBuf::from("sd"),
                vae_path: None,
                device: Some(RuntimeDevicePreference::Cpu),
                sd_version: "sdxl".to_owned(),
            },
        ));

        assert!(matches!(
            llama,
            ModelLoadRpcRequest::CandleLlama(request)
                if request.device.as_deref() == Some("cuda:0")
        ));
        assert!(matches!(
            whisper,
            ModelLoadRpcRequest::CandleWhisper(request)
                if request.device.as_deref() == Some("metal:1")
        ));
        assert!(matches!(
            diffusion,
            ModelLoadRpcRequest::CandleDiffusion(request)
                if request.device.as_deref() == Some("cpu")
        ));
    }
}
