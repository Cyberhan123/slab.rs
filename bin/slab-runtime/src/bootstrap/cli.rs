use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::{Parser, ValueEnum};

use crate::infra::config::{CliEnabledBackends, RuntimeConfig, resolve_base_lib_path};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum EnabledBackendArg {
    #[value(alias = "ggml.llama")]
    Llama,
    #[value(alias = "ggml.whisper")]
    Whisper,
    #[value(alias = "ggml.diffusion")]
    Diffusion,
    #[value(name = "candle.llama", alias = "candle-llama")]
    CandleLlama,
    #[value(name = "candle.whisper", alias = "candle-whisper")]
    CandleWhisper,
    #[value(name = "candle.diffusion", alias = "candle-diffusion")]
    CandleDiffusion,
}

#[derive(Parser, Debug, Clone)]
#[command(name = "slab-runtime", version, about = "Slab gRPC runtime worker")]
pub struct Cli {
    #[arg(long = "grpc-bind", default_value = "127.0.0.1:50051")]
    pub grpc_bind: String,
    #[arg(long = "log")]
    pub log_level: Option<String>,
    #[arg(long = "log-json", action = clap::ArgAction::SetTrue)]
    pub log_json: bool,
    #[arg(long = "queue-capacity")]
    pub queue_capacity: Option<usize>,
    #[arg(long = "backend-capacity")]
    pub backend_capacity: Option<usize>,
    #[arg(long = "lib-dir")]
    pub lib_dir: Option<PathBuf>,
    #[arg(long = "log-file")]
    pub log_file: Option<PathBuf>,
    #[arg(long = "enabled-backends", value_enum, value_delimiter = ',', ignore_case = true)]
    enabled_backends: Vec<EnabledBackendArg>,
    #[arg(long, default_value_t = false)]
    pub shutdown_on_stdin_close: bool,
}

impl Cli {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }

    pub fn into_runtime_config(self) -> anyhow::Result<RuntimeConfig> {
        let enabled_backends = if self.enabled_backends.is_empty() {
            CliEnabledBackends::all()
        } else {
            let mut enabled = CliEnabledBackends {
                llama: false,
                whisper: false,
                diffusion: false,
                candle_llama: false,
                candle_whisper: false,
                candle_diffusion: false,
            };
            for backend in self.enabled_backends {
                match backend {
                    EnabledBackendArg::Llama => enabled.llama = true,
                    EnabledBackendArg::Whisper => enabled.whisper = true,
                    EnabledBackendArg::Diffusion => enabled.diffusion = true,
                    EnabledBackendArg::CandleLlama => enabled.candle_llama = true,
                    EnabledBackendArg::CandleWhisper => enabled.candle_whisper = true,
                    EnabledBackendArg::CandleDiffusion => enabled.candle_diffusion = true,
                }
            }
            enabled
        };
        let current_dir =
            std::env::current_dir().context("failed to resolve slab-runtime current directory")?;
        let base_lib_path = resolve_base_lib_path(
            self.lib_dir.unwrap_or_else(|| Path::new("./resources/libs").to_path_buf()),
            &current_dir,
        );
        let llama_lib_dir = enabled_backends.llama.then(|| base_lib_path.clone());
        let whisper_lib_dir = enabled_backends.whisper.then(|| base_lib_path.clone());
        let diffusion_lib_dir = enabled_backends.diffusion.then(|| base_lib_path.clone());

        Ok(RuntimeConfig {
            grpc_bind: self.grpc_bind,
            log_level: self.log_level.unwrap_or_else(|| "info".to_owned()),
            log_json: self.log_json,
            queue_capacity: self.queue_capacity.unwrap_or(64),
            backend_capacity: self.backend_capacity.unwrap_or(4),
            base_lib_path,
            log_file: self.log_file,
            enabled_backends,
            shutdown_on_stdin_close: self.shutdown_on_stdin_close,
            llama_lib_dir,
            whisper_lib_dir,
            diffusion_lib_dir,
            enable_candle_llama: enabled_backends.candle_llama,
            enable_candle_whisper: enabled_backends.candle_whisper,
            enable_candle_diffusion: enabled_backends.candle_diffusion,
            onnx_enabled: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn runtime_config_defaults_to_all_backends() {
        let cli = <Cli as Parser>::try_parse_from(["slab-runtime"]).expect("parse cli");
        let config = cli.into_runtime_config().expect("build runtime config");

        assert!(config.enabled_backends.llama);
        assert!(config.enabled_backends.whisper);
        assert!(config.enabled_backends.diffusion);
        assert!(!config.enabled_backends.candle_llama);
    }

    #[test]
    fn runtime_config_accepts_legacy_backend_aliases() {
        let cli = <Cli as Parser>::try_parse_from([
            "slab-runtime",
            "--enabled-backends",
            "ggml.llama,whisper",
        ])
        .expect("parse cli");
        let config = cli.into_runtime_config().expect("build runtime config");

        assert!(config.enabled_backends.llama);
        assert!(config.enabled_backends.whisper);
        assert!(!config.enabled_backends.diffusion);
    }

    #[test]
    fn runtime_config_accepts_candle_backend_ids() {
        let cli = <Cli as Parser>::try_parse_from([
            "slab-runtime",
            "--enabled-backends",
            "candle.llama,candle.whisper,candle.diffusion",
        ])
        .expect("parse cli");
        let config = cli.into_runtime_config().expect("build runtime config");

        assert!(!config.enabled_backends.llama);
        assert!(config.enabled_backends.candle_llama);
        assert!(config.enabled_backends.candle_whisper);
        assert!(config.enabled_backends.candle_diffusion);
        assert!(config.enable_candle_llama);
        assert!(config.enable_candle_whisper);
        assert!(config.enable_candle_diffusion);
    }
}
